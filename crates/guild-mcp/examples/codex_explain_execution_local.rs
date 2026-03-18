use std::path::PathBuf;

use guild_mcp::codex::{CodexSmokeSelection, bootstrap_codex_registry, repo_root, run_codex_smoke};

fn local_registry_root() -> PathBuf {
    repo_root().join("target/dev-local-registry/codex-explain-execution-local")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bootstrap = bootstrap_codex_registry(local_registry_root(), true)?;
    let summary = run_codex_smoke(
        &bootstrap.registry_root,
        "guild-local",
        CodexSmokeSelection::ExplainExecution,
    )?;

    println!("{}", summary.render_text());

    Ok(())
}
