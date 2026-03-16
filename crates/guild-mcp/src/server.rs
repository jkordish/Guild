use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use base64::Engine as _;
use guild_registry::LocalRegistry;
use guild_runner::WasmtimeRuntimeAdapter;
use guild_types::{ExecutionRecord, ResourceReadResult};
use schemars::{schema_for, JsonSchema};
use serde::Serialize;
use serde_json::{json, Value};

use crate::protocol::{
    BlobResourceContents, CallToolParams, CallToolResult, ContentBlock, Implementation,
    InitializeParams, InitializeResult, JsonRpcError, JsonRpcErrorResponse, JsonRpcRequest,
    JsonRpcSuccessResponse, ListParams, ListResourceTemplatesResult, ListResourcesResult,
    ListToolsResult, ReadResourceParams, ReadResourceResult, Resource, ResourceContents,
    ResourceLink, ResourceTemplate, ResourcesCapabilities, ServerCapabilities, TextContent,
    TextResourceContents, Tool, ToolAnnotations, ToolsCapabilities, ERROR_INVALID_PARAMS,
    ERROR_INVALID_REQUEST, ERROR_METHOD_NOT_FOUND, ERROR_PARSE, ERROR_SERVER,
    ERROR_SERVER_NOT_INITIALIZED, JSONRPC_VERSION, METHOD_INITIALIZE, METHOD_PING,
    METHOD_RESOURCES_LIST, METHOD_RESOURCES_READ, METHOD_RESOURCE_TEMPLATES_LIST,
    METHOD_TOOLS_CALL, METHOD_TOOLS_LIST, NOTIFICATION_INITIALIZED, SUPPORTED_PROTOCOL_VERSIONS,
};
use crate::{GuildMcpFacade, InspectToolRequest, McpError, INSPECT_TOOL, SERVER_NAME};

const DEFAULT_RECENT_EXECUTION_LIMIT: usize = 50;

#[derive(Debug)]
pub enum ServerStartupError {
    MissingRegistryRoot,
    Registry(McpError),
    Runtime(McpError),
}

impl std::fmt::Display for ServerStartupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingRegistryRoot => write!(
                f,
                "missing registry root; pass --registry-root <path> or set GUILD_REGISTRY_ROOT"
            ),
            Self::Registry(error) | Self::Runtime(error) => std::fmt::Display::fmt(error, f),
        }
    }
}

impl std::error::Error for ServerStartupError {}

#[derive(Debug, Clone)]
enum SessionState {
    PreInitialize,
    WaitingForInitialized { protocol_version: String },
    Ready { protocol_version: String },
}

pub struct GuildMcpServer {
    registry: LocalRegistry,
    facade: GuildMcpFacade<LocalRegistry, WasmtimeRuntimeAdapter>,
    state: SessionState,
}

impl GuildMcpServer {
    /// Load the Guild MCP stdio server against a local registry root.
    ///
    /// # Errors
    ///
    /// Returns an error if the registry or runtime adapter cannot be initialized.
    pub fn load(registry_root: impl AsRef<Path>) -> Result<Self, ServerStartupError> {
        let registry = LocalRegistry::load(registry_root)
            .map_err(McpError::from)
            .map_err(ServerStartupError::Registry)?;
        let runtime = WasmtimeRuntimeAdapter::new()
            .map_err(McpError::from)
            .map_err(ServerStartupError::Runtime)?;
        let facade = GuildMcpFacade::new(registry.clone(), runtime);

        Ok(Self {
            registry,
            facade,
            state: SessionState::PreInitialize,
        })
    }

    /// Resolve the registry root from CLI args or `GUILD_REGISTRY_ROOT`.
    ///
    /// # Errors
    ///
    /// Returns an error if the arguments are invalid or no registry root is
    /// provided from either source.
    pub fn resolve_registry_root(
        args: impl IntoIterator<Item = String>,
        env_registry_root: Option<String>,
    ) -> Result<PathBuf, ServerStartupError> {
        let mut args = args.into_iter();
        let _program = args.next();
        let mut registry_root = None;

        while let Some(argument) = args.next() {
            match argument.as_str() {
                "--registry-root" => {
                    let value = args.next().ok_or_else(|| {
                        ServerStartupError::Registry(McpError::new(
                            "registry-root-missing",
                            "--registry-root requires a following path argument",
                        ))
                    })?;
                    registry_root = Some(PathBuf::from(value));
                }
                "--help" | "-h" => {
                    return Err(ServerStartupError::Registry(McpError::new(
                        "usage",
                        "usage: guild-mcp-server --registry-root <path>",
                    )));
                }
                _ => {
                    return Err(ServerStartupError::Registry(McpError::new(
                        "unexpected-argument",
                        format!("unexpected argument `{argument}`"),
                    )));
                }
            }
        }

        if let Some(path) = registry_root {
            return Ok(path);
        }

        if let Some(path) = env_registry_root {
            return Ok(PathBuf::from(path));
        }

        Err(ServerStartupError::MissingRegistryRoot)
    }

    /// Serve newline-delimited JSON-RPC over stdio.
    ///
    /// # Errors
    ///
    /// Returns an I/O error if reading stdin or writing stdout fails.
    pub fn serve_stdio(&mut self) -> io::Result<()> {
        let stdin = io::stdin();
        let stdout = io::stdout();
        self.serve(stdin.lock(), stdout.lock())
    }

    /// Serve newline-delimited JSON-RPC using the provided reader and writer.
    ///
    /// # Errors
    ///
    /// Returns an I/O error if reading requests or writing responses fails.
    pub fn serve<R, W>(&mut self, mut reader: R, mut writer: W) -> io::Result<()>
    where
        R: BufRead,
        W: Write,
    {
        let mut line = String::new();
        loop {
            line.clear();
            let bytes_read = reader.read_line(&mut line)?;
            if bytes_read == 0 {
                break;
            }

            let message = line.trim_end_matches(['\n', '\r']);
            if message.is_empty() {
                continue;
            }

            if let Some(response) = self.handle_message(message) {
                write_json_line(&mut writer, &response)?;
            }
        }

        Ok(())
    }

    fn handle_message(&mut self, message: &str) -> Option<Value> {
        let parsed = match serde_json::from_str::<Value>(message) {
            Ok(value) => value,
            Err(error) => {
                return Some(error_response(
                    Value::Null,
                    ERROR_PARSE,
                    "Parse error",
                    Some(json!({ "detail": error.to_string() })),
                ));
            }
        };

        if parsed.is_array() {
            return Some(error_response(
                Value::Null,
                ERROR_INVALID_REQUEST,
                "Batch JSON-RPC requests are not supported",
                None,
            ));
        }

        let request = match serde_json::from_value::<JsonRpcRequest>(parsed) {
            Ok(request) => request,
            Err(error) => {
                return Some(error_response(
                    Value::Null,
                    ERROR_INVALID_REQUEST,
                    "Invalid JSON-RPC request",
                    Some(json!({ "detail": error.to_string() })),
                ));
            }
        };

        if request.jsonrpc != JSONRPC_VERSION {
            return Some(error_response(
                request.id.unwrap_or(Value::Null),
                ERROR_INVALID_REQUEST,
                "Unsupported JSON-RPC version",
                Some(json!({ "expected": JSONRPC_VERSION, "actual": request.jsonrpc })),
            ));
        }

        if request.id.is_none() {
            self.handle_notification(&request);
            return None;
        }

        let id = request.id.expect("requests have ids");
        Some(self.handle_request(id, &request.method, request.params))
    }

    fn handle_notification(&mut self, request: &JsonRpcRequest) {
        if request.method == NOTIFICATION_INITIALIZED {
            if let SessionState::WaitingForInitialized { protocol_version } = &self.state {
                self.state = SessionState::Ready {
                    protocol_version: protocol_version.clone(),
                };
            }
        }
    }

    fn handle_request(&mut self, id: Value, method: &str, params: Option<Value>) -> Value {
        if method != METHOD_INITIALIZE && method != METHOD_PING {
            match &self.state {
                SessionState::PreInitialize | SessionState::WaitingForInitialized { .. } => {
                    return error_response(
                        id,
                        ERROR_SERVER_NOT_INITIALIZED,
                        "Server not initialized",
                        None,
                    );
                }
                SessionState::Ready { .. } => {}
            }
        }

        match method {
            METHOD_INITIALIZE => self.handle_initialize(id, params),
            METHOD_PING => success_response(id, json!({})),
            METHOD_TOOLS_LIST => Self::handle_tools_list(id, params.as_ref()),
            METHOD_TOOLS_CALL => self.handle_tools_call(id, params),
            METHOD_RESOURCES_LIST => self.handle_resources_list(id, params.as_ref()),
            METHOD_RESOURCES_READ => self.handle_resources_read(id, params),
            METHOD_RESOURCE_TEMPLATES_LIST => {
                Self::handle_resource_templates_list(id, params.as_ref())
            }
            _ => error_response(id, ERROR_METHOD_NOT_FOUND, "Method not found", None),
        }
    }

    fn handle_initialize(&mut self, id: Value, params: Option<Value>) -> Value {
        if !matches!(self.state, SessionState::PreInitialize) {
            return error_response(
                id,
                ERROR_INVALID_REQUEST,
                "Server has already been initialized",
                None,
            );
        }

        let params = match deserialize_params::<InitializeParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return error_response(id, ERROR_INVALID_PARAMS, "Invalid params", Some(error))
            }
        };

        let negotiated = negotiate_protocol_version(&params.protocol_version);
        self.state = SessionState::WaitingForInitialized {
            protocol_version: negotiated.clone(),
        };

        success_response(
            id,
            InitializeResult {
                protocol_version: negotiated,
                capabilities: ServerCapabilities {
                    tools: Some(ToolsCapabilities::default()),
                    resources: Some(ResourcesCapabilities {
                        subscribe: None,
                        list_changed: None,
                    }),
                },
                server_info: Implementation {
                    name: SERVER_NAME.into(),
                    version: env!("CARGO_PKG_VERSION").into(),
                    title: Some("Guild MCP Server".into()),
                },
                instructions: Some(
                    "Guild exposes one inspect-only tool (`guild.inspect`) plus durable execution \
                     and evidence resources over stdio MCP."
                        .into(),
                ),
            },
        )
    }

    fn handle_tools_list(id: Value, params: Option<&Value>) -> Value {
        if let Err(error) = reject_cursor(params) {
            return error_response(id, ERROR_INVALID_PARAMS, "Invalid params", Some(error));
        }

        success_response(
            id,
            ListToolsResult {
                tools: vec![Tool {
                    name: INSPECT_TOOL.into(),
                    title: Some("Guild Inspect".into()),
                    description: "Resolve and execute a Guild skill in inspect mode using the \
                                  existing local Guild runtime path."
                        .into(),
                    input_schema: schema_value::<InspectToolRequest>(),
                    output_schema: Some(schema_value::<ExecutionRecord>()),
                    annotations: Some(ToolAnnotations {
                        read_only_hint: false,
                        destructive_hint: false,
                        idempotent_hint: false,
                        open_world_hint: false,
                    }),
                }],
                next_cursor: None,
            },
        )
    }

    fn handle_tools_call(&mut self, id: Value, params: Option<Value>) -> Value {
        let params = match deserialize_params::<CallToolParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return error_response(id, ERROR_INVALID_PARAMS, "Invalid params", Some(error))
            }
        };

        if params.name != INSPECT_TOOL {
            return error_response(
                id,
                ERROR_INVALID_PARAMS,
                "Unknown tool",
                Some(json!({ "name": params.name })),
            );
        }

        let arguments = params
            .arguments
            .unwrap_or(Value::Object(serde_json::Map::default()));
        let request = match serde_json::from_value::<InspectToolRequest>(arguments) {
            Ok(request) => request,
            Err(error) => {
                return error_response(
                    id,
                    ERROR_INVALID_PARAMS,
                    "Invalid tool arguments",
                    Some(json!({ "detail": error.to_string() })),
                )
            }
        };

        match self.facade.inspect_tool(request) {
            Ok(response) => {
                let record = response.structured_content;
                let content = inspect_success_content(&record);
                success_response(
                    id,
                    CallToolResult {
                        content,
                        structured_content: Some(
                            serde_json::to_value(&record).expect("execution record serializes"),
                        ),
                        is_error: None,
                    },
                )
            }
            Err(error) => match error.receipt.as_ref() {
                Some(receipt) => match self.facade.load_execution_record(&receipt.execution_id) {
                    Ok(record) => success_response(
                        id,
                        CallToolResult {
                            content: inspect_failure_content(&record),
                            structured_content: Some(
                                serde_json::to_value(&record).expect("execution record serializes"),
                            ),
                            is_error: Some(true),
                        },
                    ),
                    Err(load_error) => error_response(
                        id,
                        ERROR_SERVER,
                        "Failed to load persisted execution record",
                        Some(guild_error_data(&load_error)),
                    ),
                },
                None => error_response(
                    id,
                    ERROR_SERVER,
                    &error.message,
                    Some(guild_error_data(&error)),
                ),
            },
        }
    }

    fn handle_resources_list(&mut self, id: Value, params: Option<&Value>) -> Value {
        if let Err(error) = reject_cursor(params) {
            return error_response(id, ERROR_INVALID_PARAMS, "Invalid params", Some(error));
        }

        match self
            .registry
            .list_recent_execution_records(DEFAULT_RECENT_EXECUTION_LIMIT)
        {
            Ok(resources) => success_response(
                id,
                ListResourcesResult {
                    resources: resources
                        .into_iter()
                        .map(|record| execution_record_to_resource(&record))
                        .collect(),
                    next_cursor: None,
                },
            ),
            Err(error) => {
                let error = McpError::from(error);
                error_response(
                    id,
                    ERROR_SERVER,
                    &error.message,
                    Some(guild_error_data(&error)),
                )
            }
        }
    }

    fn handle_resources_read(&mut self, id: Value, params: Option<Value>) -> Value {
        let params = match deserialize_params::<ReadResourceParams>(params) {
            Ok(params) => params,
            Err(error) => {
                return error_response(id, ERROR_INVALID_PARAMS, "Invalid params", Some(error))
            }
        };

        match self.facade.read_resource(&params.uri) {
            Ok(resource) => success_response(
                id,
                ReadResourceResult {
                    contents: vec![resource_contents(resource)],
                },
            ),
            Err(error) => {
                let code = if error.code == "resource-uri-invalid" {
                    ERROR_INVALID_PARAMS
                } else {
                    ERROR_SERVER
                };
                error_response(id, code, &error.message, Some(guild_error_data(&error)))
            }
        }
    }

    fn handle_resource_templates_list(id: Value, params: Option<&Value>) -> Value {
        if let Err(error) = reject_cursor(params) {
            return error_response(id, ERROR_INVALID_PARAMS, "Invalid params", Some(error));
        }

        success_response(
            id,
            ListResourceTemplatesResult {
                resource_templates: vec![
                    ResourceTemplate {
                        uri_template: "guild://executions/{execution_id}".into(),
                        name: "Guild execution record".into(),
                        title: Some("Guild Execution Record".into()),
                        description: Some(
                            "Read a persisted Guild execution record by host-minted execution id."
                                .into(),
                        ),
                        mime_type: Some("application/json".into()),
                    },
                    ResourceTemplate {
                        uri_template: "guild://objects/records/{evidence_record_id}".into(),
                        name: "Guild evidence record payload".into(),
                        title: Some("Guild Evidence Record".into()),
                        description: Some(
                            "Read a persisted evidence emission through its host-issued record URI."
                                .into(),
                        ),
                        mime_type: None,
                    },
                    ResourceTemplate {
                        uri_template: "guild://objects/sha256/{digest}".into(),
                        name: "Guild evidence blob".into(),
                        title: Some("Guild Evidence Blob".into()),
                        description: Some(
                            "Read a raw content-addressed evidence blob by its digest URI.".into(),
                        ),
                        mime_type: None,
                    },
                ],
                next_cursor: None,
            },
        )
    }

    #[must_use]
    pub fn negotiated_protocol_version(&self) -> Option<&str> {
        match &self.state {
            SessionState::PreInitialize => None,
            SessionState::WaitingForInitialized { protocol_version }
            | SessionState::Ready { protocol_version } => Some(protocol_version.as_str()),
        }
    }
}

fn deserialize_params<T>(params: Option<Value>) -> Result<T, Value>
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_value(params.unwrap_or(Value::Object(serde_json::Map::default())))
        .map_err(|error| json!({ "detail": error.to_string() }))
}

fn reject_cursor(params: Option<&Value>) -> Result<(), Value> {
    if let Some(params) = params {
        let list_params: ListParams = serde_json::from_value(params.clone())
            .map_err(|error| json!({ "detail": error.to_string() }))?;
        if list_params.cursor.is_some() {
            return Err(json!({
                "detail": "cursor-based pagination is not implemented in this milestone"
            }));
        }
    }

    Ok(())
}

fn negotiate_protocol_version(requested: &str) -> String {
    if SUPPORTED_PROTOCOL_VERSIONS.contains(&requested) {
        requested.to_owned()
    } else {
        SUPPORTED_PROTOCOL_VERSIONS
            .last()
            .expect("supported protocol versions are configured")
            .to_string()
    }
}

fn success_response<T>(id: Value, result: T) -> Value
where
    T: Serialize,
{
    serde_json::to_value(JsonRpcSuccessResponse {
        jsonrpc: JSONRPC_VERSION,
        id,
        result,
    })
    .expect("JSON-RPC success response serializes")
}

fn error_response(id: Value, code: i32, message: impl Into<String>, data: Option<Value>) -> Value {
    serde_json::to_value(JsonRpcErrorResponse {
        jsonrpc: JSONRPC_VERSION,
        id,
        error: JsonRpcError {
            code,
            message: message.into(),
            data,
        },
    })
    .expect("JSON-RPC error response serializes")
}

fn write_json_line<W, T>(writer: &mut W, value: &T) -> io::Result<()>
where
    W: Write,
    T: Serialize,
{
    serde_json::to_writer(&mut *writer, value)?;
    writer.write_all(b"\n")?;
    writer.flush()
}

fn schema_value<T>() -> Value
where
    T: JsonSchema,
{
    serde_json::to_value(schema_for!(T)).expect("JSON schema serializes")
}

fn guild_error_data(error: &McpError) -> Value {
    json!({
        "guild": {
            "code": error.code,
            "message": error.message,
            "detail": error.detail,
            "receipt": error.receipt,
        }
    })
}

fn inspect_success_content(record: &ExecutionRecord) -> Vec<ContentBlock> {
    let mut content = vec![ContentBlock::Text(TextContent {
        text: serde_json::to_string_pretty(record).expect("execution record serializes"),
    })];
    content.push(ContentBlock::ResourceLink(execution_resource_link(record)));
    content.extend(
        record
            .emitted_evidence
            .iter()
            .map(evidence_resource_link)
            .map(ContentBlock::ResourceLink),
    );
    content
}

fn inspect_failure_content(record: &ExecutionRecord) -> Vec<ContentBlock> {
    let mut content = vec![ContentBlock::Text(TextContent {
        text: serde_json::to_string_pretty(record).expect("execution record serializes"),
    })];
    content.push(ContentBlock::ResourceLink(execution_resource_link(record)));
    content.extend(
        record
            .emitted_evidence
            .iter()
            .map(evidence_resource_link)
            .map(ContentBlock::ResourceLink),
    );
    content
}

fn execution_resource_link(record: &ExecutionRecord) -> ResourceLink {
    ResourceLink {
        uri: record.receipt.uri.clone(),
        name: format!("execution-{}", record.receipt.execution_id),
        title: Some(format!("Guild execution {}", record.receipt.execution_id)),
        description: Some(format!(
            "Persisted execution record with status {}",
            status_name(record)
        )),
        mime_type: Some("application/json".into()),
        size: None,
    }
}

fn evidence_resource_link(record: &guild_types::EvidenceRecord) -> ResourceLink {
    ResourceLink {
        uri: record.uri.clone(),
        name: record
            .title
            .clone()
            .unwrap_or_else(|| format!("evidence-{}", record.sha256)),
        title: record.title.clone(),
        description: Some(
            "Persisted evidence emission addressed by host-issued evidence record URI.".into(),
        ),
        mime_type: Some(record.mime_type.clone()),
        size: Some(record.size_bytes),
    }
}

fn execution_record_to_resource(record: &ExecutionRecord) -> Resource {
    Resource {
        uri: record.receipt.uri.clone(),
        name: format!("execution-{}", record.receipt.execution_id),
        title: Some(format!("Guild execution {}", record.receipt.execution_id)),
        description: Some(format!(
            "Persisted execution record with status {}",
            status_name(record)
        )),
        mime_type: Some("application/json".into()),
        size: Some(
            serde_json::to_vec_pretty(record)
                .expect("execution record serializes")
                .len() as u64,
        ),
    }
}

fn resource_contents(resource: ResourceReadResult) -> ResourceContents {
    if is_textual_mime(&resource.mime_type) {
        if let Ok(text) = String::from_utf8(resource.bytes.clone()) {
            return ResourceContents::Text(TextResourceContents {
                uri: resource.uri,
                mime_type: resource.mime_type,
                text,
            });
        }
    }

    ResourceContents::Blob(BlobResourceContents {
        uri: resource.uri,
        mime_type: resource.mime_type,
        blob: base64::engine::general_purpose::STANDARD.encode(resource.bytes),
    })
}

fn is_textual_mime(mime_type: &str) -> bool {
    mime_type.starts_with("text/")
        || mime_type == "application/json"
        || mime_type.starts_with("application/") && mime_type.ends_with("+json")
}

fn status_name(record: &ExecutionRecord) -> &'static str {
    match record.status {
        guild_types::ExecutionStatus::Succeeded => "succeeded",
        guild_types::ExecutionStatus::Failed => "failed",
        guild_types::ExecutionStatus::Partial => "partial",
        guild_types::ExecutionStatus::Rejected => "rejected",
    }
}
