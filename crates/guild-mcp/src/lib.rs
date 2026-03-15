//! MCP-facing names and façade concepts for Guild.
//!
//! The current working baseline is an inspect-only Rust façade that normalizes
//! on host-owned `ExecutionRecord` values.

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

pub const SERVER_NAME: &str = "guild-mcp";
pub const SEARCH_TOOL: &str = "guild.search";
pub const DESCRIBE_TOOL: &str = "guild.describe";
pub const INSPECT_TOOL: &str = "guild.inspect";
pub const PLAN_TOOL: &str = "guild.plan";
pub const APPLY_TOOL: &str = "guild.apply";

static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

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
        let id = NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed);

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

    pub fn read_resource(&self, uri: impl AsRef<str>) -> Result<ResourceReadResult, McpError> {
        self.registry
            .read_resource(uri.as_ref())
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
