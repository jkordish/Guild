use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use guild_registry::{LocalRegistry, LocalSourceInstaller, SkillRegistry, execution_resource_uri};
use guild_runner::{
    HttpReplayFixture, LiveProofComparatorProfile, LiveProofScenarioResult, Runner,
    WasmtimeRuntimeAdapter,
};
use guild_types::{
    Budget, CallerRequest, CapabilityAccess, CapabilityConstraints, CapabilityGrantSet,
    CapabilityId, EmitEvidenceConstraints, EvidenceAudience, ExecutionMode, GrantedCapability,
    HttpAddressFamily, HttpMethod, HttpRequestConstraints, HttpResolutionBinding,
    HttpResolvedAddress, HttpScheme, InvokeDependencyConstraints, PolicyDecision,
    PolicyDecisionOutcome, ReadResourceConstraints, RedactionClass, RequestedSkillRef,
    ResolvedExecutionEnvelope, ResourceKind, Severity, SkillKey, VersionRequirement,
};
use serde::Serialize;
use serde_json::{Value, json};

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

fn invoke_skill_grant(aliases: &[&str]) -> GrantedCapability {
    GrantedCapability {
        id: CapabilityId::InvokeSkill,
        access: CapabilityAccess::Invoke,
        constraints: CapabilityConstraints::InvokeDependency(InvokeDependencyConstraints {
            aliases: Some(aliases.iter().map(|alias| (*alias).to_owned()).collect()),
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

struct PreparedScenario {
    _temp: TempRegistry,
    _server: Option<http_test_server::HttpTestServer>,
    registry: LocalRegistry,
    runner: Runner<WasmtimeRuntimeAdapter>,
    installed: guild_registry::InstalledSkill,
    envelope: ResolvedExecutionEnvelope,
    comparator: LiveProofComparatorProfile,
}

impl PreparedScenario {
    fn prove(&self) -> LiveProofScenarioResult {
        self.runner
            .prove_live_authority(
                &self.registry,
                &self.installed,
                &self.envelope,
                self.comparator,
            )
            .unwrap()
    }
}

#[derive(Debug, Serialize)]
struct TimingSummary {
    operation: &'static str,
    cache_present: bool,
    cache_notes: &'static str,
    cold_first_run_ms: f64,
    warmup_runs: usize,
    measured_runs: usize,
    samples_ms: Vec<f64>,
    mean_ms: f64,
    p50_ms: f64,
    p95_ms: f64,
    max_ms: f64,
}

fn duration_ms(started: Instant) -> f64 {
    started.elapsed().as_secs_f64() * 1000.0
}

fn percentile(samples: &[f64], percentile: f64) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }

    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss
    )]
    let rank = ((sorted.len() - 1) as f64 * percentile).round() as usize;
    sorted[rank]
}

fn summarize_samples(
    cold_first_run_ms: f64,
    warmup_runs: usize,
    samples_ms: Vec<f64>,
) -> TimingSummary {
    let measured_runs = samples_ms.len();
    #[allow(clippy::cast_precision_loss)]
    let mean_ms = if measured_runs == 0 {
        0.0
    } else {
        samples_ms.iter().sum::<f64>() / measured_runs as f64
    };
    let p50_ms = percentile(&samples_ms, 0.50);
    let p95_ms = percentile(&samples_ms, 0.95);
    let max_ms = samples_ms.iter().copied().fold(0.0, f64::max);

    TimingSummary {
        operation: "prove_live_authority",
        cache_present: false,
        cache_notes: "No live-runtime proof cache exists today.",
        cold_first_run_ms,
        warmup_runs,
        measured_runs,
        samples_ms,
        mean_ms,
        p50_ms,
        p95_ms,
        max_ms,
    }
}

fn benchmark_scenario(
    scenario_name: &str,
    warmup_runs: usize,
    measured_runs: usize,
    prepare: fn() -> PreparedScenario,
) -> Value {
    let scenario = prepare();
    let cold_started = Instant::now();
    let cold_result = scenario.prove();
    let cold_first_run_ms = duration_ms(cold_started);

    for _ in 0..warmup_runs {
        scenario.prove();
    }

    let mut samples_ms = Vec::with_capacity(measured_runs);
    for _ in 0..measured_runs {
        let started = Instant::now();
        scenario.prove();
        samples_ms.push(duration_ms(started));
    }

    json!({
        "kind": "guild.live_proof_benchmark",
        "version": "1.0.0",
        "scenario": scenario_name,
        "baseline_execution_record": cold_result.baseline_execution_record,
        "proof": cold_result.proof,
        "timing": summarize_samples(cold_first_run_ms, warmup_runs, samples_ms),
    })
}

#[allow(clippy::needless_pass_by_value)]
fn scenario_output(scenario_name: &str, result: LiveProofScenarioResult) -> Value {
    json!({
        "scenario": scenario_name,
        "baseline_execution_record": result.baseline_execution_record,
        "proof": result.proof,
    })
}

fn prepare_read_resource_bounded() -> PreparedScenario {
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
    PreparedScenario {
        _temp: temp,
        _server: None,
        registry,
        runner,
        installed: explain.clone(),
        envelope: envelope_for(
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
        comparator: LiveProofComparatorProfile::NormalizedInspectOutputV1,
    }
}

fn run_read_resource_bounded() -> Value {
    scenario_output(
        "read-resource-bounded",
        prepare_read_resource_bounded().prove(),
    )
}

fn prepare_read_resource_query_unsupported() -> PreparedScenario {
    let temp = TempRegistry::new();
    temp.install(repo_root().join("examples/skills/hello-inspect"));
    temp.install(repo_root().join("examples/skills/summarize-execution-query"));
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
    PreparedScenario {
        _temp: temp,
        _server: None,
        registry,
        runner,
        installed: summary.clone(),
        envelope: envelope_for(
            &summary,
            json!({
                "query_uri": "guild://queries/executions/recent/1",
            }),
            CapabilityGrantSet {
                grants: vec![read_resource_grant(&["guild://queries/executions/"])],
            },
        ),
        comparator: LiveProofComparatorProfile::NormalizedInspectOutputV1,
    }
}

fn run_read_resource_query_unsupported() -> Value {
    scenario_output(
        "read-resource-query-unsupported",
        prepare_read_resource_query_unsupported().prove(),
    )
}

fn prepare_http_request_bounded() -> PreparedScenario {
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
    PreparedScenario {
        _temp: temp,
        _server: None,
        registry,
        runner,
        installed: http_skill.clone(),
        envelope: envelope_for(
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
        comparator: LiveProofComparatorProfile::NormalizedInspectOutputV1,
    }
}

fn run_http_request_bounded() -> Value {
    scenario_output(
        "http-request-bounded",
        prepare_http_request_bounded().prove(),
    )
}

fn prepare_http_request_default_port_bounded() -> PreparedScenario {
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
    PreparedScenario {
        _temp: temp,
        _server: None,
        registry,
        runner,
        installed: http_skill.clone(),
        envelope: envelope_for(
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
        comparator: LiveProofComparatorProfile::NormalizedInspectOutputV1,
    }
}

fn run_http_request_default_port_bounded() -> Value {
    scenario_output(
        "http-request-default-port-bounded",
        prepare_http_request_default_port_bounded().prove(),
    )
}

fn prepare_http_request_head_bounded() -> PreparedScenario {
    let temp = TempRegistry::new();
    temp.install(repo_root().join("examples/skills/inspect-http-json"));
    let registry = temp.load();
    let replay_url = "http://127.0.0.1:18080/response.json";
    let runner = build_replay_runner(vec![head_http_replay_fixture(replay_url)]);
    let http_skill = registry
        .resolve(&requested_skill("inspect-http-json"))
        .unwrap();
    PreparedScenario {
        _temp: temp,
        _server: None,
        registry,
        runner,
        installed: http_skill.clone(),
        envelope: envelope_for(
            &http_skill,
            json!({
                "url": replay_url,
                "method": "head",
            }),
            CapabilityGrantSet {
                grants: vec![head_http_grant("127.0.0.1", 18080, "/response.json")],
            },
        ),
        comparator: LiveProofComparatorProfile::NormalizedInspectOutputV1,
    }
}

fn run_http_request_head_bounded() -> Value {
    scenario_output(
        "http-request-head-bounded",
        prepare_http_request_head_bounded().prove(),
    )
}

fn prepare_http_request_localhost_bounded() -> PreparedScenario {
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
    PreparedScenario {
        _temp: temp,
        _server: None,
        registry,
        runner,
        installed: http_skill.clone(),
        envelope: envelope_for(
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
        comparator: LiveProofComparatorProfile::NormalizedInspectOutputV1,
    }
}

fn run_http_request_localhost_bounded() -> Value {
    scenario_output(
        "http-request-localhost-bounded",
        prepare_http_request_localhost_bounded().prove(),
    )
}

fn prepare_http_request_head_default_port_bounded() -> PreparedScenario {
    let temp = TempRegistry::new();
    temp.install(repo_root().join("examples/skills/inspect-http-json"));
    let registry = temp.load();
    let replay_url = "http://127.0.0.1/response.json";
    let runner = build_replay_runner(vec![head_http_replay_fixture(replay_url)]);
    let http_skill = registry
        .resolve(&requested_skill("inspect-http-json"))
        .unwrap();
    PreparedScenario {
        _temp: temp,
        _server: None,
        registry,
        runner,
        installed: http_skill.clone(),
        envelope: envelope_for(
            &http_skill,
            json!({
                "url": replay_url,
                "method": "head",
            }),
            CapabilityGrantSet {
                grants: vec![head_http_grant("127.0.0.1", 80, "/response.json")],
            },
        ),
        comparator: LiveProofComparatorProfile::NormalizedInspectOutputV1,
    }
}

fn run_http_request_head_default_port_bounded() -> Value {
    scenario_output(
        "http-request-head-default-port-bounded",
        prepare_http_request_head_default_port_bounded().prove(),
    )
}

fn prepare_http_request_localhost_head_bounded() -> PreparedScenario {
    let temp = TempRegistry::new();
    temp.install(repo_root().join("examples/skills/inspect-http-json"));
    let registry = temp.load();
    let replay_url = "http://localhost:18080/response.json";
    let runner = build_replay_runner(vec![localhost_head_http_replay_fixture(replay_url, 18080)]);
    let http_skill = registry
        .resolve(&requested_skill("inspect-http-json"))
        .unwrap();
    PreparedScenario {
        _temp: temp,
        _server: None,
        registry,
        runner,
        installed: http_skill.clone(),
        envelope: envelope_for(
            &http_skill,
            json!({
                "url": replay_url,
                "method": "head",
            }),
            CapabilityGrantSet {
                grants: vec![head_http_grant("localhost", 18080, "/response.json")],
            },
        ),
        comparator: LiveProofComparatorProfile::NormalizedInspectOutputV1,
    }
}

fn run_http_request_localhost_head_bounded() -> Value {
    scenario_output(
        "http-request-localhost-head-bounded",
        prepare_http_request_localhost_head_bounded().prove(),
    )
}

fn prepare_http_request_redirect_unsupported() -> PreparedScenario {
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
    PreparedScenario {
        _temp: temp,
        _server: None,
        registry,
        runner,
        installed: http_skill.clone(),
        envelope: envelope_for(
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
        comparator: LiveProofComparatorProfile::NormalizedInspectOutputV1,
    }
}

fn run_http_request_redirect_unsupported() -> Value {
    scenario_output(
        "http-request-redirect-unsupported",
        prepare_http_request_redirect_unsupported().prove(),
    )
}

fn prepare_http_request_no_replay() -> PreparedScenario {
    let server = http_test_server::HttpTestServer::start();
    let temp = TempRegistry::new();
    temp.install(repo_root().join("examples/skills/inspect-http-json"));
    let registry = temp.load();
    let runner = build_runner();
    let http_skill = registry
        .resolve(&requested_skill("inspect-http-json"))
        .unwrap();
    let envelope = envelope_for(
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
    );
    PreparedScenario {
        _temp: temp,
        _server: Some(server),
        registry,
        runner,
        installed: http_skill,
        envelope,
        comparator: LiveProofComparatorProfile::NormalizedInspectOutputV1,
    }
}

fn run_http_request_no_replay() -> Value {
    scenario_output(
        "http-request-no-replay",
        prepare_http_request_no_replay().prove(),
    )
}

fn prepare_log_write_reduced() -> PreparedScenario {
    let temp = TempRegistry::new();
    temp.install(repo_root().join("examples/skills/hello-inspect"));
    let registry = temp.load();
    let runner = build_runner();
    let hello = registry.resolve(&requested_skill("hello-inspect")).unwrap();
    PreparedScenario {
        _temp: temp,
        _server: None,
        registry,
        runner,
        installed: hello.clone(),
        envelope: envelope_for(
            &hello,
            json!({ "name": "Ada", "emit_log": true }),
            CapabilityGrantSet {
                grants: vec![
                    emit_evidence_grant(),
                    log_grant(&[Severity::Info, Severity::Error]),
                ],
            },
        ),
        comparator: LiveProofComparatorProfile::NormalizedInspectOutputV1,
    }
}

fn run_log_write_reduced() -> Value {
    scenario_output("log-write-reduced", prepare_log_write_reduced().prove())
}

fn prepare_emit_evidence_single_sink_replay_unavailable() -> PreparedScenario {
    let temp = TempRegistry::new();
    temp.install(repo_root().join("examples/skills/hello-inspect"));
    let registry = temp.load();
    let runner = build_runner();
    let hello = registry.resolve(&requested_skill("hello-inspect")).unwrap();
    PreparedScenario {
        _temp: temp,
        _server: None,
        registry,
        runner,
        installed: hello.clone(),
        envelope: envelope_for(
            &hello,
            json!({ "name": "Ada" }),
            CapabilityGrantSet {
                grants: vec![emit_evidence_grant()],
            },
        ),
        comparator: LiveProofComparatorProfile::NormalizedInspectSingleSinkEmitEvidenceV1,
    }
}

fn run_emit_evidence_single_sink_replay_unavailable() -> Value {
    scenario_output(
        "emit-evidence-single-sink-replay-unavailable",
        prepare_emit_evidence_single_sink_replay_unavailable().prove(),
    )
}

fn prepare_invoke_skill_single_child_bounded() -> PreparedScenario {
    let temp = TempRegistry::new();
    temp.install(repo_root().join("examples/skills/invoke-child-zero"));
    temp.install(repo_root().join("examples/skills/invoke-parent-single-child"));
    let registry = temp.load();
    let runner = build_runner();
    let parent = registry
        .resolve(&requested_skill("invoke-parent-single-child"))
        .unwrap();
    PreparedScenario {
        _temp: temp,
        _server: None,
        registry,
        runner,
        installed: parent.clone(),
        envelope: envelope_for(
            &parent,
            json!({ "name": "Ada" }),
            CapabilityGrantSet {
                grants: vec![invoke_skill_grant(&["child"])],
            },
        ),
        comparator: LiveProofComparatorProfile::NormalizedInspectSingleChildInvokeV1,
    }
}

fn run_invoke_skill_single_child_bounded() -> Value {
    scenario_output(
        "invoke-skill-single-child-bounded",
        prepare_invoke_skill_single_child_bounded().prove(),
    )
}

fn prepare_invoke_skill_multi_child_unsupported() -> PreparedScenario {
    let temp = TempRegistry::new();
    temp.install(repo_root().join("examples/skills/invoke-child-zero"));
    temp.install(repo_root().join("examples/skills/invoke-parent-single-child"));
    let registry = temp.load();
    let runner = build_runner();
    let parent = registry
        .resolve(&requested_skill("invoke-parent-single-child"))
        .unwrap();

    PreparedScenario {
        _temp: temp,
        _server: None,
        registry,
        runner,
        installed: parent.clone(),
        envelope: envelope_for(
            &parent,
            json!({ "name": "Ada", "invoke_twice": true }),
            CapabilityGrantSet {
                grants: vec![invoke_skill_grant(&["child"])],
            },
        ),
        comparator: LiveProofComparatorProfile::NormalizedInspectSingleChildInvokeV1,
    }
}

fn run_invoke_skill_multi_child_unsupported() -> Value {
    scenario_output(
        "invoke-skill-multi-child-unsupported",
        prepare_invoke_skill_multi_child_unsupported().prove(),
    )
}

fn prepare_invoke_skill_child_authority_unsupported() -> PreparedScenario {
    let temp = TempRegistry::new();
    temp.install(repo_root().join("examples/skills/hello-inspect"));
    temp.install(repo_root().join("examples/skills/hello-composite"));
    let registry = temp.load();
    let runner = build_runner();
    let composite = registry
        .resolve(&requested_skill("hello-composite"))
        .unwrap();
    PreparedScenario {
        _temp: temp,
        _server: None,
        registry,
        runner,
        installed: composite.clone(),
        envelope: envelope_for(
            &composite,
            json!({ "name": "Ada" }),
            CapabilityGrantSet {
                grants: vec![invoke_skill_grant(&["hello"]), emit_evidence_grant()],
            },
        ),
        comparator: LiveProofComparatorProfile::NormalizedInspectSingleChildInvokeV1,
    }
}

fn run_invoke_skill_child_authority_unsupported() -> Value {
    scenario_output(
        "invoke-skill-child-authority-unsupported",
        prepare_invoke_skill_child_authority_unsupported().prove(),
    )
}

fn run_named_scenario(name: &str) -> Value {
    match name {
        "read-resource-bounded" => run_read_resource_bounded(),
        "read-resource-query-unsupported" => run_read_resource_query_unsupported(),
        "http-request-bounded" => run_http_request_bounded(),
        "http-request-default-port-bounded" => run_http_request_default_port_bounded(),
        "http-request-localhost-bounded" => run_http_request_localhost_bounded(),
        "http-request-localhost-head-bounded" => run_http_request_localhost_head_bounded(),
        "http-request-head-bounded" => run_http_request_head_bounded(),
        "http-request-head-default-port-bounded" => run_http_request_head_default_port_bounded(),
        "http-request-redirect-unsupported" => run_http_request_redirect_unsupported(),
        "http-request-no-replay" | "http-request-not-proven" => run_http_request_no_replay(),
        "log-write-reduced" => run_log_write_reduced(),
        "emit-evidence-single-sink-replay-unavailable" => {
            run_emit_evidence_single_sink_replay_unavailable()
        }
        "invoke-skill-single-child-bounded" => run_invoke_skill_single_child_bounded(),
        "invoke-skill-multi-child-unsupported" => run_invoke_skill_multi_child_unsupported(),
        "invoke-skill-child-authority-unsupported" => {
            run_invoke_skill_child_authority_unsupported()
        }
        other => {
            eprintln!("unknown live proof scenario: {other}");
            std::process::exit(2);
        }
    }
}

fn benchmark_named_scenario(name: &str, warmup_runs: usize, measured_runs: usize) -> Value {
    match name {
        "read-resource-bounded" => benchmark_scenario(
            "read-resource-bounded",
            warmup_runs,
            measured_runs,
            prepare_read_resource_bounded,
        ),
        "read-resource-query-unsupported" => benchmark_scenario(
            "read-resource-query-unsupported",
            warmup_runs,
            measured_runs,
            prepare_read_resource_query_unsupported,
        ),
        "http-request-bounded" => benchmark_scenario(
            "http-request-bounded",
            warmup_runs,
            measured_runs,
            prepare_http_request_bounded,
        ),
        "http-request-default-port-bounded" => benchmark_scenario(
            "http-request-default-port-bounded",
            warmup_runs,
            measured_runs,
            prepare_http_request_default_port_bounded,
        ),
        "http-request-localhost-bounded" => benchmark_scenario(
            "http-request-localhost-bounded",
            warmup_runs,
            measured_runs,
            prepare_http_request_localhost_bounded,
        ),
        "http-request-localhost-head-bounded" => benchmark_scenario(
            "http-request-localhost-head-bounded",
            warmup_runs,
            measured_runs,
            prepare_http_request_localhost_head_bounded,
        ),
        "http-request-head-bounded" => benchmark_scenario(
            "http-request-head-bounded",
            warmup_runs,
            measured_runs,
            prepare_http_request_head_bounded,
        ),
        "http-request-head-default-port-bounded" => benchmark_scenario(
            "http-request-head-default-port-bounded",
            warmup_runs,
            measured_runs,
            prepare_http_request_head_default_port_bounded,
        ),
        "http-request-redirect-unsupported" => benchmark_scenario(
            "http-request-redirect-unsupported",
            warmup_runs,
            measured_runs,
            prepare_http_request_redirect_unsupported,
        ),
        "http-request-no-replay" | "http-request-not-proven" => benchmark_scenario(
            "http-request-no-replay",
            warmup_runs,
            measured_runs,
            prepare_http_request_no_replay,
        ),
        "log-write-reduced" => benchmark_scenario(
            "log-write-reduced",
            warmup_runs,
            measured_runs,
            prepare_log_write_reduced,
        ),
        "emit-evidence-single-sink-replay-unavailable" => benchmark_scenario(
            "emit-evidence-single-sink-replay-unavailable",
            warmup_runs,
            measured_runs,
            prepare_emit_evidence_single_sink_replay_unavailable,
        ),
        "invoke-skill-single-child-bounded" => benchmark_scenario(
            "invoke-skill-single-child-bounded",
            warmup_runs,
            measured_runs,
            prepare_invoke_skill_single_child_bounded,
        ),
        "invoke-skill-multi-child-unsupported" => benchmark_scenario(
            "invoke-skill-multi-child-unsupported",
            warmup_runs,
            measured_runs,
            prepare_invoke_skill_multi_child_unsupported,
        ),
        "invoke-skill-child-authority-unsupported" => benchmark_scenario(
            "invoke-skill-child-authority-unsupported",
            warmup_runs,
            measured_runs,
            prepare_invoke_skill_child_authority_unsupported,
        ),
        other => {
            eprintln!("unknown live proof benchmark scenario: {other}");
            std::process::exit(2);
        }
    }
}

fn main() {
    let mut args = env::args().skip(1);
    let output = match args.next().as_deref() {
        Some("benchmark") => {
            let scenario = args
                .next()
                .unwrap_or_else(|| "read-resource-bounded".into());
            let warmup_runs = args
                .next()
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(2);
            let measured_runs = args
                .next()
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(10);
            benchmark_named_scenario(&scenario, warmup_runs, measured_runs)
        }
        Some(scenario) => run_named_scenario(scenario),
        None => run_named_scenario("read-resource-bounded"),
    };
    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}
