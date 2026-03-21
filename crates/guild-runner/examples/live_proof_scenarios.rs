use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use guild_registry::{LocalRegistry, LocalSourceInstaller, SkillRegistry, execution_resource_uri};
use guild_runner::{HttpReplayFixture, LiveProofComparatorProfile, Runner, WasmtimeRuntimeAdapter};
use guild_types::{
    Budget, CallerRequest, CapabilityAccess, CapabilityConstraints, CapabilityGrantSet,
    CapabilityId, EmitEvidenceConstraints, EvidenceAudience, ExecutionMode, GrantedCapability,
    HttpAddressFamily, HttpMethod, HttpRequestConstraints, HttpResolutionBinding,
    HttpResolvedAddress, HttpScheme, PolicyDecision, PolicyDecisionOutcome,
    ReadResourceConstraints, RedactionClass, RequestedSkillRef, ResolvedExecutionEnvelope,
    ResourceKind, Severity, SkillKey, VersionRequirement,
};
use serde_json::json;

#[path = "../../../test-support/http_test_server.rs"]
mod http_test_server;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
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

fn build_runner() -> Runner<WasmtimeRuntimeAdapter> {
    Runner::new(WasmtimeRuntimeAdapter::new().unwrap())
}

fn build_replay_runner(fixtures: Vec<HttpReplayFixture>) -> Runner<WasmtimeRuntimeAdapter> {
    Runner::new(WasmtimeRuntimeAdapter::new().unwrap())
        .with_http_replay_fixtures(fixtures)
        .unwrap()
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

fn log_grant(levels: &[Severity]) -> GrantedCapability {
    GrantedCapability {
        id: CapabilityId::LogWrite,
        access: CapabilityAccess::Write,
        constraints: CapabilityConstraints::Log(guild_types::LogConstraints {
            levels: Some(levels.to_vec()),
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

fn localhost_http_replay_fixture(url: &str, port: u16, body: &str) -> HttpReplayFixture {
    HttpReplayFixture {
        method: HttpMethod::Get,
        url: url.to_owned(),
        response_status: 200,
        response_content_type: Some("application/json".into()),
        response_body: body.as_bytes().to_vec(),
        redirect_location: None,
        resolution_binding: Some(localhost_resolution_binding(port)),
    }
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
    input: serde_json::Value,
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
        let root = std::env::temp_dir().join(unique_id("guild-live-proof-example"));
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

fn run_read_resource_bounded() -> serde_json::Value {
    let temp = TempRegistry::new();
    temp.install(repo_root().join("examples/skills/hello-inspect"));
    temp.install(repo_root().join("examples/skills/explain-execution"));
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
    let result = runner
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

    json!({
        "scenario": "read-resource-bounded",
        "baseline_execution_record": result.baseline_execution_record,
        "proof": result.proof,
    })
}

fn run_http_request_bounded() -> serde_json::Value {
    let temp = TempRegistry::new();
    temp.install(repo_root().join("examples/skills/inspect-http-json"));
    let registry = temp.load();
    let replay_url = "http://127.0.0.1:18080/response.json";
    let runner = build_replay_runner(vec![http_replay_fixture(
        replay_url,
        r#"{"service":"guild-http","message":"deterministic","nested":{"count":2},"items":[{"name":"alpha"},{"name":"beta"}]}"#,
    )]);
    let http_skill = registry
        .resolve(&requested_skill("inspect-http-json"))
        .unwrap();

    let result = runner
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

    json!({
        "scenario": "http-request-bounded",
        "baseline_execution_record": result.baseline_execution_record,
        "proof": result.proof,
    })
}

fn run_http_request_default_port_bounded() -> serde_json::Value {
    let temp = TempRegistry::new();
    temp.install(repo_root().join("examples/skills/inspect-http-json"));
    let registry = temp.load();
    let replay_url = "http://127.0.0.1/response.json";
    let runner = build_replay_runner(vec![http_replay_fixture(
        replay_url,
        r#"{"service":"guild-http","message":"deterministic","nested":{"count":2},"items":[{"name":"alpha"},{"name":"beta"}]}"#,
    )]);
    let http_skill = registry
        .resolve(&requested_skill("inspect-http-json"))
        .unwrap();

    let result = runner
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

    json!({
        "scenario": "http-request-default-port-bounded",
        "baseline_execution_record": result.baseline_execution_record,
        "proof": result.proof,
    })
}

fn run_http_request_head_bounded() -> serde_json::Value {
    let temp = TempRegistry::new();
    temp.install(repo_root().join("examples/skills/inspect-http-json"));
    let registry = temp.load();
    let replay_url = "http://127.0.0.1:18080/response.json";
    let runner = build_replay_runner(vec![head_http_replay_fixture(replay_url)]);
    let http_skill = registry
        .resolve(&requested_skill("inspect-http-json"))
        .unwrap();

    let result = runner
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

    json!({
        "scenario": "http-request-head-bounded",
        "baseline_execution_record": result.baseline_execution_record,
        "proof": result.proof,
    })
}

fn run_http_request_localhost_bounded() -> serde_json::Value {
    let temp = TempRegistry::new();
    temp.install(repo_root().join("examples/skills/inspect-http-json"));
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

    let result = runner
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

    json!({
        "scenario": "http-request-localhost-bounded",
        "baseline_execution_record": result.baseline_execution_record,
        "proof": result.proof,
    })
}

fn run_http_request_head_default_port_bounded() -> serde_json::Value {
    let temp = TempRegistry::new();
    temp.install(repo_root().join("examples/skills/inspect-http-json"));
    let registry = temp.load();
    let replay_url = "http://127.0.0.1/response.json";
    let runner = build_replay_runner(vec![head_http_replay_fixture(replay_url)]);
    let http_skill = registry
        .resolve(&requested_skill("inspect-http-json"))
        .unwrap();

    let result = runner
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

    json!({
        "scenario": "http-request-head-default-port-bounded",
        "baseline_execution_record": result.baseline_execution_record,
        "proof": result.proof,
    })
}

fn run_http_request_redirect_unsupported() -> serde_json::Value {
    let temp = TempRegistry::new();
    temp.install(repo_root().join("examples/skills/inspect-http-json"));
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

    let result = runner
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

    json!({
        "scenario": "http-request-redirect-unsupported",
        "baseline_execution_record": result.baseline_execution_record,
        "proof": result.proof,
    })
}

fn run_http_request_no_replay() -> serde_json::Value {
    let server = http_test_server::HttpTestServer::start();
    let temp = TempRegistry::new();
    temp.install(repo_root().join("examples/skills/inspect-http-json"));
    let registry = temp.load();
    let runner = build_runner();
    let http_skill = registry
        .resolve(&requested_skill("inspect-http-json"))
        .unwrap();

    let result = runner
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

    json!({
        "scenario": "http-request-no-replay",
        "baseline_execution_record": result.baseline_execution_record,
        "proof": result.proof,
    })
}

fn run_log_write_reduced() -> serde_json::Value {
    let temp = TempRegistry::new();
    temp.install(repo_root().join("examples/skills/hello-inspect"));
    let registry = temp.load();
    let runner = build_runner();
    let hello = registry.resolve(&requested_skill("hello-inspect")).unwrap();

    let result = runner
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

    json!({
        "scenario": "log-write-reduced",
        "baseline_execution_record": result.baseline_execution_record,
        "proof": result.proof,
    })
}

fn main() {
    let scenario = env::args()
        .nth(1)
        .unwrap_or_else(|| "read-resource-bounded".into());
    let output = match scenario.as_str() {
        "read-resource-bounded" => run_read_resource_bounded(),
        "http-request-bounded" => run_http_request_bounded(),
        "http-request-default-port-bounded" => run_http_request_default_port_bounded(),
        "http-request-localhost-bounded" => run_http_request_localhost_bounded(),
        "http-request-head-bounded" => run_http_request_head_bounded(),
        "http-request-head-default-port-bounded" => run_http_request_head_default_port_bounded(),
        "http-request-redirect-unsupported" => run_http_request_redirect_unsupported(),
        "http-request-no-replay" | "http-request-not-proven" => run_http_request_no_replay(),
        "log-write-reduced" => run_log_write_reduced(),
        other => {
            eprintln!("unknown live proof scenario: {other}");
            std::process::exit(2);
        }
    };
    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}
