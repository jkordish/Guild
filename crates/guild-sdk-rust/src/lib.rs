//! Rust authoring surface for Guild skills.

use guild_manifest::SkillManifest;
use guild_types::{ExecutionContext, ExecutionResult, SkillError};
use serde_json::Value;

pub trait GuildSkill {
    fn manifest(&self) -> SkillManifest;

    fn run(&self, ctx: ExecutionContext, input: Value) -> Result<ExecutionResult, SkillError>;
}
