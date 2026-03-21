use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use guild_registry::{LocalRegistry, LocalSourceInstaller, SkillRegistry};
use guild_runner::{ExecutionError, HttpReplayFixture, Runner, WasmtimeRuntimeAdapter};
use guild_types::{
    AuthorityObservation, AuthorityObservationStatus, Budget, CallerRequest, CapabilityAccess,
    CapabilityConstraints, CapabilityGrantSet, CapabilityId, ExecutionMode, ExecutionRecord,
    ExecutionStatus, GrantedCapability, HttpMethod, HttpRequestConstraints, HttpScheme,
    InvokeDependencyConstraints, PolicyDecision, PolicyDecisionOutcome, RequestedSkillRef,
    ResolvedExecutionEnvelope, SkillKey, VersionRequirement,
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

fn http_skill_dir() -> PathBuf {
    repo_root().join("examples/skills/inspect-http-json")
}

fn wit_dir() -> PathBuf {
    repo_root().join("wit")
}

fn prepared_registry_root() -> &'static PathBuf {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();

    ROOT.get_or_init(|| {
        let root = repo_root().join("target/test-install-registry/guild-runner-http");
        if root.exists() {
            let _ = fs::remove_dir_all(&root);
        }

        LocalSourceInstaller::new(&root)
            .unwrap()
            .install(http_skill_dir())
            .unwrap();
        root
    })
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

fn load_registry() -> LocalRegistry {
    LocalRegistry::load(prepared_registry_root()).unwrap()
}

fn build_runner() -> Runner<WasmtimeRuntimeAdapter> {
    Runner::new(WasmtimeRuntimeAdapter::new().unwrap())
}

fn build_replay_runner(fixtures: Vec<HttpReplayFixture>) -> Runner<WasmtimeRuntimeAdapter> {
    Runner::new(WasmtimeRuntimeAdapter::new().unwrap())
        .with_http_replay_fixtures(fixtures)
        .unwrap()
}

fn http_replay_fixture(url: &str, body: &str) -> HttpReplayFixture {
    HttpReplayFixture {
        method: HttpMethod::Get,
        url: url.to_owned(),
        response_status: 200,
        response_content_type: Some("application/json".into()),
        response_body: body.as_bytes().to_vec(),
        redirect_location: None,
    }
}

fn execution_request(
    installed: &guild_registry::InstalledSkill,
    request_id: impl Into<String>,
    input: Value,
    grants: CapabilityGrantSet,
    budget: Budget,
) -> ResolvedExecutionEnvelope {
    let request_id = request_id.into();

    ResolvedExecutionEnvelope {
        request: CallerRequest {
            request_id: format!("{request_id}-request"),
            skill: requested_skill(&installed.resolved_ref.key.name),
            tenant_id: "tenant-1".into(),
            actor_id: "actor-1".into(),
            mode: ExecutionMode::Inspect,
            input,
            budget,
            requested_capabilities: grants.clone(),
            idempotency_key: None,
            trace_id: format!("{request_id}-trace"),
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

fn http_grant(
    host: &str,
    port: u16,
    path_prefix: &str,
    methods: &[HttpMethod],
    schemes: &[HttpScheme],
    max_timeout_ms: u64,
    max_response_bytes: u64,
) -> GrantedCapability {
    http_grant_with_options(
        host,
        port,
        &[path_prefix],
        methods,
        schemes,
        max_timeout_ms,
        max_response_bytes,
        None,
        Some(true),
        None,
        None,
        Some(true),
    )
}

#[allow(clippy::too_many_arguments)]
fn http_grant_with_options(
    host: &str,
    port: u16,
    path_prefixes: &[&str],
    methods: &[HttpMethod],
    schemes: &[HttpScheme],
    max_timeout_ms: u64,
    max_response_bytes: u64,
    max_redirects: Option<u8>,
    allow_loopback: Option<bool>,
    allow_link_local: Option<bool>,
    allow_private_networks: Option<bool>,
    allow_ip_literals: Option<bool>,
) -> GrantedCapability {
    GrantedCapability {
        id: CapabilityId::HttpRequest,
        access: CapabilityAccess::Read,
        constraints: CapabilityConstraints::HttpRequest(HttpRequestConstraints {
            allowed_schemes: Some(schemes.to_vec()),
            allowed_hosts: Some(vec![host.to_owned()]),
            allowed_host_suffixes: None,
            allowed_ports: Some(vec![port]),
            allowed_methods: Some(methods.to_vec()),
            allowed_path_prefixes: Some(
                path_prefixes
                    .iter()
                    .map(|path_prefix| (*path_prefix).to_owned())
                    .collect(),
            ),
            max_timeout_ms: Some(max_timeout_ms),
            max_response_bytes: Some(max_response_bytes),
            follow_redirects: max_redirects.map(|_| true),
            max_redirects,
            allow_loopback,
            allow_link_local,
            allow_private_networks,
            allow_ip_literals,
        }),
    }
}

fn run_http_skill(
    registry: &LocalRegistry,
    runner: &Runner<WasmtimeRuntimeAdapter>,
    input: Value,
    grants: CapabilityGrantSet,
    budget: Budget,
) -> Result<ExecutionRecord, ExecutionError> {
    let installed = registry
        .resolve(&requested_skill("inspect-http-json"))
        .unwrap();
    runner.execute(
        registry,
        &installed,
        &execution_request(
            &installed,
            unique_id("inspect-http-json"),
            input,
            grants,
            budget,
        ),
    )
}

fn persisted_error_record(
    registry: &LocalRegistry,
    error: &ExecutionError,
) -> guild_types::ExecutionRecord {
    let receipt = error
        .receipt
        .as_ref()
        .expect("unsuccessful HTTP execution persists a receipt");
    registry
        .load_execution_record(&receipt.execution_id)
        .unwrap()
}

fn unique_id(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{prefix}-{nanos}")
}

struct TempFixtureDir {
    path: PathBuf,
}

impl TempFixtureDir {
    fn new(prefix: &str) -> Self {
        let path = std::env::temp_dir().join(unique_id(prefix));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempFixtureDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn copy_dir_recursive(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();

    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let file_type = entry.file_type().unwrap();
        let destination_path = destination.join(entry.file_name());

        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &destination_path);
        } else {
            fs::copy(entry.path(), destination_path).unwrap();
        }
    }
}

#[allow(clippy::too_many_lines)]
fn write_http_composite_fixture(root: &Path) -> PathBuf {
    let source_dir = root.join("examples/skills/http-composite-forwarder");
    fs::create_dir_all(source_dir.join("skill-rust/src")).unwrap();
    fs::create_dir_all(source_dir.join("tests")).unwrap();

    fs::write(
        source_dir.join("manifest.json"),
        r#"{
  "manifest_schema_version": "guild-manifest-v1",
  "skill_api_version": "guild-skill-v1",
  "key": {
    "namespace": "example",
    "name": "http-composite-forwarder"
  },
  "version": "0.1.0",
  "display_name": "HTTP Composite Forwarder",
  "description": "A narrow test-only composite that forwards inspect input to the HTTP child skill.",
  "runtime": {
    "kind": "wasm-component",
    "entrypoint": "guild-skill-inspect-v1",
    "guest_abi_version": "guild-skill-inspect-v1"
  },
  "interface": {
    "input_schema_uri": "./input.schema.json",
    "output_schema_uri": "./output.schema.json",
    "examples_uri": "./examples.json"
  },
  "behavior": {
    "category": "inventory",
    "mutability": "read-only",
    "idempotent": true,
    "open_world": false,
    "freshness": "deterministic",
    "modes": {
      "supported": ["inspect"],
      "apply_requires_approval": false,
      "apply_requires_idempotency_key": false
    }
  },
  "capabilities": [
    {
      "id": "invoke-skill",
      "access": "invoke",
      "required": true,
      "constraints": {
        "aliases": ["http"]
      }
    }
  ],
  "dependencies": [
    {
      "alias": "http",
      "skill": {
        "key": {
          "namespace": "example",
          "name": "inspect-http-json"
        },
        "version_req": "^0.1"
      }
    }
  ],
  "publisher": {
    "id": "local.example",
    "display_name": "Local Example",
    "homepage": null
  },
  "package": {
    "visibility": "private",
    "trust_tier": "local",
    "sbom_uri": null,
    "signature_uri": null
  },
  "build": {
    "kind": "cargo-wasm-component",
    "cargo_manifest_path": "./skill-rust/Cargo.toml",
    "target": "wasm32-wasip2",
    "profile": "release"
  },
  "tests": [
    {
      "name": "forwards-http-input",
      "fixtures_uri": "./tests/inspect-input.json",
      "expected_output_uri": "./tests/expected-output.json"
    }
  ]
}"#,
    )
    .unwrap();
    fs::write(
        source_dir.join("input.schema.json"),
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object"}"#,
    )
    .unwrap();
    fs::write(
        source_dir.join("output.schema.json"),
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object"}"#,
    )
    .unwrap();
    fs::write(
        source_dir.join("examples.json"),
        r#"[{"name":"forward-local-http","input":{"url":"http://127.0.0.1:8080/json"}}]"#,
    )
    .unwrap();
    fs::write(
        source_dir.join("tests/inspect-input.json"),
        r#"{"url":"http://127.0.0.1:8080/json"}"#,
    )
    .unwrap();
    fs::write(
        source_dir.join("tests/expected-output.json"),
        r#"{"summary":"Forwarded HTTP child execution."}"#,
    )
    .unwrap();
    fs::write(
        source_dir.join("skill-rust/Cargo.toml"),
        r#"[package]
name = "guild-test-http-composite-forwarder"
version = "0.1.0"
edition = "2024"
rust-version = "1.94"

[workspace]

[lib]
crate-type = ["cdylib"]

[dependencies]
serde_json = "1"
wit-bindgen = "0.53.1"
"#,
    )
    .unwrap();
    fs::write(
        source_dir.join("skill-rust/src/lib.rs"),
        r#"use serde_json::{json, Value};
use wit_bindgen::generate;

generate!({
    path: "../../../../wit",
    world: "guild-skill-inspect-v1",
});

use crate::exports::guild::skill::inspect_skill::{ExecutionContext, Guest, Json, SkillError, SkillOutput};
use crate::guild::skill::inspect_host as host;
use crate::guild::skill::inspect_types::DependencyInvocationRequest;

struct HttpCompositeForwarder;

impl Guest for HttpCompositeForwarder {
    fn run(_ctx: ExecutionContext, input: Json) -> Result<SkillOutput, SkillError> {
        let _: Value = serde_json::from_str(&input).map_err(|error| SkillError {
            code: "invalid-input".into(),
            message: "input JSON could not be parsed".into(),
            retryable: false,
            detail: Some(json!({ "error": error.to_string() }).to_string()),
        })?;

        let child_output = host::invoke_dependency(&DependencyInvocationRequest {
            alias: "http".into(),
            input,
        })?;
        let child_structured: Value =
            serde_json::from_str(&child_output.structured).unwrap_or(Value::Null);

        Ok(SkillOutput {
            summary: "Forwarded HTTP child execution.".into(),
            structured: json!({
                "child_summary": child_output.summary,
                "child_structured": child_structured,
            })
            .to_string(),
            diagnostics: Vec::new(),
            effects: Vec::new(),
            evidence: Vec::new(),
        })
    }
}

export!(HttpCompositeForwarder with_types_in self);
"#,
    )
    .unwrap();

    source_dir
}

#[test]
fn http_happy_path_executes_through_real_host_path() {
    let server = http_test_server::HttpTestServer::start();
    let registry = load_registry();
    let runner = build_runner();

    let record = run_http_skill(
        &registry,
        &runner,
        json!({
            "url": server.json_url(),
            "method": "get",
            "json_pointers": ["/message", "/nested/count"],
        }),
        CapabilityGrantSet {
            grants: vec![http_grant(
                http_test_server::HttpTestServer::host(),
                server.port(),
                "/json",
                &[HttpMethod::Get],
                &[HttpScheme::Http],
                2_000,
                4_096,
            )],
        },
        Budget::default(),
    )
    .unwrap();

    assert_eq!(record.status, ExecutionStatus::Succeeded);
    assert_eq!(record.metrics.network_requests, 1);
    let output = record.output.as_ref().unwrap();
    assert_eq!(output.structured["status"], 200);
    assert_eq!(
        output.structured["selected_fields"][0]["value"],
        Value::String("deterministic".into())
    );
    assert_eq!(output.structured["json_summary"]["root_kind"], "object");
    assert_eq!(record.authority_observations.len(), 1);
    match &record.authority_observations[0] {
        AuthorityObservation::HttpRequest { status, detail } => {
            assert_eq!(status, &AuthorityObservationStatus::Exercised);
            assert_eq!(detail.request.method, HttpMethod::Get);
            assert_eq!(detail.request.url, server.json_url());
            assert_eq!(detail.response_status, Some(200));
            assert_eq!(detail.denial, None);
            assert_eq!(detail.result_error, None);
        }
        other => panic!("expected canonical http-request observation, got {other:?}"),
    }
}

#[test]
fn proof_only_replay_transport_replays_deterministic_loopback_gets() {
    let registry = load_registry();
    let replay_url = "http://127.0.0.1:18080/response.json";
    let runner = build_replay_runner(vec![http_replay_fixture(
        replay_url,
        r#"{"service":"guild-http","message":"deterministic","nested":{"count":2},"items":[{"name":"alpha"},{"name":"beta"}]}"#,
    )]);
    let grants = CapabilityGrantSet {
        grants: vec![http_grant(
            "127.0.0.1",
            18080,
            "/response.json",
            &[HttpMethod::Get],
            &[HttpScheme::Http],
            2_000,
            4_096,
        )],
    };
    let input = json!({
        "url": replay_url,
        "method": "get",
        "json_pointers": ["/message", "/nested/count"],
    });

    let first = run_http_skill(
        &registry,
        &runner,
        input.clone(),
        grants.clone(),
        Budget::default(),
    )
    .unwrap();
    let second = run_http_skill(&registry, &runner, input, grants, Budget::default()).unwrap();

    assert_eq!(first.status, ExecutionStatus::Succeeded);
    assert_eq!(second.status, ExecutionStatus::Succeeded);
    assert_eq!(first.metrics.network_requests, 1);
    assert_eq!(second.metrics.network_requests, 1);
    assert_eq!(
        first.output.as_ref().unwrap().structured,
        second.output.as_ref().unwrap().structured
    );
    assert_eq!(first.authority_observations, second.authority_observations);
}

#[test]
fn proof_only_replay_transport_fails_closed_for_query_requests() {
    let registry = load_registry();
    let runner = build_replay_runner(vec![http_replay_fixture(
        "http://127.0.0.1:18080/response.json",
        r#"{"service":"guild-http","message":"deterministic","nested":{"count":2},"items":[{"name":"alpha"},{"name":"beta"}]}"#,
    )]);

    let error = run_http_skill(
        &registry,
        &runner,
        json!({
            "url": "http://127.0.0.1:18080/response.json?view=full",
            "method": "get",
        }),
        CapabilityGrantSet {
            grants: vec![http_grant(
                "127.0.0.1",
                18080,
                "/response.json",
                &[HttpMethod::Get],
                &[HttpScheme::Http],
                2_000,
                4_096,
            )],
        },
        Budget::default(),
    )
    .unwrap_err();

    let record = persisted_error_record(&registry, &error);
    assert_eq!(error.code, "http-replay-request-unsupported");
    assert_eq!(record.status, ExecutionStatus::Failed);
    assert_eq!(record.metrics.network_requests, 1);
}

#[test]
fn unauthorized_host_is_rejected_by_host_owned_http_denial() {
    let server = http_test_server::HttpTestServer::start();
    let registry = load_registry();
    let runner = build_runner();

    let error = run_http_skill(
        &registry,
        &runner,
        json!({ "url": server.json_url(), "method": "get" }),
        CapabilityGrantSet {
            grants: vec![http_grant(
                "localhost",
                server.port(),
                "/json",
                &[HttpMethod::Get],
                &[HttpScheme::Http],
                2_000,
                4_096,
            )],
        },
        Budget::default(),
    )
    .unwrap_err();

    let record = persisted_error_record(&registry, &error);
    assert_eq!(error.code, "http-request-host-not-granted");
    assert_eq!(record.status, ExecutionStatus::Rejected);
    assert_eq!(record.metrics.network_requests, 0);
    assert_eq!(
        record.termination.as_ref().unwrap().code,
        "http-request-host-not-granted"
    );
    assert_eq!(record.authority_observations.len(), 1);
    match &record.authority_observations[0] {
        AuthorityObservation::HttpRequest { status, detail } => {
            assert_eq!(status, &AuthorityObservationStatus::Blocked);
            assert_eq!(detail.request.url, server.json_url());
            assert_eq!(detail.response_status, None);
            assert_eq!(
                detail.denial.as_ref().map(|value| value.code.as_str()),
                Some("http-request-host-not-granted")
            );
            assert_eq!(detail.result_error, None);
        }
        other => panic!("expected blocked canonical http-request observation, got {other:?}"),
    }
}

#[test]
fn unauthorized_method_is_rejected_by_host_owned_http_denial() {
    let server = http_test_server::HttpTestServer::start();
    let registry = load_registry();
    let runner = build_runner();

    let error = run_http_skill(
        &registry,
        &runner,
        json!({ "url": server.json_url(), "method": "head" }),
        CapabilityGrantSet {
            grants: vec![http_grant(
                http_test_server::HttpTestServer::host(),
                server.port(),
                "/json",
                &[HttpMethod::Get],
                &[HttpScheme::Http],
                2_000,
                4_096,
            )],
        },
        Budget::default(),
    )
    .unwrap_err();

    let record = persisted_error_record(&registry, &error);
    assert_eq!(error.code, "http-request-method-not-granted");
    assert_eq!(record.status, ExecutionStatus::Rejected);
    assert_eq!(record.metrics.network_requests, 0);
}

#[test]
fn unauthorized_scheme_is_rejected_by_host_owned_http_denial() {
    let server = http_test_server::HttpTestServer::start();
    let registry = load_registry();
    let runner = build_runner();

    let error = run_http_skill(
        &registry,
        &runner,
        json!({
            "url": format!(
                "https://{}:{}/json",
                http_test_server::HttpTestServer::host(),
                server.port()
            ),
            "method": "get",
        }),
        CapabilityGrantSet {
            grants: vec![http_grant(
                http_test_server::HttpTestServer::host(),
                server.port(),
                "/json",
                &[HttpMethod::Get],
                &[HttpScheme::Http],
                2_000,
                4_096,
            )],
        },
        Budget::default(),
    )
    .unwrap_err();

    let record = persisted_error_record(&registry, &error);
    assert_eq!(error.code, "http-request-scheme-not-granted");
    assert_eq!(record.status, ExecutionStatus::Rejected);
    assert_eq!(record.metrics.network_requests, 0);
}

#[test]
fn unauthorized_port_is_rejected_by_host_owned_http_denial() {
    let server = http_test_server::HttpTestServer::start();
    let registry = load_registry();
    let runner = build_runner();

    let error = run_http_skill(
        &registry,
        &runner,
        json!({ "url": server.json_url(), "method": "get" }),
        CapabilityGrantSet {
            grants: vec![http_grant(
                http_test_server::HttpTestServer::host(),
                server.port().saturating_add(1),
                "/json",
                &[HttpMethod::Get],
                &[HttpScheme::Http],
                2_000,
                4_096,
            )],
        },
        Budget::default(),
    )
    .unwrap_err();

    let record = persisted_error_record(&registry, &error);
    assert_eq!(error.code, "http-request-port-not-granted");
    assert_eq!(record.status, ExecutionStatus::Rejected);
    assert_eq!(record.metrics.network_requests, 0);
}

#[test]
fn unauthorized_path_is_rejected_by_host_owned_http_denial() {
    let server = http_test_server::HttpTestServer::start();
    let registry = load_registry();
    let runner = build_runner();

    let error = run_http_skill(
        &registry,
        &runner,
        json!({ "url": server.json_url(), "method": "get" }),
        CapabilityGrantSet {
            grants: vec![http_grant(
                http_test_server::HttpTestServer::host(),
                server.port(),
                "/other",
                &[HttpMethod::Get],
                &[HttpScheme::Http],
                2_000,
                4_096,
            )],
        },
        Budget::default(),
    )
    .unwrap_err();

    let record = persisted_error_record(&registry, &error);
    assert_eq!(error.code, "http-request-path-not-granted");
    assert_eq!(record.status, ExecutionStatus::Rejected);
    assert_eq!(record.metrics.network_requests, 0);
}

#[test]
fn loopback_hostname_requires_explicit_loopback_allowance() {
    let server = http_test_server::HttpTestServer::start();
    let registry = load_registry();
    let runner = build_runner();

    let error = run_http_skill(
        &registry,
        &runner,
        json!({ "url": server.localhost_json_url(), "method": "get" }),
        CapabilityGrantSet {
            grants: vec![http_grant_with_options(
                "localhost",
                server.port(),
                &["/json"],
                &[HttpMethod::Get],
                &[HttpScheme::Http],
                2_000,
                4_096,
                None,
                None,
                None,
                None,
                None,
            )],
        },
        Budget::default(),
    )
    .unwrap_err();

    let record = persisted_error_record(&registry, &error);
    assert_eq!(error.code, "http-request-loopback-not-granted");
    assert_eq!(record.status, ExecutionStatus::Rejected);
    assert_eq!(record.metrics.network_requests, 0);
}

#[test]
fn ip_literal_requires_explicit_allowance() {
    let server = http_test_server::HttpTestServer::start();
    let registry = load_registry();
    let runner = build_runner();

    let error = run_http_skill(
        &registry,
        &runner,
        json!({ "url": server.json_url(), "method": "get" }),
        CapabilityGrantSet {
            grants: vec![http_grant_with_options(
                http_test_server::HttpTestServer::host(),
                server.port(),
                &["/json"],
                &[HttpMethod::Get],
                &[HttpScheme::Http],
                2_000,
                4_096,
                None,
                Some(true),
                None,
                None,
                None,
            )],
        },
        Budget::default(),
    )
    .unwrap_err();

    let record = persisted_error_record(&registry, &error);
    assert_eq!(error.code, "http-request-ip-literal-not-granted");
    assert_eq!(record.status, ExecutionStatus::Rejected);
    assert_eq!(record.metrics.network_requests, 0);
}

#[test]
fn private_network_ip_requires_explicit_allowance() {
    let registry = load_registry();
    let runner = build_runner();

    let error = run_http_skill(
        &registry,
        &runner,
        json!({ "url": "http://192.168.10.20:8080/json", "method": "get" }),
        CapabilityGrantSet {
            grants: vec![http_grant_with_options(
                "192.168.10.20",
                8080,
                &["/json"],
                &[HttpMethod::Get],
                &[HttpScheme::Http],
                2_000,
                4_096,
                None,
                None,
                None,
                None,
                Some(true),
            )],
        },
        Budget::default(),
    )
    .unwrap_err();

    let record = persisted_error_record(&registry, &error);
    assert_eq!(error.code, "http-request-private-network-not-granted");
    assert_eq!(record.status, ExecutionStatus::Rejected);
    assert_eq!(record.metrics.network_requests, 0);
}

#[test]
fn link_local_ip_requires_explicit_allowance() {
    let registry = load_registry();
    let runner = build_runner();

    let error = run_http_skill(
        &registry,
        &runner,
        json!({ "url": "http://169.254.10.20:8080/json", "method": "get" }),
        CapabilityGrantSet {
            grants: vec![http_grant_with_options(
                "169.254.10.20",
                8080,
                &["/json"],
                &[HttpMethod::Get],
                &[HttpScheme::Http],
                2_000,
                4_096,
                None,
                None,
                None,
                None,
                Some(true),
            )],
        },
        Budget::default(),
    )
    .unwrap_err();

    let record = persisted_error_record(&registry, &error);
    assert_eq!(error.code, "http-request-link-local-not-granted");
    assert_eq!(record.status, ExecutionStatus::Rejected);
    assert_eq!(record.metrics.network_requests, 0);
}

#[test]
fn oversized_http_response_fails_with_explicit_host_bound() {
    let server = http_test_server::HttpTestServer::start();
    let registry = load_registry();
    let runner = build_runner();

    let error = run_http_skill(
        &registry,
        &runner,
        json!({ "url": server.large_json_url(), "method": "get" }),
        CapabilityGrantSet {
            grants: vec![http_grant(
                http_test_server::HttpTestServer::host(),
                server.port(),
                "/large",
                &[HttpMethod::Get],
                &[HttpScheme::Http],
                2_000,
                512,
            )],
        },
        Budget::default(),
    )
    .unwrap_err();

    let record = persisted_error_record(&registry, &error);
    assert_eq!(error.code, "http-request-response-too-large");
    assert_eq!(record.status, ExecutionStatus::Failed);
    assert_eq!(record.metrics.network_requests, 1);
    assert!(http_test_server::large_response_bytes() > 512);
}

#[test]
fn slow_http_response_times_out_with_explicit_host_bound() {
    let server = http_test_server::HttpTestServer::start();
    let registry = load_registry();
    let runner = build_runner();

    let error = run_http_skill(
        &registry,
        &runner,
        json!({
            "url": server.slow_json_url(),
            "method": "get",
            "timeout_ms": 50,
        }),
        CapabilityGrantSet {
            grants: vec![http_grant(
                http_test_server::HttpTestServer::host(),
                server.port(),
                "/slow",
                &[HttpMethod::Get],
                &[HttpScheme::Http],
                50,
                4_096,
            )],
        },
        Budget::default(),
    )
    .unwrap_err();

    let record = persisted_error_record(&registry, &error);
    assert_eq!(error.code, "http-request-timeout");
    assert_eq!(record.status, ExecutionStatus::Failed);
    assert_eq!(record.metrics.network_requests, 1);
    assert!(http_test_server::slow_response_ms() > 50);
}

#[test]
fn redirect_is_denied_when_following_is_not_granted() {
    let server = http_test_server::HttpTestServer::start();
    let registry = load_registry();
    let runner = build_runner();

    let error = run_http_skill(
        &registry,
        &runner,
        json!({ "url": server.redirect_json_url(), "method": "get" }),
        CapabilityGrantSet {
            grants: vec![http_grant_with_options(
                http_test_server::HttpTestServer::host(),
                server.port(),
                &["/redirect-json", "/json"],
                &[HttpMethod::Get],
                &[HttpScheme::Http],
                2_000,
                4_096,
                None,
                Some(true),
                None,
                None,
                Some(true),
            )],
        },
        Budget::default(),
    )
    .unwrap_err();

    let record = persisted_error_record(&registry, &error);
    assert_eq!(error.code, "http-request-redirect-not-allowed");
    assert_eq!(record.status, ExecutionStatus::Rejected);
    assert_eq!(record.metrics.network_requests, 1);
}

#[test]
fn redirect_target_must_still_be_granted() {
    let server = http_test_server::HttpTestServer::start();
    let registry = load_registry();
    let runner = build_runner();

    let error = run_http_skill(
        &registry,
        &runner,
        json!({ "url": server.redirect_json_url(), "method": "get" }),
        CapabilityGrantSet {
            grants: vec![http_grant_with_options(
                http_test_server::HttpTestServer::host(),
                server.port(),
                &["/redirect-json"],
                &[HttpMethod::Get],
                &[HttpScheme::Http],
                2_000,
                4_096,
                Some(2),
                Some(true),
                None,
                None,
                Some(true),
            )],
        },
        Budget::default(),
    )
    .unwrap_err();

    let record = persisted_error_record(&registry, &error);
    assert_eq!(error.code, "http-request-redirect-target-not-granted");
    assert_eq!(record.status, ExecutionStatus::Rejected);
    assert_eq!(record.metrics.network_requests, 1);
}

#[test]
fn allowed_redirect_follows_bounded_hops() {
    let server = http_test_server::HttpTestServer::start();
    let registry = load_registry();
    let runner = build_runner();

    let record = run_http_skill(
        &registry,
        &runner,
        json!({
            "url": server.redirect_json_url(),
            "method": "get",
            "json_pointers": ["/message"],
        }),
        CapabilityGrantSet {
            grants: vec![http_grant_with_options(
                http_test_server::HttpTestServer::host(),
                server.port(),
                &["/redirect-json", "/json"],
                &[HttpMethod::Get],
                &[HttpScheme::Http],
                2_000,
                4_096,
                Some(2),
                Some(true),
                None,
                None,
                Some(true),
            )],
        },
        Budget::default(),
    )
    .unwrap();

    assert_eq!(record.status, ExecutionStatus::Succeeded);
    assert_eq!(record.metrics.network_requests, 2);
    assert_eq!(
        record.output.as_ref().unwrap().structured["selected_fields"][0]["value"],
        Value::String("deterministic".into())
    );
}

#[test]
fn redirect_hop_limit_is_enforced() {
    let server = http_test_server::HttpTestServer::start();
    let registry = load_registry();
    let runner = build_runner();

    let error = run_http_skill(
        &registry,
        &runner,
        json!({ "url": server.redirect_chain_url(), "method": "get" }),
        CapabilityGrantSet {
            grants: vec![http_grant_with_options(
                http_test_server::HttpTestServer::host(),
                server.port(),
                &["/redirect-chain-1", "/redirect-chain-2", "/json"],
                &[HttpMethod::Get],
                &[HttpScheme::Http],
                2_000,
                4_096,
                Some(1),
                Some(true),
                None,
                None,
                Some(true),
            )],
        },
        Budget::default(),
    )
    .unwrap_err();

    let record = persisted_error_record(&registry, &error);
    assert_eq!(error.code, "http-request-redirect-hop-limit-exceeded");
    assert_eq!(record.status, ExecutionStatus::Rejected);
    assert_eq!(record.metrics.network_requests, 2);
}

#[test]
fn composite_child_cannot_expand_parent_http_authority() {
    let server = http_test_server::HttpTestServer::start();
    let temp = TempFixtureDir::new("guild-http-nested");
    let workspace_root = temp.path().join("workspace");
    let registry_root = temp.path().join("registry");
    let http_source_root = workspace_root.join("examples/skills/inspect-http-json");
    let composite_source_root = write_http_composite_fixture(&workspace_root);

    copy_dir_recursive(&http_skill_dir(), &http_source_root);
    copy_dir_recursive(&wit_dir(), &workspace_root.join("wit"));

    let installer = LocalSourceInstaller::new(&registry_root).unwrap();
    installer.install(&http_source_root).unwrap();
    let composite = installer.install(&composite_source_root).unwrap();

    let registry = LocalRegistry::load(&registry_root).unwrap();
    let runner = build_runner();
    let error = runner
        .execute(
            &registry,
            &composite,
            &execution_request(
                &composite,
                unique_id("http-composite"),
                json!({
                    "url": server.json_url(),
                    "method": "get",
                    "json_pointers": ["/message"],
                }),
                CapabilityGrantSet {
                    grants: vec![
                        GrantedCapability {
                            id: CapabilityId::InvokeSkill,
                            access: CapabilityAccess::Invoke,
                            constraints: CapabilityConstraints::InvokeDependency(
                                InvokeDependencyConstraints {
                                    aliases: Some(vec!["http".into()]),
                                },
                            ),
                        },
                        http_grant(
                            http_test_server::HttpTestServer::host(),
                            server.port(),
                            "/blocked",
                            &[HttpMethod::Get],
                            &[HttpScheme::Http],
                            2_000,
                            4_096,
                        ),
                    ],
                },
                Budget::default(),
            ),
        )
        .unwrap_err();

    let parent_record = persisted_error_record(&registry, &error);
    let child_record = registry
        .load_execution_record(&parent_record.child_executions[0].execution_id)
        .unwrap();
    let child_http_constraints = child_record
        .granted_capabilities
        .grants
        .iter()
        .find(|grant| grant.id == CapabilityId::HttpRequest)
        .and_then(|grant| grant.constraints.as_http_request())
        .expect("child receives narrowed http-request grant");

    assert_eq!(error.code, "child-invocation-failed");
    assert_eq!(parent_record.status, ExecutionStatus::Failed);
    assert_eq!(child_record.status, ExecutionStatus::Rejected);
    assert_eq!(
        child_record.termination.as_ref().unwrap().code,
        "http-request-path-not-granted"
    );
    assert_eq!(
        child_http_constraints.allowed_path_prefixes,
        Some(vec!["/blocked".into()])
    );
}
