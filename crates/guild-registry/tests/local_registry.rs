use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use guild_manifest::PublisherRef;
use guild_registry::{
    BundleSignatureEnvelope, ExecutionPlanSignatureEnvelope, InstalledSkill, InstalledSkillBundle,
    LocalPublisherIdentity, LocalRegistry, LocalSourceInstaller, OciRegistryAuth,
    OciRegistryReference, OciRegistryTarget, OciRegistryTransportOptions, SkillRegistry,
    VerificationStatus, execution_query_resource_uri, execution_resource_uri, sign_execution_plan,
    verify_execution_plan,
};
use guild_types::{
    AbiVersion, CapabilityGrantSet, EvidenceAudience, EvidenceEmissionRequest, EvidenceRecord,
    ExecutionPhase, ExecutionQueryResource, ExecutionQueryResult, ExecutionReceipt,
    ExecutionRecord, ExecutionStatus, InstalledVerificationState, LocalPolicyConfig,
    LocalTrustTier, PolicyDecision, PolicyDecisionOutcome, PolicyReason, Provenance,
    RedactionClass, RequestedSkillRef, ResolvedSkillRef, SkillKey, SkillVersion, TerminationDetail,
    VersionRequirement,
};
use sha2::{Digest as _, Sha256};

#[path = "../../../test-support/oci_registry_test_server.rs"]
mod oci_registry_test_server;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn example_source_dir() -> PathBuf {
    repo_root()
        .join("examples/skills/hello-inspect")
        .canonicalize()
        .unwrap()
}

fn composite_source_dir() -> PathBuf {
    repo_root()
        .join("examples/skills/hello-composite")
        .canonicalize()
        .unwrap()
}

fn draft_plan_path(name: &str) -> PathBuf {
    repo_root()
        .join("docs/schemas/draft-v1/examples")
        .join(name)
}

fn execution_plan_json(name: &str) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(draft_plan_path(name)).unwrap()).unwrap()
}

fn bundle_json(bundle_root: &Path) -> InstalledSkillBundle {
    serde_json::from_str(&fs::read_to_string(bundle_root.join("bundle.json")).unwrap()).unwrap()
}

fn bundle_signature(bundle_root: &Path) -> BundleSignatureEnvelope {
    serde_json::from_str(&fs::read_to_string(bundle_root.join("bundle.signature.json")).unwrap())
        .unwrap()
}

fn oci_blob_path(layout_root: &Path, digest: &str) -> PathBuf {
    layout_root
        .join("blobs/sha256")
        .join(digest.strip_prefix("sha256:").unwrap())
}

fn oci_index(layout_root: &Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(layout_root.join("index.json")).unwrap()).unwrap()
}

fn oci_root_manifest(layout_root: &Path) -> serde_json::Value {
    let index = oci_index(layout_root);
    let digest = index["manifests"][0]["digest"].as_str().unwrap();
    serde_json::from_str(&fs::read_to_string(oci_blob_path(layout_root, digest)).unwrap()).unwrap()
}

fn oci_bundle_json(layout_root: &Path) -> InstalledSkillBundle {
    let manifest = oci_root_manifest(layout_root);
    let digest = manifest["config"]["digest"].as_str().unwrap();
    serde_json::from_str(&fs::read_to_string(oci_blob_path(layout_root, digest)).unwrap()).unwrap()
}

fn registry_reference(
    server: &oci_registry_test_server::OciRegistryTestServer,
    repository: &str,
    tag: &str,
) -> OciRegistryReference {
    OciRegistryReference {
        registry: server.registry(),
        repository: repository.into(),
        target: OciRegistryTarget::Tag(tag.into()),
    }
}

fn registry_options() -> OciRegistryTransportOptions {
    OciRegistryTransportOptions {
        auth: OciRegistryAuth::Anonymous,
        allow_http: true,
    }
}

fn json_value(bytes: &[u8]) -> serde_json::Value {
    serde_json::from_slice(bytes).unwrap()
}

fn sha256_digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn publisher_identity(installed: &InstalledSkill, path: &Path) -> LocalPublisherIdentity {
    let identity = LocalPublisherIdentity::generate(installed.manifest.publisher.clone()).unwrap();
    identity.save(path).unwrap();
    LocalPublisherIdentity::load(path).unwrap()
}

fn publisher_identity_for_ref(publisher: PublisherRef, path: &Path) -> LocalPublisherIdentity {
    let identity = LocalPublisherIdentity::generate(publisher).unwrap();
    identity.save(path).unwrap();
    LocalPublisherIdentity::load(path).unwrap()
}

fn prepared_registry_root() -> &'static PathBuf {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();

    ROOT.get_or_init(|| {
        let root = repo_root().join("target/test-install-registry/local-registry");
        if root.exists() {
            let _ = fs::remove_dir_all(&root);
        }

        LocalSourceInstaller::new(&root)
            .unwrap()
            .install(example_source_dir())
            .unwrap();

        root
    })
}

fn requested_hello_inspect() -> RequestedSkillRef {
    RequestedSkillRef {
        key: SkillKey {
            namespace: "example".into(),
            name: "hello-inspect".into(),
        },
        version_req: VersionRequirement::parse("^0.1").unwrap(),
    }
}

fn requested_hello_composite() -> RequestedSkillRef {
    RequestedSkillRef {
        key: SkillKey {
            namespace: "example".into(),
            name: "hello-composite".into(),
        },
        version_req: VersionRequirement::parse("^0.1").unwrap(),
    }
}

fn query_result_from_resource(
    registry: &LocalRegistry,
    query: &ExecutionQueryResource,
) -> ExecutionQueryResult {
    serde_json::from_slice(
        &registry
            .read_resource(&execution_query_resource_uri(query))
            .unwrap()
            .bytes,
    )
    .unwrap()
}

fn sample_resolved_skill(name: &str, digest_seed: &str) -> ResolvedSkillRef {
    let digest = format!("sha256:{:x}", Sha256::digest(digest_seed.as_bytes()));
    ResolvedSkillRef {
        key: SkillKey {
            namespace: "example".into(),
            name: name.into(),
        },
        version: SkillVersion::parse("0.1.0").unwrap(),
        digest,
    }
}

fn sample_evidence_record(evidence_record_id: &str, execution_id: &str) -> EvidenceRecord {
    let digest_hex = format!("{:x}", Sha256::digest(evidence_record_id.as_bytes()));
    EvidenceRecord {
        uri: format!("guild://objects/records/{evidence_record_id}"),
        blob_uri: format!("guild://objects/sha256/{digest_hex}"),
        mime_type: "application/json".into(),
        sha256: format!("sha256:{digest_hex}"),
        size_bytes: 32,
        title: Some("sample evidence".into()),
        audience: EvidenceAudience::Internal,
        redaction: RedactionClass::None,
        freshness: Some("deterministic".into()),
        produced_by_execution: Some(execution_id.into()),
    }
}

#[allow(clippy::too_many_arguments)]
fn sample_execution_record(
    execution_id: &str,
    skill_name: &str,
    status: ExecutionStatus,
    started_at_utc: &str,
    finished_at_utc: &str,
    policy_outcome: PolicyDecisionOutcome,
    policy_reason_code: Option<&str>,
    termination: Option<(&str, ExecutionPhase)>,
    evidence_count: usize,
) -> ExecutionRecord {
    let resolved_skill = sample_resolved_skill(skill_name, execution_id);
    let mut emitted_evidence = Vec::new();
    for index in 0..evidence_count {
        emitted_evidence.push(sample_evidence_record(
            &format!("{execution_id}-evidence-{index}"),
            execution_id,
        ));
    }

    ExecutionRecord {
        receipt: ExecutionReceipt {
            execution_id: execution_id.into(),
            uri: execution_resource_uri(execution_id),
            trace_id: format!("trace-{execution_id}"),
            status: status.clone(),
        },
        request: guild_types::CallerRequest {
            request_id: format!("request-{execution_id}"),
            skill: RequestedSkillRef {
                key: resolved_skill.key.clone(),
                version_req: VersionRequirement::parse("^0.1").unwrap(),
            },
            tenant_id: "tenant-test".into(),
            actor_id: "actor-test".into(),
            mode: guild_types::ExecutionMode::Inspect,
            input: serde_json::json!({ "execution_id": execution_id }),
            budget: guild_types::Budget::default(),
            requested_capabilities: CapabilityGrantSet::default(),
            idempotency_key: None,
            trace_id: format!("trace-{execution_id}"),
        },
        policy_decision: PolicyDecision {
            outcome: policy_outcome,
            summary: format!("policy summary for {execution_id}"),
            profile_name: "default".into(),
            trust_tier: LocalTrustTier::LocalDev,
            verification_state: InstalledVerificationState::LocalSource,
            reasons: policy_reason_code
                .map(|code| {
                    vec![PolicyReason {
                        code: code.into(),
                        message: format!("reason {code}"),
                        detail: None,
                    }]
                })
                .unwrap_or_default(),
            detail: None,
        },
        resolved_skill: resolved_skill.clone(),
        parent_execution_id: None,
        status,
        output: None,
        termination: termination.map(|(code, phase)| TerminationDetail {
            phase,
            code: code.into(),
            message: format!("termination {code}"),
            retryable: false,
            detail: None,
        }),
        granted_capabilities: CapabilityGrantSet::default(),
        emitted_evidence,
        metrics: guild_types::ExecutionMetrics::default(),
        provenance: Provenance {
            resolved_skill,
            abi: AbiVersion::GuildSkillV1,
            dependency_digests: Vec::new(),
            started_at_utc: Some(started_at_utc.into()),
            finished_at_utc: Some(finished_at_utc.into()),
        },
        child_executions: Vec::new(),
    }
}

#[test]
fn load_policy_config_defaults_when_policy_file_is_missing() {
    let temp = TempFixtureDir::new();
    let registry = LocalRegistry::load(temp.path()).unwrap();

    let policy = registry.load_policy_config().unwrap();

    assert_eq!(policy, LocalPolicyConfig::default());
}

#[test]
fn load_policy_config_fails_closed_when_policy_file_is_invalid() {
    let temp = TempFixtureDir::new();
    fs::write(temp.path().join("policy.json"), "{ not valid json").unwrap();

    let registry = LocalRegistry::load(temp.path()).unwrap();
    let error = registry.load_policy_config().unwrap_err();

    assert_eq!(error.code, "policy-parse-failed");
}

#[test]
fn execution_query_happy_path_is_bounded_and_deterministic() {
    let temp = TempFixtureDir::new();
    let registry = LocalRegistry::load(temp.path()).unwrap();
    registry
        .persist_execution_record(&sample_execution_record(
            "exec-001",
            "inspect-http-json",
            ExecutionStatus::Succeeded,
            "2026-03-17T00:00:01Z",
            "2026-03-17T00:00:02Z",
            PolicyDecisionOutcome::Allowed,
            None,
            None,
            1,
        ))
        .unwrap();
    registry
        .persist_execution_record(&sample_execution_record(
            "exec-002",
            "inspect-http-json",
            ExecutionStatus::Failed,
            "2026-03-17T00:00:03Z",
            "2026-03-17T00:00:04Z",
            PolicyDecisionOutcome::Allowed,
            Some("runtime-failed"),
            Some(("invalid-method", ExecutionPhase::RuntimeExec)),
            0,
        ))
        .unwrap();
    registry
        .persist_execution_record(&sample_execution_record(
            "exec-003",
            "summarize-execution-query",
            ExecutionStatus::Rejected,
            "2026-03-17T00:00:05Z",
            "2026-03-17T00:00:06Z",
            PolicyDecisionOutcome::Rejected,
            Some("policy-denied"),
            Some(("required-capability-missing", ExecutionPhase::Grant)),
            0,
        ))
        .unwrap();

    let recent = registry
        .query_execution_records(&ExecutionQueryResource::Recent { limit: 2 })
        .unwrap();
    assert_eq!(recent.total_matches, 3);
    assert_eq!(recent.returned_matches, 2);
    assert!(recent.truncated);
    assert_eq!(recent.results[0].receipt.execution_id, "exec-003");
    assert_eq!(recent.results[1].receipt.execution_id, "exec-002");

    let failures = registry
        .query_execution_records(&ExecutionQueryResource::FailuresRecent { limit: 10 })
        .unwrap();
    assert_eq!(failures.total_matches, 2);
    assert_eq!(failures.returned_matches, 2);
    assert_eq!(failures.results[0].status, ExecutionStatus::Rejected);
    assert_eq!(failures.results[1].status, ExecutionStatus::Failed);
    assert_eq!(
        failures.results[1].termination.as_ref().unwrap().code,
        "invalid-method"
    );

    let by_skill = registry
        .query_execution_records(&ExecutionQueryResource::BySkill {
            namespace: "example".into(),
            name: "inspect-http-json".into(),
            limit: 10,
        })
        .unwrap();
    assert_eq!(by_skill.total_matches, 2);
    assert_eq!(by_skill.results[0].receipt.execution_id, "exec-002");
    assert_eq!(by_skill.results[1].receipt.execution_id, "exec-001");
    assert_eq!(by_skill.results[1].evidence_count, 1);
    assert_eq!(by_skill.results[1].sample_evidence_record_uris.len(), 1);
}

#[test]
fn execution_query_orders_fractional_rfc3339_timestamps_chronologically() {
    let temp = TempFixtureDir::new();
    let registry = LocalRegistry::load(temp.path()).unwrap();
    registry
        .persist_execution_record(&sample_execution_record(
            "exec-plain-second",
            "inspect-http-json",
            ExecutionStatus::Succeeded,
            "2026-03-17T00:00:05Z",
            "2026-03-17T00:00:06Z",
            PolicyDecisionOutcome::Allowed,
            None,
            None,
            0,
        ))
        .unwrap();
    registry
        .persist_execution_record(&sample_execution_record(
            "exec-fractional-second",
            "inspect-http-json",
            ExecutionStatus::Succeeded,
            "2026-03-17T00:00:05.1Z",
            "2026-03-17T00:00:06.1Z",
            PolicyDecisionOutcome::Allowed,
            None,
            None,
            0,
        ))
        .unwrap();

    let recent = registry
        .query_execution_records(&ExecutionQueryResource::Recent { limit: 2 })
        .unwrap();

    assert_eq!(recent.results.len(), 2);
    assert_eq!(
        recent.results[0].receipt.execution_id,
        "exec-fractional-second"
    );
    assert_eq!(recent.results[1].receipt.execution_id, "exec-plain-second");
}

#[test]
fn execution_query_resource_reads_share_the_same_backend_result() {
    let temp = TempFixtureDir::new();
    let registry = LocalRegistry::load(temp.path()).unwrap();
    registry
        .persist_execution_record(&sample_execution_record(
            "exec-backend-1",
            "inspect-http-json",
            ExecutionStatus::Rejected,
            "2026-03-17T00:10:00Z",
            "2026-03-17T00:10:01Z",
            PolicyDecisionOutcome::Rejected,
            Some("policy-denied"),
            Some(("required-capability-missing", ExecutionPhase::Grant)),
            0,
        ))
        .unwrap();

    let query = ExecutionQueryResource::ByStatus {
        status: ExecutionStatus::Rejected,
        limit: 10,
    };
    let direct = registry.query_execution_records(&query).unwrap();
    let via_resource = query_result_from_resource(&registry, &query);

    assert_eq!(via_resource, direct);
    assert_eq!(
        via_resource.results[0].policy_decision.reasons[0].code,
        "policy-denied"
    );
}

#[test]
fn malformed_execution_query_resources_fail_closed() {
    let temp = TempFixtureDir::new();
    let registry = LocalRegistry::load(temp.path()).unwrap();

    let invalid_limit = registry
        .read_resource("guild://queries/executions/recent/999")
        .unwrap_err();
    assert_eq!(invalid_limit.code, "resource-uri-invalid");

    let invalid_status = registry
        .read_resource("guild://queries/executions/by-status/not-a-status/5")
        .unwrap_err();
    assert_eq!(invalid_status.code, "resource-uri-invalid");
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
        let path = std::env::temp_dir().join(format!("guild-local-registry-{unique}"));
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
fn primitive_bundle_export_contains_expected_installed_record() {
    let temp = TempFixtureDir::new();
    let registry_root = temp.path().join("registry-a");
    let bundle_root = temp.path().join("bundle");
    let source_installer = LocalSourceInstaller::new(&registry_root).unwrap();
    let installed_skill = source_installer.install(example_source_dir()).unwrap();
    let identity = publisher_identity(&installed_skill, &temp.path().join("publisher.json"));
    let registry = LocalRegistry::load(&registry_root).unwrap();

    let bundle = registry
        .export_bundle(
            &installed_skill.resolved_ref,
            false,
            &bundle_root,
            &identity,
        )
        .unwrap();
    let stored_bundle = bundle_json(&bundle_root);

    assert_eq!(bundle, stored_bundle);
    assert_eq!(bundle.format_version, "guild-installed-bundle-v2");
    assert_eq!(bundle.root_skill, installed_skill.resolved_ref);
    assert!(!bundle.includes_dependency_closure);
    assert_eq!(bundle.publisher, installed_skill.manifest.publisher);
    assert_eq!(bundle.skills.len(), 1);
    assert_eq!(bundle.skills[0].resolved_ref, bundle.root_skill);
    assert!(bundle_root.join(&bundle.skills[0].install_dir).exists());
    assert!(
        bundle_root
            .join(&bundle.skills[0].install_dir)
            .join("component.wasm")
            .exists()
    );
    assert!(
        bundle_root
            .join(&bundle.skills[0].install_dir)
            .join("input.schema.json")
            .exists()
    );
    assert!(bundle_root.join("bundle.signature.json").exists());
    assert!(
        bundle
            .files
            .iter()
            .any(|entry| entry.path.ends_with("/component.wasm"))
    );
}

#[test]
fn primitive_oci_export_contains_expected_installed_record() {
    let temp = TempFixtureDir::new();
    let registry_root = temp.path().join("registry-a");
    let layout_root = temp.path().join("oci-layout");
    let source_installer = LocalSourceInstaller::new(&registry_root).unwrap();
    let installed_skill = source_installer.install(example_source_dir()).unwrap();
    let identity = publisher_identity(&installed_skill, &temp.path().join("publisher.json"));
    let registry = LocalRegistry::load(&registry_root).unwrap();

    let bundle = registry
        .export_oci_layout(
            &installed_skill.resolved_ref,
            false,
            &layout_root,
            &identity,
        )
        .unwrap();
    let stored_bundle = oci_bundle_json(&layout_root);
    let index = oci_index(&layout_root);
    let manifest = oci_root_manifest(&layout_root);

    assert_eq!(bundle, stored_bundle);
    assert_eq!(
        fs::read_to_string(layout_root.join("oci-layout")).unwrap(),
        "{\n  \"imageLayoutVersion\": \"1.0.0\"\n}"
    );
    assert_eq!(
        index["manifests"][0]["annotations"]["org.opencontainers.image.title"]
            .as_str()
            .unwrap(),
        "example/hello-inspect:0.1.0"
    );
    assert_eq!(
        index["manifests"][0]["annotations"]["dev.guild.root-skill.digest"]
            .as_str()
            .unwrap(),
        installed_skill.resolved_ref.digest
    );
    assert_eq!(
        manifest["artifactType"].as_str().unwrap(),
        "application/vnd.guild.installed-bundle.oci.v1"
    );
    assert_eq!(
        manifest["config"]["mediaType"].as_str().unwrap(),
        "application/vnd.guild.installed-bundle.v2+json"
    );
    assert_eq!(
        manifest["layers"][0]["mediaType"].as_str().unwrap(),
        "application/vnd.guild.installed-bundle.signature.v1+json"
    );
    assert!(manifest["layers"].as_array().unwrap().iter().any(|layer| {
        layer["annotations"]["org.opencontainers.image.title"]
            .as_str()
            .is_some_and(|title| title.ends_with("/component.wasm"))
    }));
}

#[test]
fn primitive_oci_registry_push_contains_expected_root_metadata() {
    let temp = TempFixtureDir::new();
    let registry_root = temp.path().join("registry-a");
    let registry_store = temp.path().join("oci-registry-store");
    let source_installer = LocalSourceInstaller::new(&registry_root).unwrap();
    let installed_skill = source_installer.install(example_source_dir()).unwrap();
    let identity = publisher_identity(&installed_skill, &temp.path().join("publisher.json"));
    let registry = LocalRegistry::load(&registry_root).unwrap();
    let server = oci_registry_test_server::OciRegistryTestServer::start(&registry_store);
    let reference = registry_reference(&server, "guild-example-hello-inspect", "0.1.0");

    let published = registry
        .push_oci_registry(
            &installed_skill.resolved_ref,
            false,
            &reference,
            &registry_options(),
            &identity,
        )
        .unwrap();

    let index = json_value(&server.manifest_bytes_for_tag("guild-example-hello-inspect", "0.1.0"));
    let manifest_digest = index["manifests"][0]["digest"].as_str().unwrap();
    let manifest = json_value(
        &fs::read(server.digest_manifest_path("guild-example-hello-inspect", manifest_digest))
            .unwrap(),
    );

    assert_eq!(published.reference, reference);
    assert_eq!(published.bundle.root_skill, installed_skill.resolved_ref);
    assert_eq!(
        published.manifest_digest,
        sha256_digest(&server.manifest_bytes_for_tag("guild-example-hello-inspect", "0.1.0"))
    );
    assert_eq!(
        index["manifests"][0]["annotations"]["org.opencontainers.image.title"]
            .as_str()
            .unwrap(),
        "example/hello-inspect:0.1.0"
    );
    assert_eq!(
        index["manifests"][0]["annotations"]["dev.guild.root-skill.digest"]
            .as_str()
            .unwrap(),
        installed_skill.resolved_ref.digest
    );
    assert_eq!(
        manifest["artifactType"].as_str().unwrap(),
        "application/vnd.guild.installed-bundle.oci.v1"
    );
    assert_eq!(
        manifest["config"]["mediaType"].as_str().unwrap(),
        "application/vnd.guild.installed-bundle.v2+json"
    );
}

#[test]
fn signed_bundle_export_verifies_against_local_publisher_identity() {
    let temp = TempFixtureDir::new();
    let registry_root = temp.path().join("registry-a");
    let bundle_root = temp.path().join("bundle");
    let source_installer = LocalSourceInstaller::new(&registry_root).unwrap();
    let installed_skill = source_installer.install(example_source_dir()).unwrap();
    let identity = publisher_identity(&installed_skill, &temp.path().join("publisher.json"));
    let registry = LocalRegistry::load(&registry_root).unwrap();

    registry
        .export_bundle(
            &installed_skill.resolved_ref,
            false,
            &bundle_root,
            &identity,
        )
        .unwrap();

    let bundle_bytes = fs::read(bundle_root.join("bundle.json")).unwrap();
    let envelope = bundle_signature(&bundle_root);
    let public_key: [u8; 32] = base64::engine::general_purpose::STANDARD
        .decode(&identity.public_key_base64)
        .unwrap()
        .try_into()
        .unwrap();
    let signature_bytes: [u8; 64] = base64::engine::general_purpose::STANDARD
        .decode(&envelope.signature_base64)
        .unwrap()
        .try_into()
        .unwrap();
    let verifying_key = VerifyingKey::from_bytes(&public_key).unwrap();
    let signature = Signature::from_bytes(&signature_bytes);
    verifying_key.verify(&bundle_bytes, &signature).unwrap();
    assert_eq!(envelope.publisher_id, identity.publisher.id);
}

#[test]
fn execution_plans_can_be_signed_and_verified_against_trusted_publishers() {
    let temp = TempFixtureDir::new();
    let registry_root = temp.path().join("registry");
    let signer = publisher_identity_for_ref(
        PublisherRef {
            id: "local.example".into(),
            display_name: "Local Example".into(),
            homepage: None,
        },
        &temp.path().join("plan-signer.json"),
    );
    let plan = execution_plan_json("zero-authority.admit.plan.json");

    LocalRegistry::trust_publisher(&registry_root, &signer.trusted_record()).unwrap();
    let signed_plan = sign_execution_plan(&plan, &signer).unwrap();
    let signature: ExecutionPlanSignatureEnvelope =
        serde_json::from_value(signed_plan["plan_signature"].clone()).unwrap();
    let verification = verify_execution_plan(&registry_root, &signed_plan).unwrap();

    assert_eq!(signature.publisher_id, signer.publisher.id);
    assert_eq!(verification.publisher.id, signer.publisher.id);
    assert_eq!(verification.trust_tier, LocalTrustTier::TrustedImported);
    assert_eq!(verification.signed_digest.algorithm, "sha256");
}

#[test]
fn execution_plan_verification_fails_when_plan_is_tampered_after_signing() {
    let temp = TempFixtureDir::new();
    let registry_root = temp.path().join("registry");
    let signer = publisher_identity_for_ref(
        PublisherRef {
            id: "local.example".into(),
            display_name: "Local Example".into(),
            homepage: None,
        },
        &temp.path().join("plan-signer.json"),
    );
    let plan = execution_plan_json("zero-authority.admit.plan.json");

    LocalRegistry::trust_publisher(&registry_root, &signer.trusted_record()).unwrap();
    let mut signed_plan = sign_execution_plan(&plan, &signer).unwrap();
    signed_plan["decision"] = serde_json::Value::String("downgrade".into());

    let error = verify_execution_plan(&registry_root, &signed_plan).unwrap_err();
    assert_eq!(error.code, "execution-plan-signature-digest-mismatch");
}

#[test]
fn execution_plan_verification_fails_with_wrong_trusted_key_for_same_publisher() {
    let temp = TempFixtureDir::new();
    let registry_root = temp.path().join("registry");
    let publisher = PublisherRef {
        id: "local.example".into(),
        display_name: "Local Example".into(),
        homepage: None,
    };
    let signer =
        publisher_identity_for_ref(publisher.clone(), &temp.path().join("plan-signer.json"));
    let wrong_trusted_identity =
        publisher_identity_for_ref(publisher, &temp.path().join("wrong-plan-signer.json"));
    let plan = execution_plan_json("zero-authority.admit.plan.json");

    LocalRegistry::trust_publisher(&registry_root, &wrong_trusted_identity.trusted_record())
        .unwrap();
    let signed_plan = sign_execution_plan(&plan, &signer).unwrap();

    let error = verify_execution_plan(&registry_root, &signed_plan).unwrap_err();
    assert_eq!(error.code, "execution-plan-signature-invalid");
}

#[test]
fn primitive_oci_import_resolves_digest_pinned_skill_in_fresh_registry() {
    let temp = TempFixtureDir::new();
    let registry_a = temp.path().join("registry-a");
    let layout_root = temp.path().join("oci-layout");
    let registry_b = temp.path().join("registry-b");
    let source_installer = LocalSourceInstaller::new(&registry_a).unwrap();
    let installed_skill = source_installer.install(example_source_dir()).unwrap();
    let identity = publisher_identity(&installed_skill, &temp.path().join("publisher.json"));
    let registry = LocalRegistry::load(&registry_a).unwrap();

    registry
        .export_oci_layout(
            &installed_skill.resolved_ref,
            false,
            &layout_root,
            &identity,
        )
        .unwrap();
    LocalRegistry::trust_publisher(&registry_b, &identity.trusted_record()).unwrap();
    LocalRegistry::import_oci_layout(&registry_b, &layout_root).unwrap();

    let imported = LocalRegistry::load(&registry_b)
        .unwrap()
        .resolve(&requested_hello_inspect())
        .unwrap();
    assert_eq!(imported.resolved_ref, installed_skill.resolved_ref);
    assert_eq!(
        imported.manifest.package.artifact_digest,
        installed_skill.manifest.package.artifact_digest
    );
    assert!(imported.artifact_path.exists());
    let verification = imported
        .verification
        .expect("imported skills carry verification metadata");
    assert_eq!(verification.status, VerificationStatus::Verified);
    assert_eq!(verification.publisher.id, identity.publisher.id);
}

#[test]
fn primitive_oci_registry_pull_resolves_digest_pinned_skill_in_fresh_registry() {
    let temp = TempFixtureDir::new();
    let registry_a = temp.path().join("registry-a");
    let registry_store = temp.path().join("oci-registry-store");
    let registry_b = temp.path().join("registry-b");
    let source_installer = LocalSourceInstaller::new(&registry_a).unwrap();
    let installed_skill = source_installer.install(example_source_dir()).unwrap();
    let identity = publisher_identity(&installed_skill, &temp.path().join("publisher.json"));
    let registry = LocalRegistry::load(&registry_a).unwrap();
    let server = oci_registry_test_server::OciRegistryTestServer::start(&registry_store);
    let reference = registry_reference(&server, "guild-example-hello-inspect", "0.1.0");

    registry
        .push_oci_registry(
            &installed_skill.resolved_ref,
            false,
            &reference,
            &registry_options(),
            &identity,
        )
        .unwrap();
    LocalRegistry::trust_publisher(&registry_b, &identity.trusted_record()).unwrap();
    LocalRegistry::pull_oci_registry(&registry_b, &reference, &registry_options()).unwrap();

    let imported = LocalRegistry::load(&registry_b)
        .unwrap()
        .resolve(&requested_hello_inspect())
        .unwrap();
    assert_eq!(imported.resolved_ref, installed_skill.resolved_ref);
    assert_eq!(
        imported.manifest.package.artifact_digest,
        installed_skill.manifest.package.artifact_digest
    );
    assert!(imported.artifact_path.exists());
    let verification = imported
        .verification
        .expect("imported skills carry verification metadata");
    assert_eq!(verification.status, VerificationStatus::Verified);
    assert_eq!(verification.publisher.id, identity.publisher.id);
}

#[test]
fn primitive_bundle_import_resolves_digest_pinned_skill_in_fresh_registry() {
    let temp = TempFixtureDir::new();
    let registry_a = temp.path().join("registry-a");
    let bundle_root = temp.path().join("bundle");
    let registry_b = temp.path().join("registry-b");
    let source_installer = LocalSourceInstaller::new(&registry_a).unwrap();
    let installed_skill = source_installer.install(example_source_dir()).unwrap();
    let identity = publisher_identity(&installed_skill, &temp.path().join("publisher.json"));
    let registry = LocalRegistry::load(&registry_a).unwrap();

    registry
        .export_bundle(
            &installed_skill.resolved_ref,
            false,
            &bundle_root,
            &identity,
        )
        .unwrap();
    LocalRegistry::trust_publisher(&registry_b, &identity.trusted_record()).unwrap();
    LocalRegistry::import_bundle(&registry_b, &bundle_root).unwrap();

    let imported = LocalRegistry::load(&registry_b)
        .unwrap()
        .resolve(&requested_hello_inspect())
        .unwrap();
    assert_eq!(imported.resolved_ref, installed_skill.resolved_ref);
    assert_eq!(
        imported.manifest.package.artifact_digest,
        installed_skill.manifest.package.artifact_digest
    );
    assert!(imported.artifact_path.exists());
    let verification = imported
        .verification
        .expect("imported skills carry verification metadata");
    assert_eq!(verification.status, VerificationStatus::Verified);
    assert_eq!(verification.publisher.id, identity.publisher.id);
}

#[test]
fn composite_bundle_export_includes_dependency_closure() {
    let temp = TempFixtureDir::new();
    let registry_root = temp.path().join("registry-a");
    let bundle_root = temp.path().join("bundle");
    let installer = LocalSourceInstaller::new(&registry_root).unwrap();
    let primitive = installer.install(example_source_dir()).unwrap();
    let composite = installer.install(composite_source_dir()).unwrap();
    let identity = publisher_identity(&composite, &temp.path().join("publisher.json"));
    let registry = LocalRegistry::load(&registry_root).unwrap();

    let bundle = registry
        .export_bundle(&composite.resolved_ref, true, &bundle_root, &identity)
        .unwrap();

    assert!(bundle.includes_dependency_closure);
    assert_eq!(bundle.root_skill, composite.resolved_ref);
    assert_eq!(bundle.publisher, composite.manifest.publisher);
    assert_eq!(bundle.skills.len(), 2);
    assert!(
        bundle
            .skills
            .iter()
            .any(|entry| entry.resolved_ref == composite.resolved_ref)
    );
    assert!(
        bundle
            .skills
            .iter()
            .any(|entry| entry.resolved_ref == primitive.resolved_ref)
    );
    assert_eq!(composite.manifest.dependencies[0].alias, "hello");
    assert_eq!(
        composite.manifest.dependencies[0].skill,
        primitive.resolved_ref
    );
    assert!(
        bundle
            .files
            .iter()
            .any(|entry| entry.path.ends_with("/component.wasm"))
    );
}

#[test]
fn composite_oci_export_includes_dependency_closure() {
    let temp = TempFixtureDir::new();
    let registry_root = temp.path().join("registry-a");
    let layout_root = temp.path().join("oci-layout");
    let installer = LocalSourceInstaller::new(&registry_root).unwrap();
    let primitive = installer.install(example_source_dir()).unwrap();
    let composite = installer.install(composite_source_dir()).unwrap();
    let identity = publisher_identity(&composite, &temp.path().join("publisher.json"));
    let registry = LocalRegistry::load(&registry_root).unwrap();

    let bundle = registry
        .export_oci_layout(&composite.resolved_ref, true, &layout_root, &identity)
        .unwrap();
    let manifest = oci_root_manifest(&layout_root);

    assert!(bundle.includes_dependency_closure);
    assert_eq!(bundle.root_skill, composite.resolved_ref);
    assert_eq!(bundle.publisher, composite.manifest.publisher);
    assert_eq!(bundle.skills.len(), 2);
    assert!(
        bundle
            .skills
            .iter()
            .any(|entry| entry.resolved_ref == composite.resolved_ref)
    );
    assert!(
        bundle
            .skills
            .iter()
            .any(|entry| entry.resolved_ref == primitive.resolved_ref)
    );
    assert_eq!(composite.manifest.dependencies[0].alias, "hello");
    assert_eq!(
        composite.manifest.dependencies[0].skill,
        primitive.resolved_ref
    );
    assert!(manifest["layers"].as_array().unwrap().iter().any(|layer| {
        layer["annotations"]["org.opencontainers.image.title"]
            .as_str()
            .is_some_and(|title| title.contains("hello-composite"))
    }));
    assert!(manifest["layers"].as_array().unwrap().iter().any(|layer| {
        layer["annotations"]["org.opencontainers.image.title"]
            .as_str()
            .is_some_and(|title| title.contains("hello-inspect"))
    }));
}

#[test]
fn composite_oci_registry_push_includes_dependency_closure() {
    let temp = TempFixtureDir::new();
    let registry_root = temp.path().join("registry-a");
    let registry_store = temp.path().join("oci-registry-store");
    let installer = LocalSourceInstaller::new(&registry_root).unwrap();
    let primitive = installer.install(example_source_dir()).unwrap();
    let composite = installer.install(composite_source_dir()).unwrap();
    let identity = publisher_identity(&composite, &temp.path().join("publisher.json"));
    let registry = LocalRegistry::load(&registry_root).unwrap();
    let server = oci_registry_test_server::OciRegistryTestServer::start(&registry_store);
    let reference = registry_reference(&server, "guild-example-hello-composite", "0.1.0");

    let published = registry
        .push_oci_registry(
            &composite.resolved_ref,
            true,
            &reference,
            &registry_options(),
            &identity,
        )
        .unwrap();
    let index =
        json_value(&server.manifest_bytes_for_tag("guild-example-hello-composite", "0.1.0"));
    let manifest_digest = index["manifests"][0]["digest"].as_str().unwrap();
    let manifest = json_value(
        &fs::read(server.digest_manifest_path("guild-example-hello-composite", manifest_digest))
            .unwrap(),
    );
    let bundle_digest = manifest["config"]["digest"].as_str().unwrap();
    let bundle: InstalledSkillBundle =
        serde_json::from_slice(&fs::read(server.blob_path(bundle_digest)).unwrap()).unwrap();

    assert!(published.bundle.includes_dependency_closure);
    assert_eq!(bundle.root_skill, composite.resolved_ref);
    assert_eq!(bundle.skills.len(), 2);
    assert!(
        bundle
            .skills
            .iter()
            .any(|entry| entry.resolved_ref == composite.resolved_ref)
    );
    assert!(
        bundle
            .skills
            .iter()
            .any(|entry| entry.resolved_ref == primitive.resolved_ref)
    );
    assert_eq!(composite.manifest.dependencies[0].alias, "hello");
    assert_eq!(
        composite.manifest.dependencies[0].skill,
        primitive.resolved_ref
    );
    assert!(manifest["layers"].as_array().unwrap().iter().any(|layer| {
        layer["annotations"]["org.opencontainers.image.title"]
            .as_str()
            .is_some_and(|title| title.contains("hello-composite"))
    }));
    assert!(manifest["layers"].as_array().unwrap().iter().any(|layer| {
        layer["annotations"]["org.opencontainers.image.title"]
            .as_str()
            .is_some_and(|title| title.contains("hello-inspect"))
    }));
}

#[test]
fn composite_bundle_import_preserves_dependency_resolution() {
    let temp = TempFixtureDir::new();
    let registry_a = temp.path().join("registry-a");
    let bundle_root = temp.path().join("bundle");
    let registry_b = temp.path().join("registry-b");
    let installer = LocalSourceInstaller::new(&registry_a).unwrap();
    let primitive = installer.install(example_source_dir()).unwrap();
    let composite = installer.install(composite_source_dir()).unwrap();
    let identity = publisher_identity(&composite, &temp.path().join("publisher.json"));
    let registry = LocalRegistry::load(&registry_a).unwrap();

    registry
        .export_bundle(&composite.resolved_ref, true, &bundle_root, &identity)
        .unwrap();
    LocalRegistry::trust_publisher(&registry_b, &identity.trusted_record()).unwrap();
    LocalRegistry::import_bundle(&registry_b, &bundle_root).unwrap();

    let imported_registry = LocalRegistry::load(&registry_b).unwrap();
    let imported_composite = imported_registry
        .resolve(&requested_hello_composite())
        .unwrap();
    assert_eq!(imported_composite.resolved_ref, composite.resolved_ref);
    assert_eq!(
        imported_composite.manifest.dependencies[0].skill,
        primitive.resolved_ref
    );
    assert_eq!(
        imported_registry
            .resolve_exact(&imported_composite.manifest.dependencies[0].skill)
            .unwrap()
            .resolved_ref,
        primitive.resolved_ref
    );
    assert_eq!(
        imported_composite
            .verification
            .expect("verified import metadata present")
            .publisher
            .id,
        identity.publisher.id
    );
}

#[test]
fn composite_oci_import_preserves_dependency_resolution() {
    let temp = TempFixtureDir::new();
    let registry_a = temp.path().join("registry-a");
    let layout_root = temp.path().join("oci-layout");
    let registry_b = temp.path().join("registry-b");
    let installer = LocalSourceInstaller::new(&registry_a).unwrap();
    let primitive = installer.install(example_source_dir()).unwrap();
    let composite = installer.install(composite_source_dir()).unwrap();
    let identity = publisher_identity(&composite, &temp.path().join("publisher.json"));
    let registry = LocalRegistry::load(&registry_a).unwrap();

    registry
        .export_oci_layout(&composite.resolved_ref, true, &layout_root, &identity)
        .unwrap();
    LocalRegistry::trust_publisher(&registry_b, &identity.trusted_record()).unwrap();
    LocalRegistry::import_oci_layout(&registry_b, &layout_root).unwrap();

    let imported_registry = LocalRegistry::load(&registry_b).unwrap();
    let imported_composite = imported_registry
        .resolve(&requested_hello_composite())
        .unwrap();
    assert_eq!(imported_composite.resolved_ref, composite.resolved_ref);
    assert_eq!(
        imported_composite.manifest.dependencies[0].skill,
        primitive.resolved_ref
    );
    assert_eq!(
        imported_registry
            .resolve_exact(&imported_composite.manifest.dependencies[0].skill)
            .unwrap()
            .resolved_ref,
        primitive.resolved_ref
    );
    assert_eq!(
        imported_composite
            .verification
            .expect("verified import metadata present")
            .publisher
            .id,
        identity.publisher.id
    );
}

#[test]
fn composite_oci_registry_pull_preserves_dependency_resolution() {
    let temp = TempFixtureDir::new();
    let registry_a = temp.path().join("registry-a");
    let registry_store = temp.path().join("oci-registry-store");
    let registry_b = temp.path().join("registry-b");
    let installer = LocalSourceInstaller::new(&registry_a).unwrap();
    let primitive = installer.install(example_source_dir()).unwrap();
    let composite = installer.install(composite_source_dir()).unwrap();
    let identity = publisher_identity(&composite, &temp.path().join("publisher.json"));
    let registry = LocalRegistry::load(&registry_a).unwrap();
    let server = oci_registry_test_server::OciRegistryTestServer::start(&registry_store);
    let reference = registry_reference(&server, "guild-example-hello-composite", "0.1.0");

    registry
        .push_oci_registry(
            &composite.resolved_ref,
            true,
            &reference,
            &registry_options(),
            &identity,
        )
        .unwrap();
    LocalRegistry::trust_publisher(&registry_b, &identity.trusted_record()).unwrap();
    LocalRegistry::pull_oci_registry(&registry_b, &reference, &registry_options()).unwrap();

    let imported_registry = LocalRegistry::load(&registry_b).unwrap();
    let imported_composite = imported_registry
        .resolve(&requested_hello_composite())
        .unwrap();
    assert_eq!(imported_composite.resolved_ref, composite.resolved_ref);
    assert_eq!(
        imported_composite.manifest.dependencies[0].skill,
        primitive.resolved_ref
    );
    assert_eq!(
        imported_registry
            .resolve_exact(&imported_composite.manifest.dependencies[0].skill)
            .unwrap()
            .resolved_ref,
        primitive.resolved_ref
    );
    assert_eq!(
        imported_composite
            .verification
            .expect("verified import metadata present")
            .publisher
            .id,
        identity.publisher.id
    );
}

#[test]
fn bundle_import_fails_for_untrusted_signed_bundle() {
    let temp = TempFixtureDir::new();
    let registry_a = temp.path().join("registry-a");
    let bundle_root = temp.path().join("bundle");
    let registry_b = temp.path().join("registry-b");
    let source_installer = LocalSourceInstaller::new(&registry_a).unwrap();
    let installed_skill = source_installer.install(example_source_dir()).unwrap();
    let identity = publisher_identity(&installed_skill, &temp.path().join("publisher.json"));
    let registry = LocalRegistry::load(&registry_a).unwrap();

    registry
        .export_bundle(
            &installed_skill.resolved_ref,
            false,
            &bundle_root,
            &identity,
        )
        .unwrap();

    let error = LocalRegistry::import_bundle(&registry_b, &bundle_root).unwrap_err();
    assert_eq!(error.code, "bundle-publisher-untrusted");
    assert!(!registry_b.join("installed").join("example").exists());
}

#[test]
fn oci_import_fails_for_untrusted_signed_bundle() {
    let temp = TempFixtureDir::new();
    let registry_a = temp.path().join("registry-a");
    let layout_root = temp.path().join("oci-layout");
    let registry_b = temp.path().join("registry-b");
    let source_installer = LocalSourceInstaller::new(&registry_a).unwrap();
    let installed_skill = source_installer.install(example_source_dir()).unwrap();
    let identity = publisher_identity(&installed_skill, &temp.path().join("publisher.json"));
    let registry = LocalRegistry::load(&registry_a).unwrap();

    registry
        .export_oci_layout(
            &installed_skill.resolved_ref,
            false,
            &layout_root,
            &identity,
        )
        .unwrap();

    let error = LocalRegistry::import_oci_layout(&registry_b, &layout_root).unwrap_err();
    assert_eq!(error.code, "bundle-publisher-untrusted");
    assert!(!registry_b.join("installed").join("example").exists());
}

#[test]
fn oci_registry_pull_fails_for_untrusted_signed_bundle() {
    let temp = TempFixtureDir::new();
    let registry_a = temp.path().join("registry-a");
    let registry_store = temp.path().join("oci-registry-store");
    let registry_b = temp.path().join("registry-b");
    let source_installer = LocalSourceInstaller::new(&registry_a).unwrap();
    let installed_skill = source_installer.install(example_source_dir()).unwrap();
    let identity = publisher_identity(&installed_skill, &temp.path().join("publisher.json"));
    let registry = LocalRegistry::load(&registry_a).unwrap();
    let server = oci_registry_test_server::OciRegistryTestServer::start(&registry_store);
    let reference = registry_reference(&server, "guild-example-hello-inspect", "0.1.0");

    registry
        .push_oci_registry(
            &installed_skill.resolved_ref,
            false,
            &reference,
            &registry_options(),
            &identity,
        )
        .unwrap();

    let error =
        LocalRegistry::pull_oci_registry(&registry_b, &reference, &registry_options()).unwrap_err();
    assert_eq!(error.code, "bundle-publisher-untrusted");
    assert!(!registry_b.join("installed").join("example").exists());
}

#[test]
fn bundle_import_fails_on_tampered_content_even_when_signature_is_trusted() {
    let temp = TempFixtureDir::new();
    let registry_a = temp.path().join("registry-a");
    let bundle_root = temp.path().join("bundle");
    let registry_b = temp.path().join("registry-b");
    let source_installer = LocalSourceInstaller::new(&registry_a).unwrap();
    let installed_skill = source_installer.install(example_source_dir()).unwrap();
    let identity = publisher_identity(&installed_skill, &temp.path().join("publisher.json"));
    let registry = LocalRegistry::load(&registry_a).unwrap();

    registry
        .export_bundle(
            &installed_skill.resolved_ref,
            false,
            &bundle_root,
            &identity,
        )
        .unwrap();
    LocalRegistry::trust_publisher(&registry_b, &identity.trusted_record()).unwrap();
    fs::write(
        bundle_root
            .join("installed/example/hello-inspect/0.1.0")
            .join(installed_skill.resolved_ref.digest.replace(':', "-"))
            .join("component.wasm"),
        b"tampered artifact",
    )
    .unwrap();

    let error = LocalRegistry::import_bundle(&registry_b, &bundle_root).unwrap_err();
    assert_eq!(error.code, "artifact-digest-mismatch");
    assert!(!registry_b.join("installed").join("example").exists());
}

#[test]
fn oci_import_fails_on_tampered_content_even_when_signature_is_trusted() {
    let temp = TempFixtureDir::new();
    let registry_a = temp.path().join("registry-a");
    let layout_root = temp.path().join("oci-layout");
    let registry_b = temp.path().join("registry-b");
    let source_installer = LocalSourceInstaller::new(&registry_a).unwrap();
    let installed_skill = source_installer.install(example_source_dir()).unwrap();
    let identity = publisher_identity(&installed_skill, &temp.path().join("publisher.json"));
    let registry = LocalRegistry::load(&registry_a).unwrap();

    registry
        .export_oci_layout(
            &installed_skill.resolved_ref,
            false,
            &layout_root,
            &identity,
        )
        .unwrap();
    LocalRegistry::trust_publisher(&registry_b, &identity.trusted_record()).unwrap();
    fs::write(
        oci_blob_path(
            &layout_root,
            &installed_skill.manifest.package.artifact_digest,
        ),
        b"tampered artifact",
    )
    .unwrap();

    let error = LocalRegistry::import_oci_layout(&registry_b, &layout_root).unwrap_err();
    assert_eq!(error.code, "oci-layout-blob-size-mismatch");
    assert!(!registry_b.join("installed").join("example").exists());
}

#[test]
fn oci_registry_pull_fails_on_tampered_content_even_when_signature_is_trusted() {
    let temp = TempFixtureDir::new();
    let registry_a = temp.path().join("registry-a");
    let registry_store = temp.path().join("oci-registry-store");
    let registry_b = temp.path().join("registry-b");
    let source_installer = LocalSourceInstaller::new(&registry_a).unwrap();
    let installed_skill = source_installer.install(example_source_dir()).unwrap();
    let identity = publisher_identity(&installed_skill, &temp.path().join("publisher.json"));
    let registry = LocalRegistry::load(&registry_a).unwrap();
    let server = oci_registry_test_server::OciRegistryTestServer::start(&registry_store);
    let reference = registry_reference(&server, "guild-example-hello-inspect", "0.1.0");

    registry
        .push_oci_registry(
            &installed_skill.resolved_ref,
            false,
            &reference,
            &registry_options(),
            &identity,
        )
        .unwrap();
    LocalRegistry::trust_publisher(&registry_b, &identity.trusted_record()).unwrap();
    server.tamper_blob(
        &installed_skill.manifest.package.artifact_digest,
        b"tampered artifact",
    );

    let error =
        LocalRegistry::pull_oci_registry(&registry_b, &reference, &registry_options()).unwrap_err();
    assert_eq!(error.code, "oci-registry-blob-read-failed");
    assert!(!registry_b.join("installed").join("example").exists());
}

#[test]
fn oci_import_fails_when_layout_index_is_missing() {
    let temp = TempFixtureDir::new();
    let registry_a = temp.path().join("registry-a");
    let layout_root = temp.path().join("oci-layout");
    let registry_b = temp.path().join("registry-b");
    let source_installer = LocalSourceInstaller::new(&registry_a).unwrap();
    let installed_skill = source_installer.install(example_source_dir()).unwrap();
    let identity = publisher_identity(&installed_skill, &temp.path().join("publisher.json"));
    let registry = LocalRegistry::load(&registry_a).unwrap();

    registry
        .export_oci_layout(
            &installed_skill.resolved_ref,
            false,
            &layout_root,
            &identity,
        )
        .unwrap();
    LocalRegistry::trust_publisher(&registry_b, &identity.trusted_record()).unwrap();
    fs::remove_file(layout_root.join("index.json")).unwrap();

    let error = LocalRegistry::import_oci_layout(&registry_b, &layout_root).unwrap_err();
    assert_eq!(error.code, "oci-layout-index-missing");
    assert!(!registry_b.join("installed").join("example").exists());
}

#[test]
fn bundle_import_fails_when_required_content_is_missing() {
    let temp = TempFixtureDir::new();
    let registry_a = temp.path().join("registry-a");
    let bundle_root = temp.path().join("bundle");
    let registry_b = temp.path().join("registry-b");
    let source_installer = LocalSourceInstaller::new(&registry_a).unwrap();
    let installed_skill = source_installer.install(example_source_dir()).unwrap();
    let identity = publisher_identity(&installed_skill, &temp.path().join("publisher.json"));
    let registry = LocalRegistry::load(&registry_a).unwrap();

    registry
        .export_bundle(
            &installed_skill.resolved_ref,
            false,
            &bundle_root,
            &identity,
        )
        .unwrap();
    LocalRegistry::trust_publisher(&registry_b, &identity.trusted_record()).unwrap();
    fs::remove_file(
        bundle_root
            .join("installed/example/hello-inspect/0.1.0")
            .join(installed_skill.resolved_ref.digest.replace(':', "-"))
            .join("input.schema.json"),
    )
    .unwrap();

    let error = LocalRegistry::import_bundle(&registry_b, &bundle_root).unwrap_err();
    assert_eq!(error.code, "staged-file-missing");
}

#[test]
fn bundle_import_fails_when_signature_file_is_missing() {
    let temp = TempFixtureDir::new();
    let registry_a = temp.path().join("registry-a");
    let bundle_root = temp.path().join("bundle");
    let registry_b = temp.path().join("registry-b");
    let source_installer = LocalSourceInstaller::new(&registry_a).unwrap();
    let installed_skill = source_installer.install(example_source_dir()).unwrap();
    let identity = publisher_identity(&installed_skill, &temp.path().join("publisher.json"));
    let registry = LocalRegistry::load(&registry_a).unwrap();

    registry
        .export_bundle(
            &installed_skill.resolved_ref,
            false,
            &bundle_root,
            &identity,
        )
        .unwrap();
    LocalRegistry::trust_publisher(&registry_b, &identity.trusted_record()).unwrap();
    fs::remove_file(bundle_root.join("bundle.signature.json")).unwrap();

    let error = LocalRegistry::import_bundle(&registry_b, &bundle_root).unwrap_err();
    assert_eq!(error.code, "bundle-signature-missing");
}

#[test]
fn local_source_install_builds_and_stages_digest_pinned_skill() {
    let registry_root = prepared_registry_root();
    let registry = LocalRegistry::load(registry_root).unwrap();
    let installed = registry.resolve(&requested_hello_inspect()).unwrap();

    assert_eq!(installed.resolved_ref.key.namespace, "example");
    assert_eq!(installed.resolved_ref.key.name, "hello-inspect");
    assert_eq!(installed.resolved_ref.version.to_string(), "0.1.0");
    assert!(installed.resolved_ref.digest.starts_with("sha256:"));
    assert!(installed.artifact_path.ends_with("component.wasm"));
    assert!(installed.artifact_path.exists());
    assert!(installed.manifest_path.exists());
}

#[test]
fn requested_skill_resolves_to_digest_pinned_installed_skill() {
    let registry = LocalRegistry::load(prepared_registry_root()).unwrap();
    let installed = registry.resolve(&requested_hello_inspect()).unwrap();

    assert_eq!(installed.resolved_ref.key.namespace, "example");
    assert_eq!(installed.resolved_ref.key.name, "hello-inspect");
    assert_eq!(installed.resolved_ref.version.to_string(), "0.1.0");
    assert_eq!(
        installed.resolved_ref.digest,
        installed.manifest.package.artifact_digest
    );
    assert!(installed.artifact_path.exists());
}

#[test]
fn reinstall_updates_digest_when_source_changes() {
    let temp = TempFixtureDir::new();
    let workspace_root = temp.path().join("workspace");
    let source_root = workspace_root.join("examples/skills/hello-inspect");
    let registry_root = temp.path().join("registry");

    copy_dir_recursive(&example_source_dir(), &source_root);
    copy_dir_recursive(&repo_root().join("wit"), &workspace_root.join("wit"));

    let installer = LocalSourceInstaller::new(&registry_root).unwrap();
    let first = installer.install(&source_root).unwrap();

    let guest_source = source_root.join("skill-rust/src/lib.rs");
    let guest = fs::read_to_string(&guest_source)
        .unwrap()
        .replace("Guild inspect is working.", "Guild inspect was rebuilt.");
    fs::write(&guest_source, guest).unwrap();

    let second = installer.install(&source_root).unwrap();

    assert_ne!(first.resolved_ref.digest, second.resolved_ref.digest);

    let registry = LocalRegistry::load(&registry_root).unwrap();
    let error = registry.resolve(&requested_hello_inspect()).unwrap_err();
    assert_eq!(error.code, "skill-version-ambiguous");
    assert_eq!(
        registry
            .resolve_exact(&first.resolved_ref)
            .unwrap()
            .resolved_ref,
        first.resolved_ref
    );
    assert_eq!(
        registry
            .resolve_exact(&second.resolved_ref)
            .unwrap()
            .resolved_ref,
        second.resolved_ref
    );
}

#[test]
fn failed_source_reinstall_preserves_existing_working_digest() {
    let temp = TempFixtureDir::new();
    let workspace_root = temp.path().join("workspace");
    let source_root = workspace_root.join("examples/skills/hello-inspect");
    let registry_root = temp.path().join("registry");

    copy_dir_recursive(&example_source_dir(), &source_root);
    copy_dir_recursive(&repo_root().join("wit"), &workspace_root.join("wit"));

    let installer = LocalSourceInstaller::new(&registry_root).unwrap();
    let first = installer.install(&source_root).unwrap();

    let guest_source = source_root.join("skill-rust/src/lib.rs");
    let guest = fs::read_to_string(&guest_source)
        .unwrap()
        .replace("Guild inspect is working.", "Guild inspect failed staging.");
    fs::write(&guest_source, guest).unwrap();

    let manifest_path = source_root.join("manifest.json");
    let manifest = fs::read_to_string(&manifest_path)
        .unwrap()
        .replace("\"./examples.json\"", "\"./missing-examples.json\"");
    fs::write(&manifest_path, manifest).unwrap();

    let error = installer.install(&source_root).unwrap_err();
    assert_eq!(error.code, "source-file-missing");

    let registry = LocalRegistry::load(&registry_root).unwrap();
    let resolved = registry.resolve(&requested_hello_inspect()).unwrap();
    assert_eq!(resolved.resolved_ref, first.resolved_ref);
    assert_eq!(
        registry
            .resolve_exact(&first.resolved_ref)
            .unwrap()
            .resolved_ref,
        first.resolved_ref
    );
}

#[test]
fn missing_staged_artifact_fails_closed() {
    let temp = TempFixtureDir::new();
    let source_installer = LocalSourceInstaller::new(temp.path()).unwrap();
    let installed_skill = source_installer.install(example_source_dir()).unwrap();

    fs::remove_file(&installed_skill.artifact_path).unwrap();

    let error = LocalRegistry::load(temp.path()).unwrap_err();
    assert_eq!(error.code, "artifact-missing");
}

#[test]
fn source_only_skill_root_is_not_treated_as_installed_registry() {
    let error = LocalRegistry::load(example_source_dir()).unwrap_err();
    assert_eq!(error.code, "source-skill-not-installed");
}

#[test]
fn composite_install_fails_when_declared_dependency_is_missing() {
    let temp = TempFixtureDir::new();
    let installer = LocalSourceInstaller::new(temp.path()).unwrap();

    let error = installer.install(composite_source_dir()).unwrap_err();
    assert_eq!(error.code, "dependency-resolution-failed");
}

#[test]
fn composite_install_resolves_declared_dependency_to_installed_record() {
    let temp = TempFixtureDir::new();
    let installer = LocalSourceInstaller::new(temp.path()).unwrap();
    let primitive = installer.install(example_source_dir()).unwrap();
    let composite = installer.install(composite_source_dir()).unwrap();

    assert_eq!(composite.manifest.dependencies.len(), 1);
    assert_eq!(composite.manifest.dependencies[0].alias, "hello");
    assert_eq!(
        composite.manifest.dependencies[0].skill,
        primitive.resolved_ref
    );

    let registry = LocalRegistry::load(temp.path()).unwrap();
    let resolved = registry.resolve(&requested_hello_composite()).unwrap();
    assert_eq!(resolved.resolved_ref.key.name, "hello-composite");
    assert_eq!(
        resolved.manifest.dependencies[0].skill,
        primitive.resolved_ref
    );
    assert_eq!(
        registry
            .resolve_exact(&resolved.manifest.dependencies[0].skill)
            .unwrap()
            .resolved_ref,
        primitive.resolved_ref
    );
}

#[test]
fn evidence_payload_and_metadata_resources_are_distinct_and_readable() {
    let registry = LocalRegistry::load(prepared_registry_root()).unwrap();
    let first = registry
        .store_evidence(
            "execution-1",
            &EvidenceEmissionRequest {
                payload: br#"{"hello":"world"}"#.to_vec(),
                mime_type: "application/json".into(),
                title: Some("fixture".into()),
                audience: EvidenceAudience::User,
                redaction: RedactionClass::None,
                freshness: Some("deterministic".into()),
            },
        )
        .unwrap();
    let second = registry
        .store_evidence(
            "execution-2",
            &EvidenceEmissionRequest {
                payload: br#"{"hello":"world"}"#.to_vec(),
                mime_type: "application/json".into(),
                title: Some("fixture-again".into()),
                audience: EvidenceAudience::Assistant,
                redaction: RedactionClass::None,
                freshness: Some("deterministic".into()),
            },
        )
        .unwrap();

    assert_ne!(first.uri, second.uri);
    assert_eq!(first.sha256, second.sha256);

    let first_record = registry.load_evidence_record(&first.uri).unwrap();
    let second_record = registry.load_evidence_record(&second.uri).unwrap();
    let payload = registry.read_resource(&first.uri).unwrap();
    let metadata = registry
        .read_resource(&format!("{}/metadata", first.uri))
        .unwrap();
    let blob = registry.read_resource(&first_record.blob_uri).unwrap();
    let parsed_metadata: EvidenceRecord = serde_json::from_slice(&metadata.bytes).unwrap();

    assert_eq!(first_record.uri, first.uri);
    assert_eq!(first_record.sha256, first.sha256.clone().unwrap());
    assert_eq!(first_record.mime_type, "application/json");
    assert_eq!(first_record.title.as_deref(), Some("fixture"));
    assert_eq!(
        first_record.produced_by_execution.as_deref(),
        Some("execution-1")
    );
    assert_eq!(second_record.title.as_deref(), Some("fixture-again"));
    assert_eq!(
        second_record.produced_by_execution.as_deref(),
        Some("execution-2")
    );
    assert_eq!(first_record.blob_uri, second_record.blob_uri);
    assert_eq!(payload.uri, first.uri);
    assert_eq!(payload.mime_type, "application/json");
    assert_eq!(payload.bytes, br#"{"hello":"world"}"#);
    assert_eq!(payload.sha256, first.sha256);
    assert_eq!(metadata.uri, format!("{}/metadata", first.uri));
    assert_eq!(metadata.mime_type, "application/json");
    assert_eq!(parsed_metadata, first_record);
    assert_eq!(
        parsed_metadata.produced_by_execution.as_deref(),
        Some("execution-1")
    );
    assert_eq!(blob.mime_type, "application/octet-stream");
    assert_eq!(blob.bytes, br#"{"hello":"world"}"#);
    assert_eq!(blob.sha256, first.sha256);
}

#[test]
fn malformed_and_unknown_evidence_metadata_resources_fail_closed() {
    let registry = LocalRegistry::load(prepared_registry_root()).unwrap();

    let malformed = registry
        .read_resource("guild://objects/records/record-1/metadata/extra")
        .unwrap_err();
    assert_eq!(malformed.code, "resource-uri-invalid");

    let missing = registry
        .read_resource("guild://objects/records/does-not-exist/metadata")
        .unwrap_err();
    assert_eq!(missing.code, "object-not-found");
}

#[test]
fn missing_object_resource_fails_closed() {
    let registry = LocalRegistry::load(prepared_registry_root()).unwrap();
    let error = registry
        .read_resource("guild://objects/sha256/deadbeef")
        .unwrap_err();

    assert_eq!(error.code, "object-not-found");
}
