//! Registry model for publishing and resolving Guild skills.

use guild_manifest::SkillManifest;
use guild_types::{CapabilityId, SkillCategory, SkillRef};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct SearchQuery {
    pub query: String,
    pub limit: usize,
    pub category: Option<SkillCategory>,
    pub capability: Option<CapabilityId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct PublishedSkill {
    pub manifest: SkillManifest,
    pub installed_ref: SkillRef,
    pub created_at_utc: Option<String>,
}

pub trait SkillRegistry {
    fn resolve(&self, skill: &SkillRef) -> Option<PublishedSkill>;

    fn search(&self, query: &SearchQuery) -> Vec<PublishedSkill>;
}
