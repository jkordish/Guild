//! Execution boundary and runtime abstraction for Guild.

use guild_manifest::SkillManifest;
use guild_types::{
    ExecutionRequest, ExecutionResult, ExecutionStatus, RuntimeKind, SkillError,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub trait RuntimeAdapter {
    fn kind(&self) -> RuntimeKind;

    fn execute(
        &self,
        manifest: &SkillManifest,
        request: &ExecutionRequest,
    ) -> Result<ExecutionResult, SkillError>;
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ExecutionReceipt {
    pub execution_id: String,
    pub trace_id: String,
    pub status: ExecutionStatus,
}

#[derive(Debug, Default)]
pub struct Runner;

impl Runner {
    pub fn new() -> Self {
        Self
    }
}
