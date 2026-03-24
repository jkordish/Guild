#![allow(
    clippy::cargo_common_metadata,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::collapsible_else_if,
    clippy::large_types_passed_by_value,
    clippy::map_unwrap_or,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::must_use_candidate,
    clippy::multiple_crate_versions,
    clippy::needless_pass_by_value,
    clippy::redundant_closure_for_method_calls,
    clippy::struct_field_names,
    clippy::too_many_lines,
    clippy::uninlined_format_args,
    clippy::unnecessary_wraps
)]

pub mod benchmark;
pub mod compatibility;
pub mod patent_packet;
pub mod schemas;
pub mod support_matrix;
pub mod surface;
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

pub fn run_patent_packet_check() -> Result<()> {
    patent_packet::check()
}
