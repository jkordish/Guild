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
    repo_root().join("target/dev-local-registry/explain-recent-failures-local")
}

fn reset_registry_root(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if path.exists() {
        std::fs::remove_dir_all(path)?;
    }
    Ok(())
}

fn query_grant() -> GrantedCapability {
    GrantedCapability {
        id: CapabilityId::ReadResource,
        access: CapabilityAccess::Read,
        constraints: CapabilityConstraints::ReadResource(ReadResourceConstraints {
            uri_prefixes: Some(vec!["guild://queries/executions/".into()]),
            resource_kinds: Some(vec![ResourceKind::Query]),
        }),
    }
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

fn summarize_query_skill() -> RequestedSkillRef {
    RequestedSkillRef {
        key: SkillKey {
            namespace: "example".into(),
            name: "summarize-execution-query".into(),
        },
        version_req: VersionRequirement::parse("^0.1").expect("example version requirement parses"),
    }
}

fn explain_execution_skill() -> RequestedSkillRef {
    RequestedSkillRef {
        key: SkillKey {
            namespace: "example".into(),
            name: "explain-execution".into(),
        },
        version_req: VersionRequirement::parse("^0.1").expect("example version requirement parses"),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let registry_root = local_registry_root();
    reset_registry_root(&registry_root)?;
    let scenario =
        prepare_codex_scenario(&registry_root, CodexScenarioSelection::RecentFailureTriage)?;

    let facade = GuildMcpFacade::new(
        LocalRegistry::load(&registry_root)?,
        WasmtimeRuntimeAdapter::new()?,
    );

    let query_uri = scenario
        .query_uris
        .first()
        .expect("recent-failure scenario prepares one query URI");
    let query_resource = facade.read_resource(query_uri)?;
    let summary = facade.inspect(InspectRequest::new(
        summarize_query_skill(),
        serde_json::json!({
            "query_uri": query_uri,
        }),
        "tenant-dev",
        "actor-dev",
        CapabilityGrantSet {
            grants: vec![query_grant()],
        },
    ))?;

    let follow_up_execution_uri = scenario
        .subject_execution_uris
        .first()
        .expect("recent-failure scenario prepares one subject execution URI");
    let follow_up = facade.inspect(InspectRequest::new(
        explain_execution_skill(),
        serde_json::json!({
            "execution_uri": follow_up_execution_uri,
            "include_first_evidence": false,
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
    if let Some(success_uri) = scenario.comparison_execution_uris.first() {
        println!("successful execution URI: {success_uri}");
    }
    for (index, subject_uri) in scenario.subject_execution_uris.iter().enumerate() {
        println!("subject execution URI {}: {}", index + 1, subject_uri);
    }
    println!("query resource URI: {}", query_resource.uri);
    println!("{}", String::from_utf8(query_resource.bytes)?);
    println!("{}", summary.summary);
    println!(
        "{}",
        serde_json::to_string_pretty(&summary.structured_content)?
    );
    println!("follow-up execution URI: {follow_up_execution_uri}");
    println!("{}", follow_up.summary);
    println!(
        "{}",
        serde_json::to_string_pretty(&follow_up.structured_content)?
    );
    println!("recommended Codex ask: {}", scenario.recommended_codex_ask);

    Ok(())
}
