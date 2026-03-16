use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use guild_registry::{execution_resource_uri, LocalRegistry, LocalSourceInstaller, SkillRegistry};
use guild_runner::{Runner, WasmtimeRuntimeAdapter};
use guild_types::{
    CallerRequest, CapabilityAccess, CapabilityConstraints, CapabilityGrantSet, CapabilityId,
    CapabilityRequirement, EmitEvidenceConstraints, EvidenceAudience, EvidenceRef, ExecutionMode,
    ExecutionStatus, GrantedCapability, LogConstraints, PolicyDecision, PolicyDecisionOutcome,
    RedactionClass, RequestedSkillRef, ResolvedExecutionEnvelope, Severity, SkillKey,
    VersionRequirement,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

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
            summary: "test request allowed".into(),
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
    let digest_hex = format!("{:x}", Sha256::digest(&payload));

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

#[test]
fn wit_stays_aligned_with_skill_visible_execution_context() {
    let wit = include_str!("../../../wit/guild-skill-v1.wit");

    for needle in [
        "record skill-output",
        "record execution-context",
        "record evidence-emission-request",
        "record resource-read-result",
        "variant capability-constraints",
        "enum resource-kind",
        "skill: resolved-skill-ref",
        "granted-capabilities: list<granted-capability>",
        "emit-evidence,",
        "emit-evidence: func(request: evidence-emission-request) -> result<evidence-ref, string>",
        "read-resource: func(uri: string) -> result<resource-read-result, string>",
        "log: func(level: severity, message: string);",
        "invoke-dependency: func(request: dependency-invocation-request) -> result<skill-output, skill-error>",
        "run: func(ctx: execution-context, input: json) -> result<skill-output, skill-error>",
    ] {
        assert!(wit.contains(needle), "missing `{needle}` from WIT contract");
    }
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
            .map(|evidence| evidence.evidence_ref())
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

    let installer = LocalSourceInstaller::new(&registry_root).unwrap();
    let installed = installer.install(&source_root).unwrap();
    let registry = LocalRegistry::load(&registry_root).unwrap();
    let runner = build_runner();

    let error = runner
        .execute(
            &registry,
            &installed,
            &envelope_for(
                &installed,
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
    assert_eq!(error.code, "apply-disabled");
    let receipt = error.receipt.expect("rejected execution is persisted");
    let stored = registry
        .load_execution_record(&receipt.execution_id)
        .unwrap();
    assert_eq!(stored.status, ExecutionStatus::Rejected);
    assert_eq!(
        stored.termination.as_ref().unwrap().phase,
        guild_types::ExecutionPhase::Mode
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
    let receipt = error
        .receipt
        .expect("unsupported manifest capability rejection is persisted");
    let stored = registry
        .load_execution_record(&receipt.execution_id)
        .unwrap();
    assert_eq!(stored.status, ExecutionStatus::Rejected);
}
