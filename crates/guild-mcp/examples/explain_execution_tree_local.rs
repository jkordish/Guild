use std::path::{Path, PathBuf};

use guild_mcp::{GuildMcpFacade, InspectRequest};
use guild_registry::{LocalRegistry, LocalSourceInstaller};
use guild_runner::WasmtimeRuntimeAdapter;
use guild_types::{
    CapabilityAccess, CapabilityConstraints, CapabilityGrantSet, CapabilityId,
    EmitEvidenceConstraints, EvidenceAudience, GrantedCapability, InvokeDependencyConstraints,
    ReadResourceConstraints, RedactionClass, RequestedSkillRef, ResourceKind, SkillKey,
    VersionRequirement,
};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root exists")
}

fn primitive_source_dir() -> PathBuf {
    repo_root().join("examples/skills/hello-inspect")
}

fn composite_source_dir() -> PathBuf {
    repo_root().join("examples/skills/hello-composite")
}

fn explain_tree_source_dir() -> PathBuf {
    repo_root().join("examples/skills/explain-execution-tree")
}

fn local_registry_root() -> PathBuf {
    repo_root().join("target/dev-local-registry/explain-execution-tree-local")
}

fn reset_registry_root(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if path.exists() {
        std::fs::remove_dir_all(path)?;
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let registry_root = local_registry_root();
    reset_registry_root(&registry_root)?;
    let installer = LocalSourceInstaller::new(&registry_root)?;
    let primitive = installer.install(primitive_source_dir())?;
    let composite = installer.install(composite_source_dir())?;

    let registry = LocalRegistry::load(&registry_root)?;
    let facade = GuildMcpFacade::new(registry, WasmtimeRuntimeAdapter::new()?);
    let inspected = facade.inspect(InspectRequest::new(
        RequestedSkillRef {
            key: SkillKey {
                namespace: "example".into(),
                name: "hello-composite".into(),
            },
            version_req: VersionRequirement::parse("^0.1")?,
        },
        serde_json::json!({ "name": "Ada" }),
        "tenant-dev",
        "actor-dev",
        CapabilityGrantSet {
            grants: vec![
                GrantedCapability {
                    id: CapabilityId::InvokeSkill,
                    access: CapabilityAccess::Invoke,
                    constraints: CapabilityConstraints::InvokeDependency(
                        InvokeDependencyConstraints {
                            aliases: Some(vec!["hello".into()]),
                        },
                    ),
                },
                GrantedCapability {
                    id: CapabilityId::EmitEvidence,
                    access: CapabilityAccess::Write,
                    constraints: CapabilityConstraints::EmitEvidence(EmitEvidenceConstraints {
                        max_bytes: Some(65_536),
                        audiences: Some(vec![EvidenceAudience::User]),
                        redactions: Some(vec![RedactionClass::None]),
                    }),
                },
            ],
        },
    ))?;

    let explain_tree = installer.install(explain_tree_source_dir())?;
    let registry = LocalRegistry::load(&registry_root)?;
    let facade = GuildMcpFacade::new(registry, WasmtimeRuntimeAdapter::new()?);
    let explanation = facade.inspect(InspectRequest::new(
        RequestedSkillRef {
            key: SkillKey {
                namespace: "example".into(),
                name: "explain-execution-tree".into(),
            },
            version_req: VersionRequirement::parse("^0.1")?,
        },
        serde_json::json!({
            "execution_uri": inspected.structured_content.receipt.uri,
            "max_depth": 4,
            "max_nodes": 32,
            "include_evidence_resources": true,
        }),
        "tenant-dev",
        "actor-dev",
        CapabilityGrantSet {
            grants: vec![GrantedCapability {
                id: CapabilityId::ReadResource,
                access: CapabilityAccess::Read,
                constraints: CapabilityConstraints::ReadResource(ReadResourceConstraints {
                    uri_prefixes: Some(vec![
                        "guild://executions/".into(),
                        "guild://objects/records/".into(),
                    ]),
                    resource_kinds: Some(vec![ResourceKind::Execution, ResourceKind::Object]),
                }),
            }],
        },
    ))?;

    println!("installed primitive {}", primitive.resolved_ref.digest);
    println!("installed composite {}", composite.resolved_ref.digest);
    println!(
        "installed explain-tree {}",
        explain_tree.resolved_ref.digest
    );
    println!(
        "root execution URI: {}",
        inspected.structured_content.receipt.uri
    );
    println!(
        "tree explanation execution URI: {}",
        explanation.structured_content.receipt.uri
    );
    println!("{}", explanation.summary);
    let report = explanation
        .structured_content
        .output
        .as_ref()
        .expect("tree explanation returns output");
    println!("{}", serde_json::to_string_pretty(&report.structured)?);

    Ok(())
}
