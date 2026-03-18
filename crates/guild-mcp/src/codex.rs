use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::str::FromStr;

use guild_registry::{
    InstalledSkill, LocalRegistry, LocalSourceInstaller, RegistryError, SkillRegistry,
};
use guild_types::{
    CapabilityAccess, CapabilityConstraints, CapabilityId, EmitEvidenceConstraints,
    EvidenceAudience, ExecutionRecord, GrantedCapability, InvokeDependencyConstraints,
    ReadResourceConstraints, RedactionClass, RequestedSkillRef, ResourceKind, SkillKey,
    VersionRequirement,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::SERVER_BINARY_NAME;
use crate::protocol::{
    CallToolResult, InitializeResult, PROTOCOL_VERSION_2025_11_25, ReadResourceResult,
};

pub const CODEX_WORKFLOW_BINARY_NAME: &str = "guild-codex";
pub const DEFAULT_CODEX_SERVER_NAME: &str = "guild-local";
const DEFAULT_CODEX_REGISTRY_ROOT: &str = "target/dev-local-registry/codex-local";
const GUILD_MCP_MANIFEST_RELATIVE_PATH: &str = "crates/guild-mcp/Cargo.toml";
const EXAMPLE_NAMESPACE: &str = "example";
const EXAMPLE_VERSION_REQUIREMENT: &str = "^0.1";
const DEFAULT_CODEX_SKILLS: [&str; 7] = [
    "hello-inspect",
    "hello-composite",
    "explain-execution",
    "explain-execution-tree",
    "explain-capability-denial",
    "diff-execution-authority",
    "explain-http-authority",
];
const EXPLAIN_EXECUTION_ONLY: [CodexSmokeSelection; 1] = [CodexSmokeSelection::ExplainExecution];
const EXPLAIN_EXECUTION_TREE_ONLY: [CodexSmokeSelection; 1] =
    [CodexSmokeSelection::ExplainExecutionTree];
const ALL_CODEX_SMOKE_FLOWS: [CodexSmokeSelection; 2] = [
    CodexSmokeSelection::ExplainExecution,
    CodexSmokeSelection::ExplainExecutionTree,
];

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct BootstrappedSkill {
    pub namespace: String,
    pub name: String,
    pub version: String,
    pub digest: String,
    pub source_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct CodexBootstrapSummary {
    pub repo_root: PathBuf,
    pub registry_root: PathBuf,
    pub skills: Vec<BootstrappedSkill>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct CodexServerConfig {
    pub name: String,
    pub cwd: PathBuf,
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct CodexBootstrapOutput {
    pub bootstrap: CodexBootstrapSummary,
    pub config: CodexServerConfig,
    pub print_config_command: String,
    pub recommended_smoke_commands: Vec<String>,
    pub recommended_proof_commands: Vec<String>,
}

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord,
)]
#[serde(rename_all = "kebab-case")]
pub enum CodexSmokeSelection {
    ExplainExecution,
    ExplainExecutionTree,
    All,
}

impl CodexSmokeSelection {
    #[must_use]
    pub fn flows(self) -> &'static [Self] {
        match self {
            Self::ExplainExecution => &EXPLAIN_EXECUTION_ONLY,
            Self::ExplainExecutionTree => &EXPLAIN_EXECUTION_TREE_ONLY,
            Self::All => &ALL_CODEX_SMOKE_FLOWS,
        }
    }
}

impl std::fmt::Display for CodexSmokeSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Self::ExplainExecution => "explain-execution",
            Self::ExplainExecutionTree => "explain-execution-tree",
            Self::All => "all",
        };
        f.write_str(value)
    }
}

impl FromStr for CodexSmokeSelection {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "explain-execution" => Ok(Self::ExplainExecution),
            "explain-execution-tree" => Ok(Self::ExplainExecutionTree),
            "all" => Ok(Self::All),
            _ => Err(format!(
                "unknown flow `{value}`; expected explain-execution, explain-execution-tree, or all"
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct CodexSmokeFlowSummary {
    pub flow: CodexSmokeSelection,
    pub subject_execution_uri: String,
    pub report_execution_uri: String,
    pub subject_resource_items: usize,
    pub report_resource_items: usize,
    pub subject_emitted_evidence: usize,
    pub subject_child_executions: usize,
    pub report_summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct CodexSmokeSummary {
    pub registry_root: PathBuf,
    pub configured_server_name: String,
    pub mcp_server_name: String,
    pub protocol_version: String,
    pub requested_flow: CodexSmokeSelection,
    pub flows: Vec<CodexSmokeFlowSummary>,
}

impl CodexSmokeSummary {
    #[must_use]
    pub fn render_text(&self) -> String {
        let mut output = String::new();
        let _ = writeln!(output, "Guild Codex smoke ok.");
        let _ = writeln!(output, "registry root: {}", self.registry_root.display());
        let _ = writeln!(
            output,
            "initialized {} over {}",
            self.mcp_server_name, self.protocol_version
        );

        for flow in &self.flows {
            let _ = writeln!(output);
            let _ = writeln!(output, "flow: {}", flow.flow);
            let _ = writeln!(
                output,
                "subject execution uri: {}",
                flow.subject_execution_uri
            );
            let _ = writeln!(
                output,
                "report execution uri: {}",
                flow.report_execution_uri
            );
            let _ = writeln!(
                output,
                "subject resource contents: {} item(s)",
                flow.subject_resource_items
            );
            let _ = writeln!(
                output,
                "report resource contents: {} item(s)",
                flow.report_resource_items
            );
            let _ = writeln!(
                output,
                "subject emitted evidence: {}",
                flow.subject_emitted_evidence
            );
            let _ = writeln!(
                output,
                "subject child executions: {}",
                flow.subject_child_executions
            );
            let _ = writeln!(output, "report summary: {}", flow.report_summary);
        }

        output.trim_end().into()
    }
}

#[derive(Debug)]
pub struct CodexWorkflowError {
    code: String,
    message: String,
    detail: Option<Box<Value>>,
}

impl CodexWorkflowError {
    fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            detail: None,
        }
    }

    fn with_detail(mut self, detail: Value) -> Self {
        self.detail = Some(Box::new(detail));
        self
    }
}

impl std::fmt::Display for CodexWorkflowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(detail) = &self.detail {
            write!(
                f,
                "{}: {} ({})",
                self.code,
                self.message,
                serde_json::to_string(detail).unwrap_or_else(|_| "<detail unavailable>".into())
            )
        } else {
            write!(f, "{}: {}", self.code, self.message)
        }
    }
}

impl std::error::Error for CodexWorkflowError {}

impl From<RegistryError> for CodexWorkflowError {
    fn from(value: RegistryError) -> Self {
        Self {
            code: value.code,
            message: value.message,
            detail: value.detail.map(Box::new),
        }
    }
}

impl From<std::io::Error> for CodexWorkflowError {
    fn from(value: std::io::Error) -> Self {
        Self::new("codex-workflow-io", "workflow I/O failed")
            .with_detail(json!({ "io_error": value.to_string() }))
    }
}

impl From<serde_json::Error> for CodexWorkflowError {
    fn from(value: serde_json::Error) -> Self {
        Self::new("codex-workflow-json", "failed to encode or decode JSON")
            .with_detail(json!({ "json_error": value.to_string() }))
    }
}

/// Resolve the repository root used by the local Codex workflow helper.
///
/// # Panics
///
/// Panics if the workspace root cannot be resolved from the current crate
/// location.
#[must_use]
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root exists")
}

#[must_use]
pub fn guild_mcp_manifest_path() -> PathBuf {
    repo_root().join(GUILD_MCP_MANIFEST_RELATIVE_PATH)
}

#[must_use]
pub fn default_registry_root() -> PathBuf {
    repo_root().join(DEFAULT_CODEX_REGISTRY_ROOT)
}

#[must_use]
pub fn recommended_proof_commands() -> Vec<String> {
    vec![
        "cargo run -p guild-mcp --example codex_explain_execution_local".into(),
        "cargo run -p guild-mcp --example codex_explain_execution_tree_local".into(),
    ]
}

#[must_use]
pub fn print_config_command(registry_root: impl AsRef<Path>) -> String {
    format!(
        "cargo run -p guild-mcp --bin guild-codex -- print-config --registry-root {}",
        shell_quote(&absolute_path(registry_root).to_string_lossy())
    )
}

#[must_use]
pub fn recommended_smoke_commands(registry_root: impl AsRef<Path>) -> Vec<String> {
    let registry_root = absolute_path(registry_root);
    [
        CodexSmokeSelection::ExplainExecution,
        CodexSmokeSelection::ExplainExecutionTree,
    ]
    .into_iter()
    .map(|flow| {
        format!(
            "cargo run -p guild-mcp --bin guild-codex -- smoke --registry-root {} --flow {}",
            shell_quote(&registry_root.to_string_lossy()),
            flow
        )
    })
    .collect()
}

/// Build the default local Codex dogfood root by installing the example skills
/// used by the documented Codex flows.
///
/// # Errors
///
/// Returns an error if the registry root cannot be prepared or an example skill
/// cannot be installed.
pub fn bootstrap_codex_registry(
    registry_root: impl AsRef<Path>,
    reset: bool,
) -> Result<CodexBootstrapSummary, RegistryError> {
    let repo_root = repo_root();
    let registry_root = prepare_registry_root(registry_root, reset)?;
    let installer = LocalSourceInstaller::new(&registry_root)?;

    let mut skills = Vec::with_capacity(DEFAULT_CODEX_SKILLS.len());
    for skill_dir in DEFAULT_CODEX_SKILLS {
        let installed_skill =
            installer.install(repo_root.join("examples/skills").join(skill_dir))?;
        skills.push(summarize_installed_skill(skill_dir, installed_skill));
    }

    Ok(CodexBootstrapSummary {
        repo_root,
        registry_root,
        skills,
    })
}

#[must_use]
pub fn codex_server_config(
    registry_root: impl AsRef<Path>,
    name: impl Into<String>,
) -> CodexServerConfig {
    let registry_root = absolute_path(registry_root);
    let manifest_path = guild_mcp_manifest_path();
    let mut env = BTreeMap::new();
    env.insert(
        "GUILD_REGISTRY_ROOT".into(),
        registry_root.to_string_lossy().into_owned(),
    );

    CodexServerConfig {
        name: name.into(),
        cwd: repo_root(),
        command: "cargo".into(),
        args: vec![
            "run".into(),
            "-q".into(),
            "--manifest-path".into(),
            manifest_path.to_string_lossy().into_owned(),
            "--bin".into(),
            SERVER_BINARY_NAME.into(),
            "--".into(),
        ],
        env,
    }
}

impl CodexServerConfig {
    /// Render the exact local command used to launch the Guild stdio MCP server
    /// outside Codex.
    ///
    /// # Panics
    ///
    /// Panics if the config does not carry `GUILD_REGISTRY_ROOT`, which is a
    /// required invariant for instances built through `codex_server_config`.
    #[must_use]
    pub fn manual_server_command(&self) -> String {
        let registry_root = self.registry_root_env();
        format!(
            "{} --registry-root {}",
            self.quoted_command_line(),
            shell_quote(registry_root)
        )
    }

    /// Render the `codex mcp add` command matching this local stdio server
    /// configuration.
    ///
    /// # Panics
    ///
    /// Panics if the config does not carry `GUILD_REGISTRY_ROOT`, which is a
    /// required invariant for instances built through `codex_server_config`.
    #[must_use]
    pub fn codex_mcp_add_command(&self) -> String {
        let registry_root = self.registry_root_env();
        format!(
            "codex mcp add {} --env GUILD_REGISTRY_ROOT={} -- {}",
            shell_quote(&self.name),
            shell_quote(registry_root),
            self.quoted_command_line(),
        )
    }

    #[must_use]
    pub fn config_toml(&self) -> String {
        let args = self
            .args
            .iter()
            .map(|arg| toml_string(arg))
            .collect::<Vec<_>>()
            .join(", ");
        let env = self
            .env
            .iter()
            .map(|(key, value)| format!("{key} = {}", toml_string(value)))
            .collect::<Vec<_>>()
            .join(", ");

        format!(
            "[mcp_servers.{}]\ncwd = {}\ncommand = {}\nargs = [{}]\nenv = {{ {} }}",
            self.name,
            toml_string(&self.cwd.to_string_lossy()),
            toml_string(&self.command),
            args,
            env
        )
    }

    fn registry_root_env(&self) -> &str {
        self.env
            .get("GUILD_REGISTRY_ROOT")
            .map(String::as_str)
            .expect("Codex server config always carries GUILD_REGISTRY_ROOT")
    }

    fn quoted_command_line(&self) -> String {
        let mut command = shell_quote(&self.command);
        for arg in &self.args {
            let _ = write!(command, " {}", shell_quote(arg));
        }
        command
    }
}

/// Run one or both deterministic Codex dogfood flows over the documented stdio
/// server configuration for an already prepared local Guild root.
///
/// # Errors
///
/// Returns an error if the registry root cannot be loaded, the required example
/// skills are missing, the stdio server cannot be spawned, or the selected
/// flow does not produce the expected persisted resources.
pub fn run_codex_smoke(
    registry_root: impl AsRef<Path>,
    name: impl Into<String>,
    selection: CodexSmokeSelection,
) -> Result<CodexSmokeSummary, CodexWorkflowError> {
    let registry_root = absolute_path(registry_root);
    validate_codex_smoke_registry(&registry_root, selection)?;

    let config = codex_server_config(&registry_root, name);
    let mut client =
        McpStdioClient::spawn(&config.command, &config.args, &config.cwd, &config.env)?;
    let initialized = client.initialize("guild-codex-smoke")?;

    let flows = selection
        .flows()
        .iter()
        .copied()
        .map(|flow| run_single_codex_smoke_flow(&mut client, flow))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(CodexSmokeSummary {
        registry_root,
        configured_server_name: config.name,
        mcp_server_name: initialized.server_info.name,
        protocol_version: initialized.protocol_version,
        requested_flow: selection,
        flows,
    })
}

fn prepare_registry_root(
    registry_root: impl AsRef<Path>,
    reset: bool,
) -> Result<PathBuf, RegistryError> {
    let registry_root = absolute_path(registry_root);

    if reset && registry_root.exists() {
        fs::remove_dir_all(&registry_root).map_err(|error| {
            io_registry_error(
                "codex-bootstrap-reset-failed",
                "failed to reset the requested Codex registry root",
                &registry_root,
                &error,
            )
        })?;
    } else if registry_root.exists() && !directory_is_empty(&registry_root)? {
        return Err(RegistryError::new(
            "codex-bootstrap-root-not-empty",
            "registry root already exists and is not empty; pass --reset to rebuild it",
        )
        .with_detail(serde_json::json!({
            "registry_root": registry_root,
        })));
    }

    fs::create_dir_all(&registry_root).map_err(|error| {
        io_registry_error(
            "codex-bootstrap-create-failed",
            "failed to create the requested Codex registry root",
            &registry_root,
            &error,
        )
    })?;

    registry_root.canonicalize().map_err(|error| {
        io_registry_error(
            "codex-bootstrap-canonicalize-failed",
            "failed to canonicalize the requested Codex registry root",
            &registry_root,
            &error,
        )
    })
}

fn directory_is_empty(path: &Path) -> Result<bool, RegistryError> {
    let mut entries = fs::read_dir(path).map_err(|error| {
        io_registry_error(
            "codex-bootstrap-read-dir-failed",
            "failed to inspect the requested Codex registry root",
            path,
            &error,
        )
    })?;
    Ok(entries.next().is_none())
}

fn summarize_installed_skill(source_dir: &str, installed: InstalledSkill) -> BootstrappedSkill {
    BootstrappedSkill {
        namespace: installed.manifest.key.namespace,
        name: installed.manifest.key.name,
        version: installed.manifest.version.to_string(),
        digest: installed.resolved_ref.digest,
        source_dir: source_dir.into(),
    }
}

fn validate_codex_smoke_registry(
    registry_root: &Path,
    selection: CodexSmokeSelection,
) -> Result<(), RegistryError> {
    let registry = LocalRegistry::load(registry_root)?;
    let required_skill_names = selection
        .flows()
        .iter()
        .flat_map(|flow| required_skill_names_for_flow(*flow))
        .map(|skill_name| (*skill_name).to_owned())
        .collect::<BTreeSet<_>>();

    for skill_name in required_skill_names {
        registry.resolve(&requested_example_skill_ref(&skill_name))?;
    }

    Ok(())
}

fn required_skill_names_for_flow(flow: CodexSmokeSelection) -> &'static [&'static str] {
    match flow {
        CodexSmokeSelection::ExplainExecution => &["hello-inspect", "explain-execution"],
        CodexSmokeSelection::ExplainExecutionTree => {
            &["hello-inspect", "hello-composite", "explain-execution-tree"]
        }
        CodexSmokeSelection::All => unreachable!("all expands before per-flow validation"),
    }
}

fn requested_example_skill_ref(skill_name: &str) -> RequestedSkillRef {
    RequestedSkillRef {
        key: SkillKey {
            namespace: EXAMPLE_NAMESPACE.into(),
            name: skill_name.into(),
        },
        version_req: VersionRequirement::parse(EXAMPLE_VERSION_REQUIREMENT)
            .expect("example version requirement parses"),
    }
}

fn run_single_codex_smoke_flow(
    client: &mut McpStdioClient,
    flow: CodexSmokeSelection,
) -> Result<CodexSmokeFlowSummary, CodexWorkflowError> {
    match flow {
        CodexSmokeSelection::ExplainExecution => run_explain_execution_smoke(client),
        CodexSmokeSelection::ExplainExecutionTree => run_explain_execution_tree_smoke(client),
        CodexSmokeSelection::All => unreachable!("all expands before per-flow execution"),
    }
}

fn run_explain_execution_smoke(
    client: &mut McpStdioClient,
) -> Result<CodexSmokeFlowSummary, CodexWorkflowError> {
    let hello_response_value = client.request(
        "tools/call",
        &example_inspect_request(
            "hello-inspect",
            &json!({ "name": "Ada" }),
            &[emit_evidence_grant()],
        ),
    )?;
    let hello_response: CallToolResult = McpStdioClient::parse_result(&hello_response_value)?;
    let hello_record = parse_execution_record(&hello_response)?;

    let explain_response_value = client.request(
        "tools/call",
        &example_inspect_request(
            "explain-execution",
            &json!({
                "execution_uri": hello_record.receipt.uri,
                "include_first_evidence": true,
            }),
            &[execution_and_object_read_grant()],
        ),
    )?;
    let explain_response: CallToolResult = McpStdioClient::parse_result(&explain_response_value)?;
    let explain_record = parse_execution_record(&explain_response)?;

    let target_resource_value = client.request(
        "resources/read",
        &json!({ "uri": hello_record.receipt.uri }),
    )?;
    let target_resource: ReadResourceResult = McpStdioClient::parse_result(&target_resource_value)?;
    let report_resource_value = client.request(
        "resources/read",
        &json!({ "uri": explain_record.receipt.uri }),
    )?;
    let report_resource: ReadResourceResult = McpStdioClient::parse_result(&report_resource_value)?;

    Ok(CodexSmokeFlowSummary {
        flow: CodexSmokeSelection::ExplainExecution,
        subject_execution_uri: hello_record.receipt.uri,
        report_execution_uri: explain_record.receipt.uri,
        subject_resource_items: target_resource.contents.len(),
        report_resource_items: report_resource.contents.len(),
        subject_emitted_evidence: hello_record.emitted_evidence.len(),
        subject_child_executions: hello_record.child_executions.len(),
        report_summary: explain_record
            .output
            .ok_or_else(|| {
                CodexWorkflowError::new(
                    "codex-smoke-report-missing-output",
                    "explain-execution did not return skill output",
                )
            })?
            .summary,
    })
}

fn run_explain_execution_tree_smoke(
    client: &mut McpStdioClient,
) -> Result<CodexSmokeFlowSummary, CodexWorkflowError> {
    let composite_response_value = client.request(
        "tools/call",
        &example_inspect_request(
            "hello-composite",
            &json!({ "name": "Ada" }),
            &invoke_and_evidence_grants(),
        ),
    )?;
    let composite_response: CallToolResult =
        McpStdioClient::parse_result(&composite_response_value)?;
    let composite_record = parse_execution_record(&composite_response)?;

    let explain_response_value = client.request(
        "tools/call",
        &example_inspect_request(
            "explain-execution-tree",
            &json!({
                "execution_uri": composite_record.receipt.uri,
                "max_depth": 4,
                "max_nodes": 32,
                "include_evidence_resources": true,
            }),
            &[execution_and_object_read_grant()],
        ),
    )?;
    let explain_response: CallToolResult = McpStdioClient::parse_result(&explain_response_value)?;
    let explain_record = parse_execution_record(&explain_response)?;

    let subject_resource_value = client.request(
        "resources/read",
        &json!({ "uri": composite_record.receipt.uri }),
    )?;
    let subject_resource: ReadResourceResult =
        McpStdioClient::parse_result(&subject_resource_value)?;
    let report_resource_value = client.request(
        "resources/read",
        &json!({ "uri": explain_record.receipt.uri }),
    )?;
    let report_resource: ReadResourceResult = McpStdioClient::parse_result(&report_resource_value)?;

    Ok(CodexSmokeFlowSummary {
        flow: CodexSmokeSelection::ExplainExecutionTree,
        subject_execution_uri: composite_record.receipt.uri,
        report_execution_uri: explain_record.receipt.uri,
        subject_resource_items: subject_resource.contents.len(),
        report_resource_items: report_resource.contents.len(),
        subject_emitted_evidence: composite_record.emitted_evidence.len(),
        subject_child_executions: composite_record.child_executions.len(),
        report_summary: explain_record
            .output
            .ok_or_else(|| {
                CodexWorkflowError::new(
                    "codex-smoke-report-missing-output",
                    "explain-execution-tree did not return skill output",
                )
            })?
            .summary,
    })
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
        emit_evidence_grant(),
    ]
}

fn example_inspect_request(skill_name: &str, input: &Value, grants: &[Value]) -> Value {
    json!({
        "name": "guild.inspect",
        "arguments": {
            "skill": {
                "key": {
                    "namespace": EXAMPLE_NAMESPACE,
                    "name": skill_name,
                },
                "version_req": EXAMPLE_VERSION_REQUIREMENT,
            },
            "input": input,
            "grants": {
                "grants": grants,
            }
        }
    })
}

fn parse_execution_record(result: &CallToolResult) -> Result<ExecutionRecord, CodexWorkflowError> {
    serde_json::from_value(result.structured_content.clone().ok_or_else(|| {
        CodexWorkflowError::new(
            "codex-smoke-missing-structured-content",
            "inspect did not return structured content",
        )
    })?)
    .map_err(CodexWorkflowError::from)
}

struct McpStdioClient {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl McpStdioClient {
    fn spawn(
        command: impl AsRef<Path>,
        args: &[String],
        cwd: &Path,
        env: &BTreeMap<String, String>,
    ) -> Result<Self, CodexWorkflowError> {
        let mut child = Command::new(command.as_ref())
            .args(args)
            .current_dir(cwd)
            .envs(env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|error| {
                CodexWorkflowError::new(
                    "codex-smoke-server-spawn-failed",
                    "failed to spawn the Guild stdio MCP server",
                )
                .with_detail(json!({
                    "command": command.as_ref(),
                    "args": args,
                    "cwd": cwd,
                    "io_error": error.to_string(),
                }))
            })?;

        Ok(Self {
            stdin: child.stdin.take().ok_or_else(|| {
                CodexWorkflowError::new(
                    "codex-smoke-missing-stdin",
                    "spawned MCP server is missing stdin",
                )
            })?,
            stdout: BufReader::new(child.stdout.take().ok_or_else(|| {
                CodexWorkflowError::new(
                    "codex-smoke-missing-stdout",
                    "spawned MCP server is missing stdout",
                )
            })?),
            child,
            next_id: 1,
        })
    }

    fn initialize(&mut self, client_name: &str) -> Result<InitializeResult, CodexWorkflowError> {
        let response = self.request(
            "initialize",
            &json!({
                "protocolVersion": PROTOCOL_VERSION_2025_11_25,
                "capabilities": {},
                "clientInfo": {
                    "name": client_name,
                    "version": "0.1.0"
                }
            }),
        )?;
        let initialized = Self::parse_result(&response)?;
        self.notify("notifications/initialized", &json!({}))?;
        Ok(initialized)
    }

    fn request(&mut self, method: &str, params: &Value) -> Result<Value, CodexWorkflowError> {
        let id = self.next_id;
        self.next_id += 1;
        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        self.write_message(&request)?;
        self.read_message()
    }

    fn parse_result<T: serde::de::DeserializeOwned>(
        response: &Value,
    ) -> Result<T, CodexWorkflowError> {
        if let Some(error) = response.get("error") {
            return Err(CodexWorkflowError::new(
                "codex-smoke-mcp-error",
                "MCP server returned an error response",
            )
            .with_detail(error.clone()));
        }

        serde_json::from_value(response["result"].clone()).map_err(|error| {
            CodexWorkflowError::new(
                "codex-smoke-response-parse-failed",
                "failed to parse MCP success response",
            )
            .with_detail(json!({
                "json_error": error.to_string(),
                "response": response,
            }))
        })
    }

    fn notify(&mut self, method: &str, params: &Value) -> Result<(), CodexWorkflowError> {
        let request = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        self.write_message(&request)
    }

    fn write_message(&mut self, message: &Value) -> Result<(), CodexWorkflowError> {
        serde_json::to_writer(&mut self.stdin, message)?;
        self.stdin.write_all(b"\n")?;
        self.stdin.flush()?;
        Ok(())
    }

    fn read_message(&mut self) -> Result<Value, CodexWorkflowError> {
        let mut line = String::new();
        let read = self.stdout.read_line(&mut line)?;
        if read == 0 {
            return Err(CodexWorkflowError::new(
                "codex-smoke-server-exited",
                "Guild MCP server exited before returning a response",
            ));
        }

        serde_json::from_str(&line).map_err(CodexWorkflowError::from)
    }
}

impl Drop for McpStdioClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn absolute_path(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .expect("current directory available")
            .join(path)
    }
}

fn io_registry_error(
    code: &str,
    message: &str,
    path: &Path,
    error: &std::io::Error,
) -> RegistryError {
    RegistryError::new(code, message).with_detail(serde_json::json!({
        "path": path,
        "io_error": error.to_string(),
    }))
}

fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".into();
    }

    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-' | ':' | '='))
    {
        return value.into();
    }

    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn toml_string(value: &str) -> String {
    serde_json::to_string(value).expect("JSON string escaping also works for TOML basic strings")
}
