use std::path::{Path, PathBuf};

use guild_mcp::{GuildMcpFacade, InspectRequest};
use guild_registry::{LocalRegistry, LocalSourceInstaller};
use guild_runner::WasmtimeRuntimeAdapter;
use guild_types::{
    CapabilityAccess, CapabilityConstraints, CapabilityGrantSet, CapabilityId, GrantedCapability,
    ReadResourceConstraints, RequestedSkillRef, ResourceKind, SkillKey, VersionRequirement,
};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root exists")
}

fn explain_source_dir() -> PathBuf {
    repo_root().join("examples/skills/explain-execution")
}

fn local_registry_root() -> PathBuf {
    repo_root().join("target/dev-local-registry/explain-failure-local")
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
    let explain = installer.install(explain_source_dir())?;

    let registry = LocalRegistry::load(&registry_root)?;
    let facade = GuildMcpFacade::new(registry, WasmtimeRuntimeAdapter::new()?);
    let rejected = facade
        .inspect(InspectRequest::new(
            RequestedSkillRef {
                key: SkillKey {
                    namespace: "example".into(),
                    name: "explain-execution".into(),
                },
                version_req: VersionRequirement::parse("^0.1")?,
            },
            serde_json::json!({
                "execution_uri": "guild://executions/not-used",
                "include_first_evidence": false,
            }),
            "tenant-dev",
            "actor-dev",
            CapabilityGrantSet::default(),
        ))
        .expect_err("missing read-resource grant should persist a rejected execution");

    let receipt = rejected
        .receipt
        .clone()
        .expect("rejected execution returns a persisted receipt");
    println!("installed explain {}", explain.resolved_ref.digest);
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

    println!("{}", explained.summary);
    println!(
        "{}",
        serde_json::to_string_pretty(&explained.structured_content)?
    );

    Ok(())
}
