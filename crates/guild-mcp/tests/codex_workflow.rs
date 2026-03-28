use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use guild_mcp::codex::{
    CodexBootstrapOutput, CodexScenarioSelection, CodexScenarioSummary, CodexServerConfig,
    CodexSmokeSelection, CodexSmokeSummary, bootstrap_codex_registry, codex_server_config,
    guild_mcp_manifest_path,
};

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

fn spawn_documented_server(
    config: &CodexServerConfig,
) -> Result<mcp_stdio_client::McpStdioClient, Box<dyn std::error::Error>> {
    mcp_stdio_client::McpStdioClient::spawn(
        &config.command,
        &config.args,
        config.cwd.as_deref(),
        &config.env,
    )
}

fn run_guild_codex_json(args: &[&str]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_guild"))
        .current_dir(repo_root())
        .arg("codex")
        .args(args)
        .output()?;

    assert!(output.status.success(), "{output:?}");
    Ok(output.stdout)
}

#[test]
fn guild_codex_bootstrap_and_config_json_match_documented_stdio_shape() {
    let temp_root = TempRegistryRoot::new("codex-workflow-bootstrap");
    let stdout = run_guild_codex_json(&[
        "bootstrap",
        "--registry-root",
        &temp_root.path().to_string_lossy(),
        "--reset",
        "--json",
    ])
    .unwrap();
    let payload: CodexBootstrapOutput = serde_json::from_slice(&stdout).unwrap();
    let skill_names = payload
        .bootstrap
        .skills
        .iter()
        .map(|skill| skill.name.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        skill_names,
        vec![
            "render-report",
            "incident-casefile",
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
        ]
    );
    assert_eq!(payload.config.command, "cargo");
    assert_eq!(
        payload.config.args,
        vec![
            "run".to_owned(),
            "-q".to_owned(),
            "--manifest-path".to_owned(),
            guild_mcp_manifest_path().to_string_lossy().into_owned(),
            "--bin".to_owned(),
            "guild".to_owned(),
            "--".to_owned(),
            "mcp".to_owned(),
            "serve".to_owned(),
            "--stdio".to_owned(),
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
    assert_eq!(
        payload.print_config_command,
        format!(
            "guild codex print-config --registry-root {}",
            payload.bootstrap.registry_root.to_string_lossy()
        )
    );
    assert_eq!(payload.recommended_scenario_commands.len(), 2);
    assert!(
        payload
            .recommended_scenario_commands
            .iter()
            .all(|command| command.starts_with("guild codex scenario "))
    );
    assert_eq!(payload.recommended_smoke_commands.len(), 9);
    assert!(
        payload
            .recommended_smoke_commands
            .iter()
            .all(|command| command.starts_with("guild codex smoke "))
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
            .contains("--manifest-path")
    );
    assert_eq!(payload.config.cwd.as_ref(), Some(&repo_root()));
    assert!(payload.config.config_toml().contains("cwd = "));
}

#[test]
fn documented_config_can_launch_the_stdio_server() {
    let temp_root = TempRegistryRoot::new("codex-workflow-startup");
    let bootstrap = bootstrap_codex_registry(temp_root.path(), true).unwrap();
    let config = codex_server_config(&bootstrap.registry_root, "guild-local");
    let mut client = spawn_documented_server(&config).unwrap();
    let initialized = client.initialize("codex-workflow-startup-smoke").unwrap();

    assert_eq!(initialized.server_info.name, "guild-mcp");
}

#[test]
fn guild_codex_smoke_explain_execution_json_produces_resources() {
    let temp_root = TempRegistryRoot::new("codex-workflow-explain");
    bootstrap_codex_registry(temp_root.path(), true).unwrap();

    let stdout = run_guild_codex_json(&[
        "smoke",
        "--registry-root",
        &temp_root.path().to_string_lossy(),
        "--flow",
        "explain-execution",
        "--json",
    ])
    .unwrap();
    let payload: CodexSmokeSummary = serde_json::from_slice(&stdout).unwrap();

    assert_eq!(
        payload.requested_flow,
        CodexSmokeSelection::ExplainExecution
    );
    assert_eq!(payload.flows.len(), 1);
    assert_eq!(payload.flows[0].flow, CodexSmokeSelection::ExplainExecution);
    assert_eq!(payload.flows[0].subject_resource_items, 1);
    assert_eq!(payload.flows[0].report_resource_items, 1);
    assert!(payload.flows[0].subject_emitted_evidence > 0);
    assert!(
        payload.flows[0]
            .report_summary
            .contains("Explained stored execution")
    );
    assert!(payload.flows[0].additional_report_execution_uris.is_empty());
    assert!(payload.flows[0].subject_query_uri.is_none());
}

#[test]
fn guild_codex_smoke_incident_casefile_json_produces_resources() {
    let temp_root = TempRegistryRoot::new("codex-workflow-incident-casefile");
    bootstrap_codex_registry(temp_root.path(), true).unwrap();

    let stdout = run_guild_codex_json(&[
        "smoke",
        "--registry-root",
        &temp_root.path().to_string_lossy(),
        "--flow",
        "incident-casefile",
        "--json",
    ])
    .unwrap();
    let payload: CodexSmokeSummary = serde_json::from_slice(&stdout).unwrap();

    assert_eq!(
        payload.requested_flow,
        CodexSmokeSelection::IncidentCasefile
    );
    assert_eq!(payload.flows.len(), 1);
    assert_eq!(payload.flows[0].flow, CodexSmokeSelection::IncidentCasefile);
    assert_eq!(payload.flows[0].subject_resource_items, 1);
    assert_eq!(payload.flows[0].report_resource_items, 1);
    assert!(payload.flows[0].subject_query_uri.is_some());
    assert!(!payload.flows[0].comparison_execution_uris.is_empty());
    assert!(
        payload.flows[0]
            .report_summary
            .contains("Prepared incident casefile")
    );
}

#[test]
fn guild_codex_smoke_incident_brief_json_produces_resources() {
    let temp_root = TempRegistryRoot::new("codex-workflow-incident-brief");
    bootstrap_codex_registry(temp_root.path(), true).unwrap();

    let stdout = run_guild_codex_json(&[
        "smoke",
        "--registry-root",
        &temp_root.path().to_string_lossy(),
        "--flow",
        "incident-brief",
        "--json",
    ])
    .unwrap();
    let payload: CodexSmokeSummary = serde_json::from_slice(&stdout).unwrap();

    assert_eq!(payload.requested_flow, CodexSmokeSelection::IncidentBrief);
    assert_eq!(payload.flows.len(), 1);
    assert_eq!(payload.flows[0].flow, CodexSmokeSelection::IncidentBrief);
    assert_eq!(payload.flows[0].subject_resource_items, 1);
    assert_eq!(payload.flows[0].report_resource_items, 1);
    assert!(payload.flows[0].subject_query_uri.is_some());
    assert!(!payload.flows[0].report_summary.is_empty());
    assert!(
        payload.flows[0]
            .report_summary
            .contains("Prepared incident brief")
    );
}

#[test]
fn guild_codex_smoke_run_diff_json_produces_resources() {
    let temp_root = TempRegistryRoot::new("codex-workflow-run-diff");
    bootstrap_codex_registry(temp_root.path(), true).unwrap();

    let stdout = run_guild_codex_json(&[
        "smoke",
        "--registry-root",
        &temp_root.path().to_string_lossy(),
        "--flow",
        "run-diff",
        "--json",
    ])
    .unwrap();
    let payload: CodexSmokeSummary = serde_json::from_slice(&stdout).unwrap();

    assert_eq!(payload.requested_flow, CodexSmokeSelection::RunDiff);
    assert_eq!(payload.flows.len(), 1);
    assert_eq!(payload.flows[0].flow, CodexSmokeSelection::RunDiff);
    assert_eq!(payload.flows[0].subject_resource_items, 1);
    assert_eq!(payload.flows[0].report_resource_items, 1);
    assert_eq!(payload.flows[0].comparison_execution_uris.len(), 1);
    assert!(payload.flows[0].subject_query_uri.is_some());
    assert!(
        payload.flows[0]
            .report_summary
            .contains("Prepared bounded run diff")
    );
}

#[test]
fn guild_codex_smoke_recent_failures_json_produces_resources() {
    let temp_root = TempRegistryRoot::new("codex-workflow-recent-failures");
    bootstrap_codex_registry(temp_root.path(), true).unwrap();

    let stdout = run_guild_codex_json(&[
        "smoke",
        "--registry-root",
        &temp_root.path().to_string_lossy(),
        "--flow",
        "recent-failures",
        "--json",
    ])
    .unwrap();
    let payload: CodexSmokeSummary = serde_json::from_slice(&stdout).unwrap();

    assert_eq!(payload.requested_flow, CodexSmokeSelection::RecentFailures);
    assert_eq!(payload.flows.len(), 1);
    assert_eq!(payload.flows[0].flow, CodexSmokeSelection::RecentFailures);
    assert_eq!(payload.flows[0].subject_resource_items, 1);
    assert_eq!(payload.flows[0].report_resource_items, 1);
    assert!(payload.flows[0].subject_query_uri.is_some());
    assert!(!payload.flows[0].comparison_execution_uris.is_empty());
    assert!(
        payload.flows[0]
            .report_summary
            .contains("Summarized recent failures")
    );
}

#[test]
fn guild_codex_smoke_evidence_summary_json_produces_resources() {
    let temp_root = TempRegistryRoot::new("codex-workflow-evidence-summary");
    bootstrap_codex_registry(temp_root.path(), true).unwrap();

    let stdout = run_guild_codex_json(&[
        "smoke",
        "--registry-root",
        &temp_root.path().to_string_lossy(),
        "--flow",
        "evidence-summary",
        "--json",
    ])
    .unwrap();
    let payload: CodexSmokeSummary = serde_json::from_slice(&stdout).unwrap();

    assert_eq!(payload.requested_flow, CodexSmokeSelection::EvidenceSummary);
    assert_eq!(payload.flows.len(), 1);
    assert_eq!(payload.flows[0].flow, CodexSmokeSelection::EvidenceSummary);
    assert_eq!(payload.flows[0].subject_resource_items, 1);
    assert_eq!(payload.flows[0].report_resource_items, 1);
    assert!(payload.flows[0].subject_emitted_evidence > 0);
    assert!(payload.flows[0].subject_query_uri.is_none());
    assert!(
        payload.flows[0]
            .report_summary
            .contains("Summarized stored evidence")
    );
}

#[test]
fn guild_codex_smoke_render_report_json_produces_resources() {
    let temp_root = TempRegistryRoot::new("codex-workflow-render-report");
    bootstrap_codex_registry(temp_root.path(), true).unwrap();

    let stdout = run_guild_codex_json(&[
        "smoke",
        "--registry-root",
        &temp_root.path().to_string_lossy(),
        "--flow",
        "render-report",
        "--json",
    ])
    .unwrap();
    let payload: CodexSmokeSummary = serde_json::from_slice(&stdout).unwrap();

    assert_eq!(payload.requested_flow, CodexSmokeSelection::RenderReport);
    assert_eq!(payload.flows.len(), 1);
    assert_eq!(payload.flows[0].flow, CodexSmokeSelection::RenderReport);
    assert_eq!(payload.flows[0].subject_resource_items, 1);
    assert_eq!(payload.flows[0].report_resource_items, 1);
    assert_eq!(
        payload.flows[0].subject_execution_uri,
        payload.flows[0].report_execution_uri
    );
    assert!(
        payload.flows[0]
            .report_summary
            .contains("Rendered starter-pack report")
    );
}

#[test]
fn guild_codex_smoke_explain_execution_tree_json_produces_resources() {
    let temp_root = TempRegistryRoot::new("codex-workflow-tree");
    bootstrap_codex_registry(temp_root.path(), true).unwrap();

    let stdout = run_guild_codex_json(&[
        "smoke",
        "--registry-root",
        &temp_root.path().to_string_lossy(),
        "--flow",
        "explain-execution-tree",
        "--json",
    ])
    .unwrap();
    let payload: CodexSmokeSummary = serde_json::from_slice(&stdout).unwrap();

    assert_eq!(
        payload.requested_flow,
        CodexSmokeSelection::ExplainExecutionTree
    );
    assert_eq!(payload.flows.len(), 1);
    assert_eq!(
        payload.flows[0].flow,
        CodexSmokeSelection::ExplainExecutionTree
    );
    assert_eq!(payload.flows[0].subject_resource_items, 1);
    assert_eq!(payload.flows[0].report_resource_items, 1);
    assert!(payload.flows[0].subject_child_executions > 0);
    assert!(!payload.flows[0].report_summary.is_empty());
    assert!(payload.flows[0].additional_report_execution_uris.is_empty());
    assert!(payload.flows[0].subject_query_uri.is_none());
}

#[test]
fn guild_codex_scenario_recent_failure_triage_json_prepares_query_and_failures() {
    let temp_root = TempRegistryRoot::new("codex-workflow-scenario-failures");
    bootstrap_codex_registry(temp_root.path(), true).unwrap();

    let stdout = run_guild_codex_json(&[
        "scenario",
        "--registry-root",
        &temp_root.path().to_string_lossy(),
        "--scenario",
        "recent-failure-triage",
        "--json",
    ])
    .unwrap();
    let payload: CodexScenarioSummary = serde_json::from_slice(&stdout).unwrap();

    assert_eq!(
        payload.scenario,
        CodexScenarioSelection::RecentFailureTriage
    );
    assert!(payload.subject_execution_uris.len() >= 2);
    assert_eq!(
        payload.query_uris,
        vec!["guild://queries/executions/failures/recent/10"]
    );
    assert!(
        payload
            .recommended_codex_ask
            .contains("summarize-execution-query")
    );
}

#[test]
fn guild_codex_scenario_policy_denial_debug_json_prepares_execution_pairs() {
    let temp_root = TempRegistryRoot::new("codex-workflow-scenario-policy");
    bootstrap_codex_registry(temp_root.path(), true).unwrap();

    let stdout = run_guild_codex_json(&[
        "scenario",
        "--registry-root",
        &temp_root.path().to_string_lossy(),
        "--scenario",
        "policy-denial-debug",
        "--json",
    ])
    .unwrap();
    let payload: CodexScenarioSummary = serde_json::from_slice(&stdout).unwrap();

    assert_eq!(payload.scenario, CodexScenarioSelection::PolicyDenialDebug);
    assert_eq!(payload.subject_execution_uris.len(), 1);
    assert_eq!(payload.comparison_execution_uris.len(), 2);
    assert!(payload.candidate_urls.len() >= 2);
    assert!(payload.query_uris.is_empty());
    assert!(
        payload
            .recommended_codex_ask
            .contains("Compare the trusted imported execution")
    );
}

#[test]
fn guild_codex_scenario_execution_tree_json_prepares_root_execution() {
    let temp_root = TempRegistryRoot::new("codex-workflow-scenario-tree");
    bootstrap_codex_registry(temp_root.path(), true).unwrap();

    let stdout = run_guild_codex_json(&[
        "scenario",
        "--registry-root",
        &temp_root.path().to_string_lossy(),
        "--scenario",
        "execution-tree",
        "--json",
    ])
    .unwrap();
    let payload: CodexScenarioSummary = serde_json::from_slice(&stdout).unwrap();

    assert_eq!(payload.scenario, CodexScenarioSelection::ExecutionTree);
    assert_eq!(payload.subject_execution_uris.len(), 1);
    assert!(payload.comparison_execution_uris.is_empty());
    assert!(
        payload
            .recommended_codex_ask
            .contains("example/explain-execution-tree")
    );
}

#[test]
fn guild_codex_smoke_recent_failure_triage_json_produces_resources() {
    let temp_root = TempRegistryRoot::new("codex-workflow-smoke-failures");
    bootstrap_codex_registry(temp_root.path(), true).unwrap();

    let stdout = run_guild_codex_json(&[
        "smoke",
        "--registry-root",
        &temp_root.path().to_string_lossy(),
        "--flow",
        "recent-failure-triage",
        "--json",
    ])
    .unwrap();
    let payload: CodexSmokeSummary = serde_json::from_slice(&stdout).unwrap();

    assert_eq!(
        payload.requested_flow,
        CodexSmokeSelection::RecentFailureTriage
    );
    assert_eq!(payload.flows.len(), 1);
    assert_eq!(
        payload.flows[0].flow,
        CodexSmokeSelection::RecentFailureTriage
    );
    assert_eq!(
        payload.flows[0].subject_query_uri.as_deref(),
        Some("guild://queries/executions/failures/recent/10")
    );
    assert!(payload.flows[0].comparison_execution_uris.len() >= 2);
    assert!(payload.flows[0].report_summary.contains("Summarized"));
}

#[test]
fn guild_codex_smoke_policy_denial_debug_json_produces_resources() {
    let temp_root = TempRegistryRoot::new("codex-workflow-smoke-policy");
    bootstrap_codex_registry(temp_root.path(), true).unwrap();

    let stdout = run_guild_codex_json(&[
        "smoke",
        "--registry-root",
        &temp_root.path().to_string_lossy(),
        "--flow",
        "policy-denial-debug",
        "--json",
    ])
    .unwrap();
    let payload: CodexSmokeSummary = serde_json::from_slice(&stdout).unwrap();

    assert_eq!(
        payload.requested_flow,
        CodexSmokeSelection::PolicyDenialDebug
    );
    assert_eq!(payload.flows.len(), 1);
    assert_eq!(
        payload.flows[0].flow,
        CodexSmokeSelection::PolicyDenialDebug
    );
    assert_eq!(payload.flows[0].comparison_execution_uris.len(), 2);
    assert_eq!(payload.flows[0].additional_report_execution_uris.len(), 2);
    assert!(
        payload.flows[0]
            .report_summary
            .contains("Explained capability denial state")
    );
}

#[test]
fn guild_codex_smoke_all_runs_all_documented_flows() {
    let temp_root = TempRegistryRoot::new("codex-workflow-all");
    bootstrap_codex_registry(temp_root.path(), true).unwrap();

    let stdout = run_guild_codex_json(&[
        "smoke",
        "--registry-root",
        &temp_root.path().to_string_lossy(),
        "--flow",
        "all",
        "--json",
    ])
    .unwrap();
    let payload: CodexSmokeSummary = serde_json::from_slice(&stdout).unwrap();
    let flow_names = payload
        .flows
        .iter()
        .map(|flow| flow.flow)
        .collect::<Vec<_>>();

    assert_eq!(payload.requested_flow, CodexSmokeSelection::All);
    assert_eq!(
        flow_names,
        vec![
            CodexSmokeSelection::IncidentCasefile,
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
    );
}
