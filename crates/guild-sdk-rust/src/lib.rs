#![warn(clippy::all, clippy::pedantic, clippy::cargo, clippy::perf)]
#![allow(clippy::multiple_crate_versions)]

//! Rust authoring surface for Guild skills.

use guild_manifest::SkillManifest;
use guild_types::{ExecutionContext, SkillError, SkillOutput};
use serde_json::Value;

pub trait GuildSkill: Send + Sync {
    fn manifest(&self) -> SkillManifest;

    /// Run the skill with the host-issued execution context and input payload.
    ///
    /// # Errors
    ///
    /// Returns a guest-domain `SkillError` when the skill cannot produce a
    /// successful `SkillOutput`.
    fn run(&self, ctx: ExecutionContext, input: Value) -> Result<SkillOutput, SkillError>;
}
