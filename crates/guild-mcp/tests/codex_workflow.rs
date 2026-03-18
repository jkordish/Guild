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
    mcp_stdio_client::McpStdioClient::spawn(&config.command, &config.args, &config.cwd, &config.env)
}

fn run_guild_codex_json(args: &[&str]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let output = Command::new("cargo")
        .current_dir(repo_root())
        .args(["run", "-q", "-p", "guild-mcp", "--bin", "guild-codex", "--"])
        .args(args)
        .output()?;

    assert!(output.status.success(), "{output:?}");
    Ok(output.stdout)
}

#[test]
fn guild_codex_bootstrap_and_config_json_match_documented_stdio_shape() {
    let temp_root = TempRegistryRoot::new("guild-codex-bootstrap");
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
            "cargo run -p guild-mcp --bin guild-codex -- print-config --registry-root {}",
            payload.bootstrap.registry_root.to_string_lossy()
        )
    );
    assert_eq!(payload.recommended_smoke_commands.len(), 4);
    assert!(
        payload
            .recommended_smoke_commands
            .iter()
            .all(|command| command.contains("guild-codex -- smoke"))
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
    assert!(payload.config.config_toml().contains("cwd = "));
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
fn guild_codex_smoke_explain_execution_json_produces_resources() {
    let temp_root = TempRegistryRoot::new("guild-codex-explain");
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
fn guild_codex_smoke_explain_execution_tree_json_produces_resources() {
    let temp_root = TempRegistryRoot::new("guild-codex-tree");
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
    let temp_root = TempRegistryRoot::new("guild-codex-scenario-failures");
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
    let temp_root = TempRegistryRoot::new("guild-codex-scenario-policy");
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
    let temp_root = TempRegistryRoot::new("guild-codex-scenario-tree");
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
    let temp_root = TempRegistryRoot::new("guild-codex-smoke-failures");
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
    let temp_root = TempRegistryRoot::new("guild-codex-smoke-policy");
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
    let temp_root = TempRegistryRoot::new("guild-codex-all");
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
            CodexSmokeSelection::ExplainExecution,
            CodexSmokeSelection::ExplainExecutionTree,
            CodexSmokeSelection::RecentFailureTriage,
            CodexSmokeSelection::PolicyDenialDebug,
        ]
    );
}
