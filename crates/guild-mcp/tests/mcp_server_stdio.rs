use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::OnceLock;

use guild_mcp::protocol::{
    CallToolResult, ContentBlock, InitializeResult, ListResourceTemplatesResult,
    ListResourcesResult, ListToolsResult, PROTOCOL_VERSION_2025_11_25, ReadResourceResult,
    ResourceContents,
};
use guild_registry::LocalSourceInstaller;
use guild_types::{
    CapabilityAccess, CapabilityConstraints, CapabilityId, EmitEvidenceConstraints,
    EvidenceAudience, ExecutionQueryResult, ExecutionStatus, GrantedCapability,
    PolicyDecisionOutcome, ReadResourceConstraints, RedactionClass, RequestedSkillRef,
    ResourceKind, SkillKey, VersionRequirement,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

#[path = "../../../test-support/guild_inspect_helpers.rs"]
mod guild_inspect_helpers;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn inspect_source_dir() -> PathBuf {
    repo_root().join("examples/skills/hello-inspect")
}

fn explain_source_dir() -> PathBuf {
    repo_root().join("examples/skills/explain-execution")
}

fn explain_tree_source_dir() -> PathBuf {
    repo_root().join("examples/skills/explain-execution-tree")
}

fn prepared_registry_root() -> &'static PathBuf {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();

    ROOT.get_or_init(|| {
        let root = repo_root().join("target/test-install-registry/guild-mcp-server");
        if root.exists() {
            let _ = std::fs::remove_dir_all(&root);
        }

        LocalSourceInstaller::new(&root)
            .unwrap()
            .install(inspect_source_dir())
            .unwrap();
        LocalSourceInstaller::new(&root)
            .unwrap()
            .install(explain_source_dir())
            .unwrap();
        LocalSourceInstaller::new(&root)
            .unwrap()
            .install(explain_tree_source_dir())
            .unwrap();
        root
    })
}

struct McpHarness {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl McpHarness {
    fn spawn() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_guild-mcp-server"))
            .arg("--registry-root")
            .arg(prepared_registry_root())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();

        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());

        Self {
            child,
            stdin,
            stdout,
            next_id: 1,
        }
    }

    fn initialize(&mut self) -> InitializeResult {
        let response = self.request(
            "initialize",
            &json!({
                "protocolVersion": PROTOCOL_VERSION_2025_11_25,
                "capabilities": {},
                "clientInfo": {
                    "name": "guild-mcp-test-client",
                    "version": "0.1.0"
                }
            }),
        );
        let result: InitializeResult =
            serde_json::from_value(response["result"].clone()).expect("initialize result parses");
        self.notify("notifications/initialized", &json!({}));
        result
    }

    fn request(&mut self, method: &str, params: &Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        self.write_message(&request);
        self.read_message()
    }

    fn notify(&mut self, method: &str, params: &Value) {
        let request = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        self.write_message(&request);
    }

    fn write_message(&mut self, request: &Value) {
        serde_json::to_writer(&mut self.stdin, request).unwrap();
        self.stdin.write_all(b"\n").unwrap();
        self.stdin.flush().unwrap();
    }

    fn read_message(&mut self) -> Value {
        let mut line = String::new();
        let read = self.stdout.read_line(&mut line).unwrap();
        assert!(read > 0, "server exited before responding");
        serde_json::from_str(&line).expect("server response is valid JSON")
    }
}

impl Drop for McpHarness {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn parse_result<T: DeserializeOwned>(response: &Value) -> T {
    serde_json::from_value(response["result"].clone()).unwrap_or_else(|error| {
        panic!(
            "response result parses: {error}; full response: {}",
            serde_json::to_string_pretty(response).expect("response serializes for debugging"),
        )
    })
}

fn inspect_request(skill_name: &str, input: &Value, grants: &Value) -> Value {
    let grant_slice = grants.as_array().map_or(&[][..], Vec::as_slice);
    guild_inspect_helpers::example_inspect_request(skill_name, input, grant_slice)
}

fn emit_evidence_grant_json() -> Value {
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

fn read_resource_grant_json(prefixes: &[&str]) -> Value {
    let resource_kinds = prefixes
        .iter()
        .filter_map(|prefix| ResourceKind::from_uri_prefix(prefix))
        .fold(Vec::new(), |mut kinds, kind| {
            if !kinds.contains(&kind) {
                kinds.push(kind);
            }
            kinds
        });

    serde_json::to_value(GrantedCapability {
        id: CapabilityId::ReadResource,
        access: CapabilityAccess::Read,
        constraints: CapabilityConstraints::ReadResource(ReadResourceConstraints {
            uri_prefixes: Some(prefixes.iter().map(|prefix| (*prefix).to_owned()).collect()),
            resource_kinds: Some(resource_kinds),
        }),
    })
    .unwrap()
}

#[test]
fn stdio_server_handshake_returns_honest_capabilities() {
    let mut harness = McpHarness::spawn();
    let initialized = harness.initialize();

    assert_eq!(initialized.protocol_version, PROTOCOL_VERSION_2025_11_25);
    assert!(initialized.capabilities.tools.is_some());
    assert!(initialized.capabilities.resources.is_some());
    let resources = initialized.capabilities.resources.unwrap();
    assert_eq!(resources.subscribe, None);
    assert_eq!(resources.list_changed, None);
    assert_eq!(initialized.server_info.name, "guild-mcp");
}

#[test]
fn initialize_negotiates_to_latest_supported_protocol_version() {
    let mut harness = McpHarness::spawn();
    let response = harness.request(
        "initialize",
        &json!({
            "protocolVersion": "2025-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "guild-mcp-test-client",
                "version": "0.1.0"
            }
        }),
    );
    let initialized: InitializeResult = parse_result(&response);

    assert_eq!(initialized.protocol_version, PROTOCOL_VERSION_2025_11_25);
}

#[test]
fn tools_list_returns_only_guild_inspect() {
    let mut harness = McpHarness::spawn();
    harness.initialize();

    let tools_response = harness.request("tools/list", &json!({}));
    let result: ListToolsResult = parse_result(&tools_response);
    assert_eq!(result.tools.len(), 1);
    assert_eq!(result.tools[0].name, "guild.inspect");
    assert!(result.tools[0].input_schema.is_object());
    assert!(result.tools[0].output_schema.as_ref().unwrap().is_object());
}

#[test]
fn guild_inspect_success_returns_structured_content_text_and_resource_links() {
    let mut harness = McpHarness::spawn();
    harness.initialize();

    let response = harness.request(
        "tools/call",
        &inspect_request(
            "hello-inspect",
            &json!({ "name": "Ada" }),
            &json!([emit_evidence_grant_json()]),
        ),
    );
    let result: CallToolResult = parse_result(&response);
    let record = guild_inspect_helpers::parse_execution_record(&result);

    assert_eq!(result.is_error, None);
    assert_eq!(record.status, ExecutionStatus::Succeeded);
    assert!(matches!(
        result.content.first(),
        Some(ContentBlock::Text(_))
    ));
    assert!(result.content.iter().any(|block| matches!(
        block,
        ContentBlock::ResourceLink(link) if link.uri == record.receipt.uri
    )));
    assert!(result.content.iter().any(|block| matches!(
        block,
        ContentBlock::ResourceLink(link) if link.uri == record.emitted_evidence[0].uri
    )));
}

#[test]
fn guild_inspect_rejection_returns_tool_error_with_persisted_receipt_record() {
    let mut harness = McpHarness::spawn();
    harness.initialize();

    let response = harness.request(
        "tools/call",
        &inspect_request(
            "explain-execution",
            &json!({
                "execution_uri": "guild://executions/not-used",
                "include_first_evidence": false
            }),
            &json!([]),
        ),
    );
    let result: CallToolResult = parse_result(&response);
    let record = guild_inspect_helpers::parse_execution_record(&result);

    assert_eq!(result.is_error, Some(true));
    assert_eq!(record.status, ExecutionStatus::Rejected);
    assert!(record.output.is_none());
    assert_eq!(record.termination.as_ref().unwrap().code, "policy-denied");
    assert_eq!(
        record.policy_decision.outcome,
        PolicyDecisionOutcome::Rejected
    );
    assert!(result.content.iter().any(|block| matches!(
        block,
        ContentBlock::ResourceLink(link) if link.uri == record.receipt.uri
    )));
}

#[test]
fn resources_read_returns_execution_evidence_payload_and_evidence_metadata_content() {
    let mut harness = McpHarness::spawn();
    harness.initialize();

    let response = harness.request(
        "tools/call",
        &inspect_request(
            "hello-inspect",
            &json!({ "name": "Ada" }),
            &json!([emit_evidence_grant_json()]),
        ),
    );
    let result: CallToolResult = parse_result(&response);
    let record = guild_inspect_helpers::parse_execution_record(&result);

    let execution_response =
        harness.request("resources/read", &json!({ "uri": record.receipt.uri }));
    let execution: ReadResourceResult = parse_result(&execution_response);
    let evidence_response = harness.request(
        "resources/read",
        &json!({ "uri": record.emitted_evidence[0].uri }),
    );
    let evidence: ReadResourceResult = parse_result(&evidence_response);
    let metadata_response = harness.request(
        "resources/read",
        &json!({ "uri": format!("{}/metadata", record.emitted_evidence[0].uri) }),
    );
    let metadata: ReadResourceResult = parse_result(&metadata_response);

    assert!(matches!(
        &execution.contents[0],
        ResourceContents::Text(text) if text.mime_type == "application/json"
    ));
    assert!(matches!(
        &evidence.contents[0],
        ResourceContents::Text(text) if text.mime_type == "application/json"
    ));
    assert!(matches!(
        &metadata.contents[0],
        ResourceContents::Text(text) if text.mime_type == "application/json"
    ));
}

#[test]
fn resources_templates_and_recent_execution_list_match_active_resource_model() {
    let mut harness = McpHarness::spawn();
    harness.initialize();

    let response = harness.request(
        "tools/call",
        &inspect_request(
            "hello-inspect",
            &json!({ "name": "Ada" }),
            &json!([emit_evidence_grant_json()]),
        ),
    );
    let result: CallToolResult = parse_result(&response);
    let record = guild_inspect_helpers::parse_execution_record(&result);

    let templates_response = harness.request("resources/templates/list", &json!({}));
    let templates: ListResourceTemplatesResult = parse_result(&templates_response);
    let resources_response = harness.request("resources/list", &json!({}));
    let resources: ListResourcesResult = parse_result(&resources_response);

    assert_eq!(templates.resource_templates.len(), 8);
    assert!(
        templates
            .resource_templates
            .iter()
            .any(|template| template.uri_template == "guild://executions/{execution_id}")
    );
    assert!(templates
        .resource_templates
        .iter()
        .any(|template| template.uri_template == "guild://objects/records/{evidence_record_id}"));
    assert!(templates.resource_templates.iter().any(|template| {
        template.uri_template == "guild://objects/records/{evidence_record_id}/metadata"
    }));
    assert!(
        templates
            .resource_templates
            .iter()
            .any(|template| template.uri_template == "guild://objects/sha256/{digest}")
    );
    assert!(
        templates
            .resource_templates
            .iter()
            .any(|template| template.uri_template == "guild://queries/executions/recent/{limit}")
    );
    assert!(templates.resource_templates.iter().any(|template| {
        template.uri_template == "guild://queries/executions/failures/recent/{limit}"
    }));
    assert!(templates.resource_templates.iter().any(|template| {
        template.uri_template == "guild://queries/executions/by-status/{status}/{limit}"
    }));
    assert!(templates.resource_templates.iter().any(|template| {
        template.uri_template == "guild://queries/executions/by-skill/{namespace}/{name}/{limit}"
    }));
    assert!(
        resources
            .resources
            .iter()
            .any(|resource| resource.uri == record.receipt.uri)
    );

    let query_response = harness.request(
        "resources/read",
        &json!({ "uri": "guild://queries/executions/recent/10" }),
    );
    let query: ReadResourceResult = parse_result(&query_response);
    let query_contents = match &query.contents[0] {
        ResourceContents::Text(text) => text,
        other @ ResourceContents::Blob(_) => {
            panic!("expected text query contents, got {other:?}")
        }
    };
    let query_result: ExecutionQueryResult = serde_json::from_str(&query_contents.text).unwrap();
    assert!(
        query_result
            .results
            .iter()
            .any(|item| item.receipt.uri == record.receipt.uri)
    );
}

#[test]
fn guest_and_mcp_reads_match_for_evidence_metadata_resources() {
    let mut harness = McpHarness::spawn();
    harness.initialize();

    let primitive_result: CallToolResult = parse_result(&harness.request(
        "tools/call",
        &inspect_request(
            "hello-inspect",
            &json!({ "name": "Ada" }),
            &json!([emit_evidence_grant_json()]),
        ),
    ));
    let primitive = guild_inspect_helpers::parse_execution_record(&primitive_result);

    let explain_result: CallToolResult = parse_result(&harness.request(
        "tools/call",
        &inspect_request(
            "explain-execution-tree",
            &json!({
                "execution_uri": primitive.receipt.uri,
                "include_evidence_resources": true,
            }),
            &json!([read_resource_grant_json(&[
                "guild://executions/",
                "guild://objects/records/",
            ])]),
        ),
    ));
    let explained = guild_inspect_helpers::parse_execution_record(&explain_result);
    let explained_output = explained.output.expect("explain tree returns output");
    let descriptor = &explained_output.structured["evidence_summary"]["resource_descriptors"][0];
    let metadata_uri = descriptor["metadata_uri"]
        .as_str()
        .expect("descriptor exposes metadata uri");

    let metadata_response = harness.request("resources/read", &json!({ "uri": metadata_uri }));
    let metadata: ReadResourceResult = parse_result(&metadata_response);
    let metadata_text = match &metadata.contents[0] {
        ResourceContents::Text(text) => text,
        other @ ResourceContents::Blob(_) => {
            panic!("expected text metadata contents, got {other:?}")
        }
    };
    let metadata_json: Value = serde_json::from_str(&metadata_text.text).unwrap();

    assert_eq!(descriptor["uri"], metadata_json["uri"]);
    assert_eq!(descriptor["blob_uri"], metadata_json["blob_uri"]);
    assert_eq!(descriptor["mime_type"], metadata_json["mime_type"]);
    assert_eq!(descriptor["sha256"], metadata_json["sha256"]);
    assert_eq!(
        descriptor["produced_by_execution"],
        metadata_json["produced_by_execution"]
    );
}

#[test]
fn malformed_resource_uri_fails_with_protocol_error_data() {
    let mut harness = McpHarness::spawn();
    harness.initialize();

    let response = harness.request(
        "resources/read",
        &json!({ "uri": "guild://executions/%GG" }),
    );
    assert!(response.get("error").is_some());
    assert_eq!(response["error"]["code"], -32602);
    assert_eq!(
        response["error"]["data"]["guild"]["code"],
        "resource-uri-invalid"
    );
}

#[test]
fn missing_resource_fails_with_server_error_data() {
    let mut harness = McpHarness::spawn();
    harness.initialize();

    let response = harness.request(
        "resources/read",
        &json!({ "uri": "guild://executions/does-not-exist" }),
    );
    assert!(response.get("error").is_some());
    assert_eq!(response["error"]["code"], -32000);
    assert_eq!(
        response["error"]["data"]["guild"]["code"],
        "execution-not-found"
    );
}

#[test]
fn inspect_tool_schema_accepts_requested_skill_refs() {
    let mut harness = McpHarness::spawn();
    harness.initialize();

    let tools_response = harness.request("tools/list", &json!({}));
    let result: ListToolsResult = parse_result(&tools_response);
    let schema = result.tools[0].input_schema.clone();

    let requested_ref = serde_json::to_value(RequestedSkillRef {
        key: SkillKey {
            namespace: "example".into(),
            name: "hello-inspect".into(),
        },
        version_req: VersionRequirement::parse("^0.1").unwrap(),
    })
    .unwrap();

    let skill_schema = schema
        .pointer("/properties/skill")
        .or_else(|| schema.pointer("/schema/properties/skill"))
        .expect("inspect tool schema exposes a skill property");
    assert!(skill_schema.is_object());
    assert_eq!(requested_ref["key"]["name"], "hello-inspect");
}
