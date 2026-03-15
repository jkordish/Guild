//! Rust authoring surface for Guild skills.

use guild_manifest::SkillManifest;
use guild_types::{ExecutionContext, SkillError, SkillOutput};
use serde_json::Value;

pub trait GuildSkill: Send + Sync {
    fn manifest(&self) -> SkillManifest;

    fn run(&self, ctx: ExecutionContext, input: Value) -> Result<SkillOutput, SkillError>;
}
