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

fn policy_demo_root() -> PathBuf {
    repo_root().join("target/dev-local-registry/inspect-policy-local")
}

fn reset_registry_root(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if path.exists() {
        std::fs::remove_dir_all(path)?;
    }
    Ok(())
}

fn explain_grant() -> GrantedCapability {
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

fn execution_read_grant() -> GrantedCapability {
    GrantedCapability {
        id: CapabilityId::ReadResource,
        access: CapabilityAccess::Read,
        constraints: CapabilityConstraints::ReadResource(ReadResourceConstraints {
            uri_prefixes: Some(vec!["guild://executions/".into()]),
            resource_kinds: Some(vec![ResourceKind::Execution]),
        }),
    }
}

fn explain_execution_request(
    execution_uri: &str,
    tenant_id: &str,
    actor_id: &str,
) -> Result<InspectRequest, Box<dyn std::error::Error>> {
    Ok(InspectRequest::new(
        RequestedSkillRef {
            key: SkillKey {
                namespace: "example".into(),
                name: "explain-execution".into(),
            },
            version_req: VersionRequirement::parse("^0.1")?,
        },
        serde_json::json!({
            "execution_uri": execution_uri,
            "include_first_evidence": false,
        }),
        tenant_id,
        actor_id,
        CapabilityGrantSet {
            grants: vec![explain_grant()],
        },
    ))
}

fn explain_capability_denial_request(
    execution_uri: &str,
    tenant_id: &str,
    actor_id: &str,
) -> Result<InspectRequest, Box<dyn std::error::Error>> {
    Ok(InspectRequest::new(
        RequestedSkillRef {
            key: SkillKey {
                namespace: "example".into(),
                name: "explain-capability-denial".into(),
            },
            version_req: VersionRequirement::parse("^0.1")?,
        },
        serde_json::json!({
            "execution_uri": execution_uri,
        }),
        tenant_id,
        actor_id,
        CapabilityGrantSet {
            grants: vec![execution_read_grant()],
        },
    ))
}

fn diff_execution_authority_request(
    left_execution_uri: &str,
    right_execution_uri: &str,
    tenant_id: &str,
    actor_id: &str,
) -> Result<InspectRequest, Box<dyn std::error::Error>> {
    Ok(InspectRequest::new(
        RequestedSkillRef {
            key: SkillKey {
                namespace: "example".into(),
                name: "diff-execution-authority".into(),
            },
            version_req: VersionRequirement::parse("^0.1")?,
        },
        serde_json::json!({
            "left_execution_uri": left_execution_uri,
            "right_execution_uri": right_execution_uri,
        }),
        tenant_id,
        actor_id,
        CapabilityGrantSet {
            grants: vec![execution_read_grant()],
        },
    ))
}

fn explain_http_authority_request(
    execution_uri: &str,
    candidate_url: &str,
    candidate_method: &str,
    timeout_ms: Option<u64>,
    tenant_id: &str,
    actor_id: &str,
) -> Result<InspectRequest, Box<dyn std::error::Error>> {
    Ok(InspectRequest::new(
        RequestedSkillRef {
            key: SkillKey {
                namespace: "example".into(),
                name: "explain-http-authority".into(),
            },
            version_req: VersionRequirement::parse("^0.1")?,
        },
        serde_json::json!({
            "execution_uri": execution_uri,
            "candidate_request": {
                "url": candidate_url,
                "method": candidate_method,
                "timeout_ms": timeout_ms,
            },
        }),
        tenant_id,
        actor_id,
        CapabilityGrantSet {
            grants: vec![execution_read_grant()],
        },
    ))
}

fn print_pretty_json(value: &impl serde::Serialize) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let registry_root = policy_demo_root();
    reset_registry_root(&registry_root)?;
    let scenario =
        prepare_codex_scenario(&registry_root, CodexScenarioSelection::PolicyDenialDebug)?;

    let facade = GuildMcpFacade::new(
        LocalRegistry::load(&registry_root)?,
        WasmtimeRuntimeAdapter::new()?,
    );

    let denied_execution_uri = scenario
        .subject_execution_uris
        .first()
        .expect("policy-denial scenario prepares a denied execution URI");
    let trusted_execution_uri = scenario
        .comparison_execution_uris
        .first()
        .expect("policy-denial scenario prepares a trusted execution URI");
    let restricted_execution_uri = scenario
        .comparison_execution_uris
        .get(1)
        .expect("policy-denial scenario prepares a restricted execution URI");
    let direct_allowed_url = scenario
        .candidate_urls
        .get(1)
        .expect("policy-denial scenario prepares a direct allowed candidate URL");
    let localhost_denied_url = scenario
        .candidate_urls
        .get(2)
        .expect("policy-denial scenario prepares a denied localhost candidate URL");

    for installed in &scenario.installed_skills {
        println!(
            "installed {}:{} {}",
            installed.namespace, installed.name, installed.digest
        );
    }

    let trusted_record = facade.read_resource(trusted_execution_uri)?;
    println!("trusted execution resource: {}", trusted_record.uri);
    println!("{}", String::from_utf8(trusted_record.bytes)?);

    let restricted_record = facade.read_resource(restricted_execution_uri)?;
    println!("restricted execution resource: {}", restricted_record.uri);
    println!("{}", String::from_utf8(restricted_record.bytes)?);

    let denied_record = facade.read_resource(denied_execution_uri)?;
    println!("denied execution resource: {}", denied_record.uri);
    println!("{}", String::from_utf8(denied_record.bytes)?);

    let explanation = facade.inspect(explain_execution_request(
        denied_execution_uri,
        "tenant-restricted",
        "actor-demo",
    )?)?;
    println!("denied explanation: {}", explanation.summary);
    print_pretty_json(&explanation.structured_content)?;

    let denial_report = facade.inspect(explain_capability_denial_request(
        denied_execution_uri,
        "tenant-restricted",
        "actor-demo",
    )?)?;
    println!("capability denial report: {}", denial_report.summary);
    print_pretty_json(&denial_report.structured_content)?;

    let authority_diff = facade.inspect(diff_execution_authority_request(
        trusted_execution_uri,
        restricted_execution_uri,
        "tenant-restricted",
        "actor-demo",
    )?)?;
    println!("authority diff: {}", authority_diff.summary);
    print_pretty_json(&authority_diff.structured_content)?;

    let http_authority_allowed = facade.inspect(explain_http_authority_request(
        denied_execution_uri,
        direct_allowed_url,
        "get",
        Some(500),
        "tenant-restricted",
        "actor-demo",
    )?)?;
    println!(
        "http authority allowed probe: {}",
        http_authority_allowed.summary
    );
    print_pretty_json(&http_authority_allowed.structured_content)?;

    let http_authority_denied = facade.inspect(explain_http_authority_request(
        denied_execution_uri,
        localhost_denied_url,
        "get",
        Some(500),
        "tenant-restricted",
        "actor-demo",
    )?)?;
    println!(
        "http authority denied probe: {}",
        http_authority_denied.summary
    );
    print_pretty_json(&http_authority_denied.structured_content)?;
    println!("recommended Codex ask: {}", scenario.recommended_codex_ask);

    Ok(())
}
