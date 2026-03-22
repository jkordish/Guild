pub mod benchmark;
pub mod compatibility;
pub mod schemas;
pub mod support_matrix;
pub mod truth;
pub mod util;

use anyhow::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactMode {
    Check,
    Write,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TruthAction {
    Truth,
    SupportMatrix,
    Compatibility,
    Benchmark,
}

pub fn run_truth_action(action: TruthAction, mode: ArtifactMode) -> Result<()> {
    match action {
        TruthAction::Truth => truth::run(mode),
        TruthAction::SupportMatrix => support_matrix::run(mode),
        TruthAction::Compatibility => compatibility::run(mode),
        TruthAction::Benchmark => benchmark::run(mode),
    }
}
