use std::fs;
use std::path::{Path, PathBuf};

use guild_manifest::SourceSkillManifest;
use guild_mcp::{GuildMcpFacade, InspectRequest};
use guild_registry::{LocalRegistry, LocalSourceInstaller};
use guild_runner::WasmtimeRuntimeAdapter;
use guild_types::{
    CapabilityAccess, CapabilityConstraints, CapabilityGrantSet, CapabilityId,
    CapabilityRequirement, EmitEvidenceConstraints, EvidenceAudience, FilesystemConstraints,
    FilesystemOperation, FilesystemRoot, GrantedCapability, ReadResourceConstraints,
    RedactionClass, RequestedSkillRef, ResourceKind, SkillKey, VersionRequirement,
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

fn explain_source_dir() -> PathBuf {
    repo_root().join("examples/skills/explain-execution")
}

fn wit_dir() -> PathBuf {
    repo_root().join("wit")
}

fn local_registry_root() -> PathBuf {
    repo_root().join("target/dev-local-registry/filesystem-rejection-local")
}

fn reset_registry_root(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}

fn copy_dir_recursive(source: &Path, destination: &Path) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(destination)?;

    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let destination_path = destination.join(entry.file_name());

        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &destination_path)?;
        } else {
            fs::copy(entry.path(), destination_path)?;
        }
    }

    Ok(())
}

fn filesystem_read_constraints() -> FilesystemConstraints {
    FilesystemConstraints {
        preopened_roots: vec![FilesystemRoot {
            name: "workspace".into(),
            guest_path_prefix: "/workspace".into(),
            host_path: "/var/lib/guild/workspace".into(),
            operations: vec![FilesystemOperation::Read],
        }],
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

fn filesystem_read_grant() -> GrantedCapability {
    GrantedCapability {
        id: CapabilityId::Filesystem,
        access: CapabilityAccess::Read,
        constraints: CapabilityConstraints::Filesystem(filesystem_read_constraints()),
    }
}

fn explain_read_grant() -> GrantedCapability {
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

fn write_filesystem_fixture(root: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let workspace_root = root.join("workspace");
    let source_root = workspace_root.join("examples/skills/hello-inspect-filesystem");
    copy_dir_recursive(&example_source_dir(), &source_root)?;
    copy_dir_recursive(&wit_dir(), &workspace_root.join("wit"))?;

    let manifest_path = source_root.join("manifest.json");
    let mut manifest: SourceSkillManifest =
        serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
    manifest.key.name = "hello-inspect-filesystem".into();
    manifest.display_name = "Hello Inspect Filesystem".into();
    manifest.description = "A proof fixture that declares the deferred filesystem contract.".into();
    manifest.capabilities.push(CapabilityRequirement {
        id: CapabilityId::Filesystem,
        access: CapabilityAccess::Read,
        constraints: CapabilityConstraints::Filesystem(filesystem_read_constraints()),
        required: true,
    });
    fs::write(manifest_path, serde_json::to_vec_pretty(&manifest)?)?;

    Ok(source_root)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let registry_root = local_registry_root();
    reset_registry_root(&registry_root)?;

    let fixture_root = registry_root.join("source-fixtures");
    fs::create_dir_all(&fixture_root)?;
    let filesystem_source = write_filesystem_fixture(&fixture_root)?;

    let source_installer = LocalSourceInstaller::new(&registry_root)?;
    let installed_skill = source_installer.install(&filesystem_source)?;
    source_installer.install(explain_source_dir())?;

    let registry = LocalRegistry::load(&registry_root)?;
    let facade = GuildMcpFacade::new(registry, WasmtimeRuntimeAdapter::new()?);
    let denied = facade
        .inspect(InspectRequest::new(
            RequestedSkillRef {
                key: SkillKey {
                    namespace: "example".into(),
                    name: "hello-inspect-filesystem".into(),
                },
                version_req: VersionRequirement::parse("^0.1")?,
            },
            serde_json::json!({ "name": "Ada" }),
            "tenant-dev",
            "actor-dev",
            CapabilityGrantSet {
                grants: vec![emit_evidence_grant(), filesystem_read_grant()],
            },
        ))
        .expect_err("filesystem contract should be rejected before guest start");

    let receipt = denied
        .receipt
        .clone()
        .expect("filesystem rejection returns a persisted receipt");
    println!("installed {}", installed_skill.resolved_ref.digest);
    println!("rejected execution URI: {}", receipt.uri);
    let rejected_resource = facade.read_resource(&receipt.uri)?;
    println!("{}", String::from_utf8(rejected_resource.bytes)?);

    let explained = facade.inspect(InspectRequest::new(
        RequestedSkillRef {
            key: SkillKey {
                namespace: "example".into(),
                name: "explain-execution".into(),
            },
            version_req: VersionRequirement::parse("^0.1")?,
        },
        serde_json::json!({
            "execution_uri": receipt.uri,
            "include_first_evidence": false,
        }),
        "tenant-dev",
        "actor-dev",
        CapabilityGrantSet {
            grants: vec![explain_read_grant()],
        },
    ))?;

    println!("{}", explained.summary);
    println!(
        "{}",
        serde_json::to_string_pretty(&explained.structured_content)?
    );

    Ok(())
}
