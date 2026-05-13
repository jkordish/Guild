use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use guild_registry::{LocalRegistry, LocalSourceInstaller, SkillRegistry, execution_resource_uri};
use guild_runner::{Runner, WasmtimeRuntimeAdapter};
use guild_types::{
    AbiVersion, AuthorityObservation, AuthorityObservationStatus, CallerRequest, CapabilityAccess,
    CapabilityConstraints, CapabilityGrantSet, CapabilityId, CapabilityRequirement,
    EmitEvidenceConstraints, EvidenceAudience, EvidenceRef, ExecutionMode, ExecutionStatus,
    FilesystemConstraints, FilesystemOperation, FilesystemRoot, GrantedCapability, HttpMethod,
    HttpRequestConstraints, HttpScheme, InstalledVerificationState, InvokeDependencyConstraints,
    LocalTrustTier, LogConstraints, PolicyDecision, PolicyDecisionOutcome, PolicyReason,
    ReadResourceConstraints, RedactionClass, RequestedSkillRef, ResolvedExecutionEnvelope,
    ResourceKind, Severity, SkillKey, VersionRequirement,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use wasmtime::component::Component;
use wasmtime::{Config, Engine};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn example_skill_dir() -> PathBuf {
    repo_root().join("examples/skills/hello-inspect")
}

fn prepared_registry_root() -> &'static PathBuf {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();

    ROOT.get_or_init(|| {
        let root = repo_root().join("target/test-install-registry/guild-runner");
        if root.exists() {
            let _ = fs::remove_dir_all(&root);
        }

        LocalSourceInstaller::new(&root)
            .unwrap()
            .install(example_skill_dir())
            .unwrap();

        root
    })
}

fn requested_skill() -> RequestedSkillRef {
    RequestedSkillRef {
        key: SkillKey {
            namespace: "example".into(),
            name: "hello-inspect".into(),
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

fn sample_request(
    grants: CapabilityGrantSet,
    mode: ExecutionMode,
) -> (guild_registry::InstalledSkill, ResolvedExecutionEnvelope) {
    let registry = load_registry();
    let installed = registry.resolve(&requested_skill()).unwrap();
    let request = envelope_for(
        &installed,
        "exec-1",
        "trace-1",
        serde_json::json!({"name": "Ada"}),
        grants,
        mode,
    );

    (installed, request)
}

fn envelope_for(
    installed: &guild_registry::InstalledSkill,
    request_id: impl Into<String>,
    trace_id: impl Into<String>,
    input: Value,
    grants: CapabilityGrantSet,
    mode: ExecutionMode,
) -> ResolvedExecutionEnvelope {
    let request_id = request_id.into();
    let trace_id = trace_id.into();

    ResolvedExecutionEnvelope {
        request: CallerRequest {
            request_id: format!("{request_id}-request"),
            skill: requested_skill(),
            tenant_id: "tenant-1".into(),
            actor_id: "actor-1".into(),
            mode,
            input,
            budget: guild_types::Budget::default(),
            requested_capabilities: grants.clone(),
            idempotency_key: None,
            trace_id,
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

fn log_info_grant() -> GrantedCapability {
    GrantedCapability {
        id: CapabilityId::LogWrite,
        access: CapabilityAccess::Write,
        constraints: CapabilityConstraints::Log(LogConstraints {
            levels: Some(vec![Severity::Info]),
        }),
    }
}

fn read_resource_grant() -> GrantedCapability {
    GrantedCapability {
        id: CapabilityId::ReadResource,
        access: CapabilityAccess::Read,
        constraints: CapabilityConstraints::ReadResource(ReadResourceConstraints {
            uri_prefixes: Some(vec![
                "guild://executions/".into(),
                "guild://queries/executions/".into(),
            ]),
            resource_kinds: Some(vec![ResourceKind::Execution, ResourceKind::Query]),
        }),
    }
}

fn invoke_dependency_grant() -> GrantedCapability {
    GrantedCapability {
        id: CapabilityId::InvokeSkill,
        access: CapabilityAccess::Invoke,
        constraints: CapabilityConstraints::InvokeDependency(InvokeDependencyConstraints {
            aliases: Some(vec!["child".into()]),
        }),
    }
}

fn http_projection_grant() -> GrantedCapability {
    GrantedCapability {
        id: CapabilityId::HttpRequest,
        access: CapabilityAccess::Read,
        constraints: CapabilityConstraints::HttpRequest(HttpRequestConstraints {
            allowed_schemes: Some(vec![HttpScheme::Http]),
            allowed_hosts: Some(vec!["127.0.0.1".into()]),
            allowed_host_suffixes: Some(vec!["example.com".into()]),
            allowed_ports: Some(vec![8080]),
            allowed_methods: Some(vec![HttpMethod::Get]),
            allowed_path_prefixes: Some(vec!["/json".into()]),
            max_timeout_ms: Some(2_000),
            max_response_bytes: Some(4_096),
            follow_redirects: Some(true),
            max_redirects: Some(2),
            allow_loopback: Some(true),
            allow_link_local: Some(false),
            allow_private_networks: Some(false),
            allow_ip_literals: Some(true),
        }),
    }
}

fn filesystem_read_grant() -> GrantedCapability {
    GrantedCapability {
        id: CapabilityId::Filesystem,
        access: CapabilityAccess::Read,
        constraints: CapabilityConstraints::Filesystem(FilesystemConstraints {
            preopened_roots: vec![FilesystemRoot {
                name: "workspace".into(),
                guest_path_prefix: "/workspace".into(),
                host_path: "/var/lib/guild/workspace".into(),
                operations: vec![FilesystemOperation::Read],
            }],
        }),
    }
}

fn expected_evidence_payload(installed: &guild_registry::InstalledSkill, input: &Value) -> Value {
    json!({
        "kind": "hello-inspect-snapshot",
        "echoed_input": input,
        "mode": "inspect",
        "skill": {
            "key": {
                "namespace": installed.resolved_ref.key.namespace,
                "name": installed.resolved_ref.key.name,
            },
            "version": installed.resolved_ref.version.to_string(),
            "digest": installed.resolved_ref.digest,
        },
    })
}

fn expected_evidence_ref(
    uri: String,
    installed: &guild_registry::InstalledSkill,
    input: &Value,
) -> EvidenceRef {
    let payload = serde_json::to_vec(&expected_evidence_payload(installed, input)).unwrap();
    let digest_hex = hex::encode(Sha256::digest(&payload));

    EvidenceRef {
        uri,
        title: Some("hello-inspect snapshot".into()),
        mime_type: Some("application/json".into()),
        sha256: Some(format!("sha256:{digest_hex}")),
        audience: guild_types::EvidenceAudience::User,
        redaction: guild_types::RedactionClass::None,
        freshness: Some("deterministic".into()),
    }
}

struct TempFixtureDir {
    path: PathBuf,
}

impl TempFixtureDir {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("guild-runner-evidence-{unique}"));
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

fn component_engine() -> Engine {
    let mut config = Config::new();
    config.wasm_component_model(true);
    Engine::new(&config).unwrap()
}

fn guild_component_import_names(component_path: &Path) -> Vec<String> {
    let engine = component_engine();
    let component = Component::from_file(&engine, component_path).unwrap();
    let mut imports: Vec<_> = component
        .component_type()
        .imports(&engine)
        .filter(|(name, _)| name.starts_with("guild:skill/"))
        .map(|(name, _)| name.to_owned())
        .collect();
    imports.sort();
    imports
}

fn assert_manifest_validation_error(detail: &Value, path: &str, message: &str) {
    let errors = detail
        .as_array()
        .expect("manifest validation detail serializes as an array");
    assert!(
        errors.iter().any(|entry| {
            entry.get("path").and_then(Value::as_str) == Some(path)
                && entry.get("message").and_then(Value::as_str) == Some(message)
        }),
        "missing manifest validation error `{path}` -> `{message}` in {detail}"
    );
}

fn broad_world_fixture_source() -> &'static str {
    r#"use serde_json::{json, Value};
use wit_bindgen::generate;

const _: &str = include_str!("../../../../../wit/guild-skill-v1.wit");

generate!({
    path: "../../../../wit",
    world: "guild-skill",
});

use crate::exports::guild::skill::skill::{
    ExecutionContext, Guest, Json, SkillError, SkillOutput,
};
use crate::guild::skill::host;
use crate::guild::skill::types::{EvidenceAudience, EvidenceEmissionRequest, RedactionClass};

struct HelloInspectBroadImport;

impl Guest for HelloInspectBroadImport {
    fn run(ctx: ExecutionContext, input: Json) -> Result<SkillOutput, SkillError> {
        let parsed_input: Value = serde_json::from_str(&input).map_err(|error| SkillError {
            code: "invalid-input".into(),
            message: "input JSON could not be parsed".into(),
            retryable: false,
            detail: Some(json!({ "error": error.to_string() }).to_string()),
        })?;

        let payload = serde_json::to_vec(&json!({
            "kind": "broad-import-fixture",
            "execution_id": ctx.execution_id,
            "input": parsed_input,
        }))
        .map_err(|error| SkillError {
            code: "evidence-payload-invalid".into(),
            message: "fixture evidence payload could not be serialized".into(),
            retryable: false,
            detail: Some(json!({ "error": error.to_string() }).to_string()),
        })?;

        let evidence = host::emit_evidence(&EvidenceEmissionRequest {
            payload,
            mime_type: "application/json".into(),
            title: Some("broad-import fixture".into()),
            audience: EvidenceAudience::User,
            redaction: RedactionClass::None,
            freshness: Some("deterministic".into()),
        })
        .map_err(|message| SkillError {
            code: "emit-evidence-failed".into(),
            message: "host failed to persist fixture evidence".into(),
            retryable: false,
            detail: Some(json!({ "error": message }).to_string()),
        })?;

        Ok(SkillOutput {
            summary: "Broad world fixture executed".into(),
            structured: json!({
                "message": "broad import fixture",
                "execution_id": ctx.execution_id,
            })
            .to_string(),
            diagnostics: Vec::new(),
            effects: Vec::new(),
            evidence: vec![evidence],
        })
    }
}

export!(HelloInspectBroadImport with_types_in self);
"#
}

fn write_broad_import_fixture(root: &Path, skill_name: &str) -> PathBuf {
    let workspace_root = root.join("workspace");
    let source_root = workspace_root.join(format!("examples/skills/{skill_name}"));
    copy_dir_recursive(&example_skill_dir(), &source_root);
    copy_dir_recursive(&repo_root().join("wit"), &workspace_root.join("wit"));

    let manifest_path = source_root.join("manifest.json");
    let mut manifest: guild_manifest::SourceSkillManifest =
        serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
    manifest.key.name = skill_name.into();
    manifest.display_name = format!("{} Broad Import", manifest.display_name);
    manifest.description =
        "A fixture that compiles the broad Guild world under an inspect manifest.".into();
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();

    let lib_path = source_root.join("skill-rust/src/lib.rs");
    fs::write(lib_path, broad_world_fixture_source()).unwrap();

    source_root
}

#[test]
fn wit_stays_aligned_with_skill_visible_execution_context() {
    let wit = include_str!("../../../wit/guild-skill-v1.wit");
    let inspect = wit
        .split("interface inspect-types")
        .nth(1)
        .expect("inspect world is defined in the WIT package");

    for needle in [
        "world guild-skill-inspect-v1",
        "interface inspect-types",
        "interface inspect-host",
        "interface inspect-skill",
    ] {
        assert!(wit.contains(needle), "missing `{needle}` from WIT package");
    }

    for needle in [
        "record execution-context",
        "record evidence-emission-request",
        "record http-request-message",
        "record http-response-message",
        "record http-request-constraints",
        "record resource-read-result",
        "variant capability-constraints",
        "enum http-method",
        "enum http-scheme",
        "enum resource-kind",
        "skill: resolved-skill-ref",
        "granted-capabilities: list<granted-capability>",
        "allowed-host-suffixes: option<list<string>>",
        "follow-redirects: option<bool>",
        "allow-loopback: option<bool>",
        "http-request(",
        "emit-evidence,",
        "http-request: func(request: http-request-message) -> result<http-response-message, string>",
        "emit-evidence: func(request: evidence-emission-request) -> result<evidence-ref, string>",
        "read-resource: func(uri: string) -> result<resource-read-result, string>",
        "log: func(level: severity, message: string);",
        "invoke-dependency: func(request: dependency-invocation-request) -> result<skill-output, skill-error>",
        "run: func(ctx: execution-context, input: json) -> result<skill-output, skill-error>",
    ] {
        assert!(
            inspect.contains(needle),
            "missing `{needle}` from inspect WIT contract"
        );
    }

    for forbidden in [
        "cache-get: func(",
        "cache-put: func(",
        "get-secret: func(",
        "monotonic-now: func(",
        "wall-clock-now: func(",
        "get-secret,",
        "cache-read,",
        "cache-write,",
        "monotonic-clock,",
        "wall-clock,",
        "mode: execution-mode,",
        "plan,",
        "apply,",
    ] {
        assert!(
            !inspect.contains(forbidden),
            "inspect WIT contract unexpectedly contains `{forbidden}`"
        );
    }
}

#[test]
fn inspect_projection_docs_stay_aligned_with_the_host_boundary() {
    let specs = fs::read_to_string(repo_root().join("SPECS.md")).unwrap();
    let architecture = fs::read_to_string(repo_root().join("ARCHITECTURE.md")).unwrap();
    let spec_delta =
        fs::read_to_string(repo_root().join("docs/spec-delta-guest-abi-host-record-boundary.md"))
            .unwrap();
    let adr = fs::read_to_string(
        repo_root().join("docs/adr/0005-capability-schema-and-active-inspect-profile.md"),
    )
    .unwrap();

    for document in [&specs, &architecture, &spec_delta, &adr] {
        assert!(
            document.contains("now_utc"),
            "inspect projection docs must describe `now_utc` as guest-visible context"
        );
        assert!(
            document.contains("termination detail"),
            "inspect projection docs must keep termination detail host-owned"
        );
        assert!(
            document.contains("child lineage"),
            "inspect projection docs must keep child lineage host-owned"
        );
    }
}

#[test]
fn active_inspect_artifacts_only_import_the_inspect_host_interface() {
    let registry = load_registry();
    let installed = registry.resolve(&requested_skill()).unwrap();

    assert_eq!(
        guild_component_import_names(&installed.artifact_path),
        vec![
            "guild:skill/inspect-host@1.0.0".to_owned(),
            "guild:skill/inspect-types@1.0.0".to_owned(),
        ]
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn broader_guild_component_imports_are_rejected_before_guest_execution() {
    let temp = TempFixtureDir::new();
    let registry_root = temp.path().join("registry");
    let broad_source = write_broad_import_fixture(temp.path(), "hello-inspect-broad-import");
    let source_installer = LocalSourceInstaller::new(&registry_root).unwrap();
    source_installer.install(&broad_source).unwrap();

    let requested = RequestedSkillRef {
        key: SkillKey {
            namespace: "example".into(),
            name: "hello-inspect-broad-import".into(),
        },
        version_req: VersionRequirement::parse("^0.1").unwrap(),
    };
    let registry = LocalRegistry::load(&registry_root).unwrap();
    let installed_skill = registry.resolve(&requested).unwrap();
    assert_eq!(
        guild_component_import_names(&installed_skill.artifact_path),
        vec![
            "guild:skill/host@1.0.0".to_owned(),
            "guild:skill/types@1.0.0".to_owned(),
        ]
    );

    let envelope = ResolvedExecutionEnvelope {
        request: CallerRequest {
            request_id: "broad-import-request".into(),
            skill: requested,
            tenant_id: "tenant-1".into(),
            actor_id: "actor-1".into(),
            mode: ExecutionMode::Inspect,
            input: json!({ "name": "Ada" }),
            budget: guild_types::Budget::default(),
            requested_capabilities: CapabilityGrantSet {
                grants: vec![emit_evidence_grant(), log_info_grant()],
            },
            idempotency_key: None,
            trace_id: "trace-broad-import".into(),
        },
        resolved_skill: installed_skill.resolved_ref.clone(),
        granted_capabilities: CapabilityGrantSet {
            grants: vec![emit_evidence_grant(), log_info_grant()],
        },
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
    };

    let runner = build_runner();
    let error = runner
        .execute(&registry, &installed_skill, &envelope)
        .unwrap_err();

    assert_eq!(error.code, "unsupported-runtime-surface");
    let receipt = error
        .receipt
        .expect("unsupported component import rejection is persisted");
    let stored = registry
        .load_execution_record(&receipt.execution_id)
        .unwrap();
    assert_eq!(stored.status, ExecutionStatus::Rejected);
    let termination = stored.termination.as_ref().unwrap();
    assert_eq!(termination.phase, guild_types::ExecutionPhase::RuntimeLoad);
    assert_eq!(termination.code, "unsupported-runtime-surface");
    assert_eq!(
        termination.detail.as_ref().unwrap()["classification"],
        "unsupported-runtime-surface"
    );
    assert_eq!(
        termination.detail.as_ref().unwrap()["surface_kind"],
        "component-import"
    );
    assert_eq!(
        termination.detail.as_ref().unwrap()["surface_id"],
        "guild:skill/host@1.0.0"
    );
    assert_eq!(
        termination.detail.as_ref().unwrap()["detail"]["allowed_guild_imports"],
        json!([
            "guild:skill/inspect-types@1.0.0",
            "guild:skill/inspect-host@1.0.0",
        ])
    );
    let mut unexpected_import_names: Vec<_> =
        termination.detail.as_ref().unwrap()["detail"]["unexpected_guild_imports"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|entry| entry.get("name").and_then(Value::as_str))
            .collect();
    unexpected_import_names.sort_unstable();
    assert_eq!(
        unexpected_import_names,
        vec!["guild:skill/host@1.0.0", "guild:skill/types@1.0.0"]
    );
    assert_eq!(
        stored.policy_decision.outcome,
        PolicyDecisionOutcome::Allowed
    );
}

#[test]
fn runner_executes_example_skill_and_wraps_execution_record() {
    let grants = CapabilityGrantSet {
        grants: vec![emit_evidence_grant(), log_info_grant()],
    };
    let (installed, request) = sample_request(grants.clone(), ExecutionMode::Inspect);
    let registry = load_registry();
    let runner = build_runner();

    let record = runner.execute(&registry, &installed, &request).unwrap();
    let stored = registry
        .load_execution_record(&record.receipt.execution_id)
        .unwrap();
    let output = record.output.as_ref().unwrap();

    assert_ne!(record.receipt.execution_id, "exec-1");
    assert_eq!(record.request.request_id, "exec-1-request");
    assert_eq!(
        record.receipt.uri,
        execution_resource_uri(&record.receipt.execution_id)
    );
    assert_eq!(record.receipt.trace_id, "trace-1");
    assert_eq!(record.parent_execution_id, None);
    assert_eq!(record.status, ExecutionStatus::Succeeded);
    assert_eq!(record.request.skill.key.name, "hello-inspect");
    assert_eq!(
        record.policy_decision.outcome,
        PolicyDecisionOutcome::Allowed
    );
    assert_eq!(record.resolved_skill.digest, installed.resolved_ref.digest);
    assert!(record.termination.is_none());
    assert_eq!(output.summary, "Hello, Ada. Guild inspect is working.");
    assert_eq!(
        output.structured["echoed_input"],
        serde_json::json!({"name": "Ada"})
    );
    assert_eq!(output.structured["mode"], "inspect");
    assert_eq!(
        output.structured["skill"]["digest"],
        installed.resolved_ref.digest
    );
    assert_eq!(
        output.structured["granted_capabilities"],
        serde_json::to_value(grants).unwrap()
    );
    assert_eq!(
        output.evidence,
        vec![expected_evidence_ref(
            record.emitted_evidence[0].uri.clone(),
            &installed,
            &json!({"name": "Ada"})
        )]
    );
    assert_eq!(
        record
            .emitted_evidence
            .iter()
            .map(guild_types::EvidenceRecord::evidence_ref)
            .collect::<Vec<_>>(),
        output.evidence
    );
    assert_eq!(record.emitted_evidence[0].mime_type, "application/json");
    assert_eq!(
        record.emitted_evidence[0].produced_by_execution.as_deref(),
        Some(record.receipt.execution_id.as_str())
    );
    assert_eq!(record.provenance.resolved_skill, installed.resolved_ref);
    assert!(record.provenance.started_at_utc.is_some());
    assert!(record.provenance.finished_at_utc.is_some());
    assert_eq!(record.authority_observations.len(), 1);
    match &record.authority_observations[0] {
        AuthorityObservation::EmitEvidence { status, detail } => {
            assert_eq!(status, &AuthorityObservationStatus::Exercised);
            assert_eq!(detail.mime_type, "application/json");
            assert_eq!(
                detail.evidence_uri.as_deref(),
                Some(record.emitted_evidence[0].uri.as_str())
            );
            assert_eq!(detail.result_error, None);
            assert_eq!(detail.denial, None);
        }
        other => panic!("expected canonical emit-evidence observation, got {other:?}"),
    }
    assert_eq!(record, stored);
    assert!(record.metrics.duration_ms <= record.metrics.duration_ms);

    let evidence = registry
        .read_resource(&record.emitted_evidence[0].uri)
        .unwrap();
    assert_eq!(evidence.mime_type, "application/json");
    assert_eq!(
        serde_json::from_slice::<Value>(&evidence.bytes).unwrap(),
        expected_evidence_payload(&installed, &json!({"name": "Ada"}))
    );
}

#[test]
fn example_fixture_expected_output_matches_real_execution() {
    let input: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(example_skill_dir().join("tests/inspect-input.json")).unwrap(),
    )
    .unwrap();
    let expected_output: guild_types::SkillOutput = serde_json::from_str(
        &fs::read_to_string(example_skill_dir().join("tests/expected-output.json")).unwrap(),
    )
    .unwrap();

    let registry = load_registry();
    let installed = registry.resolve(&requested_skill()).unwrap();
    let runner = build_runner();

    let record = runner
        .execute(
            &registry,
            &installed,
            &envelope_for(
                &installed,
                "exec-2",
                "trace-2",
                input,
                CapabilityGrantSet {
                    grants: vec![emit_evidence_grant()],
                },
                ExecutionMode::Inspect,
            ),
        )
        .unwrap();

    let mut expected_output = expected_output;
    expected_output.structured["skill"]["digest"] =
        serde_json::Value::String(installed.resolved_ref.digest.clone());
    let expected_evidence = expected_evidence_ref(
        record.output.as_ref().unwrap().evidence[0].uri.clone(),
        &installed,
        &json!({"name": "Ada"}),
    );
    expected_output.evidence[0].uri = expected_evidence.uri;
    expected_output.evidence[0].sha256 = expected_evidence.sha256;

    assert_eq!(record.output, Some(expected_output));
}

#[test]
fn emitted_evidence_is_deduped_by_digest_and_resources_are_readable() {
    let registry = load_registry();
    let installed = registry.resolve(&requested_skill()).unwrap();
    let runner = build_runner();

    let first = runner
        .execute(
            &registry,
            &installed,
            &envelope_for(
                &installed,
                "exec-evidence-1",
                "trace-evidence",
                json!({"name": "Ada"}),
                CapabilityGrantSet {
                    grants: vec![emit_evidence_grant()],
                },
                ExecutionMode::Inspect,
            ),
        )
        .unwrap();
    let second = runner
        .execute(
            &registry,
            &installed,
            &envelope_for(
                &installed,
                "exec-evidence-2",
                "trace-evidence",
                json!({"name": "Ada"}),
                CapabilityGrantSet {
                    grants: vec![emit_evidence_grant()],
                },
                ExecutionMode::Inspect,
            ),
        )
        .unwrap();

    assert_eq!(
        first.output.as_ref().unwrap().evidence[0].uri,
        first.emitted_evidence[0].uri
    );
    assert_ne!(
        first.output.as_ref().unwrap().evidence[0].uri,
        second.output.as_ref().unwrap().evidence[0].uri
    );
    assert_eq!(
        first.output.as_ref().unwrap().evidence[0].sha256,
        second.output.as_ref().unwrap().evidence[0].sha256
    );
    assert_eq!(
        first
            .emitted_evidence
            .first()
            .expect("first emitted evidence exists")
            .blob_uri,
        second
            .emitted_evidence
            .first()
            .expect("second emitted evidence exists")
            .blob_uri
    );
    let stored = registry
        .read_resource(&first.emitted_evidence[0].uri)
        .unwrap();
    assert_eq!(stored.mime_type, "application/json");
    assert_eq!(
        serde_json::from_slice::<Value>(&stored.bytes).unwrap(),
        expected_evidence_payload(&installed, &json!({"name": "Ada"}))
    );
}

#[test]
fn guest_cannot_emit_evidence_without_returning_host_issued_refs() {
    let temp = TempFixtureDir::new();
    let workspace_root = temp.path().join("workspace");
    let source_root = workspace_root.join("examples/skills/hello-inspect");
    let registry_root = temp.path().join("registry");

    copy_dir_recursive(&example_skill_dir(), &source_root);
    copy_dir_recursive(&repo_root().join("wit"), &workspace_root.join("wit"));

    let guest_source = source_root.join("skill-rust/src/lib.rs");
    let guest = fs::read_to_string(&guest_source)
        .unwrap()
        .replace("evidence: vec![evidence],", "evidence: Vec::new(),");
    fs::write(&guest_source, guest).unwrap();

    let source_installer = LocalSourceInstaller::new(&registry_root).unwrap();
    let installed_skill = source_installer.install(&source_root).unwrap();
    let registry = LocalRegistry::load(&registry_root).unwrap();
    let runner = build_runner();

    let error = runner
        .execute(
            &registry,
            &installed_skill,
            &envelope_for(
                &installed_skill,
                "exec-invalid-evidence",
                "trace-invalid-evidence",
                json!({"name": "Ada"}),
                CapabilityGrantSet {
                    grants: vec![emit_evidence_grant()],
                },
                ExecutionMode::Inspect,
            ),
        )
        .unwrap_err();

    assert_eq!(error.code, "invalid-evidence-output");
    let receipt = error.receipt.expect("failed execution is persisted");
    let stored = registry
        .load_execution_record(&receipt.execution_id)
        .unwrap();
    assert_eq!(stored.status, ExecutionStatus::Failed);
    assert!(stored.output.is_none());
    assert_eq!(
        stored.termination.as_ref().unwrap().phase,
        guild_types::ExecutionPhase::RuntimeExec
    );
    assert_eq!(stored.emitted_evidence.len(), 1);
    assert!(stored.provenance.started_at_utc.is_some());
    assert!(stored.provenance.finished_at_utc.is_some());
}

#[test]
fn unsupported_plan_mode_fails_closed() {
    let (installed, request) = sample_request(CapabilityGrantSet::default(), ExecutionMode::Plan);
    let registry = load_registry();
    let runner = build_runner();

    let error = runner.execute(&registry, &installed, &request).unwrap_err();
    assert_eq!(error.code, "unsupported-mode");
    let receipt = error.receipt.expect("rejected execution is persisted");
    let stored = registry
        .load_execution_record(&receipt.execution_id)
        .unwrap();
    assert_eq!(stored.status, ExecutionStatus::Rejected);
    assert!(stored.output.is_none());
    assert_eq!(
        stored.termination.as_ref().unwrap().phase,
        guild_types::ExecutionPhase::Mode
    );
    assert!(stored.provenance.started_at_utc.is_some());
    assert!(stored.provenance.finished_at_utc.is_some());
}

#[test]
fn apply_stays_globally_gated_even_if_manifest_declares_it() {
    let (mut installed, mut request) =
        sample_request(CapabilityGrantSet::default(), ExecutionMode::Apply);
    installed
        .manifest
        .behavior
        .modes
        .supported
        .push(ExecutionMode::Apply);
    installed.manifest.behavior.modes.apply_requires_approval = true;
    installed
        .manifest
        .behavior
        .modes
        .apply_requires_idempotency_key = true;
    request.request.idempotency_key = Some("idem-1".into());

    let registry = load_registry();
    let runner = build_runner();
    let error = runner.execute(&registry, &installed, &request).unwrap_err();
    assert_eq!(error.code, "invalid-manifest");
    let receipt = error.receipt.expect("rejected execution is persisted");
    let stored = registry
        .load_execution_record(&receipt.execution_id)
        .unwrap();
    assert_eq!(stored.status, ExecutionStatus::Rejected);
    assert_eq!(
        stored.termination.as_ref().unwrap().phase,
        guild_types::ExecutionPhase::Validation
    );
    assert!(stored.provenance.started_at_utc.is_some());
    assert!(stored.provenance.finished_at_utc.is_some());
}

#[test]
fn missing_required_grant_is_rejected_before_execution() {
    let (mut installed, request) =
        sample_request(CapabilityGrantSet::default(), ExecutionMode::Inspect);
    installed.manifest.capabilities[1].required = true;
    installed.manifest.capabilities[1].constraints = CapabilityConstraints::Log(LogConstraints {
        levels: Some(vec![Severity::Info]),
    });

    let registry = load_registry();
    let runner = build_runner();
    let error = runner.execute(&registry, &installed, &request).unwrap_err();

    assert_eq!(error.code, "capability-mismatch");
    let receipt = error.receipt.expect("rejected execution is persisted");
    let stored = registry
        .load_execution_record(&receipt.execution_id)
        .unwrap();
    assert_eq!(stored.status, ExecutionStatus::Rejected);
    assert!(stored.output.is_none());
    let termination = stored.termination.as_ref().unwrap();
    assert_eq!(termination.phase, guild_types::ExecutionPhase::Grant);
    assert_eq!(termination.code, "capability-mismatch");
    assert!(stored.provenance.started_at_utc.is_some());
    assert!(stored.provenance.finished_at_utc.is_some());
}

#[test]
fn host_log_import_fails_closed_without_grant() {
    let registry = load_registry();
    let installed = registry.resolve(&requested_skill()).unwrap();
    let runner = build_runner();

    let error = runner
        .execute(
            &registry,
            &installed,
            &envelope_for(
                &installed,
                "exec-3",
                "trace-3",
                serde_json::json!({"name": "Ada", "emit_log": true}),
                CapabilityGrantSet {
                    grants: vec![emit_evidence_grant()],
                },
                ExecutionMode::Inspect,
            ),
        )
        .unwrap_err();

    assert_eq!(error.code, "log-write-not-granted");
    assert!(error.detail.is_some());
    let receipt = error.receipt.expect("failed execution is persisted");
    let stored = registry
        .load_execution_record(&receipt.execution_id)
        .unwrap();
    assert_eq!(stored.status, ExecutionStatus::Rejected);
    assert!(stored.output.is_none());
    let termination = stored.termination.as_ref().unwrap();
    assert_eq!(termination.phase, guild_types::ExecutionPhase::Grant);
    assert_eq!(termination.code, "log-write-not-granted");
    assert!(stored.provenance.started_at_utc.is_some());
    assert!(stored.provenance.finished_at_utc.is_some());
}

#[test]
fn caller_request_ids_do_not_control_durable_execution_ids_or_overwrite_records() {
    let registry = load_registry();
    let installed = registry.resolve(&requested_skill()).unwrap();
    let runner = build_runner();
    let first_request = envelope_for(
        &installed,
        "caller-request",
        "trace-repeat",
        json!({"name": "Ada"}),
        CapabilityGrantSet {
            grants: vec![emit_evidence_grant()],
        },
        ExecutionMode::Inspect,
    );
    let second_request = envelope_for(
        &installed,
        "caller-request",
        "trace-repeat",
        json!({"name": "Ada"}),
        CapabilityGrantSet {
            grants: vec![emit_evidence_grant()],
        },
        ExecutionMode::Inspect,
    );

    let first = runner
        .execute(&registry, &installed, &first_request)
        .unwrap();
    let second = runner
        .execute(&registry, &installed, &second_request)
        .unwrap();

    assert_eq!(first.request.request_id, second.request.request_id);
    assert_ne!(first.receipt.execution_id, first.request.request_id);
    assert_ne!(first.receipt.execution_id, second.receipt.execution_id);
    assert_eq!(
        registry
            .load_execution_record(&first.receipt.execution_id)
            .unwrap()
            .receipt
            .execution_id,
        first.receipt.execution_id
    );
    assert_eq!(
        registry
            .load_execution_record(&second.receipt.execution_id)
            .unwrap()
            .receipt
            .execution_id,
        second.receipt.execution_id
    );
}

#[test]
fn duplicate_execution_record_persistence_is_rejected() {
    let registry = load_registry();
    let installed = registry.resolve(&requested_skill()).unwrap();
    let runner = build_runner();
    let record = runner
        .execute(
            &registry,
            &installed,
            &envelope_for(
                &installed,
                "duplicate-persist",
                "trace-duplicate-persist",
                json!({"name": "Ada"}),
                CapabilityGrantSet {
                    grants: vec![emit_evidence_grant()],
                },
                ExecutionMode::Inspect,
            ),
        )
        .unwrap();

    let error = registry.persist_execution_record(&record).unwrap_err();
    assert_eq!(error.code, "execution-record-exists");
}

#[test]
fn emit_evidence_denials_are_host_owned_rejections() {
    let registry = load_registry();
    let mut installed = registry.resolve(&requested_skill()).unwrap();
    installed
        .manifest
        .capabilities
        .iter_mut()
        .find(|capability| capability.id == CapabilityId::EmitEvidence)
        .expect("fixture exposes emit-evidence capability")
        .required = false;
    let runner = build_runner();
    let error = runner
        .execute(
            &registry,
            &installed,
            &envelope_for(
                &installed,
                "emit-denied",
                "trace-emit-denied",
                json!({"name": "Ada"}),
                CapabilityGrantSet::default(),
                ExecutionMode::Inspect,
            ),
        )
        .unwrap_err();

    assert_eq!(error.code, "emit-evidence-not-granted");
    let receipt = error.receipt.expect("emit-evidence rejection is persisted");
    let stored = registry
        .load_execution_record(&receipt.execution_id)
        .unwrap();
    assert_eq!(stored.status, ExecutionStatus::Rejected);
    assert_eq!(
        stored.termination.as_ref().unwrap().phase,
        guild_types::ExecutionPhase::Grant
    );
}

#[test]
fn inspect_runtime_rejects_non_inspect_guest_abi_versions_via_manifest_validation() {
    let (mut installed, request) =
        sample_request(CapabilityGrantSet::default(), ExecutionMode::Inspect);
    installed.manifest.runtime.guest_abi_version = AbiVersion::GuildSkillV1;

    let registry = load_registry();
    let runner = build_runner();
    let error = runner.execute(&registry, &installed, &request).unwrap_err();

    assert_eq!(error.code, "invalid-manifest");
    let receipt = error
        .receipt
        .expect("broad guest ABI rejection is persisted");
    let stored = registry
        .load_execution_record(&receipt.execution_id)
        .unwrap();
    assert_eq!(stored.status, ExecutionStatus::Rejected);
    let termination = stored.termination.as_ref().unwrap();
    assert_eq!(termination.phase, guild_types::ExecutionPhase::Validation);
    assert_eq!(termination.code, "invalid-manifest");
    assert_manifest_validation_error(
        termination.detail.as_ref().unwrap(),
        "runtime.guest_abi_version",
        "guild-skill-inspect-v1 entrypoint requires guest_abi_version = guild-skill-inspect-v1",
    );
}

#[test]
fn inspect_runtime_rejects_non_inspect_entrypoints_via_manifest_validation() {
    let (mut installed, request) =
        sample_request(CapabilityGrantSet::default(), ExecutionMode::Inspect);
    installed.manifest.runtime.entrypoint = "guild-skill".into();

    let registry = load_registry();
    let runner = build_runner();
    let error = runner.execute(&registry, &installed, &request).unwrap_err();

    assert_eq!(error.code, "invalid-manifest");
    let receipt = error
        .receipt
        .expect("non-inspect entrypoint rejection is persisted");
    let stored = registry
        .load_execution_record(&receipt.execution_id)
        .unwrap();
    assert_eq!(stored.status, ExecutionStatus::Rejected);
    let termination = stored.termination.as_ref().unwrap();
    assert_eq!(termination.phase, guild_types::ExecutionPhase::Validation);
    assert_eq!(termination.code, "invalid-manifest");
    assert_manifest_validation_error(
        termination.detail.as_ref().unwrap(),
        "runtime.entrypoint",
        "guild-skill-inspect-v1 guest ABI requires runtime.entrypoint = guild-skill-inspect-v1",
    );
}

#[test]
fn inspect_guest_projection_exposes_full_active_http_grant_shape() {
    let grants = CapabilityGrantSet {
        grants: vec![emit_evidence_grant(), http_projection_grant()],
    };
    let (installed, request) = sample_request(grants, ExecutionMode::Inspect);
    let registry = load_registry();
    let runner = build_runner();

    let record = runner.execute(&registry, &installed, &request).unwrap();
    let granted = &record.output.as_ref().unwrap().structured["granted_capabilities"]["grants"][1]
        ["constraints"];

    assert_eq!(granted["allowed_hosts"][0], "127.0.0.1");
    assert_eq!(granted["allowed_host_suffixes"][0], "example.com");
    assert_eq!(granted["follow_redirects"], true);
    assert_eq!(granted["max_redirects"], 2);
    assert_eq!(granted["allow_loopback"], true);
    assert_eq!(granted["allow_link_local"], false);
    assert_eq!(granted["allow_private_networks"], false);
    assert_eq!(granted["allow_ip_literals"], true);
}

#[test]
fn inspect_guest_projection_exposes_each_active_family_shape() {
    let grants = CapabilityGrantSet {
        grants: vec![
            emit_evidence_grant(),
            log_info_grant(),
            read_resource_grant(),
            invoke_dependency_grant(),
            http_projection_grant(),
        ],
    };
    let (installed, request) = sample_request(grants, ExecutionMode::Inspect);
    let registry = load_registry();
    let runner = build_runner();

    let record = runner.execute(&registry, &installed, &request).unwrap();
    let granted = record.output.as_ref().unwrap().structured["granted_capabilities"]["grants"]
        .as_array()
        .expect("granted capabilities are projected as an array");

    assert_eq!(granted.len(), 5);
    assert_eq!(granted[0]["id"], "emit-evidence");
    assert_eq!(granted[0]["constraints"]["max_bytes"], 65_536);
    assert_eq!(granted[0]["constraints"]["audiences"][0], "user");
    assert_eq!(granted[0]["constraints"]["redactions"][0], "none");

    assert_eq!(granted[1]["id"], "log-write");
    assert_eq!(granted[1]["constraints"]["levels"][0], "info");

    assert_eq!(granted[2]["id"], "read-resource");
    assert_eq!(
        granted[2]["constraints"]["uri_prefixes"][0],
        "guild://executions/"
    );
    assert_eq!(
        granted[2]["constraints"]["uri_prefixes"][1],
        "guild://queries/executions/"
    );
    assert_eq!(granted[2]["constraints"]["resource_kinds"][0], "execution");
    assert_eq!(granted[2]["constraints"]["resource_kinds"][1], "query");

    assert_eq!(granted[3]["id"], "invoke-skill");
    assert_eq!(granted[3]["constraints"]["aliases"][0], "child");

    assert_eq!(granted[4]["id"], "http-request");
    assert_eq!(granted[4]["constraints"]["allowed_hosts"][0], "127.0.0.1");
    assert_eq!(
        granted[4]["constraints"]["allowed_host_suffixes"][0],
        "example.com"
    );
    assert_eq!(granted[4]["constraints"]["follow_redirects"], true);
}

#[test]
fn durable_host_records_keep_richer_truth_than_the_guest_projection() {
    let registry = load_registry();
    let installed = registry.resolve(&requested_skill()).unwrap();
    let runner = build_runner();
    let mut request = envelope_for(
        &installed,
        "exec-reduced",
        "trace-reduced",
        json!({"name": "Ada"}),
        CapabilityGrantSet {
            grants: vec![emit_evidence_grant()],
        },
        ExecutionMode::Inspect,
    );
    request.request.requested_capabilities = CapabilityGrantSet {
        grants: vec![emit_evidence_grant(), log_info_grant()],
    };
    request.policy_decision = PolicyDecision {
        outcome: PolicyDecisionOutcome::Reduced,
        summary: "local policy removed optional log-write before guest start".into(),
        profile_name: "restricted".into(),
        trust_tier: LocalTrustTier::Restricted,
        verification_state: InstalledVerificationState::VerifiedImport,
        reasons: vec![PolicyReason {
            code: "policy-cap-reduced".into(),
            message: "optional log-write was removed from the final guest grant set".into(),
            detail: Some(json!({
                "requested_capability": "log-write",
                "granted_capabilities": ["emit-evidence"],
            })),
        }],
        detail: Some(json!({
            "host_only_truth": {
                "requested_capability_count": 2,
                "granted_capability_count": 1,
            }
        })),
    };

    let record = runner.execute(&registry, &installed, &request).unwrap();
    let output = record.output.as_ref().expect("execution succeeded");
    let guest_grants = output.structured["granted_capabilities"]["grants"]
        .as_array()
        .expect("guest-visible granted capabilities are projected");

    assert_eq!(
        record.policy_decision.outcome,
        PolicyDecisionOutcome::Reduced
    );
    assert_eq!(record.request.requested_capabilities.grants.len(), 2);
    assert_eq!(record.granted_capabilities.grants.len(), 1);
    assert_eq!(record.policy_decision.profile_name, "restricted");
    assert_eq!(
        record.policy_decision.trust_tier,
        LocalTrustTier::Restricted
    );
    assert_eq!(
        record.policy_decision.verification_state,
        InstalledVerificationState::VerifiedImport
    );
    assert_eq!(record.policy_decision.reasons.len(), 1);
    assert!(record.policy_decision.detail.is_some());

    assert_eq!(guest_grants.len(), 1);
    assert_eq!(guest_grants[0]["id"], "emit-evidence");
    assert!(
        output.structured.get("policy_decision").is_none(),
        "guest-visible inspect output should not receive host policy state implicitly"
    );
    assert!(
        output.structured.get("requested_capabilities").is_none(),
        "guest-visible inspect output should not receive caller-requested capability state implicitly"
    );
}

#[test]
fn unsupported_manifest_capabilities_are_rejected_before_execution() {
    let (mut installed, request) =
        sample_request(CapabilityGrantSet::default(), ExecutionMode::Inspect);
    installed.manifest.capabilities.push(CapabilityRequirement {
        id: CapabilityId::CacheRead,
        access: CapabilityAccess::Read,
        constraints: CapabilityConstraints::none(),
        required: false,
    });

    let registry = load_registry();
    let runner = build_runner();
    let error = runner.execute(&registry, &installed, &request).unwrap_err();

    assert_eq!(error.code, "unsupported-runtime-surface");
    assert_eq!(
        error
            .receipt
            .as_ref()
            .expect("unsupported manifest capability rejection is persisted")
            .status,
        ExecutionStatus::Rejected
    );
    let receipt = error
        .receipt
        .expect("unsupported manifest capability rejection is persisted");
    let stored = registry
        .load_execution_record(&receipt.execution_id)
        .unwrap();
    assert_eq!(stored.status, ExecutionStatus::Rejected);
    assert_eq!(
        stored
            .termination
            .as_ref()
            .unwrap()
            .detail
            .as_ref()
            .unwrap()["classification"],
        "unsupported-runtime-surface"
    );
}

#[test]
fn filesystem_manifest_capabilities_are_rejected_before_execution() {
    let (mut installed, request) =
        sample_request(CapabilityGrantSet::default(), ExecutionMode::Inspect);
    installed.manifest.capabilities.push(CapabilityRequirement {
        id: CapabilityId::Filesystem,
        access: CapabilityAccess::Read,
        constraints: filesystem_read_grant().constraints,
        required: true,
    });

    let registry = load_registry();
    let runner = build_runner();
    let error = runner.execute(&registry, &installed, &request).unwrap_err();

    assert_eq!(error.code, "filesystem-runtime-not-supported");
    let receipt = error
        .receipt
        .expect("filesystem manifest rejection is persisted");
    let stored = registry
        .load_execution_record(&receipt.execution_id)
        .unwrap();
    assert_eq!(stored.status, ExecutionStatus::Rejected);
    assert_eq!(
        stored.termination.as_ref().unwrap().phase,
        guild_types::ExecutionPhase::Validation
    );
    assert_eq!(
        stored
            .termination
            .as_ref()
            .unwrap()
            .detail
            .as_ref()
            .unwrap()["classification"],
        "unsupported-runtime-surface"
    );
}

#[test]
fn filesystem_grants_are_rejected_before_execution() {
    let (installed, request) = sample_request(
        CapabilityGrantSet {
            grants: vec![filesystem_read_grant()],
        },
        ExecutionMode::Inspect,
    );

    let registry = load_registry();
    let runner = build_runner();
    let error = runner.execute(&registry, &installed, &request).unwrap_err();

    assert_eq!(error.code, "filesystem-runtime-not-supported");
    let receipt = error
        .receipt
        .expect("filesystem grant rejection is persisted");
    let stored = registry
        .load_execution_record(&receipt.execution_id)
        .unwrap();
    assert_eq!(stored.status, ExecutionStatus::Rejected);
    assert_eq!(
        stored.termination.as_ref().unwrap().phase,
        guild_types::ExecutionPhase::Validation
    );
    assert_eq!(
        stored
            .termination
            .as_ref()
            .unwrap()
            .detail
            .as_ref()
            .unwrap()["classification"],
        "unsupported-runtime-surface"
    );
}
