use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use guild_registry::{LocalRegistry, LocalSourceInstaller, SkillRegistry, execution_resource_uri};
use guild_runner::{
    HttpReplayFixture, LiveProofComparatorProfile, LiveProofSupport, Runner, WasmtimeRuntimeAdapter,
};
use guild_types::{
    Budget, CallerRequest, CapabilityAccess, CapabilityConstraints, CapabilityGrantSet,
    CapabilityId, EmitEvidenceConstraints, EvidenceAudience, ExecutionMode, GrantedCapability,
    HttpAddressFamily, HttpMethod, HttpRequestConstraints, HttpResolutionBinding,
    HttpResolvedAddress, HttpScheme, PolicyDecision, PolicyDecisionOutcome,
    ReadResourceConstraints, RedactionClass, RequestedSkillRef, ResolvedExecutionEnvelope,
    ResourceKind, Severity, SkillKey, VersionRequirement,
};
use serde_json::{Value, json};

#[path = "../../../test-support/http_test_server.rs"]
mod http_test_server;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn hello_skill_dir() -> PathBuf {
    repo_root().join("examples/skills/hello-inspect")
}

fn explain_execution_dir() -> PathBuf {
    repo_root().join("examples/skills/explain-execution")
}

fn inspect_http_json_dir() -> PathBuf {
    repo_root().join("examples/skills/inspect-http-json")
}

fn summarize_execution_query_dir() -> PathBuf {
    repo_root().join("examples/skills/summarize-execution-query")
}

fn requested_skill(name: &str) -> RequestedSkillRef {
    RequestedSkillRef {
        key: SkillKey {
            namespace: "example".into(),
            name: name.into(),
        },
        version_req: VersionRequirement::parse("^0.1").unwrap(),
    }
}

fn unique_id(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{prefix}-{nanos}")
}

fn emit_evidence_grant() -> GrantedCapability {
    GrantedCapability {
        id: CapabilityId::EmitEvidence,
        access: CapabilityAccess::Write,
        constraints: CapabilityConstraints::EmitEvidence(EmitEvidenceConstraints {
            max_bytes: Some(65_536),
            audiences: Some(vec![EvidenceAudience::User]),
            redactions: Some(vec![RedactionClass::None]),
        }),
    }
}

fn log_grant(levels: &[Severity]) -> GrantedCapability {
    GrantedCapability {
        id: CapabilityId::LogWrite,
        access: CapabilityAccess::Write,
        constraints: CapabilityConstraints::Log(guild_types::LogConstraints {
            levels: Some(levels.to_vec()),
        }),
    }
}

fn read_resource_grant(prefixes: &[&str]) -> GrantedCapability {
    let resource_kinds = prefixes
        .iter()
        .filter_map(|prefix| ResourceKind::from_uri_prefix(prefix))
        .fold(Vec::new(), |mut kinds, kind| {
            if !kinds.contains(&kind) {
                kinds.push(kind);
            }
            kinds
        });
    GrantedCapability {
        id: CapabilityId::ReadResource,
        access: CapabilityAccess::Read,
        constraints: CapabilityConstraints::ReadResource(ReadResourceConstraints {
            uri_prefixes: Some(prefixes.iter().map(|value| (*value).to_owned()).collect()),
            resource_kinds: Some(resource_kinds),
        }),
    }
}

fn http_grant_for_method(
    host: &str,
    port: u16,
    path_prefix: &str,
    method: HttpMethod,
) -> GrantedCapability {
    GrantedCapability {
        id: CapabilityId::HttpRequest,
        access: CapabilityAccess::Read,
        constraints: CapabilityConstraints::HttpRequest(HttpRequestConstraints {
            allowed_schemes: Some(vec![HttpScheme::Http]),
            allowed_hosts: Some(vec![host.to_owned()]),
            allowed_host_suffixes: None,
            allowed_ports: Some(vec![port]),
            allowed_methods: Some(vec![method]),
            allowed_path_prefixes: Some(vec![path_prefix.to_owned()]),
            max_timeout_ms: Some(2_000),
            max_response_bytes: Some(16_384),
            follow_redirects: None,
            max_redirects: None,
            allow_loopback: Some(true),
            allow_link_local: None,
            allow_private_networks: None,
            allow_ip_literals: Some(true),
        }),
    }
}

fn http_grant(host: &str, port: u16, path_prefix: &str) -> GrantedCapability {
    http_grant_for_method(host, port, path_prefix, HttpMethod::Get)
}

fn head_http_grant(host: &str, port: u16, path_prefix: &str) -> GrantedCapability {
    http_grant_for_method(host, port, path_prefix, HttpMethod::Head)
}

fn redirect_http_grant(host: &str, port: u16, path_prefixes: &[&str]) -> GrantedCapability {
    GrantedCapability {
        id: CapabilityId::HttpRequest,
        access: CapabilityAccess::Read,
        constraints: CapabilityConstraints::HttpRequest(HttpRequestConstraints {
            allowed_schemes: Some(vec![HttpScheme::Http]),
            allowed_hosts: Some(vec![host.to_owned()]),
            allowed_host_suffixes: None,
            allowed_ports: Some(vec![port]),
            allowed_methods: Some(vec![HttpMethod::Get]),
            allowed_path_prefixes: Some(
                path_prefixes
                    .iter()
                    .map(|path_prefix| (*path_prefix).to_owned())
                    .collect(),
            ),
            max_timeout_ms: Some(2_000),
            max_response_bytes: Some(16_384),
            follow_redirects: Some(true),
            max_redirects: Some(2),
            allow_loopback: Some(true),
            allow_link_local: None,
            allow_private_networks: None,
            allow_ip_literals: Some(true),
        }),
    }
}

fn build_runner() -> Runner<WasmtimeRuntimeAdapter> {
    Runner::new(WasmtimeRuntimeAdapter::new().unwrap())
}

fn build_replay_runner(fixtures: Vec<HttpReplayFixture>) -> Runner<WasmtimeRuntimeAdapter> {
    Runner::new(WasmtimeRuntimeAdapter::new().unwrap())
        .with_http_replay_fixtures(fixtures)
        .unwrap()
}

fn http_replay_fixture_for_method(method: HttpMethod, url: &str, body: &str) -> HttpReplayFixture {
    HttpReplayFixture {
        method,
        url: url.to_owned(),
        response_status: 200,
        response_content_type: Some("application/json".into()),
        response_body: body.as_bytes().to_vec(),
        redirect_location: None,
        resolution_binding: None,
    }
}

fn http_replay_fixture(url: &str, body: &str) -> HttpReplayFixture {
    http_replay_fixture_for_method(HttpMethod::Get, url, body)
}

fn head_http_replay_fixture(url: &str) -> HttpReplayFixture {
    http_replay_fixture_for_method(HttpMethod::Head, url, "")
}

fn localhost_resolution_binding(port: u16) -> HttpResolutionBinding {
    HttpResolutionBinding {
        requested_host: "localhost".into(),
        port,
        addresses: vec![HttpResolvedAddress {
            address: "127.0.0.1".into(),
            family: HttpAddressFamily::Ipv4,
        }],
        loopback_only: true,
    }
}

fn localhost_http_replay_fixture_for_method(
    method: HttpMethod,
    url: &str,
    port: u16,
    body: &str,
) -> HttpReplayFixture {
    HttpReplayFixture {
        method,
        url: url.to_owned(),
        response_status: 200,
        response_content_type: Some("application/json".into()),
        response_body: body.as_bytes().to_vec(),
        redirect_location: None,
        resolution_binding: Some(localhost_resolution_binding(port)),
    }
}

fn localhost_http_replay_fixture(url: &str, port: u16, body: &str) -> HttpReplayFixture {
    localhost_http_replay_fixture_for_method(HttpMethod::Get, url, port, body)
}

fn localhost_head_http_replay_fixture(url: &str, port: u16) -> HttpReplayFixture {
    localhost_http_replay_fixture_for_method(HttpMethod::Head, url, port, "")
}

fn redirect_replay_fixture(url: &str, redirect_location: &str) -> HttpReplayFixture {
    HttpReplayFixture {
        method: HttpMethod::Get,
        url: url.to_owned(),
        response_status: 302,
        response_content_type: Some("application/json".into()),
        response_body: br#"{"redirect":"json"}"#.to_vec(),
        redirect_location: Some(redirect_location.to_owned()),
        resolution_binding: None,
    }
}

fn envelope_for(
    installed: &guild_registry::InstalledSkill,
    input: Value,
    grants: CapabilityGrantSet,
) -> ResolvedExecutionEnvelope {
    ResolvedExecutionEnvelope {
        request: CallerRequest {
            request_id: unique_id("proof-request"),
            skill: requested_skill(&installed.resolved_ref.key.name),
            tenant_id: "tenant-1".into(),
            actor_id: "actor-1".into(),
            mode: ExecutionMode::Inspect,
            input,
            budget: Budget::default(),
            requested_capabilities: grants.clone(),
            idempotency_key: None,
            trace_id: unique_id("trace"),
        },
        resolved_skill: installed.resolved_ref.clone(),
        granted_capabilities: grants,
        policy_decision: PolicyDecision {
            outcome: PolicyDecisionOutcome::Allowed,
            summary: "local policy granted requested capabilities".into(),
            profile_name: "default".into(),
            trust_tier: guild_types::LocalTrustTier::LocalDev,
            verification_state: guild_types::InstalledVerificationState::LocalSource,
            reasons: Vec::new(),
            detail: None,
        },
        parent_execution_id: None,
    }
}

struct TempRegistry {
    root: PathBuf,
}

impl TempRegistry {
    fn new() -> Self {
        let root = std::env::temp_dir().join(unique_id("guild-live-proof-registry"));
        fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    fn install(&self, skill_dir: impl AsRef<Path>) {
        LocalSourceInstaller::new(&self.root)
            .unwrap()
            .install(skill_dir)
            .unwrap();
    }

    fn load(&self) -> LocalRegistry {
        LocalRegistry::load(&self.root).unwrap()
    }
}

impl Drop for TempRegistry {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn read_resource_live_proof_is_bounded_and_live_linkable() {
    let temp = TempRegistry::new();
    temp.install(hello_skill_dir());
    temp.install(explain_execution_dir());
    let registry = temp.load();
    let runner = build_runner();

    let hello = registry.resolve(&requested_skill("hello-inspect")).unwrap();
    let baseline_target = runner
        .execute(
            &registry,
            &hello,
            &envelope_for(
                &hello,
                json!({ "name": "Ada" }),
                CapabilityGrantSet {
                    grants: vec![emit_evidence_grant()],
                },
            ),
        )
        .unwrap();

    let explain = registry
        .resolve(&requested_skill("explain-execution"))
        .unwrap();
    let proof_result = runner
        .prove_live_authority(
            &registry,
            &explain,
            &envelope_for(
                &explain,
                json!({
                    "execution_uri": execution_resource_uri(&baseline_target.receipt.execution_id),
                    "include_first_evidence": false,
                }),
                CapabilityGrantSet {
                    grants: vec![read_resource_grant(&[
                        "guild://executions/",
                        "guild://objects/records/",
                    ])],
                },
            ),
            LiveProofComparatorProfile::NormalizedInspectOutputV1,
        )
        .unwrap();

    assert_eq!(proof_result.proof.proof_status, "bounded_minimal");
    assert!(proof_result.proof.residual_authority.grants.is_empty());
    assert_eq!(proof_result.proof.proven_authority.grants.len(), 1);
    let grant = &proof_result.proof.proven_authority.grants[0];
    assert_eq!(grant.id, CapabilityId::ReadResource);
    match &grant.constraints {
        CapabilityConstraints::ReadResource(value) => {
            assert_eq!(
                value.uri_prefixes.as_ref().unwrap(),
                &vec!["guild://executions/".to_owned()]
            );
            assert_eq!(
                value.resource_kinds.as_ref().unwrap(),
                &vec![ResourceKind::Execution]
            );
        }
        other => panic!("expected read-resource constraints, got {other:?}"),
    }

    let family = proof_result
        .proof
        .family_statuses
        .iter()
        .find(|status| status.family == CapabilityId::ReadResource)
        .unwrap();
    assert_eq!(family.support, LiveProofSupport::BoundedLiveProof);
    assert_eq!(family.proof_status.as_deref(), Some("bounded_minimal"));
    assert!(
        proof_result
            .proof
            .candidate_trials
            .iter()
            .any(|trial| trial.change_kind == "shrink_scope" && trial.accepted)
    );
}

#[test]
fn log_write_live_proof_reduces_to_observed_levels_and_leaves_emit_evidence_residual() {
    let temp = TempRegistry::new();
    temp.install(hello_skill_dir());
    let registry = temp.load();
    let runner = build_runner();
    let hello = registry.resolve(&requested_skill("hello-inspect")).unwrap();

    let proof_result = runner
        .prove_live_authority(
            &registry,
            &hello,
            &envelope_for(
                &hello,
                json!({ "name": "Ada", "emit_log": true }),
                CapabilityGrantSet {
                    grants: vec![
                        emit_evidence_grant(),
                        log_grant(&[Severity::Info, Severity::Error]),
                    ],
                },
            ),
            LiveProofComparatorProfile::NormalizedInspectOutputV1,
        )
        .unwrap();

    assert_eq!(proof_result.proof.proof_status, "reduced");
    assert_eq!(proof_result.proof.proven_authority.grants.len(), 1);
    assert_eq!(proof_result.proof.residual_authority.grants.len(), 1);
    match &proof_result.proof.proven_authority.grants[0].constraints {
        CapabilityConstraints::Log(value) => {
            assert_eq!(value.levels.as_ref().unwrap(), &vec![Severity::Info]);
        }
        other => panic!("expected log constraints, got {other:?}"),
    }
    assert_eq!(
        proof_result.proof.residual_authority.grants[0].id,
        CapabilityId::EmitEvidence
    );

    let log_family = proof_result
        .proof
        .family_statuses
        .iter()
        .find(|status| status.family == CapabilityId::LogWrite)
        .unwrap();
    assert_eq!(log_family.support, LiveProofSupport::LiveProofSupported);
    assert_eq!(log_family.proof_status.as_deref(), Some("exact_minimal"));

    let emit_family = proof_result
        .proof
        .family_statuses
        .iter()
        .find(|status| status.family == CapabilityId::EmitEvidence)
        .unwrap();
    assert_eq!(emit_family.support, LiveProofSupport::NotProven);
    assert_eq!(emit_family.proof_status.as_deref(), Some("not_proven"));
}

#[test]
fn http_request_live_proof_is_bounded_with_replay() {
    let temp = TempRegistry::new();
    temp.install(inspect_http_json_dir());
    let registry = temp.load();
    let replay_url = "http://127.0.0.1:18080/response.json";
    let runner = build_replay_runner(vec![http_replay_fixture(
        replay_url,
        r#"{"service":"guild-http","message":"deterministic","nested":{"count":2},"items":[{"name":"alpha"},{"name":"beta"}]}"#,
    )]);
    let http_skill = registry
        .resolve(&requested_skill("inspect-http-json"))
        .unwrap();

    let proof_result = runner
        .prove_live_authority(
            &registry,
            &http_skill,
            &envelope_for(
                &http_skill,
                json!({
                    "url": replay_url,
                    "method": "get",
                    "json_pointers": ["/message", "/nested/count"],
                }),
                CapabilityGrantSet {
                    grants: vec![http_grant("127.0.0.1", 18080, "/response.json")],
                },
            ),
            LiveProofComparatorProfile::NormalizedInspectOutputV1,
        )
        .unwrap();

    assert_eq!(proof_result.proof.proof_status, "bounded_minimal");
    assert!(proof_result.proof.residual_authority.grants.is_empty());
    assert_eq!(proof_result.proof.proven_authority.grants.len(), 1);
    let family = proof_result
        .proof
        .family_statuses
        .iter()
        .find(|status| status.family == CapabilityId::HttpRequest)
        .unwrap();
    assert_eq!(family.support, LiveProofSupport::BoundedLiveProof);
    assert_eq!(family.proof_status.as_deref(), Some("bounded_minimal"));
    assert!(
        family
            .reason_codes
            .iter()
            .any(|code| code == "HTTP_LIVE_PROOF_BOUNDED")
    );
    match &proof_result.proof.proven_authority.grants[0].constraints {
        CapabilityConstraints::HttpRequest(value) => {
            assert_eq!(
                value.allowed_hosts.as_ref().unwrap(),
                &vec!["127.0.0.1".to_owned()]
            );
            assert_eq!(value.allowed_ports.as_ref().unwrap(), &vec![18080]);
            assert_eq!(
                value.allowed_path_prefixes.as_ref().unwrap(),
                &vec!["/response.json".to_owned()]
            );
            assert_eq!(value.follow_redirects, Some(false));
            assert_eq!(value.allow_loopback, Some(true));
            assert_eq!(value.allow_ip_literals, Some(true));
        }
        other => panic!("expected http-request constraints, got {other:?}"),
    }
    assert!(
        proof_result
            .proof
            .candidate_trials
            .iter()
            .any(|trial| trial.family == CapabilityId::HttpRequest
                && trial.change_kind == "shrink_scope"
                && trial.accepted)
    );
    assert!(proof_result.proof.replay_input_digest.is_some());
}

#[test]
fn http_request_live_proof_is_bounded_with_replay_for_default_port_shape() {
    let temp = TempRegistry::new();
    temp.install(inspect_http_json_dir());
    let registry = temp.load();
    let replay_url = "http://127.0.0.1/response.json";
    let runner = build_replay_runner(vec![http_replay_fixture(
        replay_url,
        r#"{"service":"guild-http","message":"deterministic","nested":{"count":2},"items":[{"name":"alpha"},{"name":"beta"}]}"#,
    )]);
    let http_skill = registry
        .resolve(&requested_skill("inspect-http-json"))
        .unwrap();

    let proof_result = runner
        .prove_live_authority(
            &registry,
            &http_skill,
            &envelope_for(
                &http_skill,
                json!({
                    "url": replay_url,
                    "method": "get",
                    "json_pointers": ["/message", "/nested/count"],
                }),
                CapabilityGrantSet {
                    grants: vec![http_grant("127.0.0.1", 80, "/response.json")],
                },
            ),
            LiveProofComparatorProfile::NormalizedInspectOutputV1,
        )
        .unwrap();

    assert_eq!(proof_result.proof.proof_status, "bounded_minimal");
    assert!(proof_result.proof.residual_authority.grants.is_empty());
    assert_eq!(proof_result.proof.proven_authority.grants.len(), 1);
    assert!(proof_result.proof.replay_input_digest.is_some());
    let family = proof_result
        .proof
        .family_statuses
        .iter()
        .find(|status| status.family == CapabilityId::HttpRequest)
        .unwrap();
    assert_eq!(family.support, LiveProofSupport::BoundedLiveProof);
    assert_eq!(family.proof_status.as_deref(), Some("bounded_minimal"));
    match &proof_result.proof.proven_authority.grants[0].constraints {
        CapabilityConstraints::HttpRequest(value) => {
            assert_eq!(
                value.allowed_hosts.as_ref().unwrap(),
                &vec!["127.0.0.1".to_owned()]
            );
            assert_eq!(value.allowed_ports.as_ref().unwrap(), &vec![80]);
            assert_eq!(
                value.allowed_path_prefixes.as_ref().unwrap(),
                &vec!["/response.json".to_owned()]
            );
            assert_eq!(value.follow_redirects, Some(false));
            assert_eq!(value.allow_loopback, Some(true));
            assert_eq!(value.allow_ip_literals, Some(true));
        }
        other => panic!("expected http-request constraints, got {other:?}"),
    }
    assert!(
        proof_result
            .proof
            .candidate_trials
            .iter()
            .any(|trial| trial.family == CapabilityId::HttpRequest
                && trial.change_kind == "shrink_scope"
                && trial.accepted)
    );
}

#[test]
fn http_request_live_proof_is_bounded_with_replay_for_localhost_explicit_port_shape() {
    let temp = TempRegistry::new();
    temp.install(inspect_http_json_dir());
    let registry = temp.load();
    let replay_url = "http://localhost:18080/response.json";
    let runner = build_replay_runner(vec![localhost_http_replay_fixture(
        replay_url,
        18080,
        r#"{"service":"guild-http","message":"deterministic","nested":{"count":2},"items":[{"name":"alpha"},{"name":"beta"}]}"#,
    )]);
    let http_skill = registry
        .resolve(&requested_skill("inspect-http-json"))
        .unwrap();

    let proof_result = runner
        .prove_live_authority(
            &registry,
            &http_skill,
            &envelope_for(
                &http_skill,
                json!({
                    "url": replay_url,
                    "method": "get",
                    "json_pointers": ["/message", "/nested/count"],
                }),
                CapabilityGrantSet {
                    grants: vec![http_grant("localhost", 18080, "/response.json")],
                },
            ),
            LiveProofComparatorProfile::NormalizedInspectOutputV1,
        )
        .unwrap();

    assert_eq!(proof_result.proof.proof_status, "bounded_minimal");
    assert!(proof_result.proof.residual_authority.grants.is_empty());
    assert!(proof_result.proof.replay_input_digest.is_some());
    let family = proof_result
        .proof
        .family_statuses
        .iter()
        .find(|status| status.family == CapabilityId::HttpRequest)
        .unwrap();
    assert_eq!(family.support, LiveProofSupport::BoundedLiveProof);
    assert_eq!(family.proof_status.as_deref(), Some("bounded_minimal"));
    assert!(
        family
            .reason_codes
            .iter()
            .any(|code| code == "HTTP_LIVE_PROOF_BOUNDED")
    );
    match &proof_result.proof.proven_authority.grants[0].constraints {
        CapabilityConstraints::HttpRequest(value) => {
            assert_eq!(
                value.allowed_hosts.as_ref().unwrap(),
                &vec!["localhost".to_owned()]
            );
            assert_eq!(value.allowed_ports.as_ref().unwrap(), &vec![18080]);
            assert_eq!(
                value.allowed_methods.as_ref().unwrap(),
                &vec![HttpMethod::Get]
            );
            assert_eq!(
                value.allowed_path_prefixes.as_ref().unwrap(),
                &vec!["/response.json".to_owned()]
            );
            assert_eq!(value.follow_redirects, Some(false));
            assert_eq!(value.allow_loopback, Some(true));
            assert_eq!(value.allow_ip_literals, Some(false));
        }
        other => panic!("expected http-request constraints, got {other:?}"),
    }
    let observation = match &proof_result
        .baseline_execution_record
        .authority_observations[0]
    {
        guild_types::AuthorityObservation::HttpRequest { detail, .. } => detail,
        other => panic!("expected http-request observation, got {other:?}"),
    };
    let resolution = observation
        .resolution
        .as_ref()
        .expect("localhost live proof should persist the bound resolution");
    assert_eq!(resolution.requested_host, "localhost");
    assert_eq!(resolution.port, 18080);
    assert!(resolution.loopback_only);
}

#[test]
fn http_request_live_proof_is_bounded_with_replay_for_localhost_head_explicit_port_shape() {
    let temp = TempRegistry::new();
    temp.install(inspect_http_json_dir());
    let registry = temp.load();
    let replay_url = "http://localhost:18080/response.json";
    let runner = build_replay_runner(vec![localhost_head_http_replay_fixture(replay_url, 18080)]);
    let http_skill = registry
        .resolve(&requested_skill("inspect-http-json"))
        .unwrap();

    let proof_result = runner
        .prove_live_authority(
            &registry,
            &http_skill,
            &envelope_for(
                &http_skill,
                json!({
                    "url": replay_url,
                    "method": "head",
                }),
                CapabilityGrantSet {
                    grants: vec![head_http_grant("localhost", 18080, "/response.json")],
                },
            ),
            LiveProofComparatorProfile::NormalizedInspectOutputV1,
        )
        .unwrap();

    assert_eq!(proof_result.proof.proof_status, "bounded_minimal");
    assert!(proof_result.proof.residual_authority.grants.is_empty());
    assert!(proof_result.proof.replay_input_digest.is_some());
    let family = proof_result
        .proof
        .family_statuses
        .iter()
        .find(|status| status.family == CapabilityId::HttpRequest)
        .unwrap();
    assert_eq!(family.support, LiveProofSupport::BoundedLiveProof);
    assert_eq!(family.proof_status.as_deref(), Some("bounded_minimal"));
    assert!(
        family
            .reason_codes
            .iter()
            .any(|code| code == "HTTP_LIVE_PROOF_BOUNDED")
    );
    match &proof_result.proof.proven_authority.grants[0].constraints {
        CapabilityConstraints::HttpRequest(value) => {
            assert_eq!(
                value.allowed_hosts.as_ref().unwrap(),
                &vec!["localhost".to_owned()]
            );
            assert_eq!(value.allowed_ports.as_ref().unwrap(), &vec![18080]);
            assert_eq!(
                value.allowed_methods.as_ref().unwrap(),
                &vec![HttpMethod::Head]
            );
            assert_eq!(
                value.allowed_path_prefixes.as_ref().unwrap(),
                &vec!["/response.json".to_owned()]
            );
            assert_eq!(value.follow_redirects, Some(false));
            assert_eq!(value.allow_loopback, Some(true));
            assert_eq!(value.allow_ip_literals, Some(false));
        }
        other => panic!("expected http-request constraints, got {other:?}"),
    }
    let observation = match &proof_result
        .baseline_execution_record
        .authority_observations[0]
    {
        guild_types::AuthorityObservation::HttpRequest { detail, .. } => detail,
        other => panic!("expected http-request observation, got {other:?}"),
    };
    assert_eq!(observation.request.method, HttpMethod::Head);
    assert_eq!(observation.response_bytes, Some(0));
    let resolution = observation
        .resolution
        .as_ref()
        .expect("localhost HEAD live proof should persist the bound resolution");
    assert_eq!(resolution.requested_host, "localhost");
    assert_eq!(resolution.port, 18080);
    assert!(resolution.loopback_only);
}

#[test]
fn http_request_live_proof_is_bounded_with_replay_for_head_shape() {
    let temp = TempRegistry::new();
    temp.install(inspect_http_json_dir());
    let registry = temp.load();
    let replay_url = "http://127.0.0.1:18080/response.json";
    let runner = build_replay_runner(vec![head_http_replay_fixture(replay_url)]);
    let http_skill = registry
        .resolve(&requested_skill("inspect-http-json"))
        .unwrap();

    let proof_result = runner
        .prove_live_authority(
            &registry,
            &http_skill,
            &envelope_for(
                &http_skill,
                json!({
                    "url": replay_url,
                    "method": "head",
                }),
                CapabilityGrantSet {
                    grants: vec![head_http_grant("127.0.0.1", 18080, "/response.json")],
                },
            ),
            LiveProofComparatorProfile::NormalizedInspectOutputV1,
        )
        .unwrap();

    assert_eq!(proof_result.proof.proof_status, "bounded_minimal");
    assert!(proof_result.proof.residual_authority.grants.is_empty());
    assert_eq!(proof_result.proof.proven_authority.grants.len(), 1);
    let family = proof_result
        .proof
        .family_statuses
        .iter()
        .find(|status| status.family == CapabilityId::HttpRequest)
        .unwrap();
    assert_eq!(family.support, LiveProofSupport::BoundedLiveProof);
    assert_eq!(family.proof_status.as_deref(), Some("bounded_minimal"));
    assert!(
        family
            .reason_codes
            .iter()
            .any(|code| code == "HTTP_LIVE_PROOF_BOUNDED")
    );
    match &proof_result.proof.proven_authority.grants[0].constraints {
        CapabilityConstraints::HttpRequest(value) => {
            assert_eq!(
                value.allowed_hosts.as_ref().unwrap(),
                &vec!["127.0.0.1".to_owned()]
            );
            assert_eq!(value.allowed_ports.as_ref().unwrap(), &vec![18080]);
            assert_eq!(
                value.allowed_methods.as_ref().unwrap(),
                &vec![HttpMethod::Head]
            );
            assert_eq!(
                value.allowed_path_prefixes.as_ref().unwrap(),
                &vec!["/response.json".to_owned()]
            );
            assert_eq!(value.follow_redirects, Some(false));
            assert_eq!(value.allow_loopback, Some(true));
            assert_eq!(value.allow_ip_literals, Some(true));
        }
        other => panic!("expected http-request constraints, got {other:?}"),
    }
    assert!(
        proof_result
            .proof
            .candidate_trials
            .iter()
            .any(|trial| trial.family == CapabilityId::HttpRequest
                && trial.change_kind == "shrink_scope"
                && trial.accepted)
    );
    assert!(proof_result.proof.replay_input_digest.is_some());
}

#[test]
fn http_request_live_proof_is_bounded_with_replay_for_head_default_port_shape() {
    let temp = TempRegistry::new();
    temp.install(inspect_http_json_dir());
    let registry = temp.load();
    let replay_url = "http://127.0.0.1/response.json";
    let runner = build_replay_runner(vec![head_http_replay_fixture(replay_url)]);
    let http_skill = registry
        .resolve(&requested_skill("inspect-http-json"))
        .unwrap();

    let proof_result = runner
        .prove_live_authority(
            &registry,
            &http_skill,
            &envelope_for(
                &http_skill,
                json!({
                    "url": replay_url,
                    "method": "head",
                }),
                CapabilityGrantSet {
                    grants: vec![head_http_grant("127.0.0.1", 80, "/response.json")],
                },
            ),
            LiveProofComparatorProfile::NormalizedInspectOutputV1,
        )
        .unwrap();

    assert_eq!(proof_result.proof.proof_status, "bounded_minimal");
    assert!(proof_result.proof.residual_authority.grants.is_empty());
    assert_eq!(proof_result.proof.proven_authority.grants.len(), 1);
    let family = proof_result
        .proof
        .family_statuses
        .iter()
        .find(|status| status.family == CapabilityId::HttpRequest)
        .unwrap();
    assert_eq!(family.support, LiveProofSupport::BoundedLiveProof);
    assert_eq!(family.proof_status.as_deref(), Some("bounded_minimal"));
    match &proof_result.proof.proven_authority.grants[0].constraints {
        CapabilityConstraints::HttpRequest(value) => {
            assert_eq!(
                value.allowed_hosts.as_ref().unwrap(),
                &vec!["127.0.0.1".to_owned()]
            );
            assert_eq!(value.allowed_ports.as_ref().unwrap(), &vec![80]);
            assert_eq!(
                value.allowed_methods.as_ref().unwrap(),
                &vec![HttpMethod::Head]
            );
            assert_eq!(
                value.allowed_path_prefixes.as_ref().unwrap(),
                &vec!["/response.json".to_owned()]
            );
            assert_eq!(value.follow_redirects, Some(false));
            assert_eq!(value.allow_loopback, Some(true));
            assert_eq!(value.allow_ip_literals, Some(true));
        }
        other => panic!("expected http-request constraints, got {other:?}"),
    }
    assert!(
        proof_result
            .proof
            .candidate_trials
            .iter()
            .any(|trial| trial.family == CapabilityId::HttpRequest
                && trial.change_kind == "shrink_scope"
                && trial.accepted)
    );
    assert!(proof_result.proof.replay_input_digest.is_some());
}

#[test]
fn http_request_live_proof_stays_not_proven_without_replay() {
    let server = http_test_server::HttpTestServer::start();
    let temp = TempRegistry::new();
    temp.install(inspect_http_json_dir());
    let registry = temp.load();
    let runner = build_runner();
    let http_skill = registry
        .resolve(&requested_skill("inspect-http-json"))
        .unwrap();

    let proof_result = runner
        .prove_live_authority(
            &registry,
            &http_skill,
            &envelope_for(
                &http_skill,
                json!({
                    "url": server.json_url(),
                    "method": "get",
                    "json_pointers": ["/message"],
                }),
                CapabilityGrantSet {
                    grants: vec![http_grant(
                        http_test_server::HttpTestServer::host(),
                        server.port(),
                        "/json",
                    )],
                },
            ),
            LiveProofComparatorProfile::NormalizedInspectOutputV1,
        )
        .unwrap();

    assert_eq!(proof_result.proof.proof_status, "not_proven");
    assert!(proof_result.proof.proven_authority.grants.is_empty());
    assert_eq!(proof_result.proof.residual_authority.grants.len(), 1);
    let family = proof_result
        .proof
        .family_statuses
        .iter()
        .find(|status| status.family == CapabilityId::HttpRequest)
        .unwrap();
    assert_eq!(family.support, LiveProofSupport::NotProven);
    assert!(
        family
            .reason_codes
            .iter()
            .any(|code| code == "HTTP_REPLAY_FIXTURE_REQUIRED")
    );
}

#[test]
fn http_request_live_proof_stays_not_proven_for_unsupported_comparator() {
    let temp = TempRegistry::new();
    temp.install(inspect_http_json_dir());
    let registry = temp.load();
    let replay_url = "http://127.0.0.1:18080/response.json";
    let runner = build_replay_runner(vec![http_replay_fixture(
        replay_url,
        r#"{"service":"guild-http","message":"deterministic","nested":{"count":2},"items":[{"name":"alpha"},{"name":"beta"}]}"#,
    )]);
    let http_skill = registry
        .resolve(&requested_skill("inspect-http-json"))
        .unwrap();

    let proof_result = runner
        .prove_live_authority(
            &registry,
            &http_skill,
            &envelope_for(
                &http_skill,
                json!({
                    "url": replay_url,
                    "method": "get",
                    "json_pointers": ["/message"],
                }),
                CapabilityGrantSet {
                    grants: vec![http_grant("127.0.0.1", 18080, "/response.json")],
                },
            ),
            LiveProofComparatorProfile::ExactOutput,
        )
        .unwrap();

    let family = proof_result
        .proof
        .family_statuses
        .iter()
        .find(|status| status.family == CapabilityId::HttpRequest)
        .unwrap();
    assert_eq!(family.support, LiveProofSupport::NotProven);
    assert!(
        family
            .reason_codes
            .iter()
            .any(|code| code == "HTTP_COMPARATOR_UNSUPPORTED_FOR_LIVE_PROOF")
    );
}

#[test]
fn http_request_live_proof_stays_not_proven_for_redirect_shape() {
    let temp = TempRegistry::new();
    temp.install(inspect_http_json_dir());
    let registry = temp.load();
    let runner = build_replay_runner(vec![
        redirect_replay_fixture("http://127.0.0.1:18080/redirect.json", "/response.json"),
        http_replay_fixture(
            "http://127.0.0.1:18080/response.json",
            r#"{"service":"guild-http","message":"deterministic","nested":{"count":2},"items":[{"name":"alpha"},{"name":"beta"}]}"#,
        ),
    ]);
    let http_skill = registry
        .resolve(&requested_skill("inspect-http-json"))
        .unwrap();

    let proof_result = runner
        .prove_live_authority(
            &registry,
            &http_skill,
            &envelope_for(
                &http_skill,
                json!({
                    "url": "http://127.0.0.1:18080/redirect.json",
                    "method": "get",
                    "json_pointers": ["/message"],
                }),
                CapabilityGrantSet {
                    grants: vec![redirect_http_grant(
                        "127.0.0.1",
                        18080,
                        &["/redirect.json", "/response.json"],
                    )],
                },
            ),
            LiveProofComparatorProfile::NormalizedInspectOutputV1,
        )
        .unwrap();

    assert_eq!(proof_result.proof.proof_status, "not_proven");
    assert!(proof_result.proof.proven_authority.grants.is_empty());
    assert_eq!(proof_result.proof.residual_authority.grants.len(), 1);
    let family = proof_result
        .proof
        .family_statuses
        .iter()
        .find(|status| status.family == CapabilityId::HttpRequest)
        .unwrap();
    assert_eq!(family.support, LiveProofSupport::NotProven);
    assert!(
        family
            .reason_codes
            .iter()
            .any(|code| code == "HTTP_REDIRECTS_UNSUPPORTED")
    );
}

#[test]
fn read_resource_live_proof_fails_closed_for_query_resources() {
    let temp = TempRegistry::new();
    temp.install(hello_skill_dir());
    temp.install(summarize_execution_query_dir());
    let registry = temp.load();
    let runner = build_runner();

    let hello = registry.resolve(&requested_skill("hello-inspect")).unwrap();
    let _ = runner
        .execute(
            &registry,
            &hello,
            &envelope_for(
                &hello,
                json!({ "name": "Ada" }),
                CapabilityGrantSet {
                    grants: vec![emit_evidence_grant()],
                },
            ),
        )
        .unwrap();

    let summary = registry
        .resolve(&requested_skill("summarize-execution-query"))
        .unwrap();
    let proof_result = runner
        .prove_live_authority(
            &registry,
            &summary,
            &envelope_for(
                &summary,
                json!({
                    "query_uri": "guild://queries/executions/recent/1",
                }),
                CapabilityGrantSet {
                    grants: vec![read_resource_grant(&["guild://queries/executions/"])],
                },
            ),
            LiveProofComparatorProfile::NormalizedInspectOutputV1,
        )
        .unwrap();

    let family = proof_result
        .proof
        .family_statuses
        .iter()
        .find(|status| status.family == CapabilityId::ReadResource)
        .unwrap();
    assert_eq!(family.support, LiveProofSupport::NotProven);
    assert!(
        family
            .reason_codes
            .iter()
            .any(|code| code == "LIVE_SCOPE_SHRINK_UNSUPPORTED")
    );
}
