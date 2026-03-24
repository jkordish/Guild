use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use guild_mcp::protocol::{
    CallToolResult, ContentBlock, InitializeResult, ListResourceTemplatesResult,
    ListResourcesResult, ListToolsResult, PROTOCOL_VERSION_2025_11_25, ReadResourceResult,
    ResourceContents, ToolTaskSupport,
};
use guild_registry::{LocalRegistry, LocalSourceInstaller};
use guild_types::{
    CapabilityAccess, CapabilityConstraints, CapabilityId, EmitEvidenceConstraints,
    EvidenceAudience, EvidenceRecord, ExecutionQueryResource, ExecutionQueryResult,
    ExecutionStatus, GrantedCapability, InvokeDependencyConstraints, PolicyDecisionOutcome,
    ReadResourceConstraints, RedactionClass, RequestedSkillRef, ResourceKind, SkillKey,
    VersionRequirement,
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

fn composite_source_dir() -> PathBuf {
    repo_root().join("examples/skills/hello-composite")
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
        LocalSourceInstaller::new(&root)
            .unwrap()
            .install(composite_source_dir())
            .unwrap();
        root
    })
}

struct TempFixtureDir {
    path: PathBuf,
}

impl TempFixtureDir {
    fn new(prefix: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("{prefix}-{unique}"));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempFixtureDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct McpHarness {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl McpHarness {
    fn spawn() -> Self {
        Self::spawn_for_root(prepared_registry_root())
    }

    fn spawn_for_root(registry_root: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_guild-mcp-server"))
            .arg("--registry-root")
            .arg(registry_root)
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

fn encode_cursor(list: &str, offset: usize) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(&json!({
            "v": 1,
            "list": list,
            "offset": offset,
        }))
        .unwrap(),
    )
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

fn invoke_hello_grant_json() -> Value {
    serde_json::to_value(GrantedCapability {
        id: CapabilityId::InvokeSkill,
        access: CapabilityAccess::Invoke,
        constraints: CapabilityConstraints::InvokeDependency(InvokeDependencyConstraints {
            aliases: Some(vec!["hello".into()]),
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
    let tools = initialized.capabilities.tools.unwrap();
    assert_eq!(tools.list_changed, Some(false));
    let resources = initialized.capabilities.resources.unwrap();
    assert_eq!(resources.subscribe, None);
    assert_eq!(resources.list_changed, None);
    assert_eq!(initialized.server_info.name, "guild-mcp");
    assert!(
        initialized
            .instructions
            .as_deref()
            .unwrap()
            .contains("tools/list")
    );
    assert!(
        initialized
            .instructions
            .as_deref()
            .unwrap()
            .contains("resources/list")
    );
    assert!(
        initialized
            .instructions
            .as_deref()
            .unwrap()
            .contains("resources/templates/list")
    );
    assert!(
        initialized
            .instructions
            .as_deref()
            .unwrap()
            .contains("resources/read")
    );
    assert!(
        initialized
            .instructions
            .as_deref()
            .unwrap()
            .contains("guild.inspect")
    );
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
fn tools_list_returns_truthful_guild_inspect_annotations() {
    let mut harness = McpHarness::spawn();
    harness.initialize();

    let tools_response = harness.request("tools/list", &json!({}));
    let result: ListToolsResult = parse_result(&tools_response);
    assert_eq!(result.tools.len(), 1);
    assert_eq!(result.tools[0].name, "guild.inspect");
    assert_eq!(result.tools[0].title.as_deref(), Some("Guild Inspect"));
    assert!(result.tools[0].input_schema.is_object());
    assert!(result.tools[0].output_schema.as_ref().unwrap().is_object());
    assert_eq!(result.next_cursor, None);
    assert!(
        result.tools[0]
            .description
            .contains("persist durable execution records")
    );
    assert!(
        result.tools[0]
            .description
            .contains("may also emit evidence")
    );

    let annotations = result.tools[0]
        .annotations
        .as_ref()
        .expect("guild.inspect exposes annotations");
    assert_eq!(annotations.title.as_deref(), Some("Guild Inspect"));
    assert!(!annotations.read_only_hint);
    assert!(!annotations.destructive_hint);
    assert!(!annotations.idempotent_hint);
    assert!(annotations.open_world_hint);

    let execution = result.tools[0]
        .execution
        .as_ref()
        .expect("guild.inspect exposes execution metadata");
    assert_eq!(execution.task_support, Some(ToolTaskSupport::Forbidden));
}

#[test]
fn tools_list_accepts_cursor_and_rejects_malformed_cursor() {
    let mut harness = McpHarness::spawn();
    harness.initialize();

    let no_cursor: ListToolsResult = parse_result(&harness.request("tools/list", &json!({})));
    let first_page_cursor = encode_cursor("tools", 0);
    let with_cursor: ListToolsResult =
        parse_result(&harness.request("tools/list", &json!({ "cursor": first_page_cursor })));

    assert_eq!(with_cursor.tools, no_cursor.tools);
    assert_eq!(with_cursor.next_cursor, None);

    let malformed = harness.request("tools/list", &json!({ "cursor": "not-a-cursor" }));
    assert_eq!(malformed["error"]["code"], -32602);
    assert!(
        malformed["error"]["data"]["detail"]
            .as_str()
            .unwrap()
            .contains("invalid cursor")
    );
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
        ContentBlock::ResourceLink(link)
            if link.uri == record.receipt.uri
                && link.description.as_deref().unwrap().contains("example/hello-inspect@0.1.0")
                && link.description.as_deref().unwrap().contains("succeeded")
    )));
    assert!(result.content.iter().any(|block| matches!(
        block,
        ContentBlock::ResourceLink(link)
            if link.uri == record.emitted_evidence[0].uri
                && link.title.as_deref().unwrap().contains("hello-inspect snapshot")
                && link
                    .description
                    .as_deref()
                    .unwrap()
                    .contains("Read the metadata URI first")
    )));
    assert!(result.content.iter().any(|block| matches!(
        block,
        ContentBlock::ResourceLink(link)
            if link.uri == format!("{}/metadata", record.emitted_evidence[0].uri)
                && link
                    .description
                    .as_deref()
                    .unwrap()
                    .contains("user audience")
                && link
                    .description
                    .as_deref()
                    .unwrap()
                    .contains("none redaction")
                && link
                    .description
                    .as_deref()
                    .unwrap()
                    .contains(&record.receipt.uri)
    )));
}

#[test]
fn guild_inspect_composite_surfaces_child_execution_resource_links() {
    let mut harness = McpHarness::spawn();
    harness.initialize();

    let response = harness.request(
        "tools/call",
        &inspect_request(
            "hello-composite",
            &json!({ "name": "Ada" }),
            &json!([invoke_hello_grant_json(), emit_evidence_grant_json()]),
        ),
    );
    let result: CallToolResult = parse_result(&response);
    let record = guild_inspect_helpers::parse_execution_record(&result);

    assert_eq!(result.is_error, None);
    assert_eq!(record.status, ExecutionStatus::Succeeded);
    assert_eq!(record.child_executions.len(), 1);
    assert!(result.content.iter().any(|block| matches!(
        block,
        ContentBlock::ResourceLink(link)
            if link.uri == record.child_executions[0].uri
                && link.title.as_deref().unwrap().contains(&record.child_executions[0].alias)
                && link
                    .description
                    .as_deref()
                    .unwrap()
                    .contains("example/hello-inspect@0.1.0")
                && link
                    .description
                    .as_deref()
                    .unwrap()
                    .contains("succeeded")
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
#[allow(clippy::too_many_lines)]
fn resources_templates_pagination_and_resources_list_match_active_resource_model() {
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

    let templates_first: ListResourceTemplatesResult =
        parse_result(&harness.request("resources/templates/list", &json!({})));
    assert_eq!(templates_first.resource_templates.len(), 4);
    let next_cursor = templates_first
        .next_cursor
        .clone()
        .expect("resource templates paginate");
    let templates_second: ListResourceTemplatesResult = parse_result(&harness.request(
        "resources/templates/list",
        &json!({ "cursor": next_cursor }),
    ));
    assert_eq!(templates_second.resource_templates.len(), 4);
    assert_eq!(templates_second.next_cursor, None);
    let templates = templates_first
        .resource_templates
        .into_iter()
        .chain(templates_second.resource_templates)
        .collect::<Vec<_>>();
    let resources_response = harness.request("resources/list", &json!({}));
    let resources: ListResourcesResult = parse_result(&resources_response);

    assert_eq!(templates.len(), 8);
    assert!(
        templates
            .windows(2)
            .all(|pair| pair[0].uri_template <= pair[1].uri_template)
    );
    assert!(
        templates
            .iter()
            .any(|template| template.uri_template == "guild://executions/{execution_id}")
    );
    assert!(templates
        .iter()
        .any(|template| template.uri_template == "guild://objects/records/{evidence_record_id}"));
    let payload_template = templates
        .iter()
        .find(|template| template.uri_template == "guild://objects/records/{evidence_record_id}")
        .unwrap();
    assert_eq!(
        payload_template.title.as_deref(),
        Some("Guild Evidence Payload")
    );
    assert!(
        payload_template
            .description
            .as_deref()
            .unwrap()
            .contains("prefer the metadata URI first")
    );
    assert!(templates.iter().any(|template| {
        template.uri_template == "guild://objects/records/{evidence_record_id}/metadata"
    }));
    let metadata_template = templates
        .iter()
        .find(|template| {
            template.uri_template == "guild://objects/records/{evidence_record_id}/metadata"
        })
        .unwrap();
    assert!(
        metadata_template
            .description
            .as_deref()
            .unwrap()
            .contains("before opening the payload URI")
    );
    assert!(
        templates
            .iter()
            .any(|template| template.uri_template == "guild://objects/sha256/{digest}")
    );
    let blob_template = templates
        .iter()
        .find(|template| template.uri_template == "guild://objects/sha256/{digest}")
        .unwrap();
    assert!(
        blob_template
            .description
            .as_deref()
            .unwrap()
            .contains("explicitly need the blob URI")
    );
    assert!(
        templates
            .iter()
            .any(|template| template.uri_template == "guild://queries/executions/recent/{limit}")
    );
    assert!(templates.iter().any(|template| {
        template.uri_template == "guild://queries/executions/failures/recent/{limit}"
    }));
    assert!(templates.iter().any(|template| {
        template.uri_template == "guild://queries/executions/by-status/{status}/{limit}"
    }));
    assert!(templates.iter().any(|template| {
        template.uri_template == "guild://queries/executions/by-skill/{namespace}/{name}/{limit}"
    }));
    assert!(
        resources
            .resources
            .iter()
            .any(|resource| resource.uri == record.receipt.uri)
    );
    let execution_resource = resources
        .resources
        .iter()
        .find(|resource| resource.uri == record.receipt.uri)
        .unwrap();
    assert!(
        execution_resource
            .description
            .as_deref()
            .unwrap()
            .contains("example/hello-inspect@0.1.0")
    );
    assert!(
        execution_resource
            .description
            .as_deref()
            .unwrap()
            .contains("succeeded")
    );
    assert_eq!(
        resources.resources[0].uri,
        ExecutionQueryResource::Recent { limit: 10 }.canonical_uri()
    );
    assert_eq!(
        resources.resources[1].uri,
        ExecutionQueryResource::FailuresRecent { limit: 10 }.canonical_uri()
    );
    let evidence_metadata_uri = format!("{}/metadata", record.emitted_evidence[0].uri);
    assert!(
        resources
            .resources
            .iter()
            .any(|resource| resource.uri == evidence_metadata_uri)
    );
    let evidence_metadata_resource = resources
        .resources
        .iter()
        .find(|resource| resource.uri == evidence_metadata_uri)
        .unwrap();
    assert!(
        evidence_metadata_resource
            .description
            .as_deref()
            .unwrap()
            .contains("user audience")
    );
    assert!(
        evidence_metadata_resource
            .description
            .as_deref()
            .unwrap()
            .contains("none redaction")
    );
    assert_eq!(
        evidence_metadata_resource.mime_type.as_deref(),
        Some("application/json")
    );
    assert!(evidence_metadata_resource.size.unwrap() > 0);

    let wrong_cursor = harness.request(
        "resources/templates/list",
        &json!({ "cursor": encode_cursor("tools", 0) }),
    );
    assert_eq!(wrong_cursor["error"]["code"], -32602);
    assert!(
        wrong_cursor["error"]["data"]["detail"]
            .as_str()
            .unwrap()
            .contains("cursor was issued for")
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

    let metadata_response =
        harness.request("resources/read", &json!({ "uri": evidence_metadata_uri }));
    let metadata: ReadResourceResult = parse_result(&metadata_response);
    let metadata_contents = match &metadata.contents[0] {
        ResourceContents::Text(text) => text,
        other @ ResourceContents::Blob(_) => {
            panic!("expected text metadata contents, got {other:?}")
        }
    };
    let evidence_record: EvidenceRecord = serde_json::from_str(&metadata_contents.text).unwrap();
    assert_eq!(evidence_record.uri, record.emitted_evidence[0].uri);
    assert_eq!(
        evidence_record.blob_uri,
        record.emitted_evidence[0].blob_uri
    );
}

#[test]
fn resources_list_cursor_pagination_preserves_bounded_recent_view() {
    const PAGINATION_FIXTURE_EXECUTION_COUNT: usize = 13;

    let temp = TempFixtureDir::new("guild-mcp-server-pagination");
    let registry_root = temp.path().join("registry");

    LocalSourceInstaller::new(&registry_root)
        .unwrap()
        .install(inspect_source_dir())
        .unwrap();

    let mut harness = McpHarness::spawn_for_root(&registry_root);
    harness.initialize();

    for index in 0..PAGINATION_FIXTURE_EXECUTION_COUNT {
        let result: CallToolResult = parse_result(&harness.request(
            "tools/call",
            &inspect_request(
                "hello-inspect",
                &json!({ "name": format!("Ada-{index}") }),
                &json!([emit_evidence_grant_json()]),
            ),
        ));
        let record = guild_inspect_helpers::parse_execution_record(&result);
        assert_eq!(record.status, ExecutionStatus::Succeeded);
    }

    let first_response = harness.request("resources/list", &json!({}));
    let first_page: ListResourcesResult = parse_result(&first_response);
    assert_eq!(first_page.resources.len(), 25);
    let next_cursor = first_page
        .next_cursor
        .clone()
        .expect("bounded recent view spills onto a second page");

    let second_response = harness.request("resources/list", &json!({ "cursor": next_cursor }));
    let second_page: ListResourcesResult = parse_result(&second_response);
    assert_eq!(second_page.resources.len(), 3);
    assert_eq!(second_page.next_cursor, None);

    let repeated_first: ListResourcesResult =
        parse_result(&harness.request("resources/list", &json!({})));
    assert_eq!(repeated_first.resources, first_page.resources);
    assert_eq!(repeated_first.next_cursor, first_page.next_cursor);

    let repeated_second: ListResourcesResult = parse_result(&harness.request(
        "resources/list",
        &json!({ "cursor": first_page.next_cursor.clone().unwrap() }),
    ));
    assert_eq!(repeated_second.resources, second_page.resources);
    assert_eq!(repeated_second.next_cursor, second_page.next_cursor);

    let expected_uris = LocalRegistry::load(&registry_root)
        .unwrap()
        .list_recent_execution_records(20)
        .unwrap()
        .into_iter()
        .map(|record| record.receipt.uri)
        .collect::<Vec<_>>();
    let expected_evidence_metadata_uris = LocalRegistry::load(&registry_root)
        .unwrap()
        .list_recent_evidence_records(20)
        .unwrap()
        .into_iter()
        .map(|record| format!("{}/metadata", record.uri))
        .collect::<Vec<_>>();
    let expected_uris = [
        vec![
            ExecutionQueryResource::Recent { limit: 10 }.canonical_uri(),
            ExecutionQueryResource::FailuresRecent { limit: 10 }.canonical_uri(),
        ],
        expected_uris,
        expected_evidence_metadata_uris,
    ]
    .concat();
    let actual_uris = first_page
        .resources
        .iter()
        .chain(second_page.resources.iter())
        .map(|resource| resource.uri.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        actual_uris[0],
        ExecutionQueryResource::Recent { limit: 10 }.canonical_uri()
    );
    assert_eq!(
        actual_uris[1],
        ExecutionQueryResource::FailuresRecent { limit: 10 }.canonical_uri()
    );
    assert_eq!(actual_uris, expected_uris);
    assert_eq!(
        actual_uris.len(),
        2 + (PAGINATION_FIXTURE_EXECUTION_COUNT * 2)
    );

    let wrong_cursor = harness.request(
        "resources/list",
        &json!({ "cursor": encode_cursor("resource-templates", 0) }),
    );
    assert_eq!(wrong_cursor["error"]["code"], -32602);
    assert!(
        wrong_cursor["error"]["data"]["detail"]
            .as_str()
            .unwrap()
            .contains("cursor was issued for")
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
