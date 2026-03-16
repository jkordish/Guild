use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use guild_registry::{LocalRegistry, LocalSourceInstaller, SkillRegistry};
use guild_runner::{ExecutionError, Runner, WasmtimeRuntimeAdapter};
use guild_types::{
    Budget, CallerRequest, CapabilityAccess, CapabilityConstraints, CapabilityGrantSet,
    CapabilityId, EmitEvidenceConstraints, EvidenceAudience, ExecutionMode, ExecutionRecord,
    GrantedCapability, InvokeDependencyConstraints, PolicyDecision, PolicyDecisionOutcome,
    ReadResourceConstraints, RedactionClass, RequestedSkillRef, ResolvedExecutionEnvelope,
    ResourceKind, SkillKey, SkillOutput, VersionRequirement,
};
use serde_json::{json, Value};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn primitive_source_dir() -> PathBuf {
    repo_root().join("examples/skills/hello-inspect")
}

fn composite_source_dir() -> PathBuf {
    repo_root().join("examples/skills/hello-composite")
}

fn explain_source_dir() -> PathBuf {
    repo_root().join("examples/skills/explain-execution")
}

fn explain_tree_source_dir() -> PathBuf {
    repo_root().join("examples/skills/explain-execution-tree")
}

fn prepared_registry_root() -> &'static PathBuf {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();

    ROOT.get_or_init(|| {
        let root = repo_root().join("target/test-install-registry/guild-runner-resource-reads");
        if root.exists() {
            let _ = fs::remove_dir_all(&root);
        }

        let installer = LocalSourceInstaller::new(&root).unwrap();
        installer.install(primitive_source_dir()).unwrap();
        installer.install(composite_source_dir()).unwrap();
        installer.install(explain_source_dir()).unwrap();
        installer.install(explain_tree_source_dir()).unwrap();
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

fn unique_id(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{prefix}-{nanos}")
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
            uri_prefixes: Some(prefixes.iter().map(|prefix| (*prefix).to_owned()).collect()),
            resource_kinds: Some(resource_kinds),
        }),
    }
}

fn invoke_grant(aliases: &[&str]) -> GrantedCapability {
    GrantedCapability {
        id: CapabilityId::InvokeSkill,
        access: CapabilityAccess::Invoke,
        constraints: CapabilityConstraints::InvokeDependency(InvokeDependencyConstraints {
            aliases: Some(aliases.iter().map(|alias| (*alias).to_owned()).collect()),
        }),
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

fn execution_request(
    skill: &guild_registry::InstalledSkill,
    request_id: impl Into<String>,
    input: Value,
    grants: CapabilityGrantSet,
) -> ResolvedExecutionEnvelope {
    let request_id = request_id.into();

    ResolvedExecutionEnvelope {
        request: CallerRequest {
            request_id: format!("{request_id}-request"),
            skill: requested_skill(&skill.resolved_ref.key.name),
            tenant_id: "tenant-1".into(),
            actor_id: "actor-1".into(),
            mode: ExecutionMode::Inspect,
            input,
            budget: Budget::default(),
            requested_capabilities: grants.clone(),
            idempotency_key: None,
            trace_id: unique_id("trace"),
        },
        resolved_skill: skill.resolved_ref.clone(),
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

fn run_hello_inspect(
    registry: &LocalRegistry,
    runner: &Runner<WasmtimeRuntimeAdapter>,
) -> ExecutionRecord {
    let installed = registry.resolve(&requested_skill("hello-inspect")).unwrap();
    runner
        .execute(
            registry,
            &installed,
            &execution_request(
                &installed,
                unique_id("hello-inspect"),
                json!({ "name": "Ada" }),
                CapabilityGrantSet {
                    grants: vec![emit_evidence_grant()],
                },
            ),
        )
        .unwrap()
}

fn run_failed_hello_inspect(
    registry: &LocalRegistry,
    runner: &Runner<WasmtimeRuntimeAdapter>,
) -> ExecutionRecord {
    let installed = registry.resolve(&requested_skill("hello-inspect")).unwrap();
    let error = runner
        .execute(
            registry,
            &installed,
            &execution_request(
                &installed,
                unique_id("hello-inspect-failed"),
                json!({ "name": "Ada", "emit_log": true }),
                CapabilityGrantSet {
                    grants: vec![emit_evidence_grant()],
                },
            ),
        )
        .unwrap_err();

    registry
        .load_execution_record(&error.receipt.unwrap().execution_id)
        .unwrap()
}

fn run_hello_composite(
    registry: &LocalRegistry,
    runner: &Runner<WasmtimeRuntimeAdapter>,
) -> ExecutionRecord {
    let installed = registry
        .resolve(&requested_skill("hello-composite"))
        .unwrap();
    runner
        .execute(
            registry,
            &installed,
            &execution_request(
                &installed,
                unique_id("hello-composite"),
                json!({ "name": "Ada" }),
                CapabilityGrantSet {
                    grants: vec![invoke_grant(&["hello"]), emit_evidence_grant()],
                },
            ),
        )
        .unwrap()
}

fn run_failed_hello_composite(
    registry: &LocalRegistry,
    runner: &Runner<WasmtimeRuntimeAdapter>,
) -> ExecutionRecord {
    let installed = registry
        .resolve(&requested_skill("hello-composite"))
        .unwrap();
    let error = runner
        .execute(
            registry,
            &installed,
            &execution_request(
                &installed,
                unique_id("hello-composite-failed"),
                json!({ "name": "Ada", "child_emit_log": true }),
                CapabilityGrantSet {
                    grants: vec![invoke_grant(&["hello"]), emit_evidence_grant()],
                },
            ),
        )
        .unwrap_err();

    registry
        .load_execution_record(&error.receipt.unwrap().execution_id)
        .unwrap()
}

fn run_rejected_explain_execution(
    registry: &LocalRegistry,
    runner: &Runner<WasmtimeRuntimeAdapter>,
) -> ExecutionRecord {
    let installed = registry
        .resolve(&requested_skill("explain-execution"))
        .unwrap();
    let error = runner
        .execute(
            registry,
            &installed,
            &execution_request(
                &installed,
                unique_id("explain-execution-rejected"),
                json!({
                    "execution_uri": "guild://executions/not-used",
                    "include_first_evidence": false,
                }),
                CapabilityGrantSet::default(),
            ),
        )
        .unwrap_err();

    registry
        .load_execution_record(&error.receipt.unwrap().execution_id)
        .unwrap()
}

fn run_explain_execution(
    registry: &LocalRegistry,
    runner: &Runner<WasmtimeRuntimeAdapter>,
    target_uri: &str,
    include_first_evidence: bool,
    grants: CapabilityGrantSet,
) -> Result<ExecutionRecord, ExecutionError> {
    let installed = registry
        .resolve(&requested_skill("explain-execution"))
        .unwrap();
    runner.execute(
        registry,
        &installed,
        &execution_request(
            &installed,
            unique_id("explain-execution"),
            json!({
                "execution_uri": target_uri,
                "include_first_evidence": include_first_evidence,
            }),
            grants,
        ),
    )
}

fn run_explain_execution_tree(
    registry: &LocalRegistry,
    runner: &Runner<WasmtimeRuntimeAdapter>,
    target_uri: &str,
    max_depth: Option<u64>,
    max_nodes: Option<u64>,
    include_evidence_resources: bool,
    grants: CapabilityGrantSet,
) -> Result<ExecutionRecord, ExecutionError> {
    let installed = registry
        .resolve(&requested_skill("explain-execution-tree"))
        .unwrap();
    let mut input = serde_json::Map::new();
    input.insert("execution_uri".into(), Value::String(target_uri.to_owned()));
    input.insert(
        "include_evidence_resources".into(),
        Value::Bool(include_evidence_resources),
    );
    if let Some(max_depth) = max_depth {
        input.insert("max_depth".into(), json!(max_depth));
    }
    if let Some(max_nodes) = max_nodes {
        input.insert("max_nodes".into(), json!(max_nodes));
    }

    runner.execute(
        registry,
        &installed,
        &execution_request(
            &installed,
            unique_id("explain-execution-tree"),
            Value::Object(input),
            grants,
        ),
    )
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
        let path = std::env::temp_dir().join(format!("guild-resource-reads-{unique}"));
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

fn write_json(path: &Path, value: &Value) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

fn resource_composite_manifest() -> Value {
    json!({
        "manifest_schema_version": "guild-manifest-v1",
        "skill_api_version": "guild-skill-v1",
        "key": {
            "namespace": "example",
            "name": "resource-composite"
        },
        "version": "0.1.0",
        "display_name": "Resource Composite",
        "description": "A test composite skill that delegates to explain-execution.",
        "runtime": {
            "kind": "wasm-component",
            "entrypoint": "guild-skill",
            "guest_abi_version": "guild-skill-v1"
        },
        "interface": {
            "input_schema_uri": "./input.schema.json",
            "output_schema_uri": "./output.schema.json",
            "examples_uri": null
        },
        "behavior": {
            "category": "explain",
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
                    "aliases": ["report"]
                }
            }
        ],
        "dependencies": [
            {
                "alias": "report",
                "skill": {
                    "key": {
                        "namespace": "example",
                        "name": "explain-execution"
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
        "tests": []
    })
}

fn resource_composite_input_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "ResourceCompositeInput",
        "type": "object",
        "properties": {
            "execution_uri": { "type": "string" }
        },
        "required": ["execution_uri"],
        "additionalProperties": false
    })
}

fn resource_composite_output_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "ResourceCompositeOutput",
        "type": "object"
    })
}

fn resource_composite_cargo_toml() -> &'static str {
    r#"[package]
name = "guild-example-resource-composite"
version = "0.1.0"
edition = "2021"

[workspace]

[lib]
crate-type = ["cdylib"]

[dependencies]
serde_json = "1"
wit-bindgen = "0.53.1"
"#
}

fn resource_composite_guest_source() -> &'static str {
    r#"use serde_json::{json, Value};
use wit_bindgen::generate;

generate!({
    path: "../../../../wit",
    world: "guild-skill",
});

use crate::exports::guild::skill::skill::{ExecutionContext, Guest, Json, SkillError, SkillOutput};
use crate::guild::skill::host;
use crate::guild::skill::types::DependencyInvocationRequest;

struct ResourceComposite;

impl Guest for ResourceComposite {
    fn run(_ctx: ExecutionContext, input: Json) -> Result<SkillOutput, SkillError> {
        let parsed_input: Value = serde_json::from_str(&input).map_err(|error| SkillError {
            code: "invalid-input".into(),
            message: "input JSON could not be parsed".into(),
            retryable: false,
            detail: Some(json!({ "error": error.to_string() }).to_string()),
        })?;
        let execution_uri = parsed_input
            .get("execution_uri")
            .and_then(Value::as_str)
            .ok_or_else(|| SkillError {
                code: "missing-execution-uri".into(),
                message: "execution_uri must be provided".into(),
                retryable: false,
                detail: None,
            })?;

        let child = host::invoke_dependency(&DependencyInvocationRequest {
            alias: "report".into(),
            input: json!({
                "execution_uri": execution_uri,
                "include_first_evidence": true
            })
            .to_string(),
        })?;
        let child_structured: Value =
            serde_json::from_str(&child.structured).unwrap_or(Value::Null);

        Ok(SkillOutput {
            summary: "Nested explain composite completed.".into(),
            structured: json!({
                "child_summary": child.summary,
                "child_structured": child_structured,
            })
            .to_string(),
            diagnostics: Vec::new(),
            effects: Vec::new(),
            evidence: Vec::new(),
        })
    }
}

export!(ResourceComposite with_types_in self);
"#
}

fn write_temp_resource_composite_skill(root: &Path) -> PathBuf {
    let workspace_root = root.join("workspace");
    let source_root = workspace_root.join("examples/skills/resource-composite");
    fs::create_dir_all(source_root.join("skill-rust/src")).unwrap();
    copy_dir_recursive(&repo_root().join("wit"), &workspace_root.join("wit"));

    write_json(
        &source_root.join("manifest.json"),
        &resource_composite_manifest(),
    );
    write_json(
        &source_root.join("input.schema.json"),
        &resource_composite_input_schema(),
    );
    write_json(
        &source_root.join("output.schema.json"),
        &resource_composite_output_schema(),
    );
    fs::write(
        source_root.join("skill-rust/Cargo.toml"),
        resource_composite_cargo_toml(),
    )
    .unwrap();
    fs::write(
        source_root.join("skill-rust/src/lib.rs"),
        resource_composite_guest_source(),
    )
    .unwrap();

    source_root
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

fn execution_record_file(root: &Path, execution_id: &str) -> PathBuf {
    root.join("executions").join(format!("{execution_id}.json"))
}

#[test]
fn explain_skill_reads_allowed_execution_and_evidence_resources() {
    let registry = load_registry();
    let runner = build_runner();
    let hello_record = run_hello_inspect(&registry, &runner);
    let hello_output = hello_record.output.as_ref().unwrap();
    let execution_resource = registry.read_resource(&hello_record.receipt.uri).unwrap();
    let evidence_resource = registry
        .read_resource(&hello_record.emitted_evidence[0].uri)
        .unwrap();

    let explain_record = run_explain_execution(
        &registry,
        &runner,
        &hello_record.receipt.uri,
        true,
        CapabilityGrantSet {
            grants: vec![read_resource_grant(&[
                "guild://executions/",
                "guild://objects/records/",
            ])],
        },
    )
    .unwrap();
    let explain_output = explain_record.output.as_ref().unwrap();

    assert_eq!(
        explain_output.summary,
        format!("Explained stored execution {}.", hello_record.receipt.uri)
    );
    assert_eq!(
        explain_output.structured["target_execution_uri"],
        hello_record.receipt.uri
    );
    assert_eq!(
        explain_output.structured["execution_resource"]["mime_type"],
        execution_resource.mime_type
    );
    assert_eq!(
        explain_output.structured["execution_resource"]["sha256"],
        execution_resource.sha256.clone().unwrap()
    );
    assert_eq!(
        explain_output.structured["target_skill"]["digest"],
        hello_record.provenance.resolved_skill.digest
    );
    assert_eq!(
        explain_output.structured["stored_summary"],
        hello_output.summary
    );
    assert_eq!(explain_output.structured["termination"], Value::Null);
    assert_eq!(explain_output.structured["evidence_count"], 1);
    assert_eq!(explain_output.structured["child_execution_count"], 0);
    assert_eq!(
        explain_output.structured["first_evidence"]["uri"],
        hello_record.emitted_evidence[0].uri
    );
    assert_eq!(
        explain_output.structured["first_evidence"]["sha256"],
        evidence_resource.sha256.clone().unwrap()
    );

    let mut expected_output: SkillOutput = serde_json::from_str(
        &fs::read_to_string(explain_source_dir().join("tests/expected-output.json")).unwrap(),
    )
    .unwrap();
    expected_output.summary = expected_output
        .summary
        .replace("__TARGET_EXECUTION_URI__", &hello_record.receipt.uri);
    expected_output.structured["target_execution_uri"] =
        Value::String(hello_record.receipt.uri.clone());
    expected_output.structured["execution_resource"]["uri"] =
        Value::String(hello_record.receipt.uri.clone());
    expected_output.structured["execution_resource"]["sha256"] =
        Value::String(execution_resource.sha256.clone().unwrap());
    expected_output.structured["target_skill"]["digest"] =
        Value::String(hello_record.provenance.resolved_skill.digest.clone());
    expected_output.structured["termination"] = Value::Null;
    expected_output.structured["first_evidence"]["uri"] =
        Value::String(hello_record.emitted_evidence[0].uri.clone());
    expected_output.structured["first_evidence"]["sha256"] =
        Value::String(evidence_resource.sha256.clone().unwrap());
    expected_output.structured["first_evidence"]["json"]["skill"]["digest"] =
        Value::String(hello_record.provenance.resolved_skill.digest.clone());
    expected_output.structured["granted_capabilities"] =
        explain_output.structured["granted_capabilities"].clone();
    expected_output.structured["policy_decision"] =
        explain_output.structured["policy_decision"].clone();

    assert_eq!(explain_record.output, Some(expected_output));
}

#[test]
fn optional_object_reads_fail_closed_without_object_scope() {
    let registry = load_registry();
    let runner = build_runner();
    let hello_record = run_hello_inspect(&registry, &runner);

    let error = run_explain_execution(
        &registry,
        &runner,
        &hello_record.receipt.uri,
        true,
        CapabilityGrantSet {
            grants: vec![read_resource_grant(&["guild://executions/"])],
        },
    )
    .unwrap_err();

    assert_eq!(error.code, "read-resource-kind-denied");
}

#[test]
fn object_record_scope_mismatch_fails_distinctly_from_missing_resource() {
    let registry = load_registry();
    let runner = build_runner();
    let hello_record = run_hello_inspect(&registry, &runner);

    let error = run_explain_execution(
        &registry,
        &runner,
        &hello_record.receipt.uri,
        true,
        CapabilityGrantSet {
            grants: vec![read_resource_grant(&[
                "guild://executions/",
                "guild://objects/sha256/",
            ])],
        },
    )
    .unwrap_err();

    assert_eq!(error.code, "read-resource-not-granted");
}

#[test]
fn missing_execution_resource_fails_cleanly() {
    let registry = load_registry();
    let runner = build_runner();
    let error = run_explain_execution(
        &registry,
        &runner,
        "guild://executions/does-not-exist",
        false,
        CapabilityGrantSet {
            grants: vec![read_resource_grant(&["guild://executions/"])],
        },
    )
    .unwrap_err();

    assert_eq!(error.code, "read-resource-failed");
    assert!(error
        .detail
        .unwrap()
        .to_string()
        .contains("execution-not-found"));
}

#[test]
fn malformed_execution_uri_fails_cleanly() {
    let registry = load_registry();
    let runner = build_runner();
    let error = run_explain_execution(
        &registry,
        &runner,
        "guild://executions/%GG",
        false,
        CapabilityGrantSet {
            grants: vec![read_resource_grant(&["guild://executions/"])],
        },
    )
    .unwrap_err();

    assert_eq!(error.code, "resource-uri-invalid");
}

#[test]
fn non_canonical_resource_scopes_are_rejected_before_execution() {
    let registry = load_registry();
    let runner = build_runner();
    let hello_record = run_hello_inspect(&registry, &runner);

    let error = run_explain_execution(
        &registry,
        &runner,
        &hello_record.receipt.uri,
        false,
        CapabilityGrantSet {
            grants: vec![GrantedCapability {
                id: CapabilityId::ReadResource,
                access: CapabilityAccess::Read,
                constraints: CapabilityConstraints::ReadResource(ReadResourceConstraints {
                    uri_prefixes: Some(vec!["guild://objects/".into()]),
                    resource_kinds: Some(vec![ResourceKind::Object]),
                }),
            }],
        },
    )
    .unwrap_err();

    assert_eq!(error.code, "capability-grant-invalid");
    assert!(error
        .detail
        .unwrap()
        .to_string()
        .contains("expected canonical roots"));
}

#[test]
fn explain_skill_reports_child_execution_linkage_for_composites() {
    let registry = load_registry();
    let runner = build_runner();
    let composite_record = run_hello_composite(&registry, &runner);

    let explain_record = run_explain_execution(
        &registry,
        &runner,
        &composite_record.receipt.uri,
        false,
        CapabilityGrantSet {
            grants: vec![read_resource_grant(&["guild://executions/"])],
        },
    )
    .unwrap();
    let explain_output = explain_record.output.as_ref().unwrap();
    let composite_output = composite_record.output.as_ref().unwrap();

    assert_eq!(explain_output.structured["child_execution_count"], 1);
    assert_eq!(
        explain_output.structured["child_execution_uris"][0],
        composite_record.child_executions[0].uri
    );
    assert_eq!(
        explain_output.structured["stored_summary"],
        composite_output.summary
    );
}

#[test]
fn explain_skill_summarizes_capability_rejections_without_output() {
    let registry = load_registry();
    let runner = build_runner();
    let failed = run_failed_hello_inspect(&registry, &runner);

    let explain_record = run_explain_execution(
        &registry,
        &runner,
        &failed.receipt.uri,
        false,
        CapabilityGrantSet {
            grants: vec![read_resource_grant(&["guild://executions/"])],
        },
    )
    .unwrap();
    let explain_output = explain_record.output.as_ref().unwrap();

    assert_eq!(explain_output.structured["target_status"], "rejected");
    assert_eq!(explain_output.structured["termination"]["phase"], "grant");
    assert_eq!(
        explain_output.structured["termination"]["code"],
        "log-write-not-granted"
    );
    assert_eq!(explain_output.structured["stored_summary"], Value::Null);
    assert_eq!(explain_output.structured["evidence_count"], 0);
}

#[test]
fn explain_skill_summarizes_rejected_execution_records_without_output() {
    let registry = load_registry();
    let runner = build_runner();
    let rejected = run_rejected_explain_execution(&registry, &runner);

    let explain_record = run_explain_execution(
        &registry,
        &runner,
        &rejected.receipt.uri,
        false,
        CapabilityGrantSet {
            grants: vec![read_resource_grant(&["guild://executions/"])],
        },
    )
    .unwrap();
    let explain_output = explain_record.output.as_ref().unwrap();

    assert_eq!(explain_output.structured["target_status"], "rejected");
    assert_eq!(explain_output.structured["termination"]["phase"], "grant");
    assert_eq!(
        explain_output.structured["termination"]["code"],
        "capability-mismatch"
    );
    assert_eq!(explain_output.structured["stored_summary"], Value::Null);
    assert_eq!(explain_output.structured["evidence_count"], 0);
}

#[allow(clippy::too_many_lines)]
#[test]
fn explain_tree_skill_reports_composite_execution_tree() {
    let registry = load_registry();
    let runner = build_runner();
    let composite_record = run_hello_composite(&registry, &runner);
    let child_record = registry
        .load_execution_record(&composite_record.child_executions[0].execution_id)
        .unwrap();
    let evidence_resource = registry
        .read_resource(&child_record.emitted_evidence[0].uri)
        .unwrap();
    let evidence_sha256 = evidence_resource.sha256.clone().unwrap();

    let explain_record = run_explain_execution_tree(
        &registry,
        &runner,
        &composite_record.receipt.uri,
        None,
        None,
        true,
        CapabilityGrantSet {
            grants: vec![read_resource_grant(&[
                "guild://executions/",
                "guild://objects/records/",
            ])],
        },
    )
    .unwrap();
    let explain_output = explain_record.output.as_ref().unwrap();

    assert_eq!(
        explain_output.summary,
        format!(
            "Execution tree rooted at {} visited 2 node(s): 2 succeeded, 0 failed, 0 rejected, 1 evidence record(s).",
            composite_record.receipt.uri
        )
    );
    assert_eq!(
        explain_output.structured["root_execution_id"],
        composite_record.receipt.execution_id
    );
    assert_eq!(explain_output.structured["root_status"], "succeeded");
    assert_eq!(explain_output.structured["child_execution_count"], 1);
    assert_eq!(explain_output.structured["descendant_execution_count"], 1);
    assert_eq!(explain_output.structured["nodes_visited"], 2);
    assert_eq!(explain_output.structured["max_depth_walked"], 1);
    assert_eq!(explain_output.structured["traversal_truncated"], false);
    assert_eq!(
        explain_output.structured["status_counts"],
        json!({
            "failed": 0,
            "partial": 0,
            "rejected": 0,
            "succeeded": 2,
        })
    );
    assert_eq!(
        explain_output.structured["executions"][1]["execution_uri"],
        child_record.receipt.uri
    );
    assert_eq!(
        explain_output.structured["executions"][1]["alias_from_parent"],
        "hello"
    );
    assert_eq!(explain_output.structured["evidence_summary"]["total"], 1);
    assert_eq!(
        explain_output.structured["evidence_summary"]["notable_evidence_uris"][0],
        child_record.emitted_evidence[0].uri
    );
    assert_eq!(
        explain_output.structured["evidence_summary"]["resource_descriptors"][0]["sha256"],
        evidence_sha256
    );
    assert!(
        explain_output.structured["evidence_summary"]["resource_descriptors"][0]
            .get("json")
            .is_none()
    );

    let mut expected_output: SkillOutput = serde_json::from_str(
        &fs::read_to_string(explain_tree_source_dir().join("tests/expected-output.json")).unwrap(),
    )
    .unwrap();
    expected_output.summary = expected_output
        .summary
        .replace("__TARGET_EXECUTION_URI__", &composite_record.receipt.uri);
    expected_output.structured["target_execution_uri"] =
        Value::String(composite_record.receipt.uri.clone());
    expected_output.structured["root_execution_id"] =
        Value::String(composite_record.receipt.execution_id.clone());
    expected_output.structured["root_execution_uri"] =
        Value::String(composite_record.receipt.uri.clone());
    expected_output.structured["root_resolved_skill"]["digest"] =
        Value::String(composite_record.resolved_skill.digest.clone());
    expected_output.structured["executions"][0]["execution_uri"] =
        Value::String(composite_record.receipt.uri.clone());
    expected_output.structured["executions"][0]["execution_id"] =
        Value::String(composite_record.receipt.execution_id.clone());
    expected_output.structured["executions"][0]["resolved_skill"]["digest"] =
        Value::String(composite_record.resolved_skill.digest.clone());
    expected_output.structured["executions"][1]["execution_uri"] =
        Value::String(child_record.receipt.uri.clone());
    expected_output.structured["executions"][1]["execution_id"] =
        Value::String(child_record.receipt.execution_id.clone());
    expected_output.structured["executions"][1]["parent_execution_id"] =
        Value::String(composite_record.receipt.execution_id.clone());
    expected_output.structured["executions"][1]["resolved_skill"]["digest"] =
        Value::String(child_record.resolved_skill.digest.clone());
    expected_output.structured["evidence_summary"]["notable_evidence_uris"][0] =
        Value::String(child_record.emitted_evidence[0].uri.clone());
    expected_output.structured["evidence_summary"]["resource_descriptors"][0]["uri"] =
        Value::String(child_record.emitted_evidence[0].uri.clone());
    expected_output.structured["evidence_summary"]["resource_descriptors"][0]["sha256"] =
        Value::String(evidence_sha256);
    expected_output.structured["granted_capabilities"] =
        explain_output.structured["granted_capabilities"].clone();

    assert_eq!(explain_record.output, Some(expected_output));
}

#[test]
fn explain_tree_skill_summarizes_failed_parent_and_rejected_child() {
    let registry = load_registry();
    let runner = build_runner();
    let failed_parent = run_failed_hello_composite(&registry, &runner);

    let explain_record = run_explain_execution_tree(
        &registry,
        &runner,
        &failed_parent.receipt.uri,
        None,
        None,
        false,
        CapabilityGrantSet {
            grants: vec![read_resource_grant(&["guild://executions/"])],
        },
    )
    .unwrap();
    let explain_output = explain_record.output.as_ref().unwrap();

    assert_eq!(explain_output.structured["root_status"], "failed");
    assert_eq!(explain_output.structured["status_counts"]["failed"], 1);
    assert_eq!(explain_output.structured["status_counts"]["rejected"], 1);
    assert_eq!(
        explain_output.structured["executions"][0]["termination"]["phase"],
        "child-invocation"
    );
    assert_eq!(
        explain_output.structured["executions"][1]["status"],
        "rejected"
    );
    assert_eq!(
        explain_output.structured["denial_summaries"][0]["code"],
        "log-write-not-granted"
    );
}

#[test]
fn explain_tree_skill_respects_depth_and_node_bounds() {
    let registry = load_registry();
    let runner = build_runner();
    let composite_record = run_hello_composite(&registry, &runner);

    let depth_limited = run_explain_execution_tree(
        &registry,
        &runner,
        &composite_record.receipt.uri,
        Some(0),
        None,
        false,
        CapabilityGrantSet {
            grants: vec![read_resource_grant(&["guild://executions/"])],
        },
    )
    .unwrap();
    let depth_output = depth_limited.output.as_ref().unwrap();
    assert_eq!(depth_output.structured["nodes_visited"], 1);
    assert_eq!(depth_output.structured["descendant_execution_count"], 0);
    assert_eq!(depth_output.structured["traversal_truncated"], true);
    assert_eq!(
        depth_output.structured["lineage_warnings"][0]["code"],
        "max-depth-reached"
    );

    let node_limited = run_explain_execution_tree(
        &registry,
        &runner,
        &composite_record.receipt.uri,
        None,
        Some(1),
        false,
        CapabilityGrantSet {
            grants: vec![read_resource_grant(&["guild://executions/"])],
        },
    )
    .unwrap();
    let node_output = node_limited.output.as_ref().unwrap();
    assert_eq!(node_output.structured["nodes_visited"], 1);
    assert_eq!(node_output.structured["descendant_execution_count"], 0);
    assert_eq!(node_output.structured["traversal_truncated"], true);
    assert_eq!(
        node_output.structured["lineage_warnings"][0]["code"],
        "max-nodes-reached"
    );
}

#[test]
fn explain_tree_skill_detects_revisited_execution_uris() {
    let temp = TempFixtureDir::new();
    let registry_root = temp.path().join("registry");
    let installer = LocalSourceInstaller::new(&registry_root).unwrap();
    installer.install(primitive_source_dir()).unwrap();
    installer.install(composite_source_dir()).unwrap();
    installer.install(explain_tree_source_dir()).unwrap();

    let registry = LocalRegistry::load(&registry_root).unwrap();
    let runner = build_runner();
    let composite_record = run_hello_composite(&registry, &runner);

    let mut parent_json: Value = serde_json::from_slice(
        &fs::read(execution_record_file(
            &registry_root,
            &composite_record.receipt.execution_id,
        ))
        .unwrap(),
    )
    .unwrap();
    parent_json["child_executions"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "alias": "loop",
            "execution_id": composite_record.receipt.execution_id,
            "uri": composite_record.receipt.uri,
            "parent_execution_id": composite_record.receipt.execution_id,
            "trace_id": composite_record.receipt.trace_id,
            "status": "succeeded",
            "policy_decision": {
                "outcome": "allowed",
                "summary": "synthetic loop for test",
                "profile_name": "default",
                "trust_tier": "local-dev",
                "verification_state": "local-source",
                "reasons": [],
                "detail": null
            },
            "termination": null,
            "granted_capabilities": {
                "grants": []
            },
            "metrics": {
                "duration_ms": 0,
                "network_requests": 0,
                "child_executions": 0,
                "cache_hits": 0,
                "cache_misses": 0
            },
            "provenance": composite_record.provenance,
        }));
    fs::write(
        execution_record_file(&registry_root, &composite_record.receipt.execution_id),
        serde_json::to_vec_pretty(&parent_json).unwrap(),
    )
    .unwrap();

    let explain_record = run_explain_execution_tree(
        &registry,
        &runner,
        &composite_record.receipt.uri,
        None,
        None,
        false,
        CapabilityGrantSet {
            grants: vec![read_resource_grant(&["guild://executions/"])],
        },
    )
    .unwrap();
    let explain_output = explain_record.output.as_ref().unwrap();

    assert_eq!(explain_output.structured["nodes_visited"], 2);
    assert_eq!(explain_output.structured["traversal_truncated"], true);
    assert_eq!(
        explain_output.structured["lineage_warnings"][0]["code"],
        "execution-uri-revisited"
    );
}

#[test]
fn explain_tree_skill_skips_optional_evidence_reads_without_object_scope() {
    let registry = load_registry();
    let runner = build_runner();
    let composite_record = run_hello_composite(&registry, &runner);

    let explain_record = run_explain_execution_tree(
        &registry,
        &runner,
        &composite_record.receipt.uri,
        None,
        None,
        true,
        CapabilityGrantSet {
            grants: vec![read_resource_grant(&["guild://executions/"])],
        },
    )
    .unwrap();
    let explain_output = explain_record.output.as_ref().unwrap();

    assert_eq!(explain_output.structured["evidence_summary"]["total"], 1);
    assert!(
        explain_output.structured["evidence_summary"]["resource_descriptors"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        explain_output.structured["lineage_warnings"][0]["code"],
        "object-scope-not-granted"
    );
}

#[test]
fn nested_child_resource_reads_cannot_expand_parent_scope() {
    let temp = TempFixtureDir::new();
    let registry_root = temp.path().join("registry");
    let installer = LocalSourceInstaller::new(&registry_root).unwrap();
    installer.install(primitive_source_dir()).unwrap();
    installer.install(explain_source_dir()).unwrap();

    let source_root = write_temp_resource_composite_skill(temp.path());
    installer.install(&source_root).unwrap();

    let registry = LocalRegistry::load(&registry_root).unwrap();
    let runner = build_runner();
    let hello_record = run_hello_inspect(&registry, &runner);
    let composite = registry
        .resolve(&requested_skill("resource-composite"))
        .unwrap();

    let error = runner
        .execute(
            &registry,
            &composite,
            &execution_request(
                &composite,
                unique_id("resource-composite"),
                json!({ "execution_uri": hello_record.receipt.uri }),
                CapabilityGrantSet {
                    grants: vec![
                        invoke_grant(&["report"]),
                        read_resource_grant(&["guild://executions/", "guild://objects/sha256/"]),
                    ],
                },
            ),
        )
        .unwrap_err();

    assert_eq!(error.code, "child-invocation-failed");
}
