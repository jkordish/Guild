#![warn(clippy::all, clippy::pedantic, clippy::cargo, clippy::perf)]
#![allow(clippy::multiple_crate_versions)]

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use guild_mcp::codex::{
    bootstrap_codex_registry, codex_server_config, default_registry_root,
    recommended_proof_commands, CodexBootstrapOutput, DEFAULT_CODEX_SERVER_NAME,
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
                    bootstrap,
                    config,
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
    }
}

#[derive(Debug)]
enum Command {
    Bootstrap(WorkflowOptions),
    PrintConfig(WorkflowOptions),
}

#[derive(Debug)]
struct WorkflowOptions {
    registry_root: PathBuf,
    name: String,
    reset: bool,
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

    let mut options = WorkflowOptions {
        registry_root: default_registry_root(),
        name: DEFAULT_CODEX_SERVER_NAME.into(),
        reset: false,
        json: false,
    };

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
            "--reset" => {
                options.reset = true;
            }
            "--json" => {
                options.json = true;
            }
            "--help" | "-h" => return Ok(None),
            _ => return Err(format!("unexpected argument `{argument}`").into()),
        }
    }

    match subcommand.as_str() {
        "bootstrap" => Ok(Some(Command::Bootstrap(options))),
        "print-config" => Ok(Some(Command::PrintConfig(options))),
        _ => Err(format!("unknown subcommand `{subcommand}`").into()),
    }
}

fn print_usage() {
    println!(
        "usage: guild-codex <bootstrap|print-config> [--registry-root <path>] [--name <server-name>] [--reset] [--json]"
    );
    println!();
    println!(
        "bootstrap      create a local Guild root for Codex and install the default dogfood skills"
    );
    println!("print-config   print the Codex MCP config and launch snippets for an existing or planned Guild root");
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
    println!("recommended deterministic MCP-path smoke commands:");
    for command in recommended_proof_commands() {
        println!("- {command}");
    }
}

fn print_config_output(config: &guild_mcp::codex::CodexServerConfig) {
    println!("manual server launch:");
    println!("{}", config.manual_server_command());
    println!();
    println!("Codex CLI registration:");
    println!("{}", config.codex_mcp_add_command());
    println!();
    println!("config snippet for ~/.codex/config.toml or .codex/config.toml:");
    println!("{}", config.config_toml());
}
