use serde_json::{Value, json};
use wit_bindgen::generate;

#[path = "../../../authority-debug-support.rs"]
mod authority_debug_support;

const _: &str = include_str!("../../../../../wit/guild-skill-v1.wit");

generate!({
    path: "../../../../wit",
    world: "guild-skill-inspect-v1",
});

use crate::exports::guild::skill::inspect_skill::{
    ExecutionContext, Guest, Json, SkillError, SkillOutput,
};
use crate::guild::skill::inspect_host as host;
use crate::guild::skill::inspect_types::ResourceReadResult;

struct ExplainCapabilityDenial;

impl Guest for ExplainCapabilityDenial {
    fn run(_ctx: ExecutionContext, input: Json) -> Result<SkillOutput, SkillError> {
        let parsed_input: Value = serde_json::from_str(&input).map_err(|error| SkillError {
            code: "invalid-input".into(),
            message: "input JSON could not be parsed".into(),
            retryable: false,
            detail: Some(json!({ "error": error.to_string() }).to_string()),
        })?;

        let execution_uri = parsed_input
            .get("execution_uri")
            .and_then(Value::as_str)
            .filter(|uri| !uri.is_empty())
            .ok_or_else(|| SkillError {
                code: "missing-execution-uri".into(),
                message: "execution_uri must be a non-empty string".into(),
                retryable: false,
                detail: None,
            })?;

        let execution_resource = read_resource(execution_uri)?;
        let execution_record = parse_json_bytes(&execution_resource, "execution resource")?;

        let structured = json!({
            "execution_uri": execution_uri,
            "execution_id": authority_debug_support::execution_id(&execution_record),
            "status": authority_debug_support::status(&execution_record),
            "termination": authority_debug_support::termination(&execution_record),
            "skill": authority_debug_support::resolved_skill(&execution_record),
            "policy_outcome": execution_record.pointer("/policy_decision/outcome").cloned().unwrap_or(Value::Null),
            "policy_summary": execution_record.pointer("/policy_decision/summary").cloned().unwrap_or(Value::Null),
            "policy_profile": authority_debug_support::policy_profile(&execution_record),
            "trust_tier": authority_debug_support::trust_tier(&execution_record),
            "verification_state": authority_debug_support::verification_state(&execution_record),
            "requested_capabilities": {
                "grants": authority_debug_support::requested_capabilities(&execution_record),
            },
            "granted_capabilities": {
                "grants": authority_debug_support::granted_capabilities(&execution_record),
            },
            "denied_or_reduced_capabilities": authority_debug_support::reduced_or_denied_capability_deltas(&execution_record),
            "required_capability_gaps": authority_debug_support::required_capability_gaps(&execution_record),
            "primary_reason": authority_debug_support::primary_reason(&execution_record),
            "reason_chain": authority_debug_support::reason_chain(&execution_record),
            "policy_reason_codes": authority_debug_support::policy_reason_codes(&execution_record),
            "child_execution_uris": authority_debug_support::child_execution_uris(&execution_record),
        });

        Ok(SkillOutput {
            summary: format!("Explained capability denial state for stored execution {execution_uri}."),
            structured: structured.to_string(),
            diagnostics: Vec::new(),
            effects: Vec::new(),
            evidence: Vec::new(),
        })
    }
}

fn read_resource(uri: &str) -> Result<ResourceReadResult, SkillError> {
    host::read_resource(uri).map_err(|message| SkillError {
        code: "read-resource-failed".into(),
        message: "host failed to read the requested Guild resource".into(),
        retryable: false,
        detail: Some(json!({ "uri": uri, "error": message }).to_string()),
    })
}

fn parse_json_bytes(resource: &ResourceReadResult, label: &str) -> Result<Value, SkillError> {
    serde_json::from_slice(&resource.bytes).map_err(|error| SkillError {
        code: "invalid-resource-json".into(),
        message: format!("{label} did not contain valid JSON"),
        retryable: false,
        detail: Some(
            json!({
                "uri": resource.uri,
                "mime_type": resource.mime_type,
                "error": error.to_string(),
            })
            .to_string(),
        ),
    })
}

export!(ExplainCapabilityDenial with_types_in self);
