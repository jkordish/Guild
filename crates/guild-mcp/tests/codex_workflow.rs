use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use guild_mcp::codex::{
    CodexBootstrapOutput, CodexServerConfig, bootstrap_codex_registry, codex_server_config,
};
use guild_mcp::protocol::{CallToolResult, ReadResourceResult};
use guild_types::{
    CapabilityAccess, CapabilityConstraints, CapabilityId, EmitEvidenceConstraints,
    EvidenceAudience, ExecutionRecord, ExecutionStatus, GrantedCapability,
    InvokeDependencyConstraints, ReadResourceConstraints, RedactionClass, ResourceKind,
};
use serde_json::{Value, json};

#[path = "../../../test-support/mcp_stdio_client.rs"]
mod mcp_stdio_client;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

struct TempRegistryRoot {
    path: PathBuf,
}

impl TempRegistryRoot {
    fn new(label: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = repo_root().join(format!(
            "target/test-install-registry/{label}-{unique}-{}",
            std::process::id()
        ));
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempRegistryRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
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
    .unwrap()
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
    .unwrap()
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
        .unwrap(),
        emit_evidence_grant(),
    ]
}

fn inspect_request(skill_name: &str, input: Value, grants: Vec<Value>) -> Value {
    json!({
        "name": "guild.inspect",
        "arguments": {
            "skill": {
                "key": {
                    "namespace": "example",
                    "name": skill_name,
                },
                "version_req": "^0.1",
            },
            "input": input,
            "grants": {
                "grants": grants,
            }
        }
    })
}

fn parse_record(result: &CallToolResult) -> ExecutionRecord {
    serde_json::from_value(result.structured_content.clone().unwrap()).unwrap()
}

fn spawn_documented_server(
    config: &CodexServerConfig,
) -> Result<mcp_stdio_client::McpStdioClient, Box<dyn std::error::Error>> {
    mcp_stdio_client::McpStdioClient::spawn(&config.command, &config.args, &config.cwd, &config.env)
}

fn spawn_built_server(
    registry_root: &Path,
) -> Result<mcp_stdio_client::McpStdioClient, Box<dyn std::error::Error>> {
    let args = vec![
        "--registry-root".into(),
        registry_root.to_string_lossy().into_owned(),
    ];
    mcp_stdio_client::McpStdioClient::spawn(
        env!("CARGO_BIN_EXE_guild-mcp-server"),
        &args,
        &repo_root(),
        &BTreeMap::new(),
    )
}

#[test]
fn guild_codex_bootstrap_and_config_json_match_documented_stdio_shape() {
    let temp_root = TempRegistryRoot::new("guild-codex-bootstrap");
    let output = Command::new("cargo")
        .current_dir(repo_root())
        .args([
            "run",
            "-q",
            "-p",
            "guild-mcp",
            "--bin",
            "guild-codex",
            "--",
            "bootstrap",
            "--registry-root",
        ])
        .arg(temp_root.path())
        .args(["--reset", "--json"])
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");

    let payload: CodexBootstrapOutput =
        serde_json::from_slice(&output.stdout).expect("bootstrap JSON parses");
    let skill_names = payload
        .bootstrap
        .skills
        .iter()
        .map(|skill| skill.name.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        skill_names,
        vec![
            "hello-inspect",
            "hello-composite",
            "explain-execution",
            "explain-execution-tree",
        ]
    );
    assert_eq!(payload.config.command, "cargo");
    assert_eq!(
        payload.config.args,
        vec![
            "run",
            "-q",
            "-p",
            "guild-mcp",
            "--bin",
            "guild-mcp-server",
            "--"
        ]
    );
    assert_eq!(
        payload.config.env.get("GUILD_REGISTRY_ROOT"),
        Some(
            &payload
                .bootstrap
                .registry_root
                .to_string_lossy()
                .into_owned()
        )
    );
    assert!(
        payload
            .recommended_proof_commands
            .iter()
            .any(|command| command.ends_with("codex_explain_execution_local"))
    );
    assert!(
        payload
            .config
            .codex_mcp_add_command()
            .contains("codex mcp add")
    );
    assert!(payload.config.config_toml().contains("[mcp_servers."));
}

#[test]
fn documented_config_can_launch_the_stdio_server() {
    let temp_root = TempRegistryRoot::new("guild-codex-startup");
    let bootstrap = bootstrap_codex_registry(temp_root.path(), true).unwrap();
    let config = codex_server_config(&bootstrap.registry_root, "guild-local");
    let mut client = spawn_documented_server(&config).unwrap();
    let initialized = client.initialize("guild-codex-startup-smoke").unwrap();

    assert_eq!(initialized.server_info.name, "guild-mcp");
}

#[test]
fn codex_explain_execution_flow_over_mcp_server_produces_resources() {
    let temp_root = TempRegistryRoot::new("guild-codex-explain");
    let bootstrap = bootstrap_codex_registry(temp_root.path(), true).unwrap();
    let mut client = spawn_built_server(&bootstrap.registry_root).unwrap();
    client.initialize("guild-codex-explain-smoke").unwrap();

    let hello_response: CallToolResult = mcp_stdio_client::parse_result(
        &client
            .request(
                "tools/call",
                &inspect_request(
                    "hello-inspect",
                    json!({ "name": "Ada" }),
                    vec![emit_evidence_grant()],
                ),
            )
            .unwrap(),
    )
    .unwrap();
    let hello_record = parse_record(&hello_response);

    let explain_response: CallToolResult = mcp_stdio_client::parse_result(
        &client
            .request(
                "tools/call",
                &inspect_request(
                    "explain-execution",
                    json!({
                        "execution_uri": hello_record.receipt.uri,
                        "include_first_evidence": true,
                    }),
                    vec![execution_and_object_read_grant()],
                ),
            )
            .unwrap(),
    )
    .unwrap();
    let explain_record = parse_record(&explain_response);

    let target_resource: ReadResourceResult = mcp_stdio_client::parse_result(
        &client
            .request(
                "resources/read",
                &json!({ "uri": hello_record.receipt.uri }),
            )
            .unwrap(),
    )
    .unwrap();
    let explanation_resource: ReadResourceResult = mcp_stdio_client::parse_result(
        &client
            .request(
                "resources/read",
                &json!({ "uri": explain_record.receipt.uri }),
            )
            .unwrap(),
    )
    .unwrap();

    assert_eq!(hello_record.status, ExecutionStatus::Succeeded);
    assert_eq!(explain_record.status, ExecutionStatus::Succeeded);
    assert!(!hello_record.emitted_evidence.is_empty());
    assert_eq!(target_resource.contents.len(), 1);
    assert_eq!(explanation_resource.contents.len(), 1);
}

#[test]
fn codex_explain_execution_tree_flow_over_mcp_server_produces_resources() {
    let temp_root = TempRegistryRoot::new("guild-codex-tree");
    let bootstrap = bootstrap_codex_registry(temp_root.path(), true).unwrap();
    let mut client = spawn_built_server(&bootstrap.registry_root).unwrap();
    client.initialize("guild-codex-tree-smoke").unwrap();

    let composite_response: CallToolResult = mcp_stdio_client::parse_result(
        &client
            .request(
                "tools/call",
                &inspect_request(
                    "hello-composite",
                    json!({ "name": "Ada" }),
                    invoke_and_evidence_grants(),
                ),
            )
            .unwrap(),
    )
    .unwrap();
    let composite_record = parse_record(&composite_response);

    let tree_response: CallToolResult = mcp_stdio_client::parse_result(
        &client
            .request(
                "tools/call",
                &inspect_request(
                    "explain-execution-tree",
                    json!({
                        "execution_uri": composite_record.receipt.uri,
                        "max_depth": 4,
                        "max_nodes": 32,
                        "include_evidence_resources": true,
                    }),
                    vec![execution_and_object_read_grant()],
                ),
            )
            .unwrap(),
    )
    .unwrap();
    let tree_record = parse_record(&tree_response);

    let root_resource: ReadResourceResult = mcp_stdio_client::parse_result(
        &client
            .request(
                "resources/read",
                &json!({ "uri": composite_record.receipt.uri }),
            )
            .unwrap(),
    )
    .unwrap();
    let tree_resource: ReadResourceResult = mcp_stdio_client::parse_result(
        &client
            .request("resources/read", &json!({ "uri": tree_record.receipt.uri }))
            .unwrap(),
    )
    .unwrap();

    assert_eq!(composite_record.status, ExecutionStatus::Succeeded);
    assert_eq!(tree_record.status, ExecutionStatus::Succeeded);
    assert!(!composite_record.child_executions.is_empty());
    assert_eq!(root_resource.contents.len(), 1);
    assert_eq!(tree_resource.contents.len(), 1);
}
