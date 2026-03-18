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

struct ExplainHttpAuthority;

impl Guest for ExplainHttpAuthority {
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
        let candidate_request = parsed_input
            .get("candidate_request")
            .and_then(Value::as_object)
            .ok_or_else(|| SkillError {
                code: "missing-candidate-request".into(),
                message: "candidate_request must be an object".into(),
                retryable: false,
                detail: None,
            })?;
        let candidate_url = candidate_request
            .get("url")
            .and_then(Value::as_str)
            .filter(|url| !url.is_empty())
            .ok_or_else(|| SkillError {
                code: "missing-candidate-url".into(),
                message: "candidate_request.url must be a non-empty string".into(),
                retryable: false,
                detail: None,
            })?;
        let candidate_method = candidate_request
            .get("method")
            .and_then(Value::as_str)
            .filter(|method| !method.is_empty())
            .ok_or_else(|| SkillError {
                code: "missing-candidate-method".into(),
                message: "candidate_request.method must be a non-empty string".into(),
                retryable: false,
                detail: None,
            })?;
        let candidate_timeout_ms = candidate_request
            .get("timeout_ms")
            .and_then(Value::as_u64);

        let execution_resource = read_resource(execution_uri)?;
        let execution_record = parse_json_bytes(&execution_resource, "execution resource")?;
        let structured = authority_debug_support::http_authority_report(
            &execution_record,
            candidate_url,
            candidate_method,
            candidate_timeout_ms,
        )
        .map_err(|error| SkillError {
            code: error.code,
            message: error.message,
            retryable: false,
            detail: error.detail.map(|detail| detail.to_string()),
        })?;

        let summary = match structured
            .get("evaluation_status")
            .and_then(Value::as_str)
            .unwrap_or("indeterminate")
        {
            "allowed" => format!(
                "Dry-ran stored HTTP authority for {execution_uri}; the candidate request is allowed."
            ),
            "denied" => format!(
                "Dry-ran stored HTTP authority for {execution_uri}; the candidate request is denied."
            ),
            _ => format!(
                "Dry-ran stored HTTP authority for {execution_uri}; the candidate request needs host-side resolution to finish evaluation."
            ),
        };

        Ok(SkillOutput {
            summary,
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

export!(ExplainHttpAuthority with_types_in self);
