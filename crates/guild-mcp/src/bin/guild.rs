#![warn(clippy::all, clippy::pedantic, clippy::cargo, clippy::perf)]
#![allow(clippy::multiple_crate_versions)]

use std::env;
use std::process::ExitCode;

use guild_mcp::cli;

fn main() -> ExitCode {
    cli::run_entrypoint(env::args(), env::var("GUILD_REGISTRY_ROOT").ok())
}
