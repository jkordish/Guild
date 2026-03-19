#![warn(clippy::all, clippy::pedantic, clippy::cargo, clippy::perf)]
#![allow(clippy::multiple_crate_versions)]

use std::env;
use std::process::ExitCode;

use guild_mcp::codex_cli;

fn main() -> ExitCode {
    match codex_cli::run_legacy_binary(env::args()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
