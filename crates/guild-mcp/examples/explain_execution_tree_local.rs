use std::path::{Path, PathBuf};

use guild_mcp::codex::{CodexScenarioSelection, prepare_codex_scenario};
use guild_mcp::{GuildMcpFacade, InspectRequest};
use guild_registry::LocalRegistry;
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

fn local_registry_root() -> PathBuf {
    repo_root().join("target/dev-local-registry/explain-execution-tree-local")
}

fn reset_registry_root(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if path.exists() {
        std::fs::remove_dir_all(path)?;
    }
    Ok(())
}

fn execution_and_object_read_grant() -> GrantedCapability {
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

fn explain_tree_skill() -> RequestedSkillRef {
    RequestedSkillRef {
        key: SkillKey {
            namespace: "example".into(),
            name: "explain-execution-tree".into(),
        },
        version_req: VersionRequirement::parse("^0.1").expect("example version requirement parses"),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let registry_root = local_registry_root();
    reset_registry_root(&registry_root)?;
    let scenario = prepare_codex_scenario(&registry_root, CodexScenarioSelection::ExecutionTree)?;

    let facade = GuildMcpFacade::new(
        LocalRegistry::load(&registry_root)?,
        WasmtimeRuntimeAdapter::new()?,
    );
    let root_execution_uri = scenario
        .subject_execution_uris
        .first()
        .expect("execution-tree scenario prepares one root execution URI");
    let explanation = facade.inspect(InspectRequest::new(
        explain_tree_skill(),
        serde_json::json!({
            "execution_uri": root_execution_uri,
            "max_depth": 4,
            "max_nodes": 32,
            "include_evidence_resources": true,
        }),
        "tenant-dev",
        "actor-dev",
        CapabilityGrantSet {
            grants: vec![execution_and_object_read_grant()],
        },
    ))?;

    for installed in &scenario.installed_skills {
        println!(
            "installed {}:{} {}",
            installed.namespace, installed.name, installed.digest
        );
    }
    println!("root execution URI: {root_execution_uri}");
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
    println!("recommended Codex ask: {}", scenario.recommended_codex_ask);

    Ok(())
}
