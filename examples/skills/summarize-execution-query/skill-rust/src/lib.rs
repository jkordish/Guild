use std::collections::BTreeMap;

use serde_json::{json, Value};
use wit_bindgen::generate;

const _: &str = include_str!("../../../../../wit/guild-skill-v1.wit");

generate!({
    path: "../../../../wit",
    world: "guild-skill",
});

use crate::exports::guild::skill::skill::{ExecutionContext, Guest, Json, SkillError, SkillOutput};
use crate::guild::skill::host;
use crate::guild::skill::types::{
    CapabilityAccess, CapabilityConstraints, CapabilityId, EvidenceAudience, GrantedCapability,
    HttpMethod, HttpScheme, RedactionClass, ResourceKind, ResourceReadResult, Severity,
};

const TOP_SKILL_LIMIT: usize = 5;
const TOP_TERMINATION_LIMIT: usize = 5;
const TOP_POLICY_REASON_LIMIT: usize = 5;
const NOTABLE_EXECUTION_LIMIT: usize = 5;
const SAMPLE_EVIDENCE_URI_LIMIT: usize = 5;

struct SummarizeExecutionQuery;

impl Guest for SummarizeExecutionQuery {
    fn run(ctx: ExecutionContext, input: Json) -> Result<SkillOutput, SkillError> {
        let parsed_input: Value = serde_json::from_str(&input).map_err(|error| SkillError {
            code: "invalid-input".into(),
            message: "input JSON could not be parsed".into(),
            retryable: false,
            detail: Some(json!({ "error": error.to_string() }).to_string()),
        })?;

        let query_uri = parsed_input
            .get("query_uri")
            .and_then(Value::as_str)
            .filter(|uri| !uri.is_empty())
            .ok_or_else(|| SkillError {
                code: "missing-query-uri".into(),
                message: "query_uri must be a non-empty string".into(),
                retryable: false,
                detail: None,
            })?;

        let query_resource = read_resource(query_uri)?;
        let query_result = parse_json_bytes(&query_resource, "execution query resource")?;
        let results = query_result
            .get("results")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid_query_resource_error(query_uri, "results must be an array"))?;

        let mut status_counts = BTreeMap::<String, usize>::new();
        let mut skill_counts = BTreeMap::<(String, String), usize>::new();
        let mut termination_counts = BTreeMap::<(String, String), usize>::new();
        let mut policy_reason_counts = BTreeMap::<String, usize>::new();
        let mut notable_execution_uris = Vec::new();
        let mut sample_evidence_record_uris = Vec::new();
        let mut executions_with_evidence = 0usize;
        let mut total_evidence_records = 0usize;

        for result in results {
            if let Some(status) = result.get("status").and_then(Value::as_str) {
                *status_counts.entry(status.to_owned()).or_default() += 1;
            }

            if let Some(uri) = result.pointer("/receipt/uri").and_then(Value::as_str) {
                if notable_execution_uris.len() < NOTABLE_EXECUTION_LIMIT {
                    notable_execution_uris.push(uri.to_owned());
                }
            }

            if let (Some(namespace), Some(name)) = (
                result.pointer("/resolved_skill/key/namespace").and_then(Value::as_str),
                result.pointer("/resolved_skill/key/name").and_then(Value::as_str),
            ) {
                *skill_counts
                    .entry((namespace.to_owned(), name.to_owned()))
                    .or_default() += 1;
            }

            if let Some(termination) = result.get("termination").and_then(Value::as_object) {
                if let (Some(phase), Some(code)) = (
                    termination.get("phase").and_then(Value::as_str),
                    termination.get("code").and_then(Value::as_str),
                ) {
                    *termination_counts
                        .entry((phase.to_owned(), code.to_owned()))
                        .or_default() += 1;
                }
            }

            if let Some(reasons) = result.pointer("/policy_decision/reasons").and_then(Value::as_array) {
                for reason in reasons {
                    if let Some(code) = reason.get("code").and_then(Value::as_str) {
                        *policy_reason_counts.entry(code.to_owned()).or_default() += 1;
                    }
                }
            }

            let evidence_count = result
                .get("evidence_count")
                .and_then(Value::as_u64)
                .and_then(|count| usize::try_from(count).ok())
                .unwrap_or(0);
            total_evidence_records += evidence_count;
            if evidence_count > 0 {
                executions_with_evidence += 1;
            }

            if let Some(sample_uris) = result
                .get("sample_evidence_record_uris")
                .and_then(Value::as_array)
            {
                for uri in sample_uris.iter().filter_map(Value::as_str) {
                    if sample_evidence_record_uris.len() >= SAMPLE_EVIDENCE_URI_LIMIT {
                        break;
                    }
                    if !sample_evidence_record_uris.iter().any(|seen| seen == uri) {
                        sample_evidence_record_uris.push(uri.to_owned());
                    }
                }
            }
        }

        let total_matches = query_result
            .get("total_matches")
            .and_then(Value::as_u64)
            .and_then(|count| usize::try_from(count).ok())
            .ok_or_else(|| invalid_query_resource_error(query_uri, "total_matches must be an integer"))?;
        let returned_matches = query_result
            .get("returned_matches")
            .and_then(Value::as_u64)
            .and_then(|count| usize::try_from(count).ok())
            .ok_or_else(|| {
                invalid_query_resource_error(query_uri, "returned_matches must be an integer")
            })?;
        let truncated = query_result
            .get("truncated")
            .and_then(Value::as_bool)
            .ok_or_else(|| invalid_query_resource_error(query_uri, "truncated must be a boolean"))?;

        Ok(SkillOutput {
            summary: format!("Summarized {returned_matches} execution matches from {query_uri}."),
            structured: json!({
                "query_uri": query_uri,
                "total_matches": total_matches,
                "returned_matches": returned_matches,
                "truncated": truncated,
                "status_counts": ordered_status_counts(status_counts),
                "top_skills": ordered_skill_counts(skill_counts),
                "termination_counts": ordered_termination_counts(termination_counts),
                "policy_reason_counts": ordered_policy_reason_counts(policy_reason_counts),
                "evidence_summary": {
                    "executions_with_evidence": executions_with_evidence,
                    "total_evidence_records": total_evidence_records,
                    "sample_evidence_record_uris": sample_evidence_record_uris,
                },
                "notable_execution_uris": notable_execution_uris,
                "query_resource": {
                    "uri": query_resource.uri,
                    "mime_type": query_resource.mime_type,
                    "sha256": query_resource.sha256,
                },
                "granted_capabilities": granted_capabilities_payload(&ctx.granted_capabilities),
            })
            .to_string(),
            diagnostics: Vec::new(),
            effects: Vec::new(),
            evidence: Vec::new(),
        })
    }
}

fn invalid_query_resource_error(query_uri: &str, message: &str) -> SkillError {
    SkillError {
        code: "invalid-query-resource".into(),
        message: "execution query resource did not match the expected Guild query shape".into(),
        retryable: false,
        detail: Some(json!({ "query_uri": query_uri, "error": message }).to_string()),
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

fn ordered_status_counts(counts: BTreeMap<String, usize>) -> Vec<Value> {
    let mut ordered = counts.into_iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    ordered
        .into_iter()
        .map(|(status, count)| json!({ "status": status, "count": count }))
        .collect()
}

fn ordered_skill_counts(counts: BTreeMap<(String, String), usize>) -> Vec<Value> {
    let mut ordered = counts.into_iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        right
            .1
            .cmp(&left.1)
            .then_with(|| left.0 .0.cmp(&right.0 .0))
            .then_with(|| left.0 .1.cmp(&right.0 .1))
    });
    ordered
        .into_iter()
        .take(TOP_SKILL_LIMIT)
        .map(|((namespace, name), count)| {
            json!({
                "namespace": namespace,
                "name": name,
                "count": count,
            })
        })
        .collect()
}

fn ordered_termination_counts(counts: BTreeMap<(String, String), usize>) -> Vec<Value> {
    let mut ordered = counts.into_iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        right
            .1
            .cmp(&left.1)
            .then_with(|| left.0 .0.cmp(&right.0 .0))
            .then_with(|| left.0 .1.cmp(&right.0 .1))
    });
    ordered
        .into_iter()
        .take(TOP_TERMINATION_LIMIT)
        .map(|((phase, code), count)| {
            json!({
                "phase": phase,
                "code": code,
                "count": count,
            })
        })
        .collect()
}

fn ordered_policy_reason_counts(counts: BTreeMap<String, usize>) -> Vec<Value> {
    let mut ordered = counts.into_iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    ordered
        .into_iter()
        .take(TOP_POLICY_REASON_LIMIT)
        .map(|(code, count)| json!({ "code": code, "count": count }))
        .collect()
}

fn granted_capabilities_payload(grants: &[GrantedCapability]) -> Value {
    json!({
        "grants": grants.iter().map(|grant| {
            json!({
                "id": capability_id_label(&grant.id),
                "access": capability_access_label(&grant.access),
                "constraints": capability_constraints_payload(&grant.constraints),
            })
        }).collect::<Vec<_>>()
    })
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

fn capability_constraints_payload(constraints: &CapabilityConstraints) -> Value {
    match constraints {
        CapabilityConstraints::None => json!({}),
        CapabilityConstraints::HttpRequest(value) => json!({
            "allowed_schemes": value.allowed_schemes.as_ref().map(|schemes| {
                schemes.iter().map(http_scheme_label).collect::<Vec<_>>()
            }),
            "allowed_hosts": value.allowed_hosts,
            "allowed_ports": value.allowed_ports,
            "allowed_methods": value.allowed_methods.as_ref().map(|methods| {
                methods.iter().map(http_method_label).collect::<Vec<_>>()
            }),
            "allowed_path_prefixes": value.allowed_path_prefixes,
            "max_timeout_ms": value.max_timeout_ms,
            "max_response_bytes": value.max_response_bytes,
        }),
        CapabilityConstraints::ReadResource(value) => json!({
            "uri_prefixes": value.uri_prefixes,
            "resource_kinds": value.resource_kinds.as_ref().map(|kinds| {
                kinds.iter().map(resource_kind_label).collect::<Vec<_>>()
            }),
        }),
        CapabilityConstraints::InvokeDependency(value) => json!({
            "aliases": value.aliases,
        }),
        CapabilityConstraints::EmitEvidence(value) => json!({
            "max_bytes": value.max_bytes,
            "audiences": value.audiences.as_ref().map(|audiences| {
                audiences.iter().map(evidence_audience_label).collect::<Vec<_>>()
            }),
            "redactions": value.redactions.as_ref().map(|redactions| {
                redactions.iter().map(redaction_label).collect::<Vec<_>>()
            }),
        }),
        CapabilityConstraints::Log(value) => json!({
            "levels": value.levels.as_ref().map(|levels| {
                levels.iter().map(severity_label).collect::<Vec<_>>()
            }),
        }),
    }
}

fn http_method_label(method: &HttpMethod) -> &'static str {
    match method {
        HttpMethod::Get => "get",
        HttpMethod::Head => "head",
    }
}

fn http_scheme_label(scheme: &HttpScheme) -> &'static str {
    match scheme {
        HttpScheme::Http => "http",
        HttpScheme::Https => "https",
    }
}

fn resource_kind_label(kind: &ResourceKind) -> &'static str {
    match kind {
        ResourceKind::Execution => "execution",
        ResourceKind::Object => "object",
        ResourceKind::Query => "query",
    }
}

fn evidence_audience_label(audience: &EvidenceAudience) -> &'static str {
    match audience {
        EvidenceAudience::User => "user",
        EvidenceAudience::Assistant => "assistant",
        EvidenceAudience::Internal => "internal",
    }
}

fn redaction_label(redaction: &RedactionClass) -> &'static str {
    match redaction {
        RedactionClass::None => "none",
        RedactionClass::SecretsRemoved => "secrets-removed",
        RedactionClass::PiiRemoved => "pii-removed",
        RedactionClass::TenantSensitive => "tenant-sensitive",
    }
}

fn severity_label(severity: &Severity) -> &'static str {
    match severity {
        Severity::Info => "info",
        Severity::Warn => "warn",
        Severity::Error => "error",
    }
}

export!(SummarizeExecutionQuery with_types_in self);
