use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use guild_manifest::SkillManifest;
use guild_registry::{LocalRegistry, LocalSourceInstaller, SkillRegistry};
use guild_runner::{Runner, WasmtimeRuntimeAdapter};
use guild_types::{
    Budget, CallerRequest, CapabilityAccess, CapabilityConstraints, CapabilityGrantSet,
    CapabilityId, EmitEvidenceConstraints, EvidenceAudience, ExecutionMode, ExecutionStatus,
    GrantedCapability, InvokeDependencyConstraints, LogConstraints, PolicyDecision,
    PolicyDecisionOutcome, RedactionClass, RequestedSkillRef, ResolvedExecutionEnvelope, Severity,
    SkillKey, VersionRequirement,
};

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

fn prepared_registry_root() -> &'static PathBuf {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();

    ROOT.get_or_init(|| {
        let root = repo_root().join("target/test-install-registry/guild-runner-composition");
        if root.exists() {
            let _ = fs::remove_dir_all(&root);
        }

        let installer = LocalSourceInstaller::new(&root).unwrap();
        installer.install(primitive_source_dir()).unwrap();
        installer.install(composite_source_dir()).unwrap();
        root
    })
}

fn requested_composite() -> RequestedSkillRef {
    RequestedSkillRef {
        key: SkillKey {
            namespace: "example".into(),
            name: "hello-composite".into(),
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

fn composite_request(
    grants: CapabilityGrantSet,
    input: serde_json::Value,
) -> ResolvedExecutionEnvelope {
    let installed = load_registry().resolve(&requested_composite()).unwrap();

    ResolvedExecutionEnvelope {
        request: CallerRequest {
            request_id: "request-composite-1".into(),
            skill: requested_composite(),
            tenant_id: "tenant-1".into(),
            actor_id: "actor-1".into(),
            mode: ExecutionMode::Inspect,
            input,
            budget: Budget::default(),
            requested_capabilities: grants.clone(),
            idempotency_key: None,
            trace_id: "trace-composite-1".into(),
        },
        resolved_skill: installed.resolved_ref,
        granted_capabilities: grants,
        policy_decision: PolicyDecision {
            outcome: PolicyDecisionOutcome::Allowed,
            summary: "local policy granted requested capabilities".into(),
            reasons: Vec::new(),
            detail: None,
        },
        parent_execution_id: None,
    }
}

fn request_for(
    installed: &guild_registry::InstalledSkill,
    grants: CapabilityGrantSet,
    input: serde_json::Value,
) -> ResolvedExecutionEnvelope {
    let mut request = composite_request(grants, input);
    request.resolved_skill = installed.resolved_ref.clone();
    request
}

fn invoke_hello_grant(aliases: &[&str]) -> GrantedCapability {
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

fn log_info_grant() -> GrantedCapability {
    GrantedCapability {
        id: CapabilityId::LogWrite,
        access: CapabilityAccess::Write,
        constraints: CapabilityConstraints::Log(LogConstraints {
            levels: Some(vec![Severity::Info]),
        }),
    }
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
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("guild-composition-{unique}"));
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

#[test]
fn composite_skill_invokes_child_and_records_host_owned_metadata() {
    let registry = load_registry();
    let installed = registry.resolve(&requested_composite()).unwrap();
    let runner = build_runner();
    let record = runner
        .execute(
            &registry,
            &installed,
            &request_for(
                &installed,
                CapabilityGrantSet {
                    grants: vec![
                        invoke_hello_grant(&["hello"]),
                        emit_evidence_grant(),
                        log_info_grant(),
                    ],
                },
                serde_json::json!({ "name": "Ada" }),
            ),
        )
        .unwrap();
    let stored_parent = registry
        .load_execution_record(&record.receipt.execution_id)
        .unwrap();
    let stored_child = registry
        .load_execution_record(&record.child_executions[0].execution_id)
        .unwrap();
    let output = record.output.as_ref().unwrap();
    let stored_child_output = stored_child.output.as_ref().unwrap();

    assert_eq!(output.summary, "Hello, Ada. Composite inspect is working.");
    assert_eq!(record.metrics.child_executions, 1);
    assert_eq!(record.child_executions.len(), 1);
    assert_eq!(record.child_executions[0].alias, "hello");
    assert_eq!(record.child_executions[0].trace_id, "trace-composite-1");
    assert_eq!(
        record.child_executions[0].parent_execution_id,
        record.receipt.execution_id
    );
    assert_eq!(record.child_executions[0].uri, stored_child.receipt.uri);
    assert_eq!(
        record.child_executions[0]
            .provenance
            .resolved_skill
            .key
            .name,
        "hello-inspect"
    );
    assert_eq!(output.structured["invoked_alias"], "hello");
    assert_eq!(
        output.structured["child"]["summary"],
        "Hello, Ada. Guild inspect is working."
    );
    assert_eq!(
        output.structured["child"]["structured"]["granted_capabilities"]["grants"],
        serde_json::json!([
            {
                "id": "emit-evidence",
                "access": "write",
                "constraints": {
                    "max_bytes": 65536,
                    "audiences": ["user"],
                    "redactions": ["none"]
                }
            },
            {
                "id": "log-write",
                "access": "write",
                "constraints": { "levels": ["info"] }
            }
        ])
    );
    assert_eq!(record, stored_parent);
    assert_eq!(
        stored_child.parent_execution_id.as_deref(),
        Some(record.receipt.execution_id.as_str())
    );
    assert!(record.provenance.started_at_utc.is_some());
    assert!(record.provenance.finished_at_utc.is_some());
    assert!(stored_child.provenance.started_at_utc.is_some());
    assert!(stored_child.provenance.finished_at_utc.is_some());
    assert_eq!(stored_child_output.evidence.len(), 1);
    assert_eq!(
        stored_child
            .emitted_evidence
            .iter()
            .map(guild_types::EvidenceRecord::evidence_ref)
            .collect::<Vec<_>>(),
        stored_child_output.evidence
    );

    let child_evidence = registry
        .read_resource(&stored_child.emitted_evidence[0].uri)
        .unwrap();
    assert_eq!(child_evidence.mime_type, "application/json");
    let child_payload: serde_json::Value = serde_json::from_slice(&child_evidence.bytes).unwrap();
    assert_eq!(
        child_payload["skill"]["key"]["name"],
        serde_json::Value::String("hello-inspect".into())
    );
}

#[test]
fn composite_fixture_expected_output_matches_real_execution() {
    let input: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(composite_source_dir().join("tests/inspect-input.json")).unwrap(),
    )
    .unwrap();
    let expected_output: guild_types::SkillOutput = serde_json::from_str(
        &fs::read_to_string(composite_source_dir().join("tests/expected-output.json")).unwrap(),
    )
    .unwrap();

    let registry = load_registry();
    let installed = registry.resolve(&requested_composite()).unwrap();
    let child_installed = registry
        .resolve_exact(&installed.manifest.dependencies[0].skill)
        .unwrap();
    let runner = build_runner();
    let record = runner
        .execute(
            &registry,
            &installed,
            &request_for(
                &installed,
                CapabilityGrantSet {
                    grants: vec![invoke_hello_grant(&["hello"]), emit_evidence_grant()],
                },
                input,
            ),
        )
        .unwrap();

    let mut expected_output = expected_output;
    expected_output.structured["skill"]["digest"] =
        serde_json::Value::String(installed.resolved_ref.digest.clone());
    expected_output.structured["child"]["structured"]["skill"]["digest"] =
        serde_json::Value::String(child_installed.resolved_ref.digest.clone());

    assert_eq!(record.output, Some(expected_output));
}

#[test]
fn undeclared_dependency_alias_is_rejected() {
    let registry = load_registry();
    let installed = registry.resolve(&requested_composite()).unwrap();
    let runner = build_runner();
    let error = runner
        .execute(
            &registry,
            &installed,
            &request_for(
                &installed,
                CapabilityGrantSet {
                    grants: vec![invoke_hello_grant(&["hello", "ghost"])],
                },
                serde_json::json!({ "name": "Ada", "child_alias": "ghost" }),
            ),
        )
        .unwrap_err();

    assert_eq!(error.code, "dependency-not-declared");
    let receipt = error.receipt.expect("failed parent execution is persisted");
    let stored = registry
        .load_execution_record(&receipt.execution_id)
        .unwrap();
    assert_eq!(stored.status, ExecutionStatus::Failed);
    assert!(stored.output.is_none());
    assert_eq!(
        stored.termination.as_ref().unwrap().phase,
        guild_types::ExecutionPhase::ChildInvocation
    );
    assert!(stored.child_executions.is_empty());
}

#[test]
fn child_capabilities_must_be_satisfied_by_parent_grants() {
    let temp = TempFixtureDir::new();
    let source_installer = LocalSourceInstaller::new(temp.path()).unwrap();
    let primitive = source_installer.install(primitive_source_dir()).unwrap();
    source_installer.install(composite_source_dir()).unwrap();

    let mut manifest: SkillManifest =
        serde_json::from_str(&fs::read_to_string(&primitive.manifest_path).unwrap()).unwrap();
    manifest.capabilities[1].required = true;
    manifest.capabilities[1].constraints = CapabilityConstraints::Log(LogConstraints {
        levels: Some(vec![Severity::Info]),
    });
    fs::write(
        &primitive.manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();

    let registry = LocalRegistry::load(temp.path()).unwrap();
    let composite_skill = registry.resolve(&requested_composite()).unwrap();
    let runner = build_runner();
    let error = runner
        .execute(
            &registry,
            &composite_skill,
            &request_for(
                &composite_skill,
                CapabilityGrantSet {
                    grants: vec![invoke_hello_grant(&["hello"]), emit_evidence_grant()],
                },
                serde_json::json!({ "name": "Ada" }),
            ),
        )
        .unwrap_err();

    assert_eq!(error.code, "child-capability-mismatch");
}

#[test]
fn unsupported_capability_grants_are_rejected_before_execution() {
    let registry = load_registry();
    let installed = registry.resolve(&requested_composite()).unwrap();
    let runner = build_runner();
    let error = runner
        .execute(
            &registry,
            &installed,
            &request_for(
                &installed,
                CapabilityGrantSet {
                    grants: vec![
                        invoke_hello_grant(&["hello"]),
                        emit_evidence_grant(),
                        GrantedCapability {
                            id: CapabilityId::CacheRead,
                            access: CapabilityAccess::Read,
                            constraints: CapabilityConstraints::none(),
                        },
                    ],
                },
                serde_json::json!({ "name": "Ada" }),
            ),
        )
        .unwrap_err();

    assert_eq!(error.code, "unsupported-runtime-surface");
    let receipt = error
        .receipt
        .expect("unsupported grant rejection is persisted");
    let stored = registry
        .load_execution_record(&receipt.execution_id)
        .unwrap();
    assert_eq!(stored.status, ExecutionStatus::Rejected);
    assert_eq!(
        stored.termination.as_ref().unwrap().phase,
        guild_types::ExecutionPhase::Validation
    );
}

#[test]
fn child_runtime_failures_persist_parent_and_child_execution_records() {
    let temp = TempFixtureDir::new();
    let source_installer = LocalSourceInstaller::new(temp.path()).unwrap();
    source_installer.install(primitive_source_dir()).unwrap();
    source_installer.install(composite_source_dir()).unwrap();

    let registry = LocalRegistry::load(temp.path()).unwrap();
    let composite_skill = registry.resolve(&requested_composite()).unwrap();
    let runner = build_runner();
    let mut request = request_for(
        &composite_skill,
        CapabilityGrantSet {
            grants: vec![invoke_hello_grant(&["hello"]), emit_evidence_grant()],
        },
        serde_json::json!({ "name": "Ada", "child_emit_log": true }),
    );
    request.request.request_id = unique_id("request-composite-child-failed");
    request.request.trace_id = unique_id("trace-composite-child-failed");
    let error = runner
        .execute(&registry, &composite_skill, &request)
        .unwrap_err();

    assert_eq!(error.code, "child-invocation-failed");
    let parent_receipt = error.receipt.expect("failed parent execution is persisted");
    let parent = registry
        .load_execution_record(&parent_receipt.execution_id)
        .unwrap();
    assert_eq!(parent.status, ExecutionStatus::Failed);
    assert!(parent.output.is_none());
    assert_eq!(parent.child_executions.len(), 1);
    assert_eq!(
        parent.termination.as_ref().unwrap().phase,
        guild_types::ExecutionPhase::ChildInvocation
    );

    let child = registry
        .load_execution_record(&parent.child_executions[0].execution_id)
        .unwrap();
    assert_eq!(child.status, ExecutionStatus::Rejected);
    assert!(child.output.is_none());
    assert_eq!(
        child.termination.as_ref().unwrap().phase,
        guild_types::ExecutionPhase::Grant
    );
    assert_eq!(
        parent.child_executions[0]
            .termination
            .as_ref()
            .unwrap()
            .code,
        "log-write-not-granted"
    );
    assert_eq!(parent.child_executions[0].uri, child.receipt.uri);
}

#[test]
fn child_execution_budget_is_decremented_and_exhaustion_fails_closed() {
    let registry = load_registry();
    let installed = registry.resolve(&requested_composite()).unwrap();
    let runner = build_runner();
    let mut request = composite_request(
        CapabilityGrantSet {
            grants: vec![invoke_hello_grant(&["hello"]), emit_evidence_grant()],
        },
        serde_json::json!({ "name": "Ada" }),
    );
    request.resolved_skill = installed.resolved_ref.clone();
    request.request.budget.max_child_executions = 0;

    let error = runner.execute(&registry, &installed, &request).unwrap_err();
    assert_eq!(error.code, "child-budget-exhausted");
}
