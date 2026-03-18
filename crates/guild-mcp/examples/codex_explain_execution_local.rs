use std::path::{Path, PathBuf};

use guild_mcp::codex::{bootstrap_codex_registry, codex_server_config};
use guild_mcp::protocol::{CallToolResult, ReadResourceResult};
use guild_types::{
    CapabilityAccess, CapabilityConstraints, CapabilityId, EmitEvidenceConstraints,
    EvidenceAudience, GrantedCapability, ReadResourceConstraints, RedactionClass, ResourceKind,
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
    repo_root().join("target/dev-local-registry/codex-explain-execution-local")
}

fn emit_evidence_grant() -> Value {
    serde_json::to_value(GrantedCapability {
        id: CapabilityId::EmitEvidence,
        access: CapabilityAccess::Write,
        constraints: CapabilityConstraints::EmitEvidence(EmitEvidenceConstraints {
            max_bytes: Some(65_536),
            audiences: Some(vec![EvidenceAudience::User]),
            redactions: Some(vec![RedactionClass::None]),
        }),
    })
    .expect("grant serializes")
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

    let initialized = client.initialize("guild-codex-explain-execution")?;
    println!(
        "initialized {} over {}",
        initialized.server_info.name, initialized.protocol_version
    );

    let hello_response: CallToolResult = mcp_stdio_client::parse_result(&client.request(
        "tools/call",
        &guild_inspect_helpers::example_inspect_request(
            "hello-inspect",
            &json!({ "name": "Ada" }),
            &[emit_evidence_grant()],
        ),
    )?)?;
    let hello_record = guild_inspect_helpers::parse_execution_record(&hello_response);

    let explain_response: CallToolResult = mcp_stdio_client::parse_result(&client.request(
        "tools/call",
        &guild_inspect_helpers::example_inspect_request(
            "explain-execution",
            &json!({
                "execution_uri": hello_record.receipt.uri,
                "include_first_evidence": true,
            }),
            &[execution_and_object_read_grant()],
        ),
    )?)?;
    let explain_record = guild_inspect_helpers::parse_execution_record(&explain_response);

    let target_resource: ReadResourceResult = mcp_stdio_client::parse_result(&client.request(
        "resources/read",
        &json!({ "uri": hello_record.receipt.uri }),
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
    println!("target execution uri: {}", hello_record.receipt.uri);
    println!("explanation execution uri: {}", explain_record.receipt.uri);
    println!(
        "target resource contents: {} item(s)",
        target_resource.contents.len()
    );
    println!(
        "explanation resource contents: {} item(s)",
        explanation_resource.contents.len()
    );
    println!("{}", explain_record.output.expect("output present").summary);

    Ok(())
}
