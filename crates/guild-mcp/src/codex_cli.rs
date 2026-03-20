use std::path::PathBuf;

use crate::codex::{
    CodexBootstrapOutput, CodexConfigWriteResult, CodexConfigWriteStatus, CodexScenarioSelection,
    CodexSmokeSelection, DEFAULT_CODEX_SERVER_NAME, bootstrap_codex_registry, codex_server_config,
    default_registry_root, prepare_codex_scenario, print_config_command,
    recommended_proof_commands, recommended_scenario_commands, recommended_smoke_commands,
    run_codex_smoke,
};

const GUILD_SUBCOMMAND_INVOCATION: &str = "guild [--registry-root <path>] codex";

#[derive(Debug)]
enum Command {
    Bootstrap(WorkflowOptions),
    PrintConfig(WorkflowOptions),
    Scenario(ScenarioOptions),
    Smoke(SmokeOptions),
}

#[derive(Debug)]
struct WorkflowOptions {
    registry_root: PathBuf,
    name: String,
    reset: bool,
    json: bool,
}

#[derive(Debug)]
struct SmokeOptions {
    registry_root: PathBuf,
    name: String,
    flow: CodexSmokeSelection,
    json: bool,
}

#[derive(Debug)]
struct ScenarioOptions {
    registry_root: PathBuf,
    scenario: CodexScenarioSelection,
    json: bool,
}

/// Run the `guild codex ...` subcommand family from the real `guild` CLI.
///
/// # Errors
///
/// Returns an error if argument parsing fails, the requested workflow cannot
/// prepare local state, or the underlying Codex helper operation fails.
pub fn run_guild_subcommand(
    args: &[String],
    default_registry_root_override: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    match parse_args(args, default_registry_root_override)? {
        None => {
            print_usage();
            Ok(())
        }
        Some(Command::Bootstrap(options)) => {
            let bootstrap = bootstrap_codex_registry(&options.registry_root, options.reset)?;
            let config = codex_server_config(&bootstrap.registry_root, options.name);
            if options.json {
                let output = CodexBootstrapOutput {
                    print_config_command: print_config_command(&bootstrap.registry_root),
                    bootstrap,
                    config,
                    recommended_scenario_commands: recommended_scenario_commands(
                        &options.registry_root,
                    ),
                    recommended_smoke_commands: recommended_smoke_commands(&options.registry_root),
                    recommended_proof_commands: recommended_proof_commands(),
                };
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                print_bootstrap_output(&bootstrap, &config);
            }
            Ok(())
        }
        Some(Command::PrintConfig(options)) => {
            let config = codex_server_config(options.registry_root, options.name);
            if options.json {
                println!("{}", serde_json::to_string_pretty(&config)?);
            } else {
                print_config_output(&config);
            }
            Ok(())
        }
        Some(Command::Scenario(options)) => {
            let summary = prepare_codex_scenario(&options.registry_root, options.scenario)?;
            if options.json {
                println!("{}", serde_json::to_string_pretty(&summary)?);
            } else {
                println!("{}", summary.render_text());
            }
            Ok(())
        }
        Some(Command::Smoke(options)) => {
            let summary = run_codex_smoke(&options.registry_root, options.name, options.flow)?;
            if options.json {
                println!("{}", serde_json::to_string_pretty(&summary)?);
            } else {
                println!("{}", summary.render_text());
            }
            Ok(())
        }
    }
}

fn parse_args(
    args: &[String],
    default_registry_root_override: Option<PathBuf>,
) -> Result<Option<Command>, Box<dyn std::error::Error>> {
    let Some(subcommand) = args.first().map(String::as_str) else {
        return Ok(None);
    };

    if matches!(subcommand, "--help" | "-h") {
        return Ok(None);
    }

    let default_registry_root = match default_registry_root_override {
        Some(path) => path,
        None => default_registry_root()?,
    };

    match subcommand {
        "bootstrap" => parse_workflow_options(&args[1..], true, default_registry_root)
            .map(Command::Bootstrap)
            .map(Some)
            .or_else(handle_help_request),
        "print-config" => parse_workflow_options(&args[1..], false, default_registry_root)
            .map(Command::PrintConfig)
            .map(Some)
            .or_else(handle_help_request),
        "scenario" => parse_scenario_options(&args[1..], default_registry_root)
            .map(Command::Scenario)
            .map(Some)
            .or_else(handle_help_request),
        "smoke" => parse_smoke_options(&args[1..], default_registry_root)
            .map(Command::Smoke)
            .map(Some)
            .or_else(handle_help_request),
        _ => Err(format!("unknown subcommand `{subcommand}`").into()),
    }
}

fn print_usage() {
    println!(
        "usage: {GUILD_SUBCOMMAND_INVOCATION} <bootstrap|print-config|scenario|smoke> [options]"
    );
    println!();
    println!(
        "note: `guild init` is the normal operator setup path; `guild codex ...` is for deterministic repo-local dogfood and smoke flows."
    );
    println!();
    println!(
        "bootstrap      create a local Guild root for Codex and install the default dogfood skills"
    );
    println!(
        "print-config   print the deterministic repo-local Codex MCP config for an existing or planned Guild root"
    );
    println!(
        "scenario       seed one deterministic Codex dogfood scenario and print the resulting Guild URIs"
    );
    println!(
        "smoke          run one or more deterministic Codex dogfood flows against an existing Guild root"
    );
}

fn handle_help_request(
    error: Box<dyn std::error::Error>,
) -> Result<Option<Command>, Box<dyn std::error::Error>> {
    if error.to_string() == "help requested" {
        Ok(None)
    } else {
        Err(error)
    }
}

fn parse_workflow_options(
    args: &[String],
    allow_reset: bool,
    default_registry_root: PathBuf,
) -> Result<WorkflowOptions, Box<dyn std::error::Error>> {
    let mut options = WorkflowOptions {
        registry_root: default_registry_root,
        name: DEFAULT_CODEX_SERVER_NAME.into(),
        reset: false,
        json: false,
    };

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--registry-root" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err("--registry-root requires a following path argument".into());
                };
                options.registry_root = PathBuf::from(value);
            }
            "--name" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err("--name requires a following server name".into());
                };
                options.name.clone_from(value);
            }
            "--reset" if allow_reset => {
                options.reset = true;
            }
            "--reset" => {
                return Err("--reset is only valid for bootstrap".into());
            }
            "--json" => {
                options.json = true;
            }
            "--help" | "-h" => return Err("help requested".into()),
            other => return Err(format!("unexpected argument `{other}`").into()),
        }
        index += 1;
    }

    Ok(options)
}

fn parse_smoke_options(
    args: &[String],
    default_registry_root: PathBuf,
) -> Result<SmokeOptions, Box<dyn std::error::Error>> {
    let mut options = SmokeOptions {
        registry_root: default_registry_root,
        name: DEFAULT_CODEX_SERVER_NAME.into(),
        flow: CodexSmokeSelection::All,
        json: false,
    };

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--registry-root" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err("--registry-root requires a following path argument".into());
                };
                options.registry_root = PathBuf::from(value);
            }
            "--name" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err("--name requires a following server name".into());
                };
                options.name.clone_from(value);
            }
            "--flow" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err("--flow requires a following flow name".into());
                };
                options.flow = value.parse()?;
            }
            "--json" => {
                options.json = true;
            }
            "--help" | "-h" => return Err("help requested".into()),
            other => return Err(format!("unexpected argument `{other}`").into()),
        }
        index += 1;
    }

    Ok(options)
}

fn parse_scenario_options(
    args: &[String],
    default_registry_root: PathBuf,
) -> Result<ScenarioOptions, Box<dyn std::error::Error>> {
    let mut options = ScenarioOptions {
        registry_root: default_registry_root,
        scenario: CodexScenarioSelection::RecentFailureTriage,
        json: false,
    };
    let mut scenario_explicit = false;

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--registry-root" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err("--registry-root requires a following path argument".into());
                };
                options.registry_root = PathBuf::from(value);
            }
            "--scenario" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err("--scenario requires a following scenario name".into());
                };
                options.scenario = value.parse()?;
                scenario_explicit = true;
            }
            "--json" => {
                options.json = true;
            }
            "--help" | "-h" => return Err("help requested".into()),
            other => return Err(format!("unexpected argument `{other}`").into()),
        }
        index += 1;
    }

    if !scenario_explicit {
        return Err("--scenario is required for guild codex scenario".into());
    }

    Ok(options)
}

fn print_bootstrap_output(
    bootstrap: &crate::codex::CodexBootstrapSummary,
    config: &crate::codex::CodexServerConfig,
) {
    println!("Guild Codex workflow ready.");
    println!("repo root: {}", bootstrap.repo_root.display());
    println!("registry root: {}", bootstrap.registry_root.display());
    println!();
    println!("installed skills:");
    for skill in &bootstrap.skills {
        println!(
            "- {}/{}@{} ({}) from examples/skills/{}",
            skill.namespace, skill.name, skill.version, skill.digest, skill.source_dir
        );
    }
    println!();
    print_config_output(config);
    println!();
    println!("recommended dogfood flows:");
    for command in recommended_scenario_commands(&bootstrap.registry_root) {
        println!("- {command}");
    }
    println!();
    println!("regression smoke commands:");
    for command in recommended_smoke_commands(&bootstrap.registry_root) {
        println!("- {command}");
    }
    println!();
    println!("helper commands:");
    println!("- {}", print_config_command(&bootstrap.registry_root));
    for command in recommended_proof_commands() {
        println!("- {command}");
    }
}

fn print_config_output(config: &crate::codex::CodexServerConfig) {
    println!("manual server launch:");
    println!("{}", config.manual_server_command());
    println!();
    println!("project-scoped config snippet (recommended for trusted repos):");
    println!("{}", config.config_toml());
    println!();
    println!("Codex CLI registration (convenience path):");
    println!("{}", config.codex_mcp_add_command());
    println!();
    println!(
        "the launcher uses an explicit Cargo manifest path and runs `guild mcp serve --stdio`, so it does not depend on the current working directory"
    );
}

pub(crate) fn print_setup_details(
    guild_binary: &std::path::Path,
    config: &crate::codex::CodexServerConfig,
    writes: &[CodexConfigWriteResult],
    print_config: bool,
) {
    println!("guild binary: {}", guild_binary.display());
    println!("registry root: {}", config.registry_root_display());
    println!();
    println!("manual server launch:");
    println!("{}", config.manual_server_command());
    println!();
    println!("Codex CLI registration:");
    println!("{}", config.codex_mcp_add_command());

    if print_config {
        println!();
        println!("Codex MCP config snippet:");
        println!("{}", config.config_toml());
    }

    if !writes.is_empty() {
        println!();
        println!("config writes:");
        for write in writes {
            let status = match write.status {
                CodexConfigWriteStatus::Updated => "updated",
                CodexConfigWriteStatus::Unchanged => "unchanged",
            };
            println!("- {status}: {}", write.path.display());
        }
    }
}

pub(crate) fn project_codex_config_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(std::env::current_dir()?.join(".codex").join("config.toml"))
}
