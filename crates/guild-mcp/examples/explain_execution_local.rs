use std::path::{Path, PathBuf};

use guild_mcp::{GuildMcpFacade, InspectRequest};
use guild_registry::{LocalRegistry, LocalSourceInstaller};
use guild_runner::WasmtimeRuntimeAdapter;
use guild_types::{
    CapabilityAccess, CapabilityConstraints, CapabilityGrantSet, CapabilityId,
    EmitEvidenceConstraints, EvidenceAudience, GrantedCapability, ReadResourceConstraints,
    RedactionClass, RequestedSkillRef, ResourceKind, SkillKey, VersionRequirement,
};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root exists")
}

fn inspect_source_dir() -> PathBuf {
    repo_root().join("examples/skills/hello-inspect")
}

fn explain_source_dir() -> PathBuf {
    repo_root().join("examples/skills/explain-execution")
}

fn local_registry_root() -> PathBuf {
    repo_root().join("target/dev-local-registry/explain-execution-local")
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
    let primitive = installer.install(inspect_source_dir())?;

    let registry = LocalRegistry::load(&registry_root)?;
    let facade = GuildMcpFacade::new(registry, WasmtimeRuntimeAdapter::new()?);
    let inspected = facade.inspect(InspectRequest::new(
        RequestedSkillRef {
            key: SkillKey {
                namespace: "example".into(),
                name: "hello-inspect".into(),
            },
            version_req: VersionRequirement::parse("^0.1")?,
        },
        serde_json::json!({ "name": "Ada" }),
        "tenant-dev",
        "actor-dev",
        CapabilityGrantSet {
            grants: vec![GrantedCapability {
                id: CapabilityId::EmitEvidence,
                access: CapabilityAccess::Write,
                constraints: CapabilityConstraints::EmitEvidence(EmitEvidenceConstraints {
                    max_bytes: Some(65_536),
                    audiences: Some(vec![EvidenceAudience::User]),
                    redactions: Some(vec![RedactionClass::None]),
                }),
            }],
        },
    ))?;

    let explained = installer.install(explain_source_dir())?;
    let registry = LocalRegistry::load(&registry_root)?;
    let facade = GuildMcpFacade::new(registry, WasmtimeRuntimeAdapter::new()?);
    let explanation = facade.inspect(InspectRequest::new(
        RequestedSkillRef {
            key: SkillKey {
                namespace: "example".into(),
                name: "explain-execution".into(),
            },
            version_req: VersionRequirement::parse("^0.1")?,
        },
        serde_json::json!({
            "execution_uri": inspected.structured_content.uri,
            "include_first_evidence": true,
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
                        "guild://objects/sha256/".into(),
                    ]),
                    resource_kinds: Some(vec![ResourceKind::Execution, ResourceKind::Object]),
                }),
            }],
        },
    ))?;

    println!("installed primitive {}", primitive.resolved_ref.digest);
    println!("installed explain {}", explained.resolved_ref.digest);
    println!("target execution URI: {}", inspected.structured_content.uri);
    println!("{}", explanation.summary);
    println!(
        "{}",
        serde_json::to_string_pretty(&explanation.structured_content)?
    );

    Ok(())
}
