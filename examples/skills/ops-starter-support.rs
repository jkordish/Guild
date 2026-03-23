#![allow(dead_code)]

use std::collections::BTreeSet;
use std::fmt::Write as _;

use serde_json::{Value, json};

use crate::exports::guild::skill::inspect_skill::SkillError;
use crate::guild::skill::inspect_host as host;
use crate::guild::skill::inspect_types::ResourceReadResult;

pub const EXECUTION_URI_PREFIX: &str = "guild://executions/";
pub const OBJECT_RECORD_URI_PREFIX: &str = "guild://objects/records/";
pub const QUERY_URI_PREFIX: &str = "guild://queries/executions/";

pub fn parse_input(input: &str) -> Result<Value, SkillError> {
    serde_json::from_str(input).map_err(|error| SkillError {
        code: "invalid-input".into(),
        message: "input JSON could not be parsed".into(),
        retryable: false,
        detail: Some(json!({ "error": error.to_string() }).to_string()),
    })
}

pub fn required_string<'a>(input: &'a Value, key: &str) -> Result<&'a str, SkillError> {
    input
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| SkillError {
            code: "missing-required-field".into(),
            message: format!("{key} must be a non-empty string"),
            retryable: false,
            detail: None,
        })
}

pub fn read_resource(uri: &str) -> Result<ResourceReadResult, SkillError> {
    host::read_resource(uri).map_err(|message| SkillError {
        code: "read-resource-failed".into(),
        message: "host failed to read the requested Guild resource".into(),
        retryable: false,
        detail: Some(json!({ "uri": uri, "error": message }).to_string()),
    })
}

pub fn parse_json_bytes(resource: &ResourceReadResult, label: &str) -> Result<Value, SkillError> {
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

pub fn read_json_resource(
    uri: &str,
    label: &str,
) -> Result<(ResourceReadResult, Value), SkillError> {
    let resource = read_resource(uri)?;
    let json = parse_json_bytes(&resource, label)?;
    Ok((resource, json))
}

pub fn evidence_metadata_uri(evidence_uri: &str) -> String {
    format!("{}/metadata", evidence_uri.trim_end_matches('/'))
}

pub fn resource_id_from_uri(uri: &str) -> &str {
    uri.rsplit('/').next().unwrap_or(uri)
}

pub fn short_prefixed_id(prefix: &str, value: &str) -> String {
    let short = value.chars().take(12).collect::<String>();
    format!("{prefix}:{short}")
}

pub fn short_execution_ref_from_uri(uri: &str) -> String {
    short_prefixed_id("exec", resource_id_from_uri(uri))
}

pub fn short_evidence_ref_from_uri(uri: &str) -> String {
    short_prefixed_id("evidence", resource_id_from_uri(uri))
}

pub fn short_object_ref_from_uri(uri: &str) -> String {
    short_prefixed_id("obj", resource_id_from_uri(uri))
}

pub fn resolved_skill_label(record: &Value) -> String {
    let namespace = record
        .pointer("/resolved_skill/key/namespace")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let name = record
        .pointer("/resolved_skill/key/name")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let version = record
        .pointer("/resolved_skill/version")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    format!("{namespace}/{name}@{version}")
}

pub fn status_label(record: &Value) -> String {
    record
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_owned()
}

pub fn primary_reason(record: &Value) -> Value {
    if let Some(termination) = record.get("termination").and_then(Value::as_object) {
        return json!({
            "source": "termination",
            "phase": termination.get("phase").cloned().unwrap_or(Value::Null),
            "code": termination.get("code").cloned().unwrap_or(Value::Null),
            "message": termination.get("message").cloned().unwrap_or(Value::Null),
            "retryable": termination.get("retryable").cloned().unwrap_or(Value::Null),
        });
    }

    record
        .pointer("/policy_decision/reasons")
        .and_then(Value::as_array)
        .and_then(|reasons| reasons.first())
        .map(|reason| {
            json!({
                "source": "policy",
                "phase": Value::Null,
                "code": reason.get("code").cloned().unwrap_or(Value::Null),
                "message": reason.get("message").cloned().unwrap_or(Value::Null),
                "retryable": Value::Null,
            })
        })
        .unwrap_or(Value::Null)
}

pub fn primary_reason_code(record: &Value) -> String {
    let reason = primary_reason(record);
    match (
        reason.get("source").and_then(Value::as_str),
        reason.get("phase").and_then(Value::as_str),
        reason.get("code").and_then(Value::as_str),
    ) {
        (Some("termination"), Some(phase), Some(code)) => format!("{phase}:{code}"),
        (_, _, Some(code)) => code.to_owned(),
        _ => "none".into(),
    }
}

pub fn primary_reason_message(record: &Value) -> String {
    primary_reason(record)
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("no termination or policy reason recorded")
        .to_owned()
}

pub fn policy_reason_codes(record: &Value) -> Vec<String> {
    let mut codes = BTreeSet::new();
    if let Some(reasons) = record.pointer("/policy_decision/reasons").and_then(Value::as_array) {
        for reason in reasons {
            if let Some(code) = reason.get("code").and_then(Value::as_str) {
                codes.insert(code.to_owned());
            }
        }
    }
    codes.into_iter().collect()
}

pub fn evidence_uris(record: &Value) -> Vec<String> {
    record
        .get("emitted_evidence")
        .and_then(Value::as_array)
        .map(|items| {
            items.iter()
                .filter_map(|item| item.get("uri").and_then(Value::as_str))
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

pub fn child_execution_uris(record: &Value) -> Vec<String> {
    record
        .get("child_executions")
        .and_then(Value::as_array)
        .map(|items| {
            items.iter()
                .filter_map(|item| item.get("uri").and_then(Value::as_str))
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

pub fn support_buckets(record: &Value) -> Vec<(String, Vec<String>)> {
    let mut proof_backed = Vec::new();
    let mut bounded = Vec::new();
    let mut not_proven = Vec::new();
    let mut refused = Vec::new();

    for capability in exercised_or_granted_capability_ids(record) {
        match capability.as_str() {
            "log-write" => proof_backed.push(capability),
            "http-request" | "read-resource" | "invoke-skill" => bounded.push(capability),
            "emit-evidence" => not_proven.push(capability),
            _ => refused.push(capability),
        }
    }

    let mut buckets = Vec::new();
    if !proof_backed.is_empty() {
        buckets.push(("proof-backed".into(), proof_backed));
    }
    if !bounded.is_empty() {
        buckets.push(("bounded".into(), bounded));
    }
    if !not_proven.is_empty() {
        buckets.push(("not_proven".into(), not_proven));
    }
    if !refused.is_empty() {
        buckets.push(("refused".into(), refused));
    }
    buckets
}

pub fn rendered_support(record: &Value) -> String {
    let buckets = support_buckets(record);
    if buckets.is_empty() {
        return "none".into();
    }

    buckets
        .iter()
        .map(|(status, capabilities)| format!("{status}({})", capabilities.join(",")))
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn overall_support(record: &Value) -> &'static str {
    let buckets = support_buckets(record);
    if buckets.iter().any(|(status, _)| status == "refused") {
        "refused"
    } else if buckets.iter().any(|(status, _)| status == "not_proven") {
        "not_proven"
    } else if buckets.iter().any(|(status, _)| status == "bounded") {
        "bounded"
    } else {
        "proof-backed"
    }
}

pub fn execution_posture(record: &Value) -> &'static str {
    if record.get("status").and_then(Value::as_str) == Some("rejected") {
        "refused"
    } else if overall_support(record) == "proof-backed" {
        "proof-backed"
    } else {
        "upper-bound"
    }
}

pub fn json_string(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(ToOwned::to_owned)
}

pub fn summarize_json_payload(
    payload: &Value,
    limit: usize,
) -> Vec<(String, String)> {
    let mut lines = Vec::new();

    if let Some(kind) = payload.get("kind").and_then(Value::as_str) {
        lines.push(("Kind".into(), kind.to_owned()));
    }
    if let Some(mode) = payload.get("mode").and_then(Value::as_str) {
        lines.push(("Mode".into(), mode.to_owned()));
    }
    if let Some(message) = payload.get("message").and_then(Value::as_str) {
        lines.push(("Message".into(), message.to_owned()));
    }
    if let Some(skill) = payload.get("skill") {
        let namespace = skill
            .pointer("/key/namespace")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let name = skill
            .pointer("/key/name")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let version = skill.get("version").and_then(Value::as_str).unwrap_or("unknown");
        lines.push(("Skill".into(), format!("{namespace}/{name}@{version}")));
    }
    if let Some(obj) = payload.as_object() {
        let mut keys = obj.keys().cloned().collect::<Vec<_>>();
        keys.sort();
        lines.push(("Top-level keys".into(), keys.join(", ")));
    }

    lines.truncate(limit);
    lines
}

pub fn render_markdown_report(spec: &Value) -> Result<String, SkillError> {
    let title = required_string(spec, "title")?;
    let summary_line = required_string(spec, "summary_line")?;
    let mut output = String::new();

    let _ = writeln!(output, "# {title}");
    let _ = writeln!(output);
    let _ = writeln!(output, "{summary_line}");

    if let Some(facts) = spec.get("facts").and_then(Value::as_array) {
        let facts = facts
            .iter()
            .filter_map(|fact| {
                let label = fact.get("label").and_then(Value::as_str)?;
                let value = fact.get("value").and_then(Value::as_str)?;
                Some((label, value))
            })
            .collect::<Vec<_>>();
        if !facts.is_empty() {
            let _ = writeln!(output);
            for (label, value) in facts {
                let _ = writeln!(output, "- {label}: {value}");
            }
        }
    }

    if let Some(sections) = spec.get("sections").and_then(Value::as_array) {
        for section in sections {
            let title = section.get("title").and_then(Value::as_str).ok_or_else(|| SkillError {
                code: "invalid-section".into(),
                message: "report sections must include a non-empty title".into(),
                retryable: false,
                detail: Some(section.to_string()),
            })?;
            let lines = section.get("lines").and_then(Value::as_array).ok_or_else(|| SkillError {
                code: "invalid-section".into(),
                message: "report sections must include a string lines array".into(),
                retryable: false,
                detail: Some(section.to_string()),
            })?;
            let lines = lines
                .iter()
                .filter_map(Value::as_str)
                .filter(|line| !line.is_empty())
                .collect::<Vec<_>>();
            if lines.is_empty() {
                continue;
            }
            let _ = writeln!(output);
            let _ = writeln!(output, "## {title}");
            for line in lines {
                let _ = writeln!(output, "{line}");
            }
        }
    }

    Ok(output.trim_end().to_owned())
}

fn exercised_or_granted_capability_ids(record: &Value) -> Vec<String> {
    let mut exercised = BTreeSet::new();
    if let Some(observations) = record.get("authority_observations").and_then(Value::as_array) {
        for observation in observations {
            if observation.get("status").and_then(Value::as_str) != Some("exercised") {
                continue;
            }
            if let Some(family) = observation.get("family").and_then(Value::as_str) {
                exercised.insert(family.to_owned());
            }
        }
    }
    if !exercised.is_empty() {
        return exercised.into_iter().collect();
    }

    let mut granted = BTreeSet::new();
    if let Some(items) = record
        .pointer("/granted_capabilities/grants")
        .and_then(Value::as_array)
    {
        for item in items {
            if let Some(id) = item.get("id").and_then(Value::as_str) {
                granted.insert(id.to_owned());
            }
        }
    }
    granted.into_iter().collect()
}
