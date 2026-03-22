#![warn(clippy::all, clippy::pedantic, clippy::cargo, clippy::perf)]
#![allow(clippy::multiple_crate_versions)]

use anyhow::{Context, Result, bail};
use guild_draft_truth::{ArtifactMode, TruthAction, run_truth_action};

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        bail!(
            "usage: cargo run -p xtask -- draft-v1 <truth|support-matrix|compatibility|benchmark> <check|write>"
        );
    };
    if command != "draft-v1" {
        bail!("unknown xtask command `{command}`");
    }

    let Some(action_name) = args.next() else {
        bail!(
            "usage: cargo run -p xtask -- draft-v1 <truth|support-matrix|compatibility|benchmark> <check|write>"
        );
    };
    let Some(mode_name) = args.next() else {
        bail!(
            "usage: cargo run -p xtask -- draft-v1 <truth|support-matrix|compatibility|benchmark> <check|write>"
        );
    };
    if args.next().is_some() {
        bail!("unexpected extra arguments");
    }

    let action = match action_name.as_str() {
        "truth" => TruthAction::Truth,
        "support-matrix" => TruthAction::SupportMatrix,
        "compatibility" => TruthAction::Compatibility,
        "benchmark" => TruthAction::Benchmark,
        other => bail!("unknown draft-v1 action `{other}`"),
    };
    let mode = match mode_name.as_str() {
        "check" => ArtifactMode::Check,
        "write" => ArtifactMode::Write,
        other => bail!("unknown draft-v1 mode `{other}`"),
    };

    run_truth_action(action, mode).with_context(|| {
        format!("failed to run `cargo run -p xtask -- draft-v1 {action_name} {mode_name}`")
    })
}
