#![warn(clippy::all, clippy::pedantic, clippy::cargo, clippy::perf)]
#![allow(clippy::multiple_crate_versions)]

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use guild_mcp::codex::{
    CodexBootstrapOutput, CodexScenarioSelection, CodexSmokeSelection, DEFAULT_CODEX_SERVER_NAME,
    bootstrap_codex_registry, codex_server_config, default_registry_root, prepare_codex_scenario,
    print_config_command, recommended_proof_commands, recommended_smoke_commands, run_codex_smoke,
};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    match parse_args(env::args())? {
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

fn parse_args(
    args: impl IntoIterator<Item = String>,
) -> Result<Option<Command>, Box<dyn std::error::Error>> {
    let mut args = args.into_iter();
    let _program = args.next();
    let Some(subcommand) = args.next() else {
        return Ok(None);
    };

    if matches!(subcommand.as_str(), "--help" | "-h") {
        return Ok(None);
    }

    match subcommand.as_str() {
        "bootstrap" => parse_workflow_options(args, true)
            .map(Command::Bootstrap)
            .map(Some)
            .or_else(handle_help_request),
        "print-config" => parse_workflow_options(args, false)
            .map(Command::PrintConfig)
            .map(Some)
            .or_else(handle_help_request),
        "scenario" => parse_scenario_options(args)
            .map(Command::Scenario)
            .map(Some)
            .or_else(handle_help_request),
        "smoke" => parse_smoke_options(args)
            .map(Command::Smoke)
            .map(Some)
            .or_else(handle_help_request),
        _ => Err(format!("unknown subcommand `{subcommand}`").into()),
    }
}

fn print_usage() {
    println!("usage: guild-codex <bootstrap|print-config|scenario|smoke> [options]");
    println!();
    println!(
        "bootstrap      create a local Guild root for Codex and install the default dogfood skills"
    );
    println!(
        "print-config   print the Codex MCP config and launch snippets for an existing or planned Guild root"
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
    args: impl IntoIterator<Item = String>,
    allow_reset: bool,
) -> Result<WorkflowOptions, Box<dyn std::error::Error>> {
    let mut options = WorkflowOptions {
        registry_root: default_registry_root(),
        name: DEFAULT_CODEX_SERVER_NAME.into(),
        reset: false,
        json: false,
    };

    let mut args = args.into_iter();
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--registry-root" => {
                let Some(value) = args.next() else {
                    return Err("--registry-root requires a following path argument".into());
                };
                options.registry_root = PathBuf::from(value);
            }
            "--name" => {
                let Some(value) = args.next() else {
                    return Err("--name requires a following server name".into());
                };
                options.name = value;
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
            _ => return Err(format!("unexpected argument `{argument}`").into()),
        }
    }

    Ok(options)
}

fn parse_smoke_options(
    args: impl IntoIterator<Item = String>,
) -> Result<SmokeOptions, Box<dyn std::error::Error>> {
    let mut options = SmokeOptions {
        registry_root: default_registry_root(),
        name: DEFAULT_CODEX_SERVER_NAME.into(),
        flow: CodexSmokeSelection::All,
        json: false,
    };

    let mut args = args.into_iter();
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--registry-root" => {
                let Some(value) = args.next() else {
                    return Err("--registry-root requires a following path argument".into());
                };
                options.registry_root = PathBuf::from(value);
            }
            "--name" => {
                let Some(value) = args.next() else {
                    return Err("--name requires a following server name".into());
                };
                options.name = value;
            }
            "--flow" => {
                let Some(value) = args.next() else {
                    return Err("--flow requires a following flow name".into());
                };
                options.flow = value.parse()?;
            }
            "--json" => {
                options.json = true;
            }
            "--help" | "-h" => return Err("help requested".into()),
            _ => return Err(format!("unexpected argument `{argument}`").into()),
        }
    }

    Ok(options)
}

fn parse_scenario_options(
    args: impl IntoIterator<Item = String>,
) -> Result<ScenarioOptions, Box<dyn std::error::Error>> {
    let mut options = ScenarioOptions {
        registry_root: default_registry_root(),
        scenario: CodexScenarioSelection::RecentFailureTriage,
        json: false,
    };
    let mut scenario_explicit = false;

    let mut args = args.into_iter();
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--registry-root" => {
                let Some(value) = args.next() else {
                    return Err("--registry-root requires a following path argument".into());
                };
                options.registry_root = PathBuf::from(value);
            }
            "--scenario" => {
                let Some(value) = args.next() else {
                    return Err("--scenario requires a following scenario name".into());
                };
                options.scenario = value.parse()?;
                scenario_explicit = true;
            }
            "--json" => {
                options.json = true;
            }
            "--help" | "-h" => return Err("help requested".into()),
            _ => return Err(format!("unexpected argument `{argument}`").into()),
        }
    }

    if !scenario_explicit {
        return Err("--scenario is required for guild-codex scenario".into());
    }

    Ok(options)
}

fn print_bootstrap_output(
    bootstrap: &guild_mcp::codex::CodexBootstrapSummary,
    config: &guild_mcp::codex::CodexServerConfig,
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
    println!("recommended next commands:");
    println!("- {}", print_config_command(&bootstrap.registry_root));
    for command in recommended_smoke_commands(&bootstrap.registry_root) {
        println!("- {command}");
    }
    println!();
    println!("compatibility proof commands:");
    for command in recommended_proof_commands() {
        println!("- {command}");
    }
}

fn print_config_output(config: &guild_mcp::codex::CodexServerConfig) {
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
        "the launcher uses an explicit Cargo manifest path, so it does not depend on the current working directory"
    );
}
