//! Core shared data structures for Guild contracts.

use std::fmt;
use std::str::FromStr;

use schemars::{
    gen::SchemaGenerator,
    schema::{InstanceType, Metadata, Schema, SchemaObject},
    JsonSchema,
};
use semver::{Version, VersionReq};
use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

pub fn mint_host_execution_id() -> String {
    Uuid::now_v7().to_string()
}

pub fn mint_host_evidence_record_id() -> String {
    Uuid::now_v7().to_string()
}

pub fn host_now_utc() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .expect("UTC timestamps format as RFC3339")
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Hash)]
pub struct SkillKey {
    pub namespace: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SkillVersion(pub Version);

impl SkillVersion {
    pub fn parse(input: &str) -> Result<Self, semver::Error> {
        Version::parse(input).map(Self)
    }

    pub fn as_semver(&self) -> &Version {
        &self.0
    }
}

impl fmt::Display for SkillVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<Version> for SkillVersion {
    fn from(value: Version) -> Self {
        Self(value)
    }
}

impl FromStr for SkillVersion {
    type Err = semver::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl Serialize for SkillVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for SkillVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(D::Error::custom)
    }
}

impl JsonSchema for SkillVersion {
    fn schema_name() -> String {
        "SkillVersion".to_owned()
    }

    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        string_schema(
            Some("semver"),
            Some("Semantic version string resolved before execution."),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VersionRequirement(pub VersionReq);

impl VersionRequirement {
    pub fn parse(input: &str) -> Result<Self, semver::Error> {
        VersionReq::parse(input).map(Self)
    }

    pub fn as_semver(&self) -> &VersionReq {
        &self.0
    }
}

impl fmt::Display for VersionRequirement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<VersionReq> for VersionRequirement {
    fn from(value: VersionReq) -> Self {
        Self(value)
    }
}

impl FromStr for VersionRequirement {
    type Err = semver::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl Serialize for VersionRequirement {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for VersionRequirement {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(D::Error::custom)
    }
}

impl JsonSchema for VersionRequirement {
    fn schema_name() -> String {
        "VersionRequirement".to_owned()
    }

    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        string_schema(
            Some("semver-req"),
            Some("Semantic version requirement resolved before execution."),
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Hash)]
pub struct RequestedSkillRef {
    pub key: SkillKey,
    pub version_req: VersionRequirement,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Hash)]
pub struct ResolvedSkillRef {
    pub key: SkillKey,
    pub version: SkillVersion,
    pub digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ManifestSchemaVersion {
    GuildManifestV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SkillApiVersion {
    GuildSkillV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AbiVersion {
    GuildSkillV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeKind {
    WasmComponent,
    InProcess,
    Process,
    Container,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SkillCategory {
    Inventory,
    Explain,
    Playbook,
    Transform,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Mutability {
    ReadOnly,
    Additive,
    Destructive,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum FreshnessClass {
    Deterministic,
    EnvironmentBound,
    TimeBound,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionMode {
    Inspect,
    Plan,
    Apply,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CapabilityId {
    HttpRequest,
    ReadResource,
    InvokeSkill,
    EmitEvidence,
    GetSecret,
    CacheRead,
    CacheWrite,
    LogWrite,
    MonotonicClock,
    WallClock,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CapabilityAccess {
    Read,
    Write,
    Invoke,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ResourceKind {
    Execution,
    Object,
}

pub const GUILD_EXECUTION_URI_PREFIX: &str = "guild://executions/";
pub const GUILD_OBJECT_BLOB_URI_PREFIX: &str = "guild://objects/sha256/";
pub const GUILD_OBJECT_RECORD_URI_PREFIX: &str = "guild://objects/records/";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum GuildResourceScope {
    Execution,
    ObjectBlob,
    ObjectRecord,
}

impl GuildResourceScope {
    pub fn parse(scope: &str) -> Result<Self, GuildResourceParseError> {
        match scope {
            GUILD_EXECUTION_URI_PREFIX => Ok(Self::Execution),
            GUILD_OBJECT_BLOB_URI_PREFIX => Ok(Self::ObjectBlob),
            GUILD_OBJECT_RECORD_URI_PREFIX => Ok(Self::ObjectRecord),
            _ => Err(GuildResourceParseError::new(format!(
                "read-resource uri_prefixes must use canonical Guild scope roots: `{}`, `{}`, or `{}`",
                GUILD_EXECUTION_URI_PREFIX,
                GUILD_OBJECT_BLOB_URI_PREFIX,
                GUILD_OBJECT_RECORD_URI_PREFIX
            ))),
        }
    }

    pub fn kind(&self) -> ResourceKind {
        match self {
            Self::Execution => ResourceKind::Execution,
            Self::ObjectBlob | Self::ObjectRecord => ResourceKind::Object,
        }
    }

    pub fn canonical_prefix(&self) -> &'static str {
        match self {
            Self::Execution => GUILD_EXECUTION_URI_PREFIX,
            Self::ObjectBlob => GUILD_OBJECT_BLOB_URI_PREFIX,
            Self::ObjectRecord => GUILD_OBJECT_RECORD_URI_PREFIX,
        }
    }

    pub fn matches(&self, uri: &GuildResourceUri) -> bool {
        matches!(
            (self, uri),
            (Self::Execution, GuildResourceUri::Execution { .. })
                | (Self::ObjectBlob, GuildResourceUri::ObjectBlob { .. })
                | (Self::ObjectRecord, GuildResourceUri::ObjectRecord { .. })
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuildResourceUri {
    Execution { execution_id: String },
    ObjectBlob { digest_hex: String },
    ObjectRecord { evidence_record_id: String },
}

impl GuildResourceUri {
    pub fn parse(uri: &str) -> Result<Self, GuildResourceParseError> {
        if let Some(encoded) = uri.strip_prefix(GUILD_EXECUTION_URI_PREFIX) {
            if encoded.is_empty() {
                return Err(GuildResourceParseError::new(
                    "execution resource URI must contain a non-empty execution identifier",
                ));
            }

            let execution_id = percent_decode_component(encoded).map_err(|error| {
                GuildResourceParseError::new(format!(
                    "execution resource URI contained invalid percent encoding: {error}"
                ))
            })?;
            if execution_id.is_empty() {
                return Err(GuildResourceParseError::new(
                    "execution resource URI must contain a non-empty execution identifier",
                ));
            }

            return Ok(Self::Execution { execution_id });
        }

        if let Some(digest_hex) = uri.strip_prefix(GUILD_OBJECT_BLOB_URI_PREFIX) {
            if digest_hex.is_empty() || !digest_hex.chars().all(is_lower_hex_digit) {
                return Err(GuildResourceParseError::new(
                    "object blob URI must contain a lowercase hexadecimal sha256 digest",
                ));
            }

            return Ok(Self::ObjectBlob {
                digest_hex: digest_hex.to_owned(),
            });
        }

        if let Some(encoded) = uri.strip_prefix(GUILD_OBJECT_RECORD_URI_PREFIX) {
            if encoded.is_empty() {
                return Err(GuildResourceParseError::new(
                    "evidence record URI must contain a non-empty record identifier",
                ));
            }

            let evidence_record_id = percent_decode_component(encoded).map_err(|error| {
                GuildResourceParseError::new(format!(
                    "evidence record URI contained invalid percent encoding: {error}"
                ))
            })?;
            if evidence_record_id.is_empty() {
                return Err(GuildResourceParseError::new(
                    "evidence record URI must contain a non-empty record identifier",
                ));
            }

            return Ok(Self::ObjectRecord { evidence_record_id });
        }

        Err(GuildResourceParseError::new(
            "resource URI did not match a supported local Guild resource",
        ))
    }

    pub fn kind(&self) -> ResourceKind {
        match self {
            Self::Execution { .. } => ResourceKind::Execution,
            Self::ObjectBlob { .. } | Self::ObjectRecord { .. } => ResourceKind::Object,
        }
    }

    pub fn scope(&self) -> GuildResourceScope {
        match self {
            Self::Execution { .. } => GuildResourceScope::Execution,
            Self::ObjectBlob { .. } => GuildResourceScope::ObjectBlob,
            Self::ObjectRecord { .. } => GuildResourceScope::ObjectRecord,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildResourceParseError {
    message: String,
}

impl GuildResourceParseError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for GuildResourceParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for GuildResourceParseError {}

impl ResourceKind {
    pub fn from_uri(uri: &str) -> Option<Self> {
        GuildResourceUri::parse(uri).ok().map(|uri| uri.kind())
    }

    pub fn from_uri_prefix(prefix: &str) -> Option<Self> {
        GuildResourceScope::parse(prefix)
            .ok()
            .map(|scope| scope.kind())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct EmptyConstraints {}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct ReadResourceConstraints {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri_prefixes: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_kinds: Option<Vec<ResourceKind>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct InvokeDependencyConstraints {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aliases: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct EmitEvidenceConstraints {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audiences: Option<Vec<EvidenceAudience>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redactions: Option<Vec<RedactionClass>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct LogConstraints {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub levels: Option<Vec<Severity>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(untagged)]
pub enum CapabilityConstraints {
    None(EmptyConstraints),
    ReadResource(ReadResourceConstraints),
    InvokeDependency(InvokeDependencyConstraints),
    EmitEvidence(EmitEvidenceConstraints),
    Log(LogConstraints),
}

impl Default for CapabilityConstraints {
    fn default() -> Self {
        Self::None(EmptyConstraints::default())
    }
}

impl CapabilityConstraints {
    pub fn none() -> Self {
        Self::default()
    }

    pub fn as_read_resource(&self) -> Option<&ReadResourceConstraints> {
        match self {
            Self::ReadResource(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_invoke_dependency(&self) -> Option<&InvokeDependencyConstraints> {
        match self {
            Self::InvokeDependency(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_emit_evidence(&self) -> Option<&EmitEvidenceConstraints> {
        match self {
            Self::EmitEvidence(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_log(&self) -> Option<&LogConstraints> {
        match self {
            Self::Log(value) => Some(value),
            _ => None,
        }
    }

    pub fn matches_capability(&self, id: &CapabilityId, access: &CapabilityAccess) -> bool {
        matches!(
            (id, access, self),
            (_, _, Self::None(_))
                | (
                    CapabilityId::ReadResource,
                    CapabilityAccess::Read,
                    Self::ReadResource(_)
                )
                | (
                    CapabilityId::InvokeSkill,
                    CapabilityAccess::Invoke,
                    Self::InvokeDependency(_)
                )
                | (
                    CapabilityId::EmitEvidence,
                    CapabilityAccess::Write,
                    Self::EmitEvidence(_)
                )
                | (
                    CapabilityId::LogWrite,
                    CapabilityAccess::Write,
                    Self::Log(_)
                )
        )
    }

    pub fn validate_for(&self, id: &CapabilityId, access: &CapabilityAccess) -> Vec<String> {
        let mut errors = Vec::new();

        if !self.matches_capability(id, access) {
            errors.push(format!(
                "constraints for {}:{} must match the capability family",
                capability_id_label(id),
                capability_access_label(access)
            ));
            return errors;
        }

        match self {
            Self::None(_) => {}
            Self::ReadResource(constraints) => errors.extend(constraints.validate()),
            Self::InvokeDependency(constraints) => errors.extend(constraints.validate()),
            Self::EmitEvidence(constraints) => errors.extend(constraints.validate()),
            Self::Log(constraints) => errors.extend(constraints.validate()),
        }

        errors
    }
}

impl ReadResourceConstraints {
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();

        if let Some(prefixes) = &self.uri_prefixes {
            if prefixes.is_empty() {
                errors.push("uri_prefixes must not be empty when provided".into());
            }

            for prefix in prefixes {
                if prefix.trim().is_empty() {
                    errors.push("uri_prefixes must not contain empty values".into());
                    continue;
                }

                let Ok(scope) = GuildResourceScope::parse(prefix) else {
                    errors.push(format!(
                        "unsupported Guild resource URI scope `{prefix}` for read-resource; expected canonical roots",
                    ));
                    continue;
                };
                let kind = scope.kind();

                if let Some(kinds) = &self.resource_kinds {
                    if !kinds.contains(&kind) {
                        errors.push(format!(
                            "uri_prefix `{prefix}` is incompatible with resource_kinds"
                        ));
                    }
                }
            }
        }

        if let Some(kinds) = &self.resource_kinds {
            if kinds.is_empty() {
                errors.push("resource_kinds must not be empty when provided".into());
            }
        }

        errors
    }
}

impl InvokeDependencyConstraints {
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();

        if let Some(aliases) = &self.aliases {
            if aliases.is_empty() {
                errors.push("aliases must not be empty when provided".into());
            }

            for alias in aliases {
                if alias.trim().is_empty() {
                    errors.push("aliases must not contain empty values".into());
                }
            }
        }

        errors
    }
}

impl EmitEvidenceConstraints {
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();

        if matches!(self.max_bytes, Some(0)) {
            errors.push("max_bytes must be greater than zero when provided".into());
        }

        if let Some(audiences) = &self.audiences {
            if audiences.is_empty() {
                errors.push("audiences must not be empty when provided".into());
            }
        }

        if let Some(redactions) = &self.redactions {
            if redactions.is_empty() {
                errors.push("redactions must not be empty when provided".into());
            }
        }

        errors
    }
}

impl LogConstraints {
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();

        if let Some(levels) = &self.levels {
            if levels.is_empty() {
                errors.push("levels must not be empty when provided".into());
            }
        }

        errors
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct CapabilityRequirement {
    pub id: CapabilityId,
    pub access: CapabilityAccess,
    #[serde(default)]
    pub constraints: CapabilityConstraints,
    pub required: bool,
}

impl CapabilityRequirement {
    pub fn validate(&self) -> Vec<String> {
        self.constraints.validate_for(&self.id, &self.access)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct GrantedCapability {
    pub id: CapabilityId,
    pub access: CapabilityAccess,
    #[serde(default)]
    pub constraints: CapabilityConstraints,
}

impl GrantedCapability {
    pub fn validate(&self) -> Vec<String> {
        self.constraints.validate_for(&self.id, &self.access)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct CapabilityGrantSet {
    #[serde(default)]
    pub grants: Vec<GrantedCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct Budget {
    pub max_millis: u64,
    pub max_memory_bytes: u64,
    pub max_output_bytes: u64,
    pub max_network_requests: u32,
    pub max_child_executions: u16,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            max_millis: 10_000,
            max_memory_bytes: 64 * 1024 * 1024,
            max_output_bytes: 512 * 1024,
            max_network_requests: 8,
            max_child_executions: 4,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ExecutionContext {
    pub execution_id: String,
    pub trace_id: String,
    pub tenant_id: String,
    pub skill: ResolvedSkillRef,
    pub mode: ExecutionMode,
    pub input_sha256: String,
    pub now_utc: Option<String>,
    pub budget: Budget,
    pub granted_capabilities: CapabilityGrantSet,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct CallerRequest {
    pub request_id: String,
    pub skill: RequestedSkillRef,
    pub tenant_id: String,
    pub actor_id: String,
    pub mode: ExecutionMode,
    pub input: Value,
    pub budget: Budget,
    pub requested_capabilities: CapabilityGrantSet,
    pub idempotency_key: Option<String>,
    pub trace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PolicyDecisionOutcome {
    Allowed,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct PolicyDecision {
    pub outcome: PolicyDecisionOutcome,
    pub summary: String,
    pub detail: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ResolvedExecutionEnvelope {
    pub request: CallerRequest,
    pub resolved_skill: ResolvedSkillRef,
    pub granted_capabilities: CapabilityGrantSet,
    pub policy_decision: PolicyDecision,
    pub parent_execution_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct DependencyInvocationRequest {
    pub alias: String,
    pub input: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Severity {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: String,
    pub message: String,
    pub retryable: bool,
    pub detail: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct Effect {
    pub kind: Mutability,
    pub target: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceAudience {
    User,
    Assistant,
    Internal,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RedactionClass {
    None,
    SecretsRemoved,
    PiiRemoved,
    TenantSensitive,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct EvidenceRef {
    pub uri: String,
    pub title: Option<String>,
    pub mime_type: Option<String>,
    pub sha256: Option<String>,
    pub audience: EvidenceAudience,
    pub redaction: RedactionClass,
    pub freshness: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct EvidenceEmissionRequest {
    pub payload: Vec<u8>,
    pub mime_type: String,
    pub title: Option<String>,
    pub audience: EvidenceAudience,
    pub redaction: RedactionClass,
    pub freshness: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ResourceReadResult {
    pub uri: String,
    pub mime_type: String,
    pub bytes: Vec<u8>,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct SkillOutput {
    pub summary: String,
    pub structured: Value,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
    #[serde(default)]
    pub effects: Vec<Effect>,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionStatus {
    Succeeded,
    Failed,
    Partial,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionPhase {
    Validation,
    Grant,
    Mode,
    RuntimeLoad,
    RuntimeExec,
    ChildInvocation,
    Persistence,
    SkillDomain,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct TerminationDetail {
    pub phase: ExecutionPhase,
    pub code: String,
    pub message: String,
    pub retryable: bool,
    pub detail: Option<Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ExecutionMetrics {
    pub duration_ms: u64,
    pub network_requests: u32,
    pub child_executions: u16,
    pub cache_hits: u32,
    pub cache_misses: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct Provenance {
    pub resolved_skill: ResolvedSkillRef,
    pub abi: AbiVersion,
    #[serde(default)]
    pub dependency_digests: Vec<String>,
    pub started_at_utc: Option<String>,
    pub finished_at_utc: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct EvidenceBlobRecord {
    pub uri: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct EvidenceRecord {
    pub uri: String,
    pub blob_uri: String,
    pub mime_type: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub title: Option<String>,
    pub audience: EvidenceAudience,
    pub redaction: RedactionClass,
    pub freshness: Option<String>,
    pub produced_by_execution: Option<String>,
}

impl EvidenceRecord {
    pub fn evidence_ref(&self) -> EvidenceRef {
        EvidenceRef {
            uri: self.uri.clone(),
            title: self.title.clone(),
            mime_type: Some(self.mime_type.clone()),
            sha256: Some(self.sha256.clone()),
            audience: self.audience.clone(),
            redaction: self.redaction.clone(),
            freshness: self.freshness.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ExecutionReceipt {
    pub execution_id: String,
    pub uri: String,
    pub trace_id: String,
    pub status: ExecutionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ExecutionRecord {
    pub receipt: ExecutionReceipt,
    pub request: CallerRequest,
    pub policy_decision: PolicyDecision,
    pub resolved_skill: ResolvedSkillRef,
    pub parent_execution_id: Option<String>,
    pub status: ExecutionStatus,
    pub output: Option<SkillOutput>,
    pub termination: Option<TerminationDetail>,
    pub granted_capabilities: CapabilityGrantSet,
    #[serde(default)]
    pub emitted_evidence: Vec<EvidenceRecord>,
    #[serde(default)]
    pub metrics: ExecutionMetrics,
    pub provenance: Provenance,
    #[serde(default)]
    pub child_executions: Vec<ChildExecutionRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct SkillError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    pub detail: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ChildExecutionRecord {
    pub alias: String,
    pub execution_id: String,
    pub uri: String,
    pub parent_execution_id: String,
    pub trace_id: String,
    pub status: ExecutionStatus,
    pub policy_decision: PolicyDecision,
    pub termination: Option<TerminationDetail>,
    pub granted_capabilities: CapabilityGrantSet,
    #[serde(default)]
    pub metrics: ExecutionMetrics,
    pub provenance: Provenance,
}

fn capability_id_label(id: &CapabilityId) -> &'static str {
    match id {
        CapabilityId::HttpRequest => "http-request",
        CapabilityId::ReadResource => "read-resource",
        CapabilityId::InvokeSkill => "invoke-skill",
        CapabilityId::EmitEvidence => "emit-evidence",
        CapabilityId::GetSecret => "get-secret",
        CapabilityId::CacheRead => "cache-read",
        CapabilityId::CacheWrite => "cache-write",
        CapabilityId::LogWrite => "log-write",
        CapabilityId::MonotonicClock => "monotonic-clock",
        CapabilityId::WallClock => "wall-clock",
    }
}

fn capability_access_label(access: &CapabilityAccess) -> &'static str {
    match access {
        CapabilityAccess::Read => "read",
        CapabilityAccess::Write => "write",
        CapabilityAccess::Invoke => "invoke",
    }
}

fn string_schema(format: Option<&str>, description: Option<&str>) -> Schema {
    let mut schema = SchemaObject {
        instance_type: Some(InstanceType::String.into()),
        format: format.map(str::to_owned),
        ..Default::default()
    };

    if let Some(description) = description {
        schema.metadata = Some(Box::new(Metadata {
            description: Some(description.to_owned()),
            ..Default::default()
        }));
    }

    Schema::Object(schema)
}

fn percent_decode_component(input: &str) -> Result<String, &'static str> {
    let mut bytes = Vec::with_capacity(input.len());
    let mut chars = input.as_bytes().iter().copied();

    while let Some(byte) = chars.next() {
        if byte == b'%' {
            let high = chars.next().ok_or("truncated escape sequence")?;
            let low = chars.next().ok_or("truncated escape sequence")?;
            let high = hex_nibble(high).ok_or("invalid escape sequence")?;
            let low = hex_nibble(low).ok_or("invalid escape sequence")?;
            bytes.push((high << 4) | low);
        } else {
            bytes.push(byte);
        }
    }

    String::from_utf8(bytes).map_err(|_| "decoded component was not valid UTF-8")
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn is_lower_hex_digit(ch: char) -> bool {
    matches!(ch, '0'..='9' | 'a'..='f')
}
