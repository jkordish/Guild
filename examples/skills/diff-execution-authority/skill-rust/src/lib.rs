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

struct DiffExecutionAuthority;

impl Guest for DiffExecutionAuthority {
    fn run(_ctx: ExecutionContext, input: Json) -> Result<SkillOutput, SkillError> {
        let parsed_input: Value = serde_json::from_str(&input).map_err(|error| SkillError {
            code: "invalid-input".into(),
            message: "input JSON could not be parsed".into(),
            retryable: false,
            detail: Some(json!({ "error": error.to_string() }).to_string()),
        })?;

        let left_execution_uri = required_uri(&parsed_input, "left_execution_uri")?;
        let right_execution_uri = required_uri(&parsed_input, "right_execution_uri")?;

        let left_resource = read_resource(left_execution_uri)?;
        let right_resource = read_resource(right_execution_uri)?;
        let left_record = parse_json_bytes(&left_resource, "left execution resource")?;
        let right_record = parse_json_bytes(&right_resource, "right execution resource")?;

        let left_skill = authority_debug_support::resolved_skill(&left_record);
        let right_skill = authority_debug_support::resolved_skill(&right_record);
        let same_skill = left_skill.pointer("/key/namespace") == right_skill.pointer("/key/namespace")
            && left_skill.pointer("/key/name") == right_skill.pointer("/key/name")
            && left_skill.get("version") == right_skill.get("version");
        let same_digest = left_skill.get("digest") == right_skill.get("digest");

        let structured = json!({
            "left_execution_uri": left_execution_uri,
            "right_execution_uri": right_execution_uri,
            "same_skill": same_skill,
            "same_digest": same_digest,
            "left_skill": left_skill,
            "right_skill": right_skill,
            "left_status": authority_debug_support::status(&left_record),
            "right_status": authority_debug_support::status(&right_record),
            "trust_tier_diff": authority_debug_support::diff_summary_object(
                serde_json::to_value(authority_debug_support::trust_tier(&left_record)).unwrap_or(Value::Null),
                serde_json::to_value(authority_debug_support::trust_tier(&right_record)).unwrap_or(Value::Null)
            ),
            "verification_state_diff": authority_debug_support::diff_summary_object(
                serde_json::to_value(authority_debug_support::verification_state(&left_record)).unwrap_or(Value::Null),
                serde_json::to_value(authority_debug_support::verification_state(&right_record)).unwrap_or(Value::Null)
            ),
            "policy_profile_diff": authority_debug_support::diff_summary_object(
                serde_json::to_value(authority_debug_support::policy_profile(&left_record)).unwrap_or(Value::Null),
                serde_json::to_value(authority_debug_support::policy_profile(&right_record)).unwrap_or(Value::Null)
            ),
            "policy_outcome_diff": authority_debug_support::diff_summary_object(
                left_record.pointer("/policy_decision/outcome").cloned().unwrap_or(Value::Null),
                right_record.pointer("/policy_decision/outcome").cloned().unwrap_or(Value::Null)
            ),
            "requested_capability_diff": authority_debug_support::compare_execution_requested_capabilities(&left_record, &right_record),
            "granted_capability_diff": authority_debug_support::compare_execution_granted_capabilities(&left_record, &right_record),
            "termination_diff": authority_debug_support::diff_summary_object(
                authority_debug_support::termination(&left_record),
                authority_debug_support::termination(&right_record)
            ),
            "child_execution_count_diff": authority_debug_support::diff_summary_object(
                Value::Number((authority_debug_support::child_execution_count(&left_record) as u64).into()),
                Value::Number((authority_debug_support::child_execution_count(&right_record) as u64).into())
            ),
            "left_primary_reason": authority_debug_support::primary_reason(&left_record),
            "right_primary_reason": authority_debug_support::primary_reason(&right_record),
            "likely_authority_drivers": authority_debug_support::likely_authority_drivers(&left_record, &right_record),
        });

        Ok(SkillOutput {
            summary: format!(
                "Compared stored authority for executions {left_execution_uri} and {right_execution_uri}."
            ),
            structured: structured.to_string(),
            diagnostics: Vec::new(),
            effects: Vec::new(),
            evidence: Vec::new(),
        })
    }
}

fn required_uri<'a>(input: &'a Value, key: &str) -> Result<&'a str, SkillError> {
    input
        .get(key)
        .and_then(Value::as_str)
        .filter(|uri| !uri.is_empty())
        .ok_or_else(|| SkillError {
            code: "missing-execution-uri".into(),
            message: format!("{key} must be a non-empty string"),
            retryable: false,
            detail: None,
        })
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

export!(DiffExecutionAuthority with_types_in self);
