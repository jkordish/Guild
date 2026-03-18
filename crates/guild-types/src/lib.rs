#![warn(clippy::all, clippy::pedantic, clippy::cargo, clippy::perf)]
#![allow(clippy::multiple_crate_versions)]

//! Core shared data structures for Guild contracts.

use std::borrow::Cow;
use std::fmt;
use std::fmt::Write as _;
use std::str::FromStr;

use schemars::{JsonSchema, Schema, SchemaGenerator};
use semver::{Version, VersionReq};
use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

/// Mint a host-owned durable execution identifier.
#[must_use]
pub fn mint_host_execution_id() -> String {
    Uuid::now_v7().to_string()
}

/// Mint a host-owned durable evidence-record identifier.
#[must_use]
pub fn mint_host_evidence_record_id() -> String {
    Uuid::now_v7().to_string()
}

/// Return the current host UTC timestamp formatted as RFC 3339.
///
/// # Panics
///
/// Panics if formatting a UTC timestamp as RFC 3339 unexpectedly fails.
#[must_use]
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
    /// Parse a semantic version string into a Guild skill version.
    ///
    /// # Errors
    ///
    /// Returns an error when `input` is not valid semantic version syntax.
    pub fn parse(input: &str) -> Result<Self, semver::Error> {
        Version::parse(input).map(Self)
    }

    #[must_use]
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
    fn schema_name() -> Cow<'static, str> {
        "SkillVersion".into()
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
    /// Parse a semantic version requirement string into a Guild version requirement.
    ///
    /// # Errors
    ///
    /// Returns an error when `input` is not valid semantic version requirement syntax.
    pub fn parse(input: &str) -> Result<Self, semver::Error> {
        VersionReq::parse(input).map(Self)
    }

    #[must_use]
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
    fn schema_name() -> Cow<'static, str> {
        "VersionRequirement".into()
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
    GuildSkillInspectV1,
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
    Filesystem,
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
pub enum HttpMethod {
    Get,
    Head,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum HttpScheme {
    Http,
    Https,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ResourceKind {
    Execution,
    Object,
    Query,
}

pub const GUILD_EXECUTION_URI_PREFIX: &str = "guild://executions/";
pub const GUILD_OBJECT_BLOB_URI_PREFIX: &str = "guild://objects/sha256/";
pub const GUILD_OBJECT_RECORD_URI_PREFIX: &str = "guild://objects/records/";
pub const GUILD_OBJECT_RECORD_METADATA_URI_SUFFIX: &str = "/metadata";
pub const GUILD_EXECUTION_QUERY_URI_PREFIX: &str = "guild://queries/executions/";
pub const MAX_EXECUTION_QUERY_LIMIT: usize = 50;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ExecutionQueryResource {
    Recent {
        limit: usize,
    },
    FailuresRecent {
        limit: usize,
    },
    ByStatus {
        status: ExecutionStatus,
        limit: usize,
    },
    BySkill {
        namespace: String,
        name: String,
        limit: usize,
    },
}

impl ExecutionQueryResource {
    /// Parse a canonical Guild execution query URI.
    ///
    /// # Errors
    ///
    /// Returns an error when `uri` is malformed, uses invalid percent encoding,
    /// names an unsupported query path, or requests an out-of-range result limit.
    pub fn parse_uri(uri: &str) -> Result<Self, GuildResourceParseError> {
        let Some(path) = uri.strip_prefix(GUILD_EXECUTION_QUERY_URI_PREFIX) else {
            return Err(GuildResourceParseError::new(format!(
                "execution query URI must start with `{GUILD_EXECUTION_QUERY_URI_PREFIX}`"
            )));
        };

        let segments = path.split('/').collect::<Vec<_>>();
        match segments.as_slice() {
            ["recent", limit] => Ok(Self::Recent {
                limit: parse_execution_query_limit(limit)?,
            }),
            ["failures", "recent", limit] => Ok(Self::FailuresRecent {
                limit: parse_execution_query_limit(limit)?,
            }),
            ["by-status", status, limit] => Ok(Self::ByStatus {
                status: parse_execution_query_status(status)?,
                limit: parse_execution_query_limit(limit)?,
            }),
            ["by-skill", namespace, name, limit] => {
                let namespace = percent_decode_component(namespace).map_err(|error| {
                    GuildResourceParseError::new(format!(
                        "execution query namespace contained invalid percent encoding: {error}"
                    ))
                })?;
                let name = percent_decode_component(name).map_err(|error| {
                    GuildResourceParseError::new(format!(
                        "execution query skill name contained invalid percent encoding: {error}"
                    ))
                })?;

                if namespace.is_empty() || name.is_empty() {
                    return Err(GuildResourceParseError::new(
                        "execution query skill path must contain non-empty namespace and name",
                    ));
                }

                Ok(Self::BySkill {
                    namespace,
                    name,
                    limit: parse_execution_query_limit(limit)?,
                })
            }
            _ => Err(GuildResourceParseError::new(
                "execution query URI did not match a supported local Guild query path",
            )),
        }
    }

    #[must_use]
    pub fn canonical_uri(&self) -> String {
        match self {
            Self::Recent { limit } => {
                format!("{GUILD_EXECUTION_QUERY_URI_PREFIX}recent/{limit}")
            }
            Self::FailuresRecent { limit } => {
                format!("{GUILD_EXECUTION_QUERY_URI_PREFIX}failures/recent/{limit}")
            }
            Self::ByStatus { status, limit } => format!(
                "{GUILD_EXECUTION_QUERY_URI_PREFIX}by-status/{}/{limit}",
                execution_status_label(status)
            ),
            Self::BySkill {
                namespace,
                name,
                limit,
            } => format!(
                "{GUILD_EXECUTION_QUERY_URI_PREFIX}by-skill/{}/{}/{}",
                percent_encode_component(namespace),
                percent_encode_component(name),
                limit
            ),
        }
    }

    #[must_use]
    pub fn limit(&self) -> usize {
        match self {
            Self::Recent { limit }
            | Self::FailuresRecent { limit }
            | Self::ByStatus { limit, .. }
            | Self::BySkill { limit, .. } => *limit,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum GuildResourceScope {
    Execution,
    ObjectBlob,
    ObjectRecord,
    ExecutionQuery,
}

impl GuildResourceScope {
    /// Parse an exact canonical Guild resource scope root.
    ///
    /// # Errors
    ///
    /// Returns an error when `scope` is not one of the supported canonical Guild
    /// resource scope roots.
    pub fn parse(scope: &str) -> Result<Self, GuildResourceParseError> {
        match scope {
            GUILD_EXECUTION_URI_PREFIX => Ok(Self::Execution),
            GUILD_OBJECT_BLOB_URI_PREFIX => Ok(Self::ObjectBlob),
            GUILD_OBJECT_RECORD_URI_PREFIX => Ok(Self::ObjectRecord),
            GUILD_EXECUTION_QUERY_URI_PREFIX => Ok(Self::ExecutionQuery),
            _ => Err(GuildResourceParseError::new(format!(
                "read-resource uri_prefixes must use canonical Guild scope roots: \
                 `{GUILD_EXECUTION_URI_PREFIX}`, `{GUILD_OBJECT_BLOB_URI_PREFIX}`, \
                 `{GUILD_OBJECT_RECORD_URI_PREFIX}`, or \
                 `{GUILD_EXECUTION_QUERY_URI_PREFIX}`"
            ))),
        }
    }

    #[must_use]
    pub fn kind(&self) -> ResourceKind {
        match self {
            Self::Execution => ResourceKind::Execution,
            Self::ObjectBlob | Self::ObjectRecord => ResourceKind::Object,
            Self::ExecutionQuery => ResourceKind::Query,
        }
    }

    #[must_use]
    pub fn canonical_prefix(&self) -> &'static str {
        match self {
            Self::Execution => GUILD_EXECUTION_URI_PREFIX,
            Self::ObjectBlob => GUILD_OBJECT_BLOB_URI_PREFIX,
            Self::ObjectRecord => GUILD_OBJECT_RECORD_URI_PREFIX,
            Self::ExecutionQuery => GUILD_EXECUTION_QUERY_URI_PREFIX,
        }
    }

    #[must_use]
    pub fn matches(&self, uri: &GuildResourceUri) -> bool {
        matches!(
            (self, uri),
            (Self::Execution, GuildResourceUri::Execution { .. })
                | (Self::ObjectBlob, GuildResourceUri::ObjectBlob { .. })
                | (
                    Self::ObjectRecord,
                    GuildResourceUri::ObjectRecord { .. }
                        | GuildResourceUri::ObjectRecordMetadata { .. }
                )
                | (
                    Self::ExecutionQuery,
                    GuildResourceUri::ExecutionQuery { .. }
                )
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuildResourceUri {
    Execution { execution_id: String },
    ObjectBlob { digest_hex: String },
    ObjectRecord { evidence_record_id: String },
    ObjectRecordMetadata { evidence_record_id: String },
    ExecutionQuery { query: ExecutionQueryResource },
}

impl GuildResourceUri {
    /// Parse a concrete Guild resource URI.
    ///
    /// # Errors
    ///
    /// Returns an error when `uri` is malformed, uses invalid percent encoding, or
    /// does not match a supported local Guild resource kind.
    pub fn parse(uri: &str) -> Result<Self, GuildResourceParseError> {
        if uri.starts_with(GUILD_EXECUTION_QUERY_URI_PREFIX) {
            return Ok(Self::ExecutionQuery {
                query: ExecutionQueryResource::parse_uri(uri)?,
            });
        }

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

            let (encoded_record_id, metadata) = encoded
                .strip_suffix(GUILD_OBJECT_RECORD_METADATA_URI_SUFFIX)
                .map_or((encoded, false), |record_id| (record_id, true));

            if encoded_record_id.is_empty() {
                return Err(GuildResourceParseError::new(
                    "evidence record URI must contain a non-empty record identifier",
                ));
            }

            if encoded_record_id.contains('/') {
                return Err(GuildResourceParseError::new(
                    "evidence record URI did not match a supported local Guild object path",
                ));
            }

            let evidence_record_id =
                percent_decode_component(encoded_record_id).map_err(|error| {
                    GuildResourceParseError::new(format!(
                        "evidence record URI contained invalid percent encoding: {error}"
                    ))
                })?;
            if evidence_record_id.is_empty() {
                return Err(GuildResourceParseError::new(
                    "evidence record URI must contain a non-empty record identifier",
                ));
            }

            return Ok(if metadata {
                Self::ObjectRecordMetadata { evidence_record_id }
            } else {
                Self::ObjectRecord { evidence_record_id }
            });
        }

        Err(GuildResourceParseError::new(
            "resource URI did not match a supported local Guild resource",
        ))
    }

    #[must_use]
    pub fn kind(&self) -> ResourceKind {
        match self {
            Self::Execution { .. } => ResourceKind::Execution,
            Self::ObjectBlob { .. }
            | Self::ObjectRecord { .. }
            | Self::ObjectRecordMetadata { .. } => ResourceKind::Object,
            Self::ExecutionQuery { .. } => ResourceKind::Query,
        }
    }

    #[must_use]
    pub fn scope(&self) -> GuildResourceScope {
        match self {
            Self::Execution { .. } => GuildResourceScope::Execution,
            Self::ObjectBlob { .. } => GuildResourceScope::ObjectBlob,
            Self::ObjectRecord { .. } | Self::ObjectRecordMetadata { .. } => {
                GuildResourceScope::ObjectRecord
            }
            Self::ExecutionQuery { .. } => GuildResourceScope::ExecutionQuery,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildResourceParseError {
    message: String,
}

impl GuildResourceParseError {
    #[must_use]
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
    #[must_use]
    pub fn from_uri(uri: &str) -> Option<Self> {
        GuildResourceUri::parse(uri).ok().map(|uri| uri.kind())
    }

    #[must_use]
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum FilesystemOperation {
    Read,
    Write,
    Create,
    Append,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct FilesystemRoot {
    pub name: String,
    pub guest_path_prefix: String,
    pub host_path: String,
    pub operations: Vec<FilesystemOperation>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct FilesystemConstraints {
    pub preopened_roots: Vec<FilesystemRoot>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct HttpRequestConstraints {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_schemes: Option<Vec<HttpScheme>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_hosts: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_host_suffixes: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_ports: Option<Vec<u16>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_methods: Option<Vec<HttpMethod>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_path_prefixes: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_timeout_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_response_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub follow_redirects: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_redirects: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_loopback: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_link_local: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_private_networks: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_ip_literals: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(untagged)]
pub enum CapabilityConstraints {
    None(EmptyConstraints),
    Filesystem(FilesystemConstraints),
    HttpRequest(HttpRequestConstraints),
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
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn as_http_request(&self) -> Option<&HttpRequestConstraints> {
        match self {
            Self::HttpRequest(value) => Some(value),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_filesystem(&self) -> Option<&FilesystemConstraints> {
        match self {
            Self::Filesystem(value) => Some(value),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_read_resource(&self) -> Option<&ReadResourceConstraints> {
        match self {
            Self::ReadResource(value) => Some(value),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_invoke_dependency(&self) -> Option<&InvokeDependencyConstraints> {
        match self {
            Self::InvokeDependency(value) => Some(value),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_emit_evidence(&self) -> Option<&EmitEvidenceConstraints> {
        match self {
            Self::EmitEvidence(value) => Some(value),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_log(&self) -> Option<&LogConstraints> {
        match self {
            Self::Log(value) => Some(value),
            _ => None,
        }
    }

    #[must_use]
    pub fn matches_capability(&self, id: &CapabilityId, access: &CapabilityAccess) -> bool {
        matches!(
            (id, access, self),
            (_, _, Self::None(_))
                | (
                    CapabilityId::Filesystem,
                    CapabilityAccess::Read | CapabilityAccess::Write,
                    Self::Filesystem(_)
                )
                | (
                    CapabilityId::HttpRequest,
                    CapabilityAccess::Read,
                    Self::HttpRequest(_)
                )
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

    #[must_use]
    pub fn validate_for(&self, id: &CapabilityId, access: &CapabilityAccess) -> Vec<String> {
        let mut errors = Vec::new();

        if matches!(id, CapabilityId::Filesystem) && matches!(self, Self::None(_)) {
            errors.push(
                "filesystem capabilities must declare explicit filesystem constraints".into(),
            );
            return errors;
        }

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
            Self::Filesystem(constraints) => errors.extend(constraints.validate(access)),
            Self::HttpRequest(constraints) => errors.extend(constraints.validate()),
            Self::ReadResource(constraints) => errors.extend(constraints.validate()),
            Self::InvokeDependency(constraints) => errors.extend(constraints.validate()),
            Self::EmitEvidence(constraints) => errors.extend(constraints.validate()),
            Self::Log(constraints) => errors.extend(constraints.validate()),
        }

        errors
    }
}

impl FilesystemConstraints {
    #[must_use]
    pub fn validate(&self, access: &CapabilityAccess) -> Vec<String> {
        let mut errors = Vec::new();

        if self.preopened_roots.is_empty() {
            errors.push("preopened_roots must not be empty".into());
            return errors;
        }

        let mut seen_names = std::collections::HashSet::new();
        let mut seen_guest_paths = std::collections::HashSet::new();

        for (index, root) in self.preopened_roots.iter().enumerate() {
            let prefix = format!("preopened_roots[{index}]");

            if root.name.trim().is_empty() {
                errors.push(format!("{prefix}.name must not be empty"));
            } else if !seen_names.insert(root.name.clone()) {
                errors.push(format!("{prefix}.name must be unique"));
            }

            if root.guest_path_prefix.trim().is_empty() {
                errors.push(format!("{prefix}.guest_path_prefix must not be empty"));
            } else {
                if let Some(message) =
                    validate_filesystem_guest_path_prefix(&root.guest_path_prefix)
                {
                    errors.push(format!("{prefix}.guest_path_prefix {message}"));
                }

                if !seen_guest_paths.insert(root.guest_path_prefix.clone()) {
                    errors.push(format!("{prefix}.guest_path_prefix must be unique"));
                }
            }

            if root.host_path.trim().is_empty() {
                errors.push(format!("{prefix}.host_path must not be empty"));
            }

            if root.operations.is_empty() {
                errors.push(format!("{prefix}.operations must not be empty"));
                continue;
            }

            let mut seen_operations = std::collections::HashSet::new();
            for operation in &root.operations {
                if !seen_operations.insert(operation) {
                    errors.push(format!(
                        "{prefix}.operations must not contain duplicate `{}` entries",
                        filesystem_operation_label(operation)
                    ));
                }

                match (access, operation) {
                    (CapabilityAccess::Read, FilesystemOperation::Read)
                    | (
                        CapabilityAccess::Write,
                        FilesystemOperation::Write
                        | FilesystemOperation::Create
                        | FilesystemOperation::Append,
                    ) => {}
                    (CapabilityAccess::Read, _) => errors.push(format!(
                        "{prefix}.operations may only contain `read` when access is `read`"
                    )),
                    (CapabilityAccess::Write, FilesystemOperation::Read) => errors.push(format!(
                        "{prefix}.operations must not contain `read` when access is `write`"
                    )),
                    (CapabilityAccess::Invoke, _) => {
                        errors.push("filesystem capabilities must use read or write access".into());
                    }
                }
            }
        }

        errors
    }
}

impl ReadResourceConstraints {
    #[must_use]
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

                if let Some(kinds) = &self.resource_kinds
                    && !kinds.contains(&kind)
                {
                    errors.push(format!(
                        "uri_prefix `{prefix}` is incompatible with resource_kinds"
                    ));
                }
            }
        }

        if let Some(kinds) = &self.resource_kinds
            && kinds.is_empty()
        {
            errors.push("resource_kinds must not be empty when provided".into());
        }

        errors
    }
}

impl InvokeDependencyConstraints {
    #[must_use]
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
    #[must_use]
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();

        if matches!(self.max_bytes, Some(0)) {
            errors.push("max_bytes must be greater than zero when provided".into());
        }

        if let Some(audiences) = &self.audiences
            && audiences.is_empty()
        {
            errors.push("audiences must not be empty when provided".into());
        }

        if let Some(redactions) = &self.redactions
            && redactions.is_empty()
        {
            errors.push("redactions must not be empty when provided".into());
        }

        errors
    }
}

impl LogConstraints {
    #[must_use]
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();

        if let Some(levels) = &self.levels
            && levels.is_empty()
        {
            errors.push("levels must not be empty when provided".into());
        }

        errors
    }
}

impl HttpRequestConstraints {
    #[must_use]
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();

        if let Some(schemes) = &self.allowed_schemes
            && schemes.is_empty()
        {
            errors.push("allowed_schemes must not be empty when provided".into());
        }

        if let Some(hosts) = &self.allowed_hosts {
            if hosts.is_empty() {
                errors.push("allowed_hosts must not be empty when provided".into());
            }

            for host in hosts {
                if let Some(message) = validate_http_host_value(host, "allowed_hosts", true) {
                    errors.push(message);
                }
            }
        }

        if let Some(suffixes) = &self.allowed_host_suffixes {
            if suffixes.is_empty() {
                errors.push("allowed_host_suffixes must not be empty when provided".into());
            }

            for suffix in suffixes {
                if let Some(message) =
                    validate_http_host_value(suffix, "allowed_host_suffixes", false)
                {
                    errors.push(message);
                }
            }
        }

        if let Some(ports) = &self.allowed_ports {
            if ports.is_empty() {
                errors.push("allowed_ports must not be empty when provided".into());
            }

            if ports.contains(&0) {
                errors.push("allowed_ports must not contain zero".into());
            }
        }

        if let Some(methods) = &self.allowed_methods
            && methods.is_empty()
        {
            errors.push("allowed_methods must not be empty when provided".into());
        }

        if let Some(prefixes) = &self.allowed_path_prefixes {
            if prefixes.is_empty() {
                errors.push("allowed_path_prefixes must not be empty when provided".into());
            }

            for prefix in prefixes {
                if prefix.trim().is_empty() {
                    errors.push("allowed_path_prefixes must not contain empty values".into());
                    continue;
                }

                if !prefix.starts_with('/') {
                    errors.push("allowed_path_prefixes must start with `/`".into());
                }
            }
        }

        if matches!(self.max_timeout_ms, Some(0)) {
            errors.push("max_timeout_ms must be greater than zero when provided".into());
        }

        if matches!(self.max_response_bytes, Some(0)) {
            errors.push("max_response_bytes must be greater than zero when provided".into());
        }

        if self.follow_redirects == Some(true) {
            if matches!(self.max_redirects, None | Some(0)) {
                errors.push(
                    "max_redirects must be greater than zero when follow_redirects is true".into(),
                );
            }
        } else if self.max_redirects.is_some() {
            errors.push("max_redirects requires follow_redirects to be true".into());
        }

        errors
    }
}

fn validate_http_host_value(value: &str, field: &str, allow_ip_literals: bool) -> Option<String> {
    if value.trim().is_empty() {
        return Some(format!("{field} must not contain empty values"));
    }

    if value != value.trim() {
        return Some(format!(
            "{field} entries must not contain leading or trailing whitespace"
        ));
    }

    if value.contains("://")
        || value.contains('/')
        || value.contains('?')
        || value.contains('#')
        || value.contains('@')
    {
        return Some(format!(
            "{field} entries must contain only canonical hostnames or IP literals without schemes, paths, fragments, queries, or credentials"
        ));
    }

    if value.starts_with('.') || value.ends_with('.') {
        return Some(format!("{field} entries must not begin or end with `.`"));
    }

    if value.parse::<std::net::IpAddr>().is_ok() {
        if allow_ip_literals {
            return None;
        }

        return Some(format!(
            "{field} entries must not use raw IP literals; use canonical domain suffixes only"
        ));
    }

    if value.contains(':') {
        return Some(format!(
            "{field} entries must not include ports; configure ports through allowed_ports"
        ));
    }

    let labels = value.split('.').collect::<Vec<_>>();
    if labels.iter().any(|label| label.is_empty()) {
        return Some(format!("{field} entries must not contain empty DNS labels"));
    }

    if labels.iter().any(|label| {
        !label
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            || label.starts_with('-')
            || label.ends_with('-')
    }) {
        return Some(format!(
            "{field} entries must use ASCII alphanumeric or `-` DNS labels"
        ));
    }

    None
}

fn validate_filesystem_guest_path_prefix(value: &str) -> Option<&'static str> {
    if !value.starts_with('/') {
        return Some("must start with `/`");
    }

    if value != "/" && value.ends_with('/') {
        return Some("must not end with `/` unless it is the root `/`");
    }

    if value.contains('\\') {
        return Some("must use `/` separators only");
    }

    if value.split('/').skip(1).any(str::is_empty) {
        return Some("must not contain empty path segments");
    }

    if value
        .split('/')
        .skip(1)
        .any(|segment| matches!(segment, "." | ".."))
    {
        return Some("must not contain `.` or `..` path segments");
    }

    None
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
    #[must_use]
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
    #[must_use]
    pub fn validate(&self) -> Vec<String> {
        self.constraints.validate_for(&self.id, &self.access)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct CapabilityGrantSet {
    #[serde(default)]
    pub grants: Vec<GrantedCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct CapabilitySelector {
    pub id: CapabilityId,
    pub access: CapabilityAccess,
}

impl CapabilitySelector {
    #[must_use]
    pub fn matches(&self, grant: &GrantedCapability) -> bool {
        self.id == grant.id && self.access == grant.access
    }
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
pub enum LocalPolicyFormatVersion {
    GuildLocalPolicyV2,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum LocalPolicyDefaultAction {
    AllowRequestedDeclared,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum LocalTrustTier {
    LocalDev,
    TrustedImported,
    Restricted,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum InstalledVerificationState {
    LocalSource,
    VerifiedImport,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PolicyRuleEffect {
    Deny,
    Cap,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PolicyRuleTarget {
    Any,
    Requested,
    Required,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct PolicyReason {
    pub code: String,
    pub message: String,
    pub detail: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct PolicyRule {
    pub name: Option<String>,
    #[serde(default)]
    pub skills: Option<Vec<SkillKey>>,
    #[serde(default)]
    pub publisher_ids: Option<Vec<String>>,
    #[serde(default)]
    pub trust_tiers: Option<Vec<LocalTrustTier>>,
    #[serde(default)]
    pub verification_states: Option<Vec<InstalledVerificationState>>,
    #[serde(default = "default_policy_rule_target")]
    pub applies_to: PolicyRuleTarget,
    pub effect: PolicyRuleEffect,
    #[serde(default)]
    pub capabilities: CapabilityGrantSet,
}

impl PolicyRule {
    #[must_use]
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();

        if let Some(name) = &self.name
            && name.trim().is_empty()
        {
            errors.push("policy rule names must not be empty when provided".into());
        }

        if let Some(skills) = &self.skills {
            if skills.is_empty() {
                errors.push("policy rule skills must not be empty when provided".into());
            }

            if skills
                .iter()
                .any(|skill| skill.namespace.trim().is_empty() || skill.name.trim().is_empty())
            {
                errors.push(
                    "policy rule skills must not contain empty namespace or name values".into(),
                );
            }
        }

        if let Some(publisher_ids) = &self.publisher_ids {
            if publisher_ids.is_empty() {
                errors.push("policy rule publisher_ids must not be empty when provided".into());
            }

            if publisher_ids
                .iter()
                .any(|publisher| publisher.trim().is_empty())
            {
                errors.push("policy rule publisher_ids must not contain empty values".into());
            }
        }

        if let Some(trust_tiers) = &self.trust_tiers
            && trust_tiers.is_empty()
        {
            errors.push("policy rule trust_tiers must not be empty when provided".into());
        }

        if let Some(verification_states) = &self.verification_states
            && verification_states.is_empty()
        {
            errors.push("policy rule verification_states must not be empty when provided".into());
        }

        if self.capabilities.grants.is_empty() {
            errors.push("policy rules must declare at least one capability ceiling".into());
        }

        for (index, grant) in self.capabilities.grants.iter().enumerate() {
            for message in grant.validate() {
                errors.push(format!("policy rule capability grant {index}: {message}"));
            }
        }

        errors
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct PolicyProfile {
    pub name: String,
    #[serde(default = "default_local_policy_action")]
    pub default_action: LocalPolicyDefaultAction,
    #[serde(default)]
    pub rules: Vec<PolicyRule>,
}

impl PolicyProfile {
    #[must_use]
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();

        if self.name.trim().is_empty() {
            errors.push("policy profile names must not be empty".into());
        }

        for (index, rule) in self.rules.iter().enumerate() {
            for message in rule.validate() {
                errors.push(format!("policy profile rules[{index}]: {message}"));
            }
        }

        errors
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct PolicyProfileBinding {
    pub name: Option<String>,
    #[serde(default)]
    pub actor_ids: Option<Vec<String>>,
    #[serde(default)]
    pub tenant_ids: Option<Vec<String>>,
    pub profile: String,
}

impl PolicyProfileBinding {
    #[must_use]
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();

        if let Some(name) = &self.name
            && name.trim().is_empty()
        {
            errors.push("policy profile binding names must not be empty when provided".into());
        }

        if self.profile.trim().is_empty() {
            errors.push("policy profile bindings must reference a non-empty profile".into());
        }

        if self.actor_ids.is_none() && self.tenant_ids.is_none() {
            errors.push(
                "policy profile bindings must declare at least one actor_ids or tenant_ids selector"
                    .into(),
            );
        }

        if let Some(actor_ids) = &self.actor_ids {
            if actor_ids.is_empty() {
                errors.push("policy profile binding actor_ids must not be empty".into());
            }

            if actor_ids.iter().any(|actor| actor.trim().is_empty()) {
                errors
                    .push("policy profile binding actor_ids must not contain empty values".into());
            }
        }

        if let Some(tenant_ids) = &self.tenant_ids {
            if tenant_ids.is_empty() {
                errors.push("policy profile binding tenant_ids must not be empty".into());
            }

            if tenant_ids.iter().any(|tenant| tenant.trim().is_empty()) {
                errors
                    .push("policy profile binding tenant_ids must not contain empty values".into());
            }
        }

        errors
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct LocalPolicyConfig {
    #[serde(default = "default_local_policy_format_version")]
    pub format_version: LocalPolicyFormatVersion,
    #[serde(default)]
    pub default_profile: String,
    #[serde(default)]
    pub profiles: Vec<PolicyProfile>,
    #[serde(default)]
    pub bindings: Vec<PolicyProfileBinding>,
}

impl Default for LocalPolicyConfig {
    fn default() -> Self {
        Self {
            format_version: default_local_policy_format_version(),
            default_profile: default_policy_profile_name(),
            profiles: vec![default_policy_profile()],
            bindings: Vec::new(),
        }
    }
}

impl LocalPolicyConfig {
    #[must_use]
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();

        if self.default_profile.trim().is_empty() {
            errors.push("local policy default_profile must not be empty".into());
        }

        if self.profiles.is_empty() {
            errors.push("local policy must declare at least one profile".into());
        }

        let mut profile_names = std::collections::HashSet::new();
        for (index, profile) in self.profiles.iter().enumerate() {
            for message in profile.validate() {
                errors.push(format!("policy profiles[{index}]: {message}"));
            }

            if !profile.name.trim().is_empty() && !profile_names.insert(profile.name.clone()) {
                errors.push(format!(
                    "policy profiles[{index}] reused duplicate profile name {}",
                    profile.name
                ));
            }
        }

        if !self
            .profiles
            .iter()
            .any(|profile| profile.name == self.default_profile)
        {
            errors.push("local policy default_profile must reference a declared profile".into());
        }

        for (index, binding) in self.bindings.iter().enumerate() {
            for message in binding.validate() {
                errors.push(format!("policy bindings[{index}]: {message}"));
            }

            if !binding.profile.trim().is_empty()
                && !self
                    .profiles
                    .iter()
                    .any(|profile| profile.name == binding.profile)
            {
                errors.push(format!(
                    "policy bindings[{index}] referenced unknown profile {}",
                    binding.profile
                ));
            }
        }

        errors
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PolicyDecisionOutcome {
    Allowed,
    Reduced,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct PolicyDecision {
    pub outcome: PolicyDecisionOutcome,
    pub summary: String,
    pub profile_name: String,
    pub trust_tier: LocalTrustTier,
    pub verification_state: InstalledVerificationState,
    #[serde(default)]
    pub reasons: Vec<PolicyReason>,
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: HttpMethod,
    pub url: String,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct HttpResponse {
    pub url: String,
    pub status: u16,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
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
    #[must_use]
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
pub struct ExecutionQueryMatch {
    pub receipt: ExecutionReceipt,
    pub resolved_skill: ResolvedSkillRef,
    pub status: ExecutionStatus,
    pub policy_decision: PolicyDecision,
    pub termination: Option<TerminationDetail>,
    pub parent_execution_id: Option<String>,
    pub evidence_count: usize,
    #[serde(default)]
    pub sample_evidence_record_uris: Vec<String>,
    pub child_execution_count: usize,
    pub started_at_utc: Option<String>,
    pub finished_at_utc: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ExecutionQueryResult {
    pub query_uri: String,
    pub total_matches: usize,
    pub returned_matches: usize,
    pub truncated: bool,
    #[serde(default)]
    pub results: Vec<ExecutionQueryMatch>,
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
        CapabilityId::Filesystem => "filesystem",
        CapabilityId::MonotonicClock => "monotonic-clock",
        CapabilityId::WallClock => "wall-clock",
    }
}

fn filesystem_operation_label(operation: &FilesystemOperation) -> &'static str {
    match operation {
        FilesystemOperation::Read => "read",
        FilesystemOperation::Write => "write",
        FilesystemOperation::Create => "create",
        FilesystemOperation::Append => "append",
    }
}

fn capability_access_label(access: &CapabilityAccess) -> &'static str {
    match access {
        CapabilityAccess::Read => "read",
        CapabilityAccess::Write => "write",
        CapabilityAccess::Invoke => "invoke",
    }
}

fn default_local_policy_format_version() -> LocalPolicyFormatVersion {
    LocalPolicyFormatVersion::GuildLocalPolicyV2
}

fn default_local_policy_action() -> LocalPolicyDefaultAction {
    LocalPolicyDefaultAction::AllowRequestedDeclared
}

fn default_policy_rule_target() -> PolicyRuleTarget {
    PolicyRuleTarget::Any
}

fn default_policy_profile_name() -> String {
    "default".into()
}

fn default_policy_profile() -> PolicyProfile {
    PolicyProfile {
        name: default_policy_profile_name(),
        default_action: default_local_policy_action(),
        rules: Vec::new(),
    }
}

fn string_schema(format: Option<&str>, description: Option<&str>) -> Schema {
    let mut schema = serde_json::Map::from_iter([("type".into(), Value::String("string".into()))]);

    if let Some(format) = format {
        schema.insert("format".into(), Value::String(format.to_owned()));
    }

    if let Some(description) = description {
        schema.insert("description".into(), Value::String(description.to_owned()));
    }

    Schema::try_from(Value::Object(schema))
        .expect("string schema helper produces a valid JSON Schema")
}

fn execution_status_label(status: &ExecutionStatus) -> &'static str {
    match status {
        ExecutionStatus::Succeeded => "succeeded",
        ExecutionStatus::Failed => "failed",
        ExecutionStatus::Partial => "partial",
        ExecutionStatus::Rejected => "rejected",
    }
}

fn parse_execution_query_status(segment: &str) -> Result<ExecutionStatus, GuildResourceParseError> {
    match segment {
        "succeeded" => Ok(ExecutionStatus::Succeeded),
        "failed" => Ok(ExecutionStatus::Failed),
        "partial" => Ok(ExecutionStatus::Partial),
        "rejected" => Ok(ExecutionStatus::Rejected),
        _ => Err(GuildResourceParseError::new(format!(
            "unsupported execution query status `{segment}`"
        ))),
    }
}

fn parse_execution_query_limit(segment: &str) -> Result<usize, GuildResourceParseError> {
    let limit = segment.parse::<usize>().map_err(|_| {
        GuildResourceParseError::new("execution query limit must be a positive base-10 integer")
    })?;
    if (1..=MAX_EXECUTION_QUERY_LIMIT).contains(&limit) {
        Ok(limit)
    } else {
        Err(GuildResourceParseError::new(format!(
            "execution query limit must be between 1 and {MAX_EXECUTION_QUERY_LIMIT}",
        )))
    }
}

fn percent_encode_component(input: &str) -> String {
    let mut encoded = String::with_capacity(input.len());

    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => {
                encoded.push('%');
                let _ = write!(encoded, "{byte:02X}");
            }
        }
    }

    encoded
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
