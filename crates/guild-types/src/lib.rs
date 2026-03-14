//! Core shared data structures for Guild contracts.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Hash)]
pub struct SkillKey {
    pub namespace: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Hash)]
pub struct SkillRef {
    pub key: SkillKey,
    pub version: String,
    pub digest: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct CapabilityRequirement {
    pub id: CapabilityId,
    pub access: CapabilityAccess,
    #[serde(default)]
    pub constraints: Value,
    pub required: bool,
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
    pub mode: ExecutionMode,
    pub input_sha256: String,
    pub now_utc: Option<String>,
    pub budget: Budget,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ExecutionRequest {
    pub execution_id: String,
    pub skill: SkillRef,
    pub tenant_id: String,
    pub actor_id: String,
    pub mode: ExecutionMode,
    pub input: Value,
    pub budgets: Budget,
    pub idempotency_key: Option<String>,
    pub parent_execution_id: Option<String>,
    pub trace_id: String,
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionStatus {
    Succeeded,
    Failed,
    Partial,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ExecutionMetrics {
    pub duration_ms: u64,
    pub network_requests: u32,
    pub child_executions: u16,
    pub cache_hits: u32,
    pub cache_misses: u32,
}

impl Default for ExecutionMetrics {
    fn default() -> Self {
        Self {
            duration_ms: 0,
            network_requests: 0,
            child_executions: 0,
            cache_hits: 0,
            cache_misses: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct Provenance {
    pub skill: SkillRef,
    pub abi: AbiVersion,
    pub resolved_digest: String,
    pub dependency_digests: Vec<String>,
    pub started_at_utc: Option<String>,
    pub finished_at_utc: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ExecutionResult {
    pub status: ExecutionStatus,
    pub summary: String,
    pub structured: Value,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
    #[serde(default)]
    pub effects: Vec<Effect>,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub metrics: ExecutionMetrics,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct SkillError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    pub detail: Option<Value>,
}
