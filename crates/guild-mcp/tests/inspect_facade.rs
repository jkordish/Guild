use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use guild_mcp::{GuildMcpFacade, InspectRequest};
use guild_registry::{InstalledSkill, LocalPublisherIdentity, LocalRegistry, LocalSourceInstaller};
use guild_runner::WasmtimeRuntimeAdapter;
use guild_types::{
    CapabilityAccess, CapabilityConstraints, CapabilityGrantSet, CapabilityId,
    EmitEvidenceConstraints, EvidenceAudience, ExecutionStatus, GrantedCapability,
    InvokeDependencyConstraints, LogConstraints, ReadResourceConstraints, RedactionClass,
    RequestedSkillRef, ResourceKind, Severity, SkillKey, VersionRequirement,
};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn example_source_dir() -> PathBuf {
    repo_root().join("examples/skills/hello-inspect")
}

fn composite_source_dir() -> PathBuf {
    repo_root().join("examples/skills/hello-composite")
}

fn explain_source_dir() -> PathBuf {
    repo_root().join("examples/skills/explain-execution")
}

fn wit_dir() -> PathBuf {
    repo_root().join("wit")
}

fn publisher_identity(installed: &InstalledSkill, path: &Path) -> LocalPublisherIdentity {
    let identity = LocalPublisherIdentity::generate(installed.manifest.publisher.clone()).unwrap();
    identity.save(path).unwrap();
    LocalPublisherIdentity::load(path).unwrap()
}

fn prepared_registry_root() -> &'static PathBuf {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();

    ROOT.get_or_init(|| {
        let root = repo_root().join("target/test-install-registry/guild-mcp");
        if root.exists() {
            let _ = std::fs::remove_dir_all(&root);
        }

        LocalSourceInstaller::new(&root)
            .unwrap()
            .install(example_source_dir())
            .unwrap();
        LocalSourceInstaller::new(&root)
            .unwrap()
            .install(composite_source_dir())
            .unwrap();
        LocalSourceInstaller::new(&root)
            .unwrap()
            .install(explain_source_dir())
            .unwrap();

        root
    })
}

fn composite_request() -> InspectRequest {
    InspectRequest::new(
        RequestedSkillRef {
            key: SkillKey {
                namespace: "example".into(),
                name: "hello-composite".into(),
            },
            version_req: VersionRequirement::parse("^0.1").unwrap(),
        },
        serde_json::json!({ "name": "Ada" }),
        "tenant-1",
        "actor-1",
        CapabilityGrantSet {
            grants: vec![invoke_hello_grant(), emit_evidence_grant()],
        },
    )
}

fn build_facade() -> GuildMcpFacade<LocalRegistry, WasmtimeRuntimeAdapter> {
    let registry = LocalRegistry::load(prepared_registry_root()).unwrap();
    GuildMcpFacade::new(registry, WasmtimeRuntimeAdapter::new().unwrap())
}

fn explain_request(uri: &str) -> InspectRequest {
    InspectRequest::new(
        RequestedSkillRef {
            key: SkillKey {
                namespace: "example".into(),
                name: "explain-execution".into(),
            },
            version_req: VersionRequirement::parse("^0.1").unwrap(),
        },
        serde_json::json!({
            "execution_uri": uri,
            "include_first_evidence": true,
        }),
        "tenant-1",
        "actor-1",
        CapabilityGrantSet {
            grants: vec![read_resource_grant()],
        },
    )
}

fn invoke_hello_grant() -> GrantedCapability {
    GrantedCapability {
        id: CapabilityId::InvokeSkill,
        access: CapabilityAccess::Invoke,
        constraints: CapabilityConstraints::InvokeDependency(InvokeDependencyConstraints {
            aliases: Some(vec!["hello".into()]),
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

fn read_resource_grant() -> GrantedCapability {
    GrantedCapability {
        id: CapabilityId::ReadResource,
        access: CapabilityAccess::Read,
        constraints: CapabilityConstraints::ReadResource(ReadResourceConstraints {
            uri_prefixes: Some(vec![
                "guild://executions/".into(),
                "guild://objects/records/".into(),
            ]),
            resource_kinds: Some(vec![ResourceKind::Execution, ResourceKind::Object]),
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

struct TempFixtureDir {
    path: PathBuf,
}

impl TempFixtureDir {
    fn new(prefix: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("{prefix}-{unique}"));
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
fn guild_inspect_uses_real_registry_and_runner_path() {
    let facade = build_facade();
    let request = InspectRequest::new(
        RequestedSkillRef {
            key: SkillKey {
                namespace: "example".into(),
                name: "hello-inspect".into(),
            },
            version_req: VersionRequirement::parse("^0.1").unwrap(),
        },
        serde_json::json!({ "name": "Ada" }),
        "tenant-1",
        "actor-1",
        CapabilityGrantSet {
            grants: vec![emit_evidence_grant(), log_info_grant()],
        },
    );

    let response = facade.inspect(request).unwrap();
    let stored = facade
        .read_resource(&response.structured_content.receipt.uri)
        .unwrap();
    let output = response.structured_content.output.as_ref().unwrap();

    assert_eq!(response.summary, output.summary);
    assert_eq!(output.structured["mode"], "inspect");
    assert_eq!(
        response
            .structured_content
            .provenance
            .resolved_skill
            .key
            .name,
        "hello-inspect"
    );
    assert_eq!(stored.mime_type, "application/json");
}

#[test]
fn guild_inspect_executes_composite_skill_through_nested_path() {
    let facade = build_facade();
    let response = facade.inspect(composite_request()).unwrap();
    let parent_resource = facade
        .read_resource(&response.structured_content.receipt.uri)
        .unwrap();
    let child_resource = facade
        .read_resource(&response.structured_content.child_executions[0].uri)
        .unwrap();
    let output = response.structured_content.output.as_ref().unwrap();

    assert_eq!(response.summary, output.summary);
    assert_eq!(output.structured["invoked_alias"], "hello");
    assert_eq!(response.structured_content.child_executions.len(), 1);
    assert_eq!(
        response.structured_content.child_executions[0]
            .provenance
            .resolved_skill
            .key
            .name,
        "hello-inspect"
    );
    assert_eq!(parent_resource.mime_type, "application/json");
    assert_eq!(child_resource.mime_type, "application/json");
    let child_record: guild_types::ExecutionRecord =
        serde_json::from_slice(&child_resource.bytes).unwrap();
    let evidence_resource = facade
        .read_resource(&child_record.emitted_evidence[0].uri)
        .unwrap();
    assert_eq!(evidence_resource.mime_type, "application/json");
}

#[test]
fn missing_resource_read_fails_closed() {
    let facade = build_facade();
    let error = facade
        .read_resource("guild://executions/does-not-exist")
        .unwrap_err();

    assert_eq!(error.code, "execution-not-found");
}

#[test]
fn explain_skill_reads_the_same_resources_mcp_exposes() {
    let facade = build_facade();
    let primitive = facade
        .inspect(InspectRequest::new(
            RequestedSkillRef {
                key: SkillKey {
                    namespace: "example".into(),
                    name: "hello-inspect".into(),
                },
                version_req: VersionRequirement::parse("^0.1").unwrap(),
            },
            serde_json::json!({ "name": "Ada" }),
            "tenant-1",
            "actor-1",
            CapabilityGrantSet {
                grants: vec![emit_evidence_grant()],
            },
        ))
        .unwrap();

    let execution_resource = facade
        .read_resource(&primitive.structured_content.receipt.uri)
        .unwrap();
    let evidence_resource = facade
        .read_resource(&primitive.structured_content.emitted_evidence[0].uri)
        .unwrap();
    let explained = facade
        .inspect(explain_request(&primitive.structured_content.receipt.uri))
        .unwrap();
    let explained_output = explained.structured_content.output.as_ref().unwrap();
    let primitive_output = primitive.structured_content.output.as_ref().unwrap();

    assert_eq!(
        explained_output.structured["target_execution_uri"],
        primitive.structured_content.receipt.uri
    );
    assert_eq!(
        explained_output.structured["execution_resource"]["sha256"],
        execution_resource.sha256.unwrap()
    );
    assert_eq!(
        explained_output.structured["first_evidence"]["sha256"],
        evidence_resource.sha256.unwrap()
    );
    assert_eq!(
        explained_output.structured["stored_summary"],
        primitive_output.summary
    );
}

#[test]
fn mcp_can_read_persisted_rejected_execution_resources() {
    let facade = build_facade();
    let error = facade
        .inspect(InspectRequest::new(
            RequestedSkillRef {
                key: SkillKey {
                    namespace: "example".into(),
                    name: "explain-execution".into(),
                },
                version_req: VersionRequirement::parse("^0.1").unwrap(),
            },
            serde_json::json!({
                "execution_uri": "guild://executions/does-not-matter",
                "include_first_evidence": false,
            }),
            "tenant-1",
            "actor-1",
            CapabilityGrantSet::default(),
        ))
        .unwrap_err();

    assert_eq!(error.code, "capability-mismatch");
    let receipt = error.receipt.expect("rejected execution exposes a receipt");
    let resource = facade.read_resource(&receipt.uri).unwrap();
    let record: guild_types::ExecutionRecord = serde_json::from_slice(&resource.bytes).unwrap();
    assert_eq!(record.status, ExecutionStatus::Rejected);
    assert!(record.output.is_none());
}

#[test]
fn mcp_can_read_persisted_failed_execution_resources() {
    let facade = build_facade();
    let error = facade
        .inspect(InspectRequest::new(
            RequestedSkillRef {
                key: SkillKey {
                    namespace: "example".into(),
                    name: "hello-inspect".into(),
                },
                version_req: VersionRequirement::parse("^0.1").unwrap(),
            },
            serde_json::json!({ "name": "Ada", "emit_log": true }),
            "tenant-1",
            "actor-1",
            CapabilityGrantSet {
                grants: vec![emit_evidence_grant()],
            },
        ))
        .unwrap_err();

    assert_eq!(error.code, "log-write-not-granted");
    let receipt = error.receipt.expect("failed execution exposes a receipt");
    let resource = facade.read_resource(&receipt.uri).unwrap();
    let record: guild_types::ExecutionRecord = serde_json::from_slice(&resource.bytes).unwrap();
    assert_eq!(record.status, ExecutionStatus::Rejected);
    assert!(record.output.is_none());
}

#[test]
fn imported_primitive_bundle_executes_without_source_workspace() {
    let temp = TempFixtureDir::new("guild-portable-primitive");
    let workspace_root = temp.path().join("workspace");
    let source_root = workspace_root.join("examples/skills/hello-inspect");
    let registry_a = temp.path().join("registry-a");
    let bundle_root = temp.path().join("bundle");
    let registry_b = temp.path().join("registry-b");

    copy_dir_recursive(&example_source_dir(), &source_root);
    copy_dir_recursive(&wit_dir(), &workspace_root.join("wit"));

    let installer = LocalSourceInstaller::new(&registry_a).unwrap();
    let installed = installer.install(&source_root).unwrap();
    let identity = publisher_identity(&installed, &temp.path().join("publisher.json"));
    let registry = LocalRegistry::load(&registry_a).unwrap();
    registry
        .export_bundle(&installed.resolved_ref, false, &bundle_root, &identity)
        .unwrap();

    fs::remove_dir_all(&workspace_root).unwrap();

    LocalRegistry::trust_publisher(&registry_b, &identity.trusted_record()).unwrap();
    LocalRegistry::import_bundle(&registry_b, &bundle_root).unwrap();
    let facade = GuildMcpFacade::new(
        LocalRegistry::load(&registry_b).unwrap(),
        WasmtimeRuntimeAdapter::new().unwrap(),
    );
    let response = facade
        .inspect(InspectRequest::new(
            RequestedSkillRef {
                key: SkillKey {
                    namespace: "example".into(),
                    name: "hello-inspect".into(),
                },
                version_req: VersionRequirement::parse("^0.1").unwrap(),
            },
            serde_json::json!({ "name": "Ada" }),
            "tenant-1",
            "actor-1",
            CapabilityGrantSet {
                grants: vec![emit_evidence_grant()],
            },
        ))
        .unwrap();

    assert_eq!(
        response.structured_content.status,
        ExecutionStatus::Succeeded
    );
    assert_eq!(
        response.structured_content.provenance.resolved_skill.digest,
        installed.resolved_ref.digest
    );
    assert_eq!(
        facade
            .read_resource(&response.structured_content.receipt.uri)
            .unwrap()
            .mime_type,
        "application/json"
    );
}

#[test]
fn imported_composite_bundle_executes_through_normal_nested_path() {
    let temp = TempFixtureDir::new("guild-portable-composite");
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

    fs::remove_dir_all(&registry_a).unwrap();

    LocalRegistry::trust_publisher(&registry_b, &identity.trusted_record()).unwrap();
    LocalRegistry::import_bundle(&registry_b, &bundle_root).unwrap();
    let facade = GuildMcpFacade::new(
        LocalRegistry::load(&registry_b).unwrap(),
        WasmtimeRuntimeAdapter::new().unwrap(),
    );
    let response = facade.inspect(composite_request()).unwrap();

    assert_eq!(
        response.structured_content.status,
        ExecutionStatus::Succeeded
    );
    assert_eq!(
        response.structured_content.provenance.resolved_skill.digest,
        composite.resolved_ref.digest
    );
    assert_eq!(response.structured_content.child_executions.len(), 1);
    assert_eq!(
        response.structured_content.child_executions[0]
            .provenance
            .resolved_skill
            .digest,
        primitive.resolved_ref.digest
    );
    let child_resource = facade
        .read_resource(&response.structured_content.child_executions[0].uri)
        .unwrap();
    assert_eq!(child_resource.mime_type, "application/json");
}
