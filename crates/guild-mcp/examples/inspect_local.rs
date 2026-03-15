use std::path::{Path, PathBuf};

use guild_mcp::{GuildMcpFacade, InspectRequest};
use guild_registry::{LocalRegistry, LocalSourceInstaller};
use guild_runner::WasmtimeRuntimeAdapter;
use guild_types::{
    CapabilityAccess, CapabilityConstraints, CapabilityGrantSet, CapabilityId,
    EmitEvidenceConstraints, EvidenceAudience, GrantedCapability, RedactionClass,
    RequestedSkillRef, SkillKey, VersionRequirement,
};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root exists")
}

fn example_source_dir() -> PathBuf {
    repo_root().join("examples/skills/hello-inspect")
}

fn local_registry_root() -> PathBuf {
    repo_root().join("target/dev-local-registry/inspect-local")
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
    let installed = installer.install(example_source_dir())?;

    let registry = LocalRegistry::load(&registry_root)?;
    let facade = GuildMcpFacade::new(registry, WasmtimeRuntimeAdapter::new()?);
    let response = facade.inspect(InspectRequest::new(
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

    println!("installed {}", installed.resolved_ref.digest);
    println!("{}", response.summary);
    println!(
        "{}",
        serde_json::to_string_pretty(&response.structured_content)?
    );

    let stored_execution = facade.read_resource(&response.structured_content.receipt.uri)?;
    println!("execution resource: {}", stored_execution.uri);
    println!("{}", String::from_utf8(stored_execution.bytes)?);

    if let Some(evidence) = response.structured_content.emitted_evidence.first() {
        let stored_evidence = facade.read_resource(&evidence.uri)?;
        println!("evidence resource: {}", stored_evidence.uri);
        if stored_evidence.mime_type == "application/json" {
            let json: serde_json::Value = serde_json::from_slice(&stored_evidence.bytes)?;
            println!("{}", serde_json::to_string_pretty(&json)?);
        } else {
            println!("{}", String::from_utf8(stored_evidence.bytes)?);
        }
    }

    Ok(())
}
