#![warn(clippy::all, clippy::pedantic, clippy::cargo, clippy::perf)]
#![allow(clippy::multiple_crate_versions)]

use std::env;
use std::process::ExitCode;

use guild_mcp::server::{GuildMcpServer, ServerStartupError};

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
    let args: Vec<String> = env::args().collect();
    let registry_root =
        match GuildMcpServer::resolve_registry_root(args, env::var("GUILD_REGISTRY_ROOT").ok()) {
            Ok(path) => path,
            Err(ServerStartupError::Registry(error)) if error.code == "usage" => {
                eprintln!("{}", error.message);
                return Ok(());
            }
            Err(error) => return Err(Box::new(error)),
        };

    let mut server = GuildMcpServer::load(registry_root)?;
    server.serve_stdio()?;
    Ok(())
}
