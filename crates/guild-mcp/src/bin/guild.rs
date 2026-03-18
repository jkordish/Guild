#![warn(clippy::all, clippy::pedantic, clippy::cargo, clippy::perf)]
#![allow(clippy::multiple_crate_versions)]

use std::env;
use std::process::ExitCode;

use guild_mcp::cli;

fn main() -> ExitCode {
    match cli::run(env::args(), env::var("GUILD_REGISTRY_ROOT").ok()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
