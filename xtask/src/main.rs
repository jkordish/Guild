#![warn(clippy::all, clippy::pedantic, clippy::cargo, clippy::perf)]
#![allow(clippy::multiple_crate_versions)]

mod axiom_plan;
mod effect_kernel;

use anyhow::{Context, Result, bail};
use guild_draft_truth::{
    ArtifactMode, TruthAction, run_patent_packet_check, run_project_positioning_check,
    run_truth_action,
};

const USAGE: &str = "usage: cargo run -p xtask -- draft-v1 <truth|support-matrix|compatibility|benchmark> <check|write>\n       cargo run -p xtask -- patent-packet check\n       cargo run -p xtask -- project-positioning check\n       cargo run -p xtask -- axiom-plan validate <path>\n       cargo run -p xtask -- axiom-plan validate-examples\n       cargo run -p xtask -- axiom-plan preview <path> [--json]\n       cargo run -p xtask -- axiom-plan check-goldens [--update]\n       cargo run -p xtask -- effect-kernel check-dependencies";

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        bail!("{USAGE}");
    };
    if command == "axiom-plan" {
        return axiom_plan::run(args);
    }
    if command == "effect-kernel" {
        return effect_kernel::run(args);
    }
    if command == "patent-packet" {
        let Some(mode_name) = args.next() else {
            bail!("usage: cargo run -p xtask -- patent-packet check");
        };
        if args.next().is_some() {
            bail!("unexpected extra arguments");
        }
        if mode_name != "check" {
            bail!("unknown patent-packet mode `{mode_name}`");
        }
        return run_patent_packet_check()
            .with_context(|| "failed to run `cargo run -p xtask -- patent-packet check`");
    }
    if command == "project-positioning" {
        let Some(mode_name) = args.next() else {
            bail!("usage: cargo run -p xtask -- project-positioning check");
        };
        if args.next().is_some() {
            bail!("unexpected extra arguments");
        }
        if mode_name != "check" {
            bail!("unknown project-positioning mode `{mode_name}`");
        }
        return run_project_positioning_check()
            .with_context(|| "failed to run `cargo run -p xtask -- project-positioning check`");
    }
    if command != "draft-v1" {
        bail!("unknown xtask command `{command}`");
    }

    let Some(action_name) = args.next() else {
        bail!("{USAGE}");
    };
    let Some(mode_name) = args.next() else {
        bail!("{USAGE}");
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
