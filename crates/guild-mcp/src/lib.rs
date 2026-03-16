//! MCP-facing names and façade concepts for Guild.
//!
//! The current working baseline is an inspect-only Rust façade that normalizes
//! on host-owned `ExecutionRecord` values and powers the stdio MCP server.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use guild_registry::{RegistryError, SkillRegistry};
use guild_runner::{ExecutionError, Runner, RuntimeAdapter};
use guild_types::{
    Budget, CallerRequest, CapabilityGrantSet, ExecutionMode, ExecutionReceipt, ExecutionRecord,
    PolicyDecision, PolicyDecisionOutcome, RequestedSkillRef, ResolvedExecutionEnvelope,
    ResourceReadResult,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub mod protocol;
pub mod server;

pub const SERVER_NAME: &str = "guild-mcp";
pub const SERVER_BINARY_NAME: &str = "guild-mcp-server";
pub const INSPECT_TOOL: &str = "guild.inspect";

static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

fn next_request_sequence() -> u64 {
    NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed)
}

fn minted_request_id(sequence: u64) -> String {
    format!("inspect-{sequence}")
}

fn minted_trace_id(sequence: u64) -> String {
    format!("trace-{sequence}")
}

fn default_tenant_id() -> String {
    "local".into()
}

fn default_actor_id() -> String {
    "mcp-client".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct InspectRequest {
    pub skill: RequestedSkillRef,
    pub input: Value,
    pub tenant_id: String,
    pub actor_id: String,
    #[serde(default = "Budget::default")]
    pub budget: Budget,
    #[serde(default)]
    pub grants: CapabilityGrantSet,
    pub request_id: String,
    pub trace_id: String,
}

impl InspectRequest {
    pub fn new(
        skill: RequestedSkillRef,
        input: Value,
        tenant_id: impl Into<String>,
        actor_id: impl Into<String>,
        grants: CapabilityGrantSet,
    ) -> Self {
        let id = next_request_sequence();

        Self {
            skill,
            input,
            tenant_id: tenant_id.into(),
            actor_id: actor_id.into(),
            budget: Budget::default(),
            grants,
            request_id: format!("inspect-{id}"),
            trace_id: format!("trace-{id}"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct InspectToolRequest {
    pub skill: RequestedSkillRef,
    pub input: Value,
    #[serde(default)]
    pub tenant_id: Option<String>,
    #[serde(default)]
    pub actor_id: Option<String>,
    #[serde(default)]
    pub budget: Option<Budget>,
    #[serde(default)]
    pub grants: CapabilityGrantSet,
    #[serde(default)]
    pub request_id: Option<String>,
    #[serde(default)]
    pub trace_id: Option<String>,
}

impl InspectToolRequest {
    pub fn into_inspect_request(self) -> InspectRequest {
        let sequence = next_request_sequence();
        InspectRequest {
            skill: self.skill,
            input: self.input,
            tenant_id: self.tenant_id.unwrap_or_else(default_tenant_id),
            actor_id: self.actor_id.unwrap_or_else(default_actor_id),
            budget: self.budget.unwrap_or_default(),
            grants: self.grants,
            request_id: self
                .request_id
                .unwrap_or_else(|| minted_request_id(sequence)),
            trace_id: self.trace_id.unwrap_or_else(|| minted_trace_id(sequence)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct InspectResponse {
    pub summary: String,
    pub structured_content: ExecutionRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct McpError {
    pub code: String,
    pub message: String,
    pub detail: Option<Box<Value>>,
    pub receipt: Option<Box<ExecutionReceipt>>,
}

impl McpError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            detail: None,
            receipt: None,
        }
    }
}

impl fmt::Display for McpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for McpError {}

impl From<RegistryError> for McpError {
    fn from(value: RegistryError) -> Self {
        Self {
            code: value.code,
            message: value.message,
            detail: value.detail.map(Box::new),
            receipt: None,
        }
    }
}

impl From<ExecutionError> for McpError {
    fn from(value: ExecutionError) -> Self {
        Self {
            code: value.code,
            message: value.message,
            detail: value.detail,
            receipt: value.receipt,
        }
    }
}

pub struct GuildMcpFacade<R, A> {
    registry: R,
    runner: Runner<A>,
}

impl<R, A> GuildMcpFacade<R, A>
where
    R: SkillRegistry + Clone + Send + Sync + 'static,
    A: RuntimeAdapter + Clone + 'static,
{
    pub fn new(registry: R, runtime: A) -> Self {
        Self {
            registry,
            runner: Runner::new(runtime),
        }
    }

    pub fn inspect(&self, request: InspectRequest) -> Result<InspectResponse, McpError> {
        let installed = self.registry.resolve(&request.skill)?;
        let execution_request = ResolvedExecutionEnvelope {
            request: CallerRequest {
                request_id: request.request_id,
                skill: request.skill,
                tenant_id: request.tenant_id,
                actor_id: request.actor_id,
                mode: ExecutionMode::Inspect,
                input: request.input,
                budget: request.budget,
                requested_capabilities: request.grants.clone(),
                idempotency_key: None,
                trace_id: request.trace_id,
            },
            resolved_skill: installed.resolved_ref.clone(),
            granted_capabilities: request.grants,
            policy_decision: PolicyDecision {
                outcome: PolicyDecisionOutcome::Allowed,
                summary: "inspect request allowed".into(),
                detail: None,
            },
            parent_execution_id: None,
        };

        let record = self
            .runner
            .execute(&self.registry, &installed, execution_request)?;
        Ok(InspectResponse {
            summary: record
                .output
                .as_ref()
                .expect("successful execution records include skill output")
                .summary
                .clone(),
            structured_content: record,
        })
    }

    pub fn inspect_tool(&self, request: InspectToolRequest) -> Result<InspectResponse, McpError> {
        self.inspect(request.into_inspect_request())
    }

    pub fn read_resource(&self, uri: impl AsRef<str>) -> Result<ResourceReadResult, McpError> {
        self.registry
            .read_resource(uri.as_ref())
            .map_err(McpError::from)
    }

    pub fn load_execution_record(
        &self,
        execution_id: impl AsRef<str>,
    ) -> Result<ExecutionRecord, McpError> {
        self.registry
            .load_execution_record(execution_id.as_ref())
            .map_err(McpError::from)
    }

    pub fn plan(&self) -> Result<(), McpError> {
        Err(McpError::new(
            "plan-not-implemented",
            "guild.plan is not implemented in the inspect-only milestone",
        ))
    }

    pub fn apply(&self) -> Result<(), McpError> {
        Err(McpError::new(
            "apply-disabled",
            "guild.apply remains globally gated in the current milestone",
        ))
    }
}
