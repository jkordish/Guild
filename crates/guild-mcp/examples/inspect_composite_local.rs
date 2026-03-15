use std::path::{Path, PathBuf};

use guild_mcp::{GuildMcpFacade, InspectRequest};
use guild_registry::{LocalRegistry, LocalSourceInstaller};
use guild_runner::WasmtimeRuntimeAdapter;
use guild_types::{
    CapabilityAccess, CapabilityConstraints, CapabilityGrantSet, CapabilityId,
    EmitEvidenceConstraints, EvidenceAudience, ExecutionRecord, GrantedCapability,
    InvokeDependencyConstraints, RedactionClass, RequestedSkillRef, SkillKey, VersionRequirement,
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

fn local_registry_root() -> PathBuf {
    repo_root().join("target/dev-local-registry/inspect-composite-local")
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
    let response = facade.inspect(InspectRequest::new(
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

    println!("installed primitive {}", primitive.resolved_ref.digest);
    println!("installed composite {}", composite.resolved_ref.digest);
    println!("{}", response.summary);
    println!(
        "{}",
        serde_json::to_string_pretty(&response.structured_content)?
    );

    let parent_resource = facade.read_resource(&response.structured_content.uri)?;
    println!("parent execution resource: {}", parent_resource.uri);
    println!("{}", String::from_utf8(parent_resource.bytes)?);

    if let Some(child_link) = response.structured_content.child_executions.first() {
        let child_resource = facade.read_resource(&child_link.uri)?;
        let child_record: ExecutionRecord = serde_json::from_slice(&child_resource.bytes)?;
        println!("child execution resource: {}", child_resource.uri);
        println!("{}", serde_json::to_string_pretty(&child_record)?);

        if let Some(evidence) = child_record.emitted_evidence.first() {
            let child_evidence = facade.read_resource(&evidence.uri)?;
            println!("child evidence resource: {}", child_evidence.uri);
            if child_evidence.mime_type == "application/json" {
                let json: serde_json::Value = serde_json::from_slice(&child_evidence.bytes)?;
                println!("{}", serde_json::to_string_pretty(&json)?);
            } else {
                println!("{}", String::from_utf8(child_evidence.bytes)?);
            }
        }
    }

    Ok(())
}
