//! Manifest model for published Guild skills.

use guild_types::{
    AbiVersion, CapabilityRequirement, FreshnessClass, Mutability, RuntimeKind, SkillCategory, SkillKey,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct SkillManifest {
    pub api_version: AbiVersion,
    pub key: SkillKey,
    pub version: String,
    pub display_name: String,
    pub description: String,
    pub runtime: RuntimeSpec,
    pub interface: InterfaceSpec,
    pub behavior: BehaviorSpec,
    #[serde(default)]
    pub capabilities: Vec<CapabilityRequirement>,
    #[serde(default)]
    pub dependencies: Vec<DependencySpec>,
    pub publisher: PublisherRef,
    pub package: PackageSpec,
    #[serde(default)]
    pub tests: Vec<TestSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct RuntimeSpec {
    pub kind: RuntimeKind,
    pub entrypoint: String,
    pub abi: AbiVersion,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct InterfaceSpec {
    pub input_schema_uri: String,
    pub output_schema_uri: String,
    pub examples_uri: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct BehaviorSpec {
    pub category: SkillCategory,
    pub mutability: Mutability,
    pub idempotent: bool,
    pub open_world: bool,
    pub freshness: FreshnessClass,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct DependencySpec {
    pub skill: SkillKey,
    pub version_req: String,
    pub pinned_digest: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct PublisherRef {
    pub id: String,
    pub display_name: String,
    pub homepage: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Visibility {
    Private,
    Org,
    Public,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TrustTier {
    Local,
    Community,
    Verified,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct PackageSpec {
    pub visibility: Visibility,
    pub trust_tier: TrustTier,
    pub artifact_uri: String,
    pub artifact_digest: String,
    pub sbom_uri: Option<String>,
    pub signature_uri: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct TestSpec {
    pub name: String,
    pub fixtures_uri: String,
    pub expected_output_uri: String,
}
