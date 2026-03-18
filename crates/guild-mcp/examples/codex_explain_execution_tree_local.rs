use std::path::{Path, PathBuf};

use guild_mcp::codex::{bootstrap_codex_registry, codex_server_config};
use guild_mcp::protocol::{CallToolResult, ReadResourceResult};
use guild_types::{
    CapabilityAccess, CapabilityConstraints, CapabilityId, EmitEvidenceConstraints,
    EvidenceAudience, GrantedCapability, InvokeDependencyConstraints, ReadResourceConstraints,
    RedactionClass, ResourceKind,
};
use serde_json::{Value, json};

#[path = "../../../test-support/guild_inspect_helpers.rs"]
mod guild_inspect_helpers;
#[path = "../../../test-support/mcp_stdio_client.rs"]
mod mcp_stdio_client;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root exists")
}

fn local_registry_root() -> PathBuf {
    repo_root().join("target/dev-local-registry/codex-explain-execution-tree-local")
}

fn invoke_and_evidence_grants() -> Vec<Value> {
    vec![
        serde_json::to_value(GrantedCapability {
            id: CapabilityId::InvokeSkill,
            access: CapabilityAccess::Invoke,
            constraints: CapabilityConstraints::InvokeDependency(InvokeDependencyConstraints {
                aliases: Some(vec!["hello".into()]),
            }),
        })
        .expect("grant serializes"),
        serde_json::to_value(GrantedCapability {
            id: CapabilityId::EmitEvidence,
            access: CapabilityAccess::Write,
            constraints: CapabilityConstraints::EmitEvidence(EmitEvidenceConstraints {
                max_bytes: Some(65_536),
                audiences: Some(vec![EvidenceAudience::User]),
                redactions: Some(vec![RedactionClass::None]),
            }),
        })
        .expect("grant serializes"),
    ]
}

fn execution_and_object_read_grant() -> Value {
    serde_json::to_value(GrantedCapability {
        id: CapabilityId::ReadResource,
        access: CapabilityAccess::Read,
        constraints: CapabilityConstraints::ReadResource(ReadResourceConstraints {
            uri_prefixes: Some(vec![
                "guild://executions/".into(),
                "guild://objects/records/".into(),
            ]),
            resource_kinds: Some(vec![ResourceKind::Execution, ResourceKind::Object]),
        }),
    })
    .expect("grant serializes")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bootstrap = bootstrap_codex_registry(local_registry_root(), true)?;
    let config = codex_server_config(&bootstrap.registry_root, "guild-local");
    let mut client = mcp_stdio_client::McpStdioClient::spawn(
        &config.command,
        &config.args,
        &config.cwd,
        &config.env,
    )?;

    let initialized = client.initialize("guild-codex-explain-execution-tree")?;
    println!(
        "initialized {} over {}",
        initialized.server_info.name, initialized.protocol_version
    );

    let composite_response: CallToolResult = mcp_stdio_client::parse_result(&client.request(
        "tools/call",
        &guild_inspect_helpers::example_inspect_request(
            "hello-composite",
            &json!({ "name": "Ada" }),
            &invoke_and_evidence_grants(),
        ),
    )?)?;
    let composite_record = guild_inspect_helpers::parse_execution_record(&composite_response);

    let explain_response: CallToolResult = mcp_stdio_client::parse_result(&client.request(
        "tools/call",
        &guild_inspect_helpers::example_inspect_request(
            "explain-execution-tree",
            &json!({
                "execution_uri": composite_record.receipt.uri,
                "max_depth": 4,
                "max_nodes": 32,
                "include_evidence_resources": true,
            }),
            &[execution_and_object_read_grant()],
        ),
    )?)?;
    let explain_record = guild_inspect_helpers::parse_execution_record(&explain_response);

    let root_resource: ReadResourceResult = mcp_stdio_client::parse_result(&client.request(
        "resources/read",
        &json!({ "uri": composite_record.receipt.uri }),
    )?)?;
    let explanation_resource: ReadResourceResult =
        mcp_stdio_client::parse_result(&client.request(
            "resources/read",
            &json!({ "uri": explain_record.receipt.uri }),
        )?)?;

    println!(
        "bootstrap registry root: {}",
        bootstrap.registry_root.display()
    );
    println!("root execution uri: {}", composite_record.receipt.uri);
    println!(
        "tree explanation execution uri: {}",
        explain_record.receipt.uri
    );
    println!(
        "root resource contents: {} item(s)",
        root_resource.contents.len()
    );
    println!(
        "tree explanation resource contents: {} item(s)",
        explanation_resource.contents.len()
    );
    println!("{}", explain_record.output.expect("output present").summary);

    Ok(())
}
