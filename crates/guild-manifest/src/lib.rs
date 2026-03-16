#![warn(clippy::all, clippy::pedantic, clippy::cargo, clippy::perf)]
#![allow(clippy::multiple_crate_versions)]

//! Manifest model for published Guild skills.

use guild_types::{
    AbiVersion, CapabilityRequirement, ExecutionMode, FreshnessClass, ManifestSchemaVersion,
    Mutability, RequestedSkillRef, ResolvedSkillRef, RuntimeKind, SkillApiVersion, SkillCategory,
    SkillKey, SkillVersion,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct SkillManifest {
    pub manifest_schema_version: ManifestSchemaVersion,
    pub skill_api_version: SkillApiVersion,
    pub key: SkillKey,
    pub version: SkillVersion,
    pub display_name: String,
    pub description: String,
    pub runtime: RuntimeSpec,
    pub interface: InterfaceSpec,
    pub behavior: BehaviorSpec,
    #[serde(default)]
    pub capabilities: Vec<CapabilityRequirement>,
    #[serde(default)]
    pub dependencies: Vec<InstalledDependencySpec>,
    pub publisher: PublisherRef,
    pub package: PackageSpec,
    #[serde(default)]
    pub tests: Vec<TestSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct SourceSkillManifest {
    pub manifest_schema_version: ManifestSchemaVersion,
    pub skill_api_version: SkillApiVersion,
    pub key: SkillKey,
    pub version: SkillVersion,
    pub display_name: String,
    pub description: String,
    pub runtime: RuntimeSpec,
    pub interface: InterfaceSpec,
    pub behavior: BehaviorSpec,
    #[serde(default)]
    pub capabilities: Vec<CapabilityRequirement>,
    #[serde(default)]
    pub dependencies: Vec<SourceDependencySpec>,
    pub publisher: PublisherRef,
    pub package: SourcePackageSpec,
    pub build: SourceBuildSpec,
    #[serde(default)]
    pub tests: Vec<TestSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct RuntimeSpec {
    pub kind: RuntimeKind,
    pub entrypoint: String,
    pub guest_abi_version: AbiVersion,
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
    pub modes: ModePolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ModePolicy {
    pub supported: Vec<ExecutionMode>,
    pub apply_requires_approval: bool,
    pub apply_requires_idempotency_key: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct SourceDependencySpec {
    pub alias: String,
    pub skill: RequestedSkillRef,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct InstalledDependencySpec {
    pub alias: String,
    pub skill: ResolvedSkillRef,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
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
pub struct SourcePackageSpec {
    pub visibility: Visibility,
    pub trust_tier: TrustTier,
    pub sbom_uri: Option<String>,
    pub signature_uri: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SourceBuildKind {
    CargoWasmComponent,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum BuildProfile {
    Release,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct SourceBuildSpec {
    pub kind: SourceBuildKind,
    pub cargo_manifest_path: String,
    #[serde(default = "default_wasm_target")]
    pub target: String,
    #[serde(default = "default_build_profile")]
    pub profile: BuildProfile,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ManifestValidationError {
    pub path: String,
    pub message: String,
}

fn default_wasm_target() -> String {
    "wasm32-wasip2".into()
}

fn default_build_profile() -> BuildProfile {
    BuildProfile::Release
}

impl ModePolicy {
    #[must_use]
    pub fn supports(&self, mode: &ExecutionMode) -> bool {
        self.supported.iter().any(|candidate| candidate == mode)
    }

    #[must_use]
    pub fn validate(&self) -> Vec<ManifestValidationError> {
        let mut errors = Vec::new();

        if self.supported.is_empty() {
            errors.push(ManifestValidationError {
                path: "behavior.modes.supported".into(),
                message: "supported execution modes must not be empty".into(),
            });
        }

        if !self.supports(&ExecutionMode::Apply) && self.apply_requires_approval {
            errors.push(ManifestValidationError {
                path: "behavior.modes.apply_requires_approval".into(),
                message: "apply approval cannot be required when apply mode is unsupported".into(),
            });
        }

        if !self.supports(&ExecutionMode::Apply) && self.apply_requires_idempotency_key {
            errors.push(ManifestValidationError {
                path: "behavior.modes.apply_requires_idempotency_key".into(),
                message: "apply idempotency cannot be required when apply mode is unsupported"
                    .into(),
            });
        }

        errors
    }
}

impl SkillManifest {
    /// Validate an installed manifest before it is accepted as executable state.
    ///
    /// # Errors
    ///
    /// Returns every manifest validation error found in the installed manifest.
    pub fn validate(&self) -> Result<(), Vec<ManifestValidationError>> {
        let mut errors = self.behavior.modes.validate();
        errors.extend(validate_installed_dependencies(&self.dependencies));
        errors.extend(validate_capabilities(&self.capabilities));

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    #[must_use]
    pub fn supports_mode(&self, mode: &ExecutionMode) -> bool {
        self.behavior.modes.supports(mode)
    }
}

impl SourceSkillManifest {
    /// Validate a source manifest before build or install.
    ///
    /// # Errors
    ///
    /// Returns every manifest validation error found in the source manifest.
    pub fn validate(&self) -> Result<(), Vec<ManifestValidationError>> {
        let mut errors = self.behavior.modes.validate();
        errors.extend(validate_source_dependencies(&self.dependencies));
        errors.extend(validate_capabilities(&self.capabilities));

        if self.build.cargo_manifest_path.trim().is_empty() {
            errors.push(ManifestValidationError {
                path: "build.cargo_manifest_path".into(),
                message: "build.cargo_manifest_path must not be empty".into(),
            });
        }

        if self.build.target.trim().is_empty() {
            errors.push(ManifestValidationError {
                path: "build.target".into(),
                message: "build.target must not be empty".into(),
            });
        }

        if self.build.kind == SourceBuildKind::CargoWasmComponent
            && self.runtime.kind != RuntimeKind::WasmComponent
        {
            errors.push(ManifestValidationError {
                path: "runtime.kind".into(),
                message: "cargo-wasm-component builds require runtime.kind = wasm-component".into(),
            });
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn into_installed(
        self,
        artifact_uri: impl Into<String>,
        artifact_digest: impl Into<String>,
        dependencies: Vec<InstalledDependencySpec>,
    ) -> SkillManifest {
        SkillManifest {
            manifest_schema_version: self.manifest_schema_version,
            skill_api_version: self.skill_api_version,
            key: self.key,
            version: self.version,
            display_name: self.display_name,
            description: self.description,
            runtime: self.runtime,
            interface: self.interface,
            behavior: self.behavior,
            capabilities: self.capabilities,
            dependencies,
            publisher: self.publisher,
            package: PackageSpec {
                visibility: self.package.visibility,
                trust_tier: self.package.trust_tier,
                artifact_uri: artifact_uri.into(),
                artifact_digest: artifact_digest.into(),
                sbom_uri: self.package.sbom_uri,
                signature_uri: self.package.signature_uri,
            },
            tests: self.tests,
        }
    }
}

fn validate_source_dependencies(
    dependencies: &[SourceDependencySpec],
) -> Vec<ManifestValidationError> {
    let mut errors = Vec::new();
    let mut seen = std::collections::BTreeSet::new();

    for (index, dependency) in dependencies.iter().enumerate() {
        if dependency.alias.trim().is_empty() {
            errors.push(ManifestValidationError {
                path: format!("dependencies[{index}].alias"),
                message: "dependency aliases must not be empty".into(),
            });
        }

        if !seen.insert(dependency.alias.clone()) {
            errors.push(ManifestValidationError {
                path: format!("dependencies[{index}].alias"),
                message: "dependency aliases must be unique".into(),
            });
        }
    }

    errors
}

fn validate_installed_dependencies(
    dependencies: &[InstalledDependencySpec],
) -> Vec<ManifestValidationError> {
    let mut errors = Vec::new();
    let mut seen = std::collections::BTreeSet::new();

    for (index, dependency) in dependencies.iter().enumerate() {
        if dependency.alias.trim().is_empty() {
            errors.push(ManifestValidationError {
                path: format!("dependencies[{index}].alias"),
                message: "dependency aliases must not be empty".into(),
            });
        }

        if !seen.insert(dependency.alias.clone()) {
            errors.push(ManifestValidationError {
                path: format!("dependencies[{index}].alias"),
                message: "dependency aliases must be unique".into(),
            });
        }

        if dependency.skill.digest.trim().is_empty() {
            errors.push(ManifestValidationError {
                path: format!("dependencies[{index}].skill.digest"),
                message: "installed dependency digests are required for execution".into(),
            });
        }
    }

    errors
}

fn validate_capabilities(capabilities: &[CapabilityRequirement]) -> Vec<ManifestValidationError> {
    let mut errors = Vec::new();

    for (index, capability) in capabilities.iter().enumerate() {
        for message in capability.validate() {
            errors.push(ManifestValidationError {
                path: format!("capabilities[{index}].constraints"),
                message,
            });
        }
    }

    errors
}
