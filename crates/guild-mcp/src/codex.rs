use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::str::FromStr;

use guild_registry::{
    InstalledSkill, LocalPublisherIdentity, LocalRegistry, LocalSourceInstaller, RegistryError,
    SkillRegistry, execution_query_resource_uri,
};
use guild_runner::WasmtimeRuntimeAdapter;
use guild_types::{
    CapabilityAccess, CapabilityConstraints, CapabilityGrantSet, CapabilityId,
    EmitEvidenceConstraints, EvidenceAudience, ExecutionQueryResource, ExecutionRecord,
    GrantedCapability, HttpMethod, HttpRequestConstraints, HttpScheme, InstalledVerificationState,
    InvokeDependencyConstraints, LocalPolicyConfig, LocalTrustTier, PolicyProfile,
    PolicyProfileBinding, PolicyRule, PolicyRuleEffect, PolicyRuleTarget, ReadResourceConstraints,
    RedactionClass, RequestedSkillRef, ResourceKind, SkillKey, VersionRequirement,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use toml_edit::{Array, DocumentMut, InlineTable, Item, Table, Value as TomlValue};

use crate::paths;
use crate::protocol::{
    CallToolResult, InitializeResult, PROTOCOL_VERSION_2025_11_25, ReadResourceResult,
};
use crate::{CLI_BINARY_NAME, GuildMcpFacade, InspectRequest, McpError};

#[path = "../../../test-support/http_test_server.rs"]
mod http_test_server;

pub const DEFAULT_CODEX_SERVER_NAME: &str = "guild-local";
const GUILD_MCP_MANIFEST_RELATIVE_PATH: &str = "crates/guild-mcp/Cargo.toml";
const EXAMPLE_NAMESPACE: &str = "example";
const EXAMPLE_VERSION_REQUIREMENT: &str = "^0.1";
const DEFAULT_CODEX_SKILLS: [&str; 14] = [
    "render-report",
    "incident-brief",
    "run-diff",
    "recent-failures",
    "evidence-summary",
    "hello-inspect",
    "hello-composite",
    "explain-execution",
    "explain-execution-tree",
    "explain-capability-denial",
    "diff-execution-authority",
    "explain-http-authority",
    "inspect-http-json",
    "summarize-execution-query",
];
const RECENT_FAILURE_TRIAGE_SKILLS: [&str; 3] = [
    "inspect-http-json",
    "summarize-execution-query",
    "explain-execution",
];
const POLICY_DENIAL_DEBUG_SKILLS: [&str; 4] = [
    "explain-execution",
    "explain-capability-denial",
    "diff-execution-authority",
    "explain-http-authority",
];
const EXECUTION_TREE_SCENARIO_SKILLS: [&str; 3] =
    ["hello-inspect", "hello-composite", "explain-execution-tree"];
const EXPLAIN_EXECUTION_ONLY: [CodexSmokeSelection; 1] = [CodexSmokeSelection::ExplainExecution];
const EXPLAIN_EXECUTION_TREE_ONLY: [CodexSmokeSelection; 1] =
    [CodexSmokeSelection::ExplainExecutionTree];
const INCIDENT_BRIEF_ONLY: [CodexSmokeSelection; 1] = [CodexSmokeSelection::IncidentBrief];
const RUN_DIFF_ONLY: [CodexSmokeSelection; 1] = [CodexSmokeSelection::RunDiff];
const RECENT_FAILURES_ONLY: [CodexSmokeSelection; 1] = [CodexSmokeSelection::RecentFailures];
const EVIDENCE_SUMMARY_ONLY: [CodexSmokeSelection; 1] = [CodexSmokeSelection::EvidenceSummary];
const RENDER_REPORT_ONLY: [CodexSmokeSelection; 1] = [CodexSmokeSelection::RenderReport];
const RECENT_FAILURE_TRIAGE_ONLY: [CodexSmokeSelection; 1] =
    [CodexSmokeSelection::RecentFailureTriage];
const POLICY_DENIAL_DEBUG_ONLY: [CodexSmokeSelection; 1] = [CodexSmokeSelection::PolicyDenialDebug];
const ALL_CODEX_SMOKE_FLOWS: [CodexSmokeSelection; 9] = [
    CodexSmokeSelection::IncidentBrief,
    CodexSmokeSelection::RunDiff,
    CodexSmokeSelection::RecentFailures,
    CodexSmokeSelection::EvidenceSummary,
    CodexSmokeSelection::RenderReport,
    CodexSmokeSelection::ExplainExecution,
    CodexSmokeSelection::ExplainExecutionTree,
    CodexSmokeSelection::RecentFailureTriage,
    CodexSmokeSelection::PolicyDenialDebug,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<PathBuf>,
    pub command: String,
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CodexConfigWriteStatus {
    Updated,
    Unchanged,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct CodexConfigWriteResult {
    pub path: PathBuf,
    pub status: CodexConfigWriteStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct CodexBootstrapOutput {
    pub bootstrap: CodexBootstrapSummary,
    pub config: CodexServerConfig,
    pub print_config_command: String,
    pub recommended_scenario_commands: Vec<String>,
    pub recommended_smoke_commands: Vec<String>,
    pub recommended_proof_commands: Vec<String>,
}

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord,
)]
#[serde(rename_all = "kebab-case")]
pub enum CodexScenarioSelection {
    RecentFailureTriage,
    PolicyDenialDebug,
    ExecutionTree,
}

impl std::fmt::Display for CodexScenarioSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Self::RecentFailureTriage => "recent-failure-triage",
            Self::PolicyDenialDebug => "policy-denial-debug",
            Self::ExecutionTree => "execution-tree",
        };
        f.write_str(value)
    }
}

impl FromStr for CodexScenarioSelection {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "recent-failure-triage" => Ok(Self::RecentFailureTriage),
            "policy-denial-debug" => Ok(Self::PolicyDenialDebug),
            "execution-tree" => Ok(Self::ExecutionTree),
            _ => Err(format!(
                "unknown scenario `{value}`; expected recent-failure-triage, policy-denial-debug, or execution-tree"
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct CodexScenarioSummary {
    pub registry_root: PathBuf,
    pub scenario: CodexScenarioSelection,
    pub installed_skills: Vec<BootstrappedSkill>,
    pub subject_execution_uris: Vec<String>,
    #[serde(default)]
    pub comparison_execution_uris: Vec<String>,
    #[serde(default)]
    pub query_uris: Vec<String>,
    #[serde(default)]
    pub candidate_urls: Vec<String>,
    pub recommended_codex_ask: String,
}

impl CodexScenarioSummary {
    #[must_use]
    pub fn render_text(&self) -> String {
        let mut output = String::new();
        let _ = writeln!(output, "Guild Codex scenario ready.");
        let _ = writeln!(output, "registry root: {}", self.registry_root.display());
        let _ = writeln!(output, "scenario: {}", self.scenario);

        if !self.installed_skills.is_empty() {
            let _ = writeln!(output);
            let _ = writeln!(output, "installed skills:");
            for skill in &self.installed_skills {
                let _ = writeln!(
                    output,
                    "- {}/{}@{} ({})",
                    skill.namespace, skill.name, skill.version, skill.digest
                );
            }
        }

        if !self.subject_execution_uris.is_empty() {
            let _ = writeln!(output);
            let _ = writeln!(output, "subject execution URIs:");
            for uri in &self.subject_execution_uris {
                let _ = writeln!(output, "- {uri}");
            }
        }

        if !self.comparison_execution_uris.is_empty() {
            let _ = writeln!(output);
            let _ = writeln!(output, "comparison execution URIs:");
            for uri in &self.comparison_execution_uris {
                let _ = writeln!(output, "- {uri}");
            }
        }

        if !self.query_uris.is_empty() {
            let _ = writeln!(output);
            let _ = writeln!(output, "query URIs:");
            for uri in &self.query_uris {
                let _ = writeln!(output, "- {uri}");
            }
        }

        if !self.candidate_urls.is_empty() {
            let _ = writeln!(output);
            let _ = writeln!(output, "candidate URLs:");
            for url in &self.candidate_urls {
                let _ = writeln!(output, "- {url}");
            }
        }

        let _ = writeln!(output);
        let _ = writeln!(output, "recommended Codex ask:");
        let _ = writeln!(output, "{}", self.recommended_codex_ask);

        output.trim_end().into()
    }
}

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord,
)]
#[serde(rename_all = "kebab-case")]
pub enum CodexSmokeSelection {
    IncidentBrief,
    RunDiff,
    RecentFailures,
    EvidenceSummary,
    RenderReport,
    ExplainExecution,
    ExplainExecutionTree,
    RecentFailureTriage,
    PolicyDenialDebug,
    All,
}

impl CodexSmokeSelection {
    #[must_use]
    pub fn flows(self) -> &'static [Self] {
        match self {
            Self::IncidentBrief => &INCIDENT_BRIEF_ONLY,
            Self::RunDiff => &RUN_DIFF_ONLY,
            Self::RecentFailures => &RECENT_FAILURES_ONLY,
            Self::EvidenceSummary => &EVIDENCE_SUMMARY_ONLY,
            Self::RenderReport => &RENDER_REPORT_ONLY,
            Self::ExplainExecution => &EXPLAIN_EXECUTION_ONLY,
            Self::ExplainExecutionTree => &EXPLAIN_EXECUTION_TREE_ONLY,
            Self::RecentFailureTriage => &RECENT_FAILURE_TRIAGE_ONLY,
            Self::PolicyDenialDebug => &POLICY_DENIAL_DEBUG_ONLY,
            Self::All => &ALL_CODEX_SMOKE_FLOWS,
        }
    }
}

impl std::fmt::Display for CodexSmokeSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Self::IncidentBrief => "incident-brief",
            Self::RunDiff => "run-diff",
            Self::RecentFailures => "recent-failures",
            Self::EvidenceSummary => "evidence-summary",
            Self::RenderReport => "render-report",
            Self::ExplainExecution => "explain-execution",
            Self::ExplainExecutionTree => "explain-execution-tree",
            Self::RecentFailureTriage => "recent-failure-triage",
            Self::PolicyDenialDebug => "policy-denial-debug",
            Self::All => "all",
        };
        f.write_str(value)
    }
}

impl FromStr for CodexSmokeSelection {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "incident-brief" => Ok(Self::IncidentBrief),
            "run-diff" => Ok(Self::RunDiff),
            "recent-failures" => Ok(Self::RecentFailures),
            "evidence-summary" => Ok(Self::EvidenceSummary),
            "render-report" => Ok(Self::RenderReport),
            "explain-execution" => Ok(Self::ExplainExecution),
            "explain-execution-tree" => Ok(Self::ExplainExecutionTree),
            "recent-failure-triage" => Ok(Self::RecentFailureTriage),
            "policy-denial-debug" => Ok(Self::PolicyDenialDebug),
            "all" => Ok(Self::All),
            _ => Err(format!(
                "unknown flow `{value}`; expected incident-brief, run-diff, recent-failures, evidence-summary, render-report, explain-execution, explain-execution-tree, recent-failure-triage, policy-denial-debug, or all"
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct CodexSmokeFlowSummary {
    pub flow: CodexSmokeSelection,
    pub subject_execution_uri: String,
    pub report_execution_uri: String,
    #[serde(default)]
    pub additional_report_execution_uris: Vec<String>,
    #[serde(default)]
    pub comparison_execution_uris: Vec<String>,
    #[serde(default)]
    pub subject_query_uri: Option<String>,
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
            if !flow.additional_report_execution_uris.is_empty() {
                let _ = writeln!(output, "additional report execution URIs:");
                for uri in &flow.additional_report_execution_uris {
                    let _ = writeln!(output, "- {uri}");
                }
            }
            if !flow.comparison_execution_uris.is_empty() {
                let _ = writeln!(output, "comparison execution URIs:");
                for uri in &flow.comparison_execution_uris {
                    let _ = writeln!(output, "- {uri}");
                }
            }
            if let Some(query_uri) = &flow.subject_query_uri {
                let _ = writeln!(output, "subject query uri: {query_uri}");
            }
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

impl From<McpError> for CodexWorkflowError {
    fn from(value: McpError) -> Self {
        let receipt = value.receipt.as_ref().map(|receipt| {
            json!({
                "uri": receipt.uri,
                "execution_id": receipt.execution_id,
            })
        });
        let mut error = Self::new(value.code, value.message);
        let detail = match (value.detail, receipt) {
            (Some(detail), Some(receipt)) => Some(json!({
                "detail": detail,
                "receipt": receipt,
            })),
            (Some(detail), None) => Some(*detail),
            (None, Some(receipt)) => Some(json!({ "receipt": receipt })),
            (None, None) => None,
        };
        if let Some(detail) = detail {
            error = error.with_detail(detail);
        }
        error
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

/// Resolve the operator default Guild registry root.
///
/// # Errors
///
/// Returns an error if the current user's home directory cannot be resolved.
pub fn default_registry_root() -> Result<PathBuf, CodexWorkflowError> {
    paths::default_registry_root().map_err(|error| {
        CodexWorkflowError::new(
            "codex-default-root-unavailable",
            "failed to resolve the default Guild root",
        )
        .with_detail(json!({ "cause": error.to_string() }))
    })
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
        "{CLI_BINARY_NAME} codex print-config --registry-root {}",
        shell_quote(&absolute_path(registry_root).to_string_lossy())
    )
}

#[must_use]
pub fn recommended_scenario_commands(registry_root: impl AsRef<Path>) -> Vec<String> {
    let registry_root = absolute_path(registry_root);
    [
        CodexScenarioSelection::RecentFailureTriage,
        CodexScenarioSelection::PolicyDenialDebug,
    ]
    .into_iter()
    .map(|scenario| {
        format!(
            "{CLI_BINARY_NAME} codex scenario --registry-root {} --scenario {} --json",
            shell_quote(&registry_root.to_string_lossy()),
            scenario
        )
    })
    .collect()
}

#[must_use]
pub fn recommended_smoke_commands(registry_root: impl AsRef<Path>) -> Vec<String> {
    let registry_root = absolute_path(registry_root);
    [
        CodexSmokeSelection::IncidentBrief,
        CodexSmokeSelection::RunDiff,
        CodexSmokeSelection::RecentFailures,
        CodexSmokeSelection::EvidenceSummary,
        CodexSmokeSelection::RenderReport,
        CodexSmokeSelection::ExplainExecution,
        CodexSmokeSelection::ExplainExecutionTree,
        CodexSmokeSelection::RecentFailureTriage,
        CodexSmokeSelection::PolicyDenialDebug,
    ]
    .into_iter()
    .map(|flow| {
        format!(
            "{CLI_BINARY_NAME} codex smoke --registry-root {} --flow {}",
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
    let skills = ensure_example_skills_installed(&registry_root, &DEFAULT_CODEX_SKILLS)?;

    Ok(CodexBootstrapSummary {
        repo_root,
        registry_root,
        skills,
    })
}

/// Prepare one deterministic local Codex dogfood scenario and return the
/// resulting execution/query URIs plus one recommended Codex ask string.
///
/// # Errors
///
/// Returns an error if the requested registry root cannot be opened, the
/// required example skills cannot be installed, or the scenario cannot be
/// seeded into durable Guild resources.
pub fn prepare_codex_scenario(
    registry_root: impl AsRef<Path>,
    scenario: CodexScenarioSelection,
) -> Result<CodexScenarioSummary, CodexWorkflowError> {
    let registry_root = absolute_path(registry_root);

    match scenario {
        CodexScenarioSelection::RecentFailureTriage => {
            prepare_recent_failure_triage_scenario(&registry_root)
        }
        CodexScenarioSelection::PolicyDenialDebug => {
            prepare_policy_denial_debug_scenario(&registry_root)
        }
        CodexScenarioSelection::ExecutionTree => prepare_execution_tree_scenario(&registry_root),
    }
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
        cwd: Some(repo_root()),
        command: "cargo".into(),
        args: vec![
            "run".into(),
            "-q".into(),
            "--manifest-path".into(),
            manifest_path.to_string_lossy().into_owned(),
            "--bin".into(),
            CLI_BINARY_NAME.into(),
            "--".into(),
            "mcp".into(),
            "serve".into(),
            "--stdio".into(),
        ],
        env,
    }
}

/// Resolve the absolute path to the running `guild` binary.
///
/// # Errors
///
/// Returns an error if the current executable path cannot be discovered or
/// canonicalized.
pub fn running_guild_binary() -> Result<PathBuf, CodexWorkflowError> {
    let path = std::env::current_exe().map_err(|error| {
        CodexWorkflowError::new(
            "codex-guild-binary-unavailable",
            "failed to resolve the running `guild` binary path",
        )
        .with_detail(json!({ "cause": error.to_string() }))
    })?;

    path.canonicalize().map_err(|error| {
        CodexWorkflowError::new(
            "codex-guild-binary-open-failed",
            "failed to canonicalize the running `guild` binary path",
        )
        .with_detail(json!({
            "path": path.display().to_string(),
            "cause": error.to_string(),
        }))
    })
}

/// Build the persistent Codex stdio server configuration for an installed
/// `guild` binary.
///
/// # Errors
///
/// Returns an error if the requested `guild` binary path cannot be
/// canonicalized or the default Guild root cannot be resolved.
pub fn installed_guild_server_config(
    registry_root: impl AsRef<Path>,
    name: impl Into<String>,
    guild_binary: impl AsRef<Path>,
) -> Result<CodexServerConfig, CodexWorkflowError> {
    let registry_root = absolute_path(registry_root);
    let default_registry_root = default_registry_root()?;
    let guild_binary = guild_binary.as_ref().canonicalize().map_err(|error| {
        CodexWorkflowError::new(
            "codex-guild-binary-open-failed",
            "failed to canonicalize the requested `guild` binary path",
        )
        .with_detail(json!({
            "path": guild_binary.as_ref().display().to_string(),
            "cause": error.to_string(),
        }))
    })?;

    let mut args = Vec::new();
    if registry_root != default_registry_root {
        args.push("--registry-root".into());
        args.push(registry_root.to_string_lossy().into_owned());
    }
    args.extend(["mcp", "serve", "--stdio"].into_iter().map(str::to_owned));

    Ok(CodexServerConfig {
        name: name.into(),
        cwd: None,
        command: guild_binary.to_string_lossy().into_owned(),
        args,
        env: BTreeMap::new(),
    })
}

/// Upsert one Guild MCP server entry into a Codex TOML config file.
///
/// # Errors
///
/// Returns an error if the existing config cannot be read or parsed, if the
/// parent directory cannot be created, or if the updated TOML cannot be
/// written back to disk.
pub fn write_codex_config(
    path: impl AsRef<Path>,
    config: &CodexServerConfig,
) -> Result<CodexConfigWriteResult, CodexWorkflowError> {
    let path = path.as_ref().to_path_buf();
    let original = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => {
            return Err(CodexWorkflowError::new(
                "codex-config-read-failed",
                "failed to read Codex config file",
            )
            .with_detail(json!({
                "path": path.display().to_string(),
                "cause": error.to_string(),
            })));
        }
    };

    let mut document = if original.is_empty() {
        DocumentMut::new()
    } else {
        original.parse::<DocumentMut>().map_err(|error| {
            CodexWorkflowError::new(
                "codex-config-parse-failed",
                "failed to parse Codex config TOML",
            )
            .with_detail(json!({
                "path": path.display().to_string(),
                "cause": error.to_string(),
            }))
        })?
    };
    upsert_server_config(&mut document, config)?;
    let updated = document.to_string();
    let status = if updated == original {
        CodexConfigWriteStatus::Unchanged
    } else {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                CodexWorkflowError::new(
                    "codex-config-parent-create-failed",
                    "failed to create the Codex config parent directory",
                )
                .with_detail(json!({
                    "path": parent.display().to_string(),
                    "cause": error.to_string(),
                }))
            })?;
        }
        fs::write(&path, updated).map_err(|error| {
            CodexWorkflowError::new(
                "codex-config-write-failed",
                "failed to write Codex config TOML",
            )
            .with_detail(json!({
                "path": path.display().to_string(),
                "cause": error.to_string(),
            }))
        })?;
        CodexConfigWriteStatus::Updated
    };

    Ok(CodexConfigWriteResult { path, status })
}

impl CodexServerConfig {
    /// Render the exact local command used to launch the Guild stdio MCP server
    /// outside Codex.
    #[must_use]
    pub fn manual_server_command(&self) -> String {
        match self.quoted_env_assignments() {
            Some(env) => format!("{env} {}", self.quoted_command_line()),
            None => self.quoted_command_line(),
        }
    }

    /// Render the `codex mcp add` command matching this local stdio server
    /// configuration.
    #[must_use]
    pub fn codex_mcp_add_command(&self) -> String {
        let mut command = format!("codex mcp add {}", shell_quote(&self.name));
        for (key, value) in &self.env {
            let _ = write!(command, " --env {}", shell_quote(&format!("{key}={value}")));
        }
        let _ = write!(command, " -- {}", self.quoted_command_line());
        command
    }

    /// Render just the TOML snippet for this Codex MCP server entry.
    ///
    /// # Panics
    ///
    /// Panics if this in-memory server config cannot be represented as TOML.
    /// That indicates a programming error in Guild's own config renderer.
    #[must_use]
    pub fn config_toml(&self) -> String {
        let mut document = DocumentMut::new();
        upsert_server_config(&mut document, self)
            .expect("Codex server config always renders to valid TOML");
        document.to_string().trim_end().to_owned()
    }

    #[must_use]
    pub fn registry_root_display(&self) -> String {
        if let Some(path) = self.env.get("GUILD_REGISTRY_ROOT") {
            return path.clone();
        }

        if let Some(index) = self.args.iter().position(|arg| arg == "--registry-root")
            && let Some(path) = self.args.get(index + 1)
        {
            return path.clone();
        }

        default_registry_root()
            .map_or_else(|_| "~/.guild".into(), |path| path.display().to_string())
    }

    fn quoted_command_line(&self) -> String {
        let mut command = shell_quote(&self.command);
        for arg in &self.args {
            let _ = write!(command, " {}", shell_quote(arg));
        }
        command
    }

    fn quoted_env_assignments(&self) -> Option<String> {
        if self.env.is_empty() {
            return None;
        }

        Some(
            self.env
                .iter()
                .map(|(key, value)| format!("{key}={}", shell_quote(value)))
                .collect::<Vec<_>>()
                .join(" "),
        )
    }
}

fn upsert_server_config(
    document: &mut DocumentMut,
    config: &CodexServerConfig,
) -> Result<(), CodexWorkflowError> {
    if !matches!(document.get("mcp_servers"), None | Some(Item::Table(_))) {
        return Err(CodexWorkflowError::new(
            "codex-config-invalid",
            "existing `mcp_servers` config was not a TOML table",
        ));
    }

    if document.get("mcp_servers").is_none() {
        document["mcp_servers"] = Item::Table(Table::new());
    }

    let mcp_servers = document["mcp_servers"]
        .as_table_mut()
        .expect("mcp_servers was validated as a table");

    if !matches!(mcp_servers.get(&config.name), None | Some(Item::Table(_))) {
        return Err(CodexWorkflowError::new(
            "codex-config-invalid",
            "existing MCP server entry was not a TOML table",
        )
        .with_detail(json!({ "server_name": config.name })));
    }

    if mcp_servers.get(&config.name).is_none() {
        mcp_servers[&config.name] = Item::Table(Table::new());
    }

    let server = mcp_servers[&config.name]
        .as_table_mut()
        .expect("server entry was validated as a table");
    server["command"] = Item::Value(TomlValue::from(config.command.as_str()));

    let mut args = Array::new();
    for arg in &config.args {
        args.push(arg.as_str());
    }
    server["args"] = Item::Value(TomlValue::Array(args));

    if let Some(cwd) = &config.cwd {
        server["cwd"] = Item::Value(TomlValue::from(cwd.to_string_lossy().as_ref()));
    } else {
        server.remove("cwd");
    }

    if config.env.is_empty() {
        server.remove("env");
    } else {
        let mut env = InlineTable::new();
        for (key, value) in &config.env {
            env.insert(key.as_str(), TomlValue::from(value.as_str()));
        }
        server["env"] = Item::Value(TomlValue::InlineTable(env));
    }

    Ok(())
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
    let mut client = McpStdioClient::spawn(
        &config.command,
        &config.args,
        config.cwd.as_deref(),
        &config.env,
    )?;
    let initialized = client.initialize("codex-workflow-smoke")?;

    let flows = selection
        .flows()
        .iter()
        .copied()
        .map(|flow| run_single_codex_smoke_flow(&registry_root, &mut client, flow))
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
        CodexSmokeSelection::IncidentBrief => &["render-report", "incident-brief"],
        CodexSmokeSelection::RunDiff => &["render-report", "run-diff"],
        CodexSmokeSelection::RecentFailures => &["recent-failures"],
        CodexSmokeSelection::EvidenceSummary => &["hello-inspect", "evidence-summary"],
        CodexSmokeSelection::RenderReport => &["render-report"],
        CodexSmokeSelection::ExplainExecution => &["hello-inspect", "explain-execution"],
        CodexSmokeSelection::ExplainExecutionTree => {
            &["hello-inspect", "hello-composite", "explain-execution-tree"]
        }
        CodexSmokeSelection::RecentFailureTriage | CodexSmokeSelection::PolicyDenialDebug => &[],
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
    registry_root: &Path,
    client: &mut McpStdioClient,
    flow: CodexSmokeSelection,
) -> Result<CodexSmokeFlowSummary, CodexWorkflowError> {
    match flow {
        CodexSmokeSelection::IncidentBrief => run_incident_brief_smoke(registry_root, client),
        CodexSmokeSelection::RunDiff => run_run_diff_smoke(registry_root, client),
        CodexSmokeSelection::RecentFailures => run_recent_failures_smoke(registry_root, client),
        CodexSmokeSelection::EvidenceSummary => run_evidence_summary_smoke(client),
        CodexSmokeSelection::RenderReport => run_render_report_smoke(client),
        CodexSmokeSelection::ExplainExecution => run_explain_execution_smoke(client),
        CodexSmokeSelection::ExplainExecutionTree => run_explain_execution_tree_smoke(client),
        CodexSmokeSelection::RecentFailureTriage => {
            run_recent_failure_triage_smoke(registry_root, client)
        }
        CodexSmokeSelection::PolicyDenialDebug => {
            run_policy_denial_debug_smoke(registry_root, client)
        }
        CodexSmokeSelection::All => unreachable!("all expands before per-flow execution"),
    }
}

fn run_incident_brief_smoke(
    registry_root: &Path,
    client: &mut McpStdioClient,
) -> Result<CodexSmokeFlowSummary, CodexWorkflowError> {
    let scenario =
        prepare_codex_scenario(registry_root, CodexScenarioSelection::RecentFailureTriage)?;
    let subject_execution_uri = scenario
        .subject_execution_uris
        .first()
        .cloned()
        .ok_or_else(|| {
            CodexWorkflowError::new(
                "codex-smoke-missing-subject-execution",
                "incident-brief smoke did not get a subject execution URI",
            )
        })?;

    let response_value = client.request(
        "tools/call",
        &example_inspect_request(
            "incident-brief",
            &json!({ "execution_uri": subject_execution_uri }),
            &[execution_read_grant(), render_report_invoke_grant()],
        ),
    )?;
    let response: CallToolResult = McpStdioClient::parse_result(&response_value)?;
    let record = parse_execution_record(&response)?;
    let _ = output_markdown_string(
        &record,
        "codex-smoke-incident-brief-missing-output",
        "incident-brief did not return markdown output",
    )?;

    let subject_resource_value =
        client.request("resources/read", &json!({ "uri": subject_execution_uri }))?;
    let subject_resource: ReadResourceResult =
        McpStdioClient::parse_result(&subject_resource_value)?;
    let report_resource_value =
        client.request("resources/read", &json!({ "uri": record.receipt.uri }))?;
    let report_resource: ReadResourceResult = McpStdioClient::parse_result(&report_resource_value)?;

    Ok(CodexSmokeFlowSummary {
        flow: CodexSmokeSelection::IncidentBrief,
        subject_execution_uri,
        report_execution_uri: record.receipt.uri,
        additional_report_execution_uris: Vec::new(),
        comparison_execution_uris: scenario
            .subject_execution_uris
            .iter()
            .skip(1)
            .cloned()
            .collect(),
        subject_query_uri: scenario.query_uris.first().cloned(),
        subject_resource_items: subject_resource.contents.len(),
        report_resource_items: report_resource.contents.len(),
        subject_emitted_evidence: 0,
        subject_child_executions: 0,
        report_summary: record
            .output
            .ok_or_else(|| {
                CodexWorkflowError::new(
                    "codex-smoke-report-missing-output",
                    "incident-brief did not return skill output",
                )
            })?
            .summary,
    })
}

fn run_run_diff_smoke(
    registry_root: &Path,
    client: &mut McpStdioClient,
) -> Result<CodexSmokeFlowSummary, CodexWorkflowError> {
    let scenario =
        prepare_codex_scenario(registry_root, CodexScenarioSelection::RecentFailureTriage)?;
    let left_execution_uri = scenario
        .subject_execution_uris
        .first()
        .cloned()
        .ok_or_else(|| {
            CodexWorkflowError::new(
                "codex-smoke-missing-left-execution",
                "run-diff smoke did not get a left execution URI",
            )
        })?;
    let right_execution_uri = scenario
        .subject_execution_uris
        .get(1)
        .cloned()
        .ok_or_else(|| {
            CodexWorkflowError::new(
                "codex-smoke-missing-right-execution",
                "run-diff smoke did not get a right execution URI",
            )
        })?;

    let response_value = client.request(
        "tools/call",
        &example_inspect_request(
            "run-diff",
            &json!({
                "left_execution_uri": left_execution_uri,
                "right_execution_uri": right_execution_uri,
            }),
            &[execution_read_grant(), render_report_invoke_grant()],
        ),
    )?;
    let response: CallToolResult = McpStdioClient::parse_result(&response_value)?;
    let record = parse_execution_record(&response)?;
    let _ = output_markdown_string(
        &record,
        "codex-smoke-run-diff-missing-output",
        "run-diff did not return markdown output",
    )?;

    let subject_resource_value =
        client.request("resources/read", &json!({ "uri": left_execution_uri }))?;
    let subject_resource: ReadResourceResult =
        McpStdioClient::parse_result(&subject_resource_value)?;
    let report_resource_value =
        client.request("resources/read", &json!({ "uri": record.receipt.uri }))?;
    let report_resource: ReadResourceResult = McpStdioClient::parse_result(&report_resource_value)?;

    Ok(CodexSmokeFlowSummary {
        flow: CodexSmokeSelection::RunDiff,
        subject_execution_uri: left_execution_uri,
        report_execution_uri: record.receipt.uri,
        additional_report_execution_uris: Vec::new(),
        comparison_execution_uris: vec![right_execution_uri],
        subject_query_uri: scenario.query_uris.first().cloned(),
        subject_resource_items: subject_resource.contents.len(),
        report_resource_items: report_resource.contents.len(),
        subject_emitted_evidence: 0,
        subject_child_executions: 0,
        report_summary: record
            .output
            .ok_or_else(|| {
                CodexWorkflowError::new(
                    "codex-smoke-report-missing-output",
                    "run-diff did not return skill output",
                )
            })?
            .summary,
    })
}

fn run_recent_failures_smoke(
    registry_root: &Path,
    client: &mut McpStdioClient,
) -> Result<CodexSmokeFlowSummary, CodexWorkflowError> {
    let scenario =
        prepare_codex_scenario(registry_root, CodexScenarioSelection::RecentFailureTriage)?;
    let subject_execution_uri = scenario
        .subject_execution_uris
        .first()
        .cloned()
        .ok_or_else(|| {
            CodexWorkflowError::new(
                "codex-smoke-missing-subject-execution",
                "recent-failures smoke did not get a subject execution URI",
            )
        })?;
    let query_uri = scenario.query_uris.first().cloned().ok_or_else(|| {
        CodexWorkflowError::new(
            "codex-smoke-missing-query-uri",
            "recent-failures smoke did not get a query URI",
        )
    })?;

    let response_value = client.request(
        "tools/call",
        &example_inspect_request(
            "recent-failures",
            &json!({ "query_uri": query_uri }),
            &[query_read_grant(), execution_read_grant()],
        ),
    )?;
    let response: CallToolResult = McpStdioClient::parse_result(&response_value)?;
    let record = parse_execution_record(&response)?;
    let _ = output_markdown_string(
        &record,
        "codex-smoke-recent-failures-missing-output",
        "recent-failures did not return markdown output",
    )?;

    let subject_resource_value =
        client.request("resources/read", &json!({ "uri": subject_execution_uri }))?;
    let subject_resource: ReadResourceResult =
        McpStdioClient::parse_result(&subject_resource_value)?;
    let report_resource_value =
        client.request("resources/read", &json!({ "uri": record.receipt.uri }))?;
    let report_resource: ReadResourceResult = McpStdioClient::parse_result(&report_resource_value)?;

    Ok(CodexSmokeFlowSummary {
        flow: CodexSmokeSelection::RecentFailures,
        subject_execution_uri,
        report_execution_uri: record.receipt.uri,
        additional_report_execution_uris: Vec::new(),
        comparison_execution_uris: scenario
            .subject_execution_uris
            .iter()
            .skip(1)
            .cloned()
            .chain(scenario.comparison_execution_uris.iter().cloned())
            .collect(),
        subject_query_uri: Some(query_uri),
        subject_resource_items: subject_resource.contents.len(),
        report_resource_items: report_resource.contents.len(),
        subject_emitted_evidence: 0,
        subject_child_executions: 0,
        report_summary: record
            .output
            .ok_or_else(|| {
                CodexWorkflowError::new(
                    "codex-smoke-report-missing-output",
                    "recent-failures did not return skill output",
                )
            })?
            .summary,
    })
}

fn run_evidence_summary_smoke(
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
    let evidence_uri = hello_record
        .emitted_evidence
        .first()
        .map(|evidence| evidence.uri.clone())
        .ok_or_else(|| {
            CodexWorkflowError::new(
                "codex-smoke-missing-evidence",
                "hello-inspect smoke did not emit evidence for evidence-summary",
            )
        })?;

    let response_value = client.request(
        "tools/call",
        &example_inspect_request(
            "evidence-summary",
            &json!({ "evidence_uri": evidence_uri }),
            &[object_read_grant()],
        ),
    )?;
    let response: CallToolResult = McpStdioClient::parse_result(&response_value)?;
    let record = parse_execution_record(&response)?;
    let _ = output_markdown_string(
        &record,
        "codex-smoke-evidence-summary-missing-output",
        "evidence-summary did not return markdown output",
    )?;

    let subject_resource_value = client.request(
        "resources/read",
        &json!({ "uri": hello_record.receipt.uri }),
    )?;
    let subject_resource: ReadResourceResult =
        McpStdioClient::parse_result(&subject_resource_value)?;
    let report_resource_value =
        client.request("resources/read", &json!({ "uri": record.receipt.uri }))?;
    let report_resource: ReadResourceResult = McpStdioClient::parse_result(&report_resource_value)?;

    Ok(CodexSmokeFlowSummary {
        flow: CodexSmokeSelection::EvidenceSummary,
        subject_execution_uri: hello_record.receipt.uri,
        report_execution_uri: record.receipt.uri,
        additional_report_execution_uris: Vec::new(),
        comparison_execution_uris: Vec::new(),
        subject_query_uri: None,
        subject_resource_items: subject_resource.contents.len(),
        report_resource_items: report_resource.contents.len(),
        subject_emitted_evidence: hello_record.emitted_evidence.len(),
        subject_child_executions: hello_record.child_executions.len(),
        report_summary: record
            .output
            .ok_or_else(|| {
                CodexWorkflowError::new(
                    "codex-smoke-report-missing-output",
                    "evidence-summary did not return skill output",
                )
            })?
            .summary,
    })
}

fn run_render_report_smoke(
    client: &mut McpStdioClient,
) -> Result<CodexSmokeFlowSummary, CodexWorkflowError> {
    let response_value = client.request(
        "tools/call",
        &example_inspect_request(
            "render-report",
            &json!({
                "title": "Smoke Report",
                "summary_line": "failed  upper-bound  exec:abc123  example/inspect-http-json@0.1.0",
                "facts": [
                    { "label": "Status", "value": "failed" },
                    { "label": "Skill", "value": "example/inspect-http-json@0.1.0" }
                ],
                "sections": [
                    {
                        "title": "Primary reason",
                        "lines": [
                            "runtime-exec:http-method-not-allowed",
                            "POST was requested against a GET-only grant"
                        ]
                    }
                ]
            }),
            &[],
        ),
    )?;
    let response: CallToolResult = McpStdioClient::parse_result(&response_value)?;
    let record = parse_execution_record(&response)?;
    let _ = output_markdown_string(
        &record,
        "codex-smoke-render-report-missing-output",
        "render-report did not return markdown output",
    )?;

    let resource_value = client.request("resources/read", &json!({ "uri": record.receipt.uri }))?;
    let resource: ReadResourceResult = McpStdioClient::parse_result(&resource_value)?;

    Ok(CodexSmokeFlowSummary {
        flow: CodexSmokeSelection::RenderReport,
        subject_execution_uri: record.receipt.uri.clone(),
        report_execution_uri: record.receipt.uri,
        additional_report_execution_uris: Vec::new(),
        comparison_execution_uris: Vec::new(),
        subject_query_uri: None,
        subject_resource_items: resource.contents.len(),
        report_resource_items: resource.contents.len(),
        subject_emitted_evidence: 0,
        subject_child_executions: 0,
        report_summary: record
            .output
            .ok_or_else(|| {
                CodexWorkflowError::new(
                    "codex-smoke-report-missing-output",
                    "render-report did not return skill output",
                )
            })?
            .summary,
    })
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
        additional_report_execution_uris: Vec::new(),
        comparison_execution_uris: Vec::new(),
        subject_query_uri: None,
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
        additional_report_execution_uris: Vec::new(),
        comparison_execution_uris: Vec::new(),
        subject_query_uri: None,
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

type LocalFacade = GuildMcpFacade<LocalRegistry, WasmtimeRuntimeAdapter>;

fn run_recent_failure_triage_smoke(
    registry_root: &Path,
    client: &mut McpStdioClient,
) -> Result<CodexSmokeFlowSummary, CodexWorkflowError> {
    let scenario =
        prepare_codex_scenario(registry_root, CodexScenarioSelection::RecentFailureTriage)?;
    let subject_execution_uri = scenario
        .subject_execution_uris
        .first()
        .cloned()
        .ok_or_else(|| {
            CodexWorkflowError::new(
                "codex-smoke-missing-subject-execution",
                "recent-failure-triage scenario did not produce a subject execution URI",
            )
        })?;
    let query_uri = scenario.query_uris.first().cloned().ok_or_else(|| {
        CodexWorkflowError::new(
            "codex-smoke-missing-query-uri",
            "recent-failure-triage scenario did not produce a query URI",
        )
    })?;

    let summarize_response_value = client.request(
        "tools/call",
        &example_inspect_request(
            "summarize-execution-query",
            &json!({ "query_uri": query_uri }),
            &[query_read_grant()],
        ),
    )?;
    let summarize_response: CallToolResult =
        McpStdioClient::parse_result(&summarize_response_value)?;
    let summarize_record = parse_execution_record(&summarize_response)?;
    let summarize_output = output_structured_value(
        &summarize_record,
        "codex-smoke-query-summary-missing-output",
        "summarize-execution-query did not return structured output",
    )?;
    let returned_matches = summarize_output
        .get("returned_matches")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            CodexWorkflowError::new(
                "codex-smoke-query-summary-invalid",
                "summarize-execution-query did not report returned_matches",
            )
        })?;
    if returned_matches == 0 {
        return Err(CodexWorkflowError::new(
            "codex-smoke-query-summary-empty",
            "recent-failure-triage summary returned zero matches",
        ));
    }

    let subject_resource_value =
        client.request("resources/read", &json!({ "uri": subject_execution_uri }))?;
    let subject_resource: ReadResourceResult =
        McpStdioClient::parse_result(&subject_resource_value)?;
    let report_resource_value = client.request(
        "resources/read",
        &json!({ "uri": summarize_record.receipt.uri }),
    )?;
    let report_resource: ReadResourceResult = McpStdioClient::parse_result(&report_resource_value)?;

    Ok(CodexSmokeFlowSummary {
        flow: CodexSmokeSelection::RecentFailureTriage,
        subject_execution_uri,
        report_execution_uri: summarize_record.receipt.uri,
        additional_report_execution_uris: Vec::new(),
        comparison_execution_uris: scenario
            .subject_execution_uris
            .iter()
            .skip(1)
            .cloned()
            .chain(scenario.comparison_execution_uris.iter().cloned())
            .collect(),
        subject_query_uri: Some(query_uri),
        subject_resource_items: subject_resource.contents.len(),
        report_resource_items: report_resource.contents.len(),
        subject_emitted_evidence: 0,
        subject_child_executions: 0,
        report_summary: summarize_record
            .output
            .ok_or_else(|| {
                CodexWorkflowError::new(
                    "codex-smoke-report-missing-output",
                    "summarize-execution-query did not return skill output",
                )
            })?
            .summary,
    })
}

fn run_policy_denial_debug_smoke(
    registry_root: &Path,
    client: &mut McpStdioClient,
) -> Result<CodexSmokeFlowSummary, CodexWorkflowError> {
    let inputs = prepare_policy_denial_smoke_inputs(registry_root)?;
    let denial_record = run_capability_denial_report(client, &inputs.denied_execution_uri)?;
    let diff_record = run_authority_diff_report(
        client,
        &inputs.trusted_execution_uri,
        &inputs.restricted_execution_uri,
    )?;
    let http_record =
        run_http_authority_report(client, &inputs.denied_execution_uri, &inputs.candidate_url)?;
    let (subject_resource_items, report_resource_items) = read_subject_and_report_resource_counts(
        client,
        &inputs.denied_execution_uri,
        &denial_record.receipt.uri,
    )?;

    Ok(CodexSmokeFlowSummary {
        flow: CodexSmokeSelection::PolicyDenialDebug,
        subject_execution_uri: inputs.denied_execution_uri,
        report_execution_uri: denial_record.receipt.uri,
        additional_report_execution_uris: vec![
            diff_record.receipt.uri.clone(),
            http_record.receipt.uri.clone(),
        ],
        comparison_execution_uris: inputs.comparison_execution_uris,
        subject_query_uri: None,
        subject_resource_items,
        report_resource_items,
        subject_emitted_evidence: 0,
        subject_child_executions: 0,
        report_summary: denial_record
            .output
            .ok_or_else(|| {
                CodexWorkflowError::new(
                    "codex-smoke-report-missing-output",
                    "explain-capability-denial did not return skill output",
                )
            })?
            .summary,
    })
}

struct PolicyDenialSmokeInputs {
    denied_execution_uri: String,
    trusted_execution_uri: String,
    restricted_execution_uri: String,
    candidate_url: String,
    comparison_execution_uris: Vec<String>,
}

fn prepare_policy_denial_smoke_inputs(
    registry_root: &Path,
) -> Result<PolicyDenialSmokeInputs, CodexWorkflowError> {
    let scenario =
        prepare_codex_scenario(registry_root, CodexScenarioSelection::PolicyDenialDebug)?;
    Ok(PolicyDenialSmokeInputs {
        denied_execution_uri: scenario
            .subject_execution_uris
            .first()
            .cloned()
            .ok_or_else(|| {
                CodexWorkflowError::new(
                    "codex-smoke-missing-subject-execution",
                    "policy-denial-debug scenario did not produce a denied execution URI",
                )
            })?,
        trusted_execution_uri: scenario
            .comparison_execution_uris
            .first()
            .cloned()
            .ok_or_else(|| {
                CodexWorkflowError::new(
                    "codex-smoke-missing-comparison-execution",
                    "policy-denial-debug scenario did not produce the trusted imported execution URI",
                )
            })?,
        restricted_execution_uri: scenario
            .comparison_execution_uris
            .get(1)
            .cloned()
            .ok_or_else(|| {
                CodexWorkflowError::new(
                    "codex-smoke-missing-comparison-execution",
                    "policy-denial-debug scenario did not produce the restricted imported execution URI",
                )
            })?,
        candidate_url: scenario.candidate_urls.last().cloned().ok_or_else(|| {
            CodexWorkflowError::new(
                "codex-smoke-missing-candidate-url",
                "policy-denial-debug scenario did not provide a candidate HTTP URL",
            )
        })?,
        comparison_execution_uris: scenario.comparison_execution_uris,
    })
}

fn run_capability_denial_report(
    client: &mut McpStdioClient,
    execution_uri: &str,
) -> Result<ExecutionRecord, CodexWorkflowError> {
    let response_value = client.request(
        "tools/call",
        &example_inspect_request(
            "explain-capability-denial",
            &json!({ "execution_uri": execution_uri }),
            &[execution_read_grant()],
        ),
    )?;
    let response: CallToolResult = McpStdioClient::parse_result(&response_value)?;
    let record = parse_execution_record(&response)?;
    let output = output_structured_value(
        &record,
        "codex-smoke-denial-report-missing-output",
        "explain-capability-denial did not return structured output",
    )?;
    if output
        .get("primary_reason")
        .filter(|value| !value.is_null())
        .is_none()
    {
        return Err(CodexWorkflowError::new(
            "codex-smoke-denial-report-invalid",
            "capability denial report did not include a primary_reason",
        ));
    }
    Ok(record)
}

fn run_authority_diff_report(
    client: &mut McpStdioClient,
    left_execution_uri: &str,
    right_execution_uri: &str,
) -> Result<ExecutionRecord, CodexWorkflowError> {
    let response_value = client.request(
        "tools/call",
        &example_inspect_request(
            "diff-execution-authority",
            &json!({
                "left_execution_uri": left_execution_uri,
                "right_execution_uri": right_execution_uri,
            }),
            &[execution_read_grant()],
        ),
    )?;
    let response: CallToolResult = McpStdioClient::parse_result(&response_value)?;
    let record = parse_execution_record(&response)?;
    let output = output_structured_value(
        &record,
        "codex-smoke-authority-diff-missing-output",
        "diff-execution-authority did not return structured output",
    )?;
    if output
        .get("likely_authority_drivers")
        .and_then(Value::as_array)
        .is_none()
    {
        return Err(CodexWorkflowError::new(
            "codex-smoke-authority-diff-invalid",
            "authority diff did not include likely_authority_drivers",
        ));
    }
    Ok(record)
}

fn run_http_authority_report(
    client: &mut McpStdioClient,
    execution_uri: &str,
    candidate_url: &str,
) -> Result<ExecutionRecord, CodexWorkflowError> {
    let response_value = client.request(
        "tools/call",
        &example_inspect_request(
            "explain-http-authority",
            &json!({
                "execution_uri": execution_uri,
                "candidate_request": {
                    "url": candidate_url,
                    "method": "get",
                    "timeout_ms": 500,
                },
            }),
            &[execution_read_grant()],
        ),
    )?;
    let response: CallToolResult = McpStdioClient::parse_result(&response_value)?;
    let record = parse_execution_record(&response)?;
    let output = output_structured_value(
        &record,
        "codex-smoke-http-authority-missing-output",
        "explain-http-authority did not return structured output",
    )?;
    if output
        .get("evaluation_status")
        .and_then(Value::as_str)
        .is_none()
    {
        return Err(CodexWorkflowError::new(
            "codex-smoke-http-authority-invalid",
            "HTTP authority report did not include evaluation_status",
        ));
    }
    Ok(record)
}

fn read_subject_and_report_resource_counts(
    client: &mut McpStdioClient,
    subject_execution_uri: &str,
    report_execution_uri: &str,
) -> Result<(usize, usize), CodexWorkflowError> {
    let subject_resource_value =
        client.request("resources/read", &json!({ "uri": subject_execution_uri }))?;
    let subject_resource: ReadResourceResult =
        McpStdioClient::parse_result(&subject_resource_value)?;
    let report_resource_value =
        client.request("resources/read", &json!({ "uri": report_execution_uri }))?;
    let report_resource: ReadResourceResult = McpStdioClient::parse_result(&report_resource_value)?;
    Ok((
        subject_resource.contents.len(),
        report_resource.contents.len(),
    ))
}

fn ensure_example_skills_installed(
    registry_root: &Path,
    skill_dirs: &[&str],
) -> Result<Vec<BootstrappedSkill>, RegistryError> {
    let installer = LocalSourceInstaller::new(registry_root)?;
    let repo_root = repo_root();

    skill_dirs
        .iter()
        .map(|skill_dir| {
            let installed_skill =
                installer.install(repo_root.join("examples/skills").join(skill_dir))?;
            Ok(summarize_installed_skill(skill_dir, installed_skill))
        })
        .collect()
}

fn prepare_recent_failure_triage_scenario(
    registry_root: &Path,
) -> Result<CodexScenarioSummary, CodexWorkflowError> {
    let installed_skills =
        ensure_example_skills_installed(registry_root, &RECENT_FAILURE_TRIAGE_SKILLS)?;
    let server = http_test_server::HttpTestServer::start();
    let facade = local_facade(registry_root)?;

    let success = inspect_success_record(
        &facade,
        inspect_http_json_request(
            &server.json_url(),
            "get",
            "tenant-dev",
            "actor-dev",
            vec![http_request_granted_capability(
                http_test_server::HttpTestServer::host(),
                server.port(),
                &["/json"],
                Some(vec![HttpMethod::Get]),
                None,
            )],
        ),
    )?;
    let failed = inspect_expected_error_record(
        &facade,
        inspect_http_json_request(
            &server.json_url(),
            "post",
            "tenant-dev",
            "actor-dev",
            vec![http_request_granted_capability(
                http_test_server::HttpTestServer::host(),
                server.port(),
                &["/json"],
                Some(vec![HttpMethod::Get]),
                None,
            )],
        ),
        "recent-failure-triage failed HTTP execution",
    )?;
    let rejected = inspect_expected_error_record(
        &facade,
        inspect_http_json_request(
            &server.json_url(),
            "get",
            "tenant-dev",
            "actor-dev",
            Vec::new(),
        ),
        "recent-failure-triage rejected HTTP execution",
    )?;
    let query_uri =
        execution_query_resource_uri(&ExecutionQueryResource::FailuresRecent { limit: 10 });

    Ok(CodexScenarioSummary {
        registry_root: registry_root.to_path_buf(),
        scenario: CodexScenarioSelection::RecentFailureTriage,
        installed_skills,
        subject_execution_uris: vec![rejected.receipt.uri.clone(), failed.receipt.uri.clone()],
        comparison_execution_uris: vec![success.receipt.uri],
        query_uris: vec![query_uri.clone()],
        candidate_urls: vec![server.json_url()],
        recommended_codex_ask: format!(
            "Summarize recent failures from {query_uri} using example/summarize-execution-query, then explain one of the stored failed or rejected executions if the query summary needs a deeper root-cause read."
        ),
    })
}

struct PreparedPolicyDenialBundle {
    installed_skills: Vec<BootstrappedSkill>,
    existing_inspect_http_digest: Option<String>,
    bundle_root: PathBuf,
    identity: LocalPublisherIdentity,
}

fn prepare_policy_denial_debug_scenario(
    registry_root: &Path,
) -> Result<CodexScenarioSummary, CodexWorkflowError> {
    let prepared = prepare_policy_denial_bundle(registry_root)?;
    let publisher_id = prepared.identity.publisher.id.clone();
    with_restored_policy_denial_support(registry_root, &publisher_id, || {
        seed_policy_denial_debug_summary(registry_root, prepared)
    })
}

fn prepare_policy_denial_bundle(
    registry_root: &Path,
) -> Result<PreparedPolicyDenialBundle, CodexWorkflowError> {
    let installed_skills =
        ensure_example_skills_installed(registry_root, &POLICY_DENIAL_DEBUG_SKILLS)?;
    let support_root = policy_support_root(registry_root);
    if support_root.exists() {
        fs::remove_dir_all(&support_root)?;
    }
    fs::create_dir_all(&support_root)?;

    let source_root = support_root.join("source-root");
    let bundle_root = support_root.join("bundle");
    let identity_path = support_root.join("publisher.json");

    let source_installer = LocalSourceInstaller::new(&source_root)?;
    let source_skill = source_installer.install(example_skill_source_dir("inspect-http-json"))?;
    let identity = LocalPublisherIdentity::generate(source_skill.manifest.publisher.clone())?;
    identity.save(&identity_path)?;
    let registry = LocalRegistry::load(&source_root)?;
    registry.export_bundle(&source_skill.resolved_ref, false, &bundle_root, &identity)?;

    let existing_inspect_http_digest = LocalRegistry::load(registry_root)?
        .resolve(&requested_example_skill_ref("inspect-http-json"))
        .ok()
        .map(|installed| installed.resolved_ref.digest);

    Ok(PreparedPolicyDenialBundle {
        installed_skills,
        existing_inspect_http_digest,
        bundle_root,
        identity,
    })
}

fn with_restored_policy_denial_support<T>(
    registry_root: &Path,
    publisher_id: &str,
    operation: impl FnOnce() -> Result<T, CodexWorkflowError>,
) -> Result<T, CodexWorkflowError> {
    let policy_path = registry_root.join("policy.json");
    let policy_backup = read_optional_file(&policy_path)?;
    let publisher_path = trusted_publisher_file_path(registry_root, publisher_id);
    let publisher_backup = read_optional_file(&publisher_path)?;
    let result = operation();
    let restore_policy = restore_optional_file(&policy_path, policy_backup.as_deref());
    let restore_publisher = restore_optional_file(&publisher_path, publisher_backup.as_deref());
    match result {
        Ok(value) => {
            restore_policy?;
            restore_publisher?;
            Ok(value)
        }
        Err(error) => {
            let _ = restore_policy;
            let _ = restore_publisher;
            Err(error)
        }
    }
}

fn seed_policy_denial_debug_summary(
    registry_root: &Path,
    mut prepared: PreparedPolicyDenialBundle,
) -> Result<CodexScenarioSummary, CodexWorkflowError> {
    let server = http_test_server::HttpTestServer::start();
    LocalRegistry::trust_publisher(
        registry_root,
        &prepared
            .identity
            .trusted_record_with_tier(LocalTrustTier::TrustedImported),
    )?;
    LocalRegistry::import_bundle(registry_root, &prepared.bundle_root)?;
    write_policy_config(registry_root)?;

    let trusted = inspect_success_record(
        &local_facade(registry_root)?,
        policy_http_request(
            &server.redirect_json_url(),
            "tenant-trusted",
            "actor-demo",
            server.port(),
        ),
    )?;

    LocalRegistry::trust_publisher(
        registry_root,
        &prepared
            .identity
            .trusted_record_with_tier(LocalTrustTier::Restricted),
    )?;

    let restricted_allowed = inspect_success_record(
        &local_facade(registry_root)?,
        policy_http_request(
            &server.redirect_json_url(),
            "tenant-trusted",
            "actor-demo",
            server.port(),
        ),
    )?;
    let denied = inspect_expected_error_record(
        &local_facade(registry_root)?,
        policy_http_request(
            &server.redirect_json_url(),
            "tenant-restricted",
            "actor-demo",
            server.port(),
        ),
        "policy-denial-debug restricted tenant redirect denial",
    )?;

    append_imported_http_skill_if_needed(registry_root, &mut prepared)?;

    Ok(CodexScenarioSummary {
        registry_root: registry_root.to_path_buf(),
        scenario: CodexScenarioSelection::PolicyDenialDebug,
        installed_skills: prepared.installed_skills,
        subject_execution_uris: vec![denied.receipt.uri.clone()],
        comparison_execution_uris: vec![
            trusted.receipt.uri.clone(),
            restricted_allowed.receipt.uri.clone(),
        ],
        query_uris: Vec::new(),
        candidate_urls: vec![
            server.redirect_json_url(),
            server.json_url(),
            server.localhost_json_url(),
        ],
        recommended_codex_ask: format!(
            "Compare the trusted imported execution {} with the restricted imported execution {}, explain why stored execution {} was denied, and dry-run whether direct GET requests to {} and {} should be allowed.",
            trusted.receipt.uri,
            restricted_allowed.receipt.uri,
            denied.receipt.uri,
            server.json_url(),
            server.localhost_json_url(),
        ),
    })
}

fn append_imported_http_skill_if_needed(
    registry_root: &Path,
    prepared: &mut PreparedPolicyDenialBundle,
) -> Result<(), CodexWorkflowError> {
    let imported_skill = LocalRegistry::load(registry_root)?
        .resolve(&requested_example_skill_ref("inspect-http-json"))?;
    if prepared.existing_inspect_http_digest.as_deref()
        != Some(imported_skill.resolved_ref.digest.as_str())
    {
        prepared.installed_skills.push(summarize_installed_skill(
            "inspect-http-json",
            imported_skill,
        ));
    }
    Ok(())
}

fn prepare_execution_tree_scenario(
    registry_root: &Path,
) -> Result<CodexScenarioSummary, CodexWorkflowError> {
    let installed_skills =
        ensure_example_skills_installed(registry_root, &EXECUTION_TREE_SCENARIO_SKILLS)?;
    let facade = local_facade(registry_root)?;
    let root_execution = inspect_success_record(
        &facade,
        InspectRequest::new(
            requested_example_skill_ref("hello-composite"),
            json!({ "name": "Ada" }),
            "tenant-dev",
            "actor-dev",
            CapabilityGrantSet {
                grants: vec![
                    invoke_dependency_granted_capability(&["hello"]),
                    emit_evidence_granted_capability(),
                ],
            },
        ),
    )?;

    Ok(CodexScenarioSummary {
        registry_root: registry_root.to_path_buf(),
        scenario: CodexScenarioSelection::ExecutionTree,
        installed_skills,
        subject_execution_uris: vec![root_execution.receipt.uri.clone()],
        comparison_execution_uris: Vec::new(),
        query_uris: Vec::new(),
        candidate_urls: Vec::new(),
        recommended_codex_ask: format!(
            "Run example/explain-execution-tree against {} and identify the first failing or denied node, or confirm that the current stored tree is clean.",
            root_execution.receipt.uri
        ),
    })
}

fn local_facade(registry_root: &Path) -> Result<LocalFacade, CodexWorkflowError> {
    let registry = LocalRegistry::load(registry_root)?;
    let runtime = WasmtimeRuntimeAdapter::new().map_err(|error| {
        CodexWorkflowError::new(
            "codex-scenario-runtime-init-failed",
            "failed to initialize the Wasmtime runtime for Codex scenario prep",
        )
        .with_detail(json!({ "error": error.to_string() }))
    })?;
    Ok(GuildMcpFacade::new(registry, runtime))
}

fn inspect_success_record(
    facade: &LocalFacade,
    request: InspectRequest,
) -> Result<ExecutionRecord, CodexWorkflowError> {
    Ok(facade.inspect(request)?.structured_content)
}

fn inspect_expected_error_record(
    facade: &LocalFacade,
    request: InspectRequest,
    expectation: &str,
) -> Result<ExecutionRecord, CodexWorkflowError> {
    match facade.inspect(request) {
        Ok(response) => Err(CodexWorkflowError::new(
            "codex-scenario-expected-error",
            "scenario expected a persisted unsuccessful execution but the call succeeded",
        )
        .with_detail(json!({
            "expectation": expectation,
            "execution_uri": response.structured_content.receipt.uri,
        }))),
        Err(error) => load_execution_record_from_error(facade, &error, expectation),
    }
}

fn load_execution_record_from_error(
    facade: &LocalFacade,
    error: &McpError,
    expectation: &str,
) -> Result<ExecutionRecord, CodexWorkflowError> {
    let receipt = error.receipt.as_ref().ok_or_else(|| {
        CodexWorkflowError::new(
            "codex-scenario-missing-receipt",
            "scenario expected a persisted execution receipt for the failed call",
        )
        .with_detail(json!({
            "expectation": expectation,
            "code": error.code,
            "message": error.message,
        }))
    })?;
    load_execution_record_from_uri(facade, &receipt.uri)
}

fn load_execution_record_from_uri(
    facade: &LocalFacade,
    execution_uri: &str,
) -> Result<ExecutionRecord, CodexWorkflowError> {
    let resource = facade.read_resource(execution_uri)?;
    serde_json::from_slice(&resource.bytes).map_err(|error| {
        CodexWorkflowError::new(
            "codex-scenario-record-parse-failed",
            "persisted execution resource did not contain valid JSON",
        )
        .with_detail(json!({
            "execution_uri": execution_uri,
            "json_error": error.to_string(),
        }))
    })
}

fn inspect_http_json_request(
    url: &str,
    method: &str,
    tenant_id: &str,
    actor_id: &str,
    grants: Vec<GrantedCapability>,
) -> InspectRequest {
    InspectRequest::new(
        requested_example_skill_ref("inspect-http-json"),
        json!({
            "url": url,
            "method": method,
            "json_pointers": ["/message"],
        }),
        tenant_id,
        actor_id,
        CapabilityGrantSet { grants },
    )
}

fn policy_http_request(url: &str, tenant_id: &str, actor_id: &str, port: u16) -> InspectRequest {
    inspect_http_json_request(
        url,
        "get",
        tenant_id,
        actor_id,
        vec![http_request_granted_capability(
            http_test_server::HttpTestServer::host(),
            port,
            &["/redirect-json", "/json"],
            Some(vec![HttpMethod::Get]),
            Some(2),
        )],
    )
}

fn emit_evidence_granted_capability() -> GrantedCapability {
    GrantedCapability {
        id: CapabilityId::EmitEvidence,
        access: CapabilityAccess::Write,
        constraints: CapabilityConstraints::EmitEvidence(EmitEvidenceConstraints {
            max_bytes: Some(65_536),
            audiences: Some(vec![EvidenceAudience::User]),
            redactions: Some(vec![RedactionClass::None]),
        }),
    }
}

fn execution_and_object_read_granted_capability() -> GrantedCapability {
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

fn execution_read_granted_capability() -> GrantedCapability {
    GrantedCapability {
        id: CapabilityId::ReadResource,
        access: CapabilityAccess::Read,
        constraints: CapabilityConstraints::ReadResource(ReadResourceConstraints {
            uri_prefixes: Some(vec!["guild://executions/".into()]),
            resource_kinds: Some(vec![ResourceKind::Execution]),
        }),
    }
}

fn query_read_granted_capability() -> GrantedCapability {
    GrantedCapability {
        id: CapabilityId::ReadResource,
        access: CapabilityAccess::Read,
        constraints: CapabilityConstraints::ReadResource(ReadResourceConstraints {
            uri_prefixes: Some(vec!["guild://queries/executions/".into()]),
            resource_kinds: Some(vec![ResourceKind::Query]),
        }),
    }
}

fn object_read_granted_capability() -> GrantedCapability {
    GrantedCapability {
        id: CapabilityId::ReadResource,
        access: CapabilityAccess::Read,
        constraints: CapabilityConstraints::ReadResource(ReadResourceConstraints {
            uri_prefixes: Some(vec!["guild://objects/records/".into()]),
            resource_kinds: Some(vec![ResourceKind::Object]),
        }),
    }
}

fn invoke_dependency_granted_capability(aliases: &[&str]) -> GrantedCapability {
    GrantedCapability {
        id: CapabilityId::InvokeSkill,
        access: CapabilityAccess::Invoke,
        constraints: CapabilityConstraints::InvokeDependency(InvokeDependencyConstraints {
            aliases: Some(aliases.iter().map(|alias| (*alias).to_owned()).collect()),
        }),
    }
}

fn http_request_granted_capability(
    host: &str,
    port: u16,
    paths: &[&str],
    methods: Option<Vec<HttpMethod>>,
    max_redirects: Option<u8>,
) -> GrantedCapability {
    GrantedCapability {
        id: CapabilityId::HttpRequest,
        access: CapabilityAccess::Read,
        constraints: CapabilityConstraints::HttpRequest(HttpRequestConstraints {
            allowed_schemes: Some(vec![HttpScheme::Http]),
            allowed_hosts: Some(vec![host.to_owned()]),
            allowed_host_suffixes: None,
            allowed_ports: Some(vec![port]),
            allowed_methods: methods,
            allowed_path_prefixes: Some(paths.iter().map(|path| (*path).to_owned()).collect()),
            max_timeout_ms: Some(2_000),
            max_response_bytes: Some(8_192),
            follow_redirects: max_redirects.map(|_| true),
            max_redirects,
            allow_loopback: Some(true),
            allow_link_local: None,
            allow_private_networks: None,
            allow_ip_literals: Some(true),
        }),
    }
}

fn granted_capability_value(grant: GrantedCapability) -> Value {
    serde_json::to_value(grant).expect("grant serializes")
}

fn output_structured_value(
    record: &ExecutionRecord,
    code: &str,
    message: &str,
) -> Result<Value, CodexWorkflowError> {
    let output = record.output.as_ref().ok_or_else(|| {
        CodexWorkflowError::new(code, message).with_detail(json!({
            "execution_uri": record.receipt.uri,
        }))
    })?;
    Ok(output.structured.clone())
}

fn output_markdown_string(
    record: &ExecutionRecord,
    code: &str,
    message: &str,
) -> Result<String, CodexWorkflowError> {
    output_structured_value(record, code, message)?
        .as_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            CodexWorkflowError::new(code, message).with_detail(json!({
                "execution_uri": record.receipt.uri,
                "expected": "string",
            }))
        })
}

fn example_skill_source_dir(skill_dir: &str) -> PathBuf {
    repo_root().join("examples/skills").join(skill_dir)
}

fn policy_support_root(registry_root: &Path) -> PathBuf {
    registry_root
        .join(".codex-scenarios")
        .join("policy-denial-debug")
}

fn read_optional_file(path: &Path) -> Result<Option<Vec<u8>>, CodexWorkflowError> {
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(fs::read(path)?))
}

fn restore_optional_file(path: &Path, contents: Option<&[u8]>) -> Result<(), CodexWorkflowError> {
    match contents {
        Some(bytes) => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, bytes)?;
        }
        None if path.exists() => {
            fs::remove_file(path)?;
        }
        None => {}
    }
    Ok(())
}

fn trusted_publisher_file_path(root: &Path, publisher_id: &str) -> PathBuf {
    root.join("trust")
        .join("publishers")
        .join(format!("{publisher_id}.json"))
}

fn write_policy_config(root: &Path) -> Result<(), CodexWorkflowError> {
    fs::create_dir_all(root)?;
    let policy = LocalPolicyConfig {
        default_profile: "trusted-networked".into(),
        profiles: vec![
            PolicyProfile {
                name: "trusted-networked".into(),
                default_action: guild_types::LocalPolicyDefaultAction::AllowRequestedDeclared,
                rules: Vec::new(),
            },
            PolicyProfile {
                name: "restricted-networked".into(),
                default_action: guild_types::LocalPolicyDefaultAction::AllowRequestedDeclared,
                rules: vec![PolicyRule {
                    name: Some("cap-restricted-http-redirects".into()),
                    skills: Some(vec![SkillKey {
                        namespace: EXAMPLE_NAMESPACE.into(),
                        name: "inspect-http-json".into(),
                    }]),
                    publisher_ids: None,
                    trust_tiers: Some(vec![LocalTrustTier::Restricted]),
                    verification_states: Some(vec![InstalledVerificationState::VerifiedImport]),
                    applies_to: PolicyRuleTarget::Any,
                    effect: PolicyRuleEffect::Cap,
                    capabilities: guild_types::CapabilityGrantSet {
                        grants: vec![GrantedCapability {
                            id: CapabilityId::HttpRequest,
                            access: CapabilityAccess::Read,
                            constraints: CapabilityConstraints::HttpRequest(
                                HttpRequestConstraints {
                                    allowed_schemes: None,
                                    allowed_hosts: None,
                                    allowed_host_suffixes: None,
                                    allowed_ports: None,
                                    allowed_methods: None,
                                    allowed_path_prefixes: None,
                                    max_timeout_ms: None,
                                    max_response_bytes: None,
                                    follow_redirects: Some(false),
                                    max_redirects: None,
                                    allow_loopback: None,
                                    allow_link_local: None,
                                    allow_private_networks: None,
                                    allow_ip_literals: None,
                                },
                            ),
                        }],
                    },
                }],
            },
        ],
        bindings: vec![PolicyProfileBinding {
            name: Some("restricted-tenant".into()),
            actor_ids: None,
            tenant_ids: Some(vec!["tenant-restricted".into()]),
            profile: "restricted-networked".into(),
        }],
        ..LocalPolicyConfig::default()
    };
    fs::write(
        root.join("policy.json"),
        serde_json::to_vec_pretty(&policy)?,
    )?;
    Ok(())
}

fn emit_evidence_grant() -> Value {
    granted_capability_value(emit_evidence_granted_capability())
}

fn execution_and_object_read_grant() -> Value {
    granted_capability_value(execution_and_object_read_granted_capability())
}

fn execution_read_grant() -> Value {
    granted_capability_value(execution_read_granted_capability())
}

fn query_read_grant() -> Value {
    granted_capability_value(query_read_granted_capability())
}

fn object_read_grant() -> Value {
    granted_capability_value(object_read_granted_capability())
}

fn render_report_invoke_grant() -> Value {
    granted_capability_value(invoke_dependency_granted_capability(&["renderer"]))
}

fn invoke_and_evidence_grants() -> Vec<Value> {
    vec![
        granted_capability_value(invoke_dependency_granted_capability(&["hello"])),
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
        cwd: Option<&Path>,
        env: &BTreeMap<String, String>,
    ) -> Result<Self, CodexWorkflowError> {
        let mut builder = Command::new(command.as_ref());
        builder
            .args(args)
            .envs(env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        if let Some(cwd) = cwd {
            builder.current_dir(cwd);
        }
        let mut child = builder.spawn().map_err(|error| {
            CodexWorkflowError::new(
                "codex-smoke-server-spawn-failed",
                "failed to spawn the Guild stdio MCP server",
            )
            .with_detail(json!({
                "command": command.as_ref(),
                "args": args,
                "cwd": cwd.map(|path| path.display().to_string()),
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
