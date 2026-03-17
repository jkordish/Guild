use serde_json::{json, Value};
use wit_bindgen::generate;

generate!({
    path: "../../../../wit",
    world: "guild-skill",
});

use crate::exports::guild::skill::skill::{ExecutionContext, Guest, Json, SkillError, SkillOutput};
use crate::guild::skill::host;
use crate::guild::skill::types::{
    CapabilityAccess, CapabilityConstraints, CapabilityId, ExecutionMode, GrantedCapability,
    HttpMethod, HttpScheme, ResolvedSkillRef, ResourceKind, ResourceReadResult, Severity,
};

struct ExplainExecution;

impl Guest for ExplainExecution {
    fn run(ctx: ExecutionContext, input: Json) -> Result<SkillOutput, SkillError> {
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
        let include_first_evidence = parsed_input
            .get("include_first_evidence")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let execution_resource = read_resource(execution_uri)?;
        let execution_record = parse_json_bytes(&execution_resource, "execution resource")?;
        let first_evidence = if include_first_evidence {
            execution_record
                .pointer("/emitted_evidence/0/uri")
                .and_then(Value::as_str)
                .map(read_resource)
                .transpose()?
                .map(|resource| describe_resource_json(&resource))
                .transpose()?
        } else {
            None
        };

        Ok(SkillOutput {
            summary: format!("Explained stored execution {execution_uri}."),
            structured: json!({
                "target_execution_uri": execution_uri,
                "execution_resource": {
                    "uri": execution_resource.uri,
                    "mime_type": execution_resource.mime_type,
                    "sha256": execution_resource.sha256,
                },
                "target_status": execution_record
                    .get("status")
                    .cloned()
                    .unwrap_or(Value::Null),
                "target_skill": execution_record
                    .pointer("/resolved_skill")
                    .cloned()
                    .unwrap_or(Value::Null),
                "policy_decision": execution_record
                    .get("policy_decision")
                    .cloned()
                    .unwrap_or(Value::Null),
                "stored_summary": execution_record.pointer("/output/summary").cloned().unwrap_or(Value::Null),
                "termination": execution_record
                    .get("termination")
                    .cloned()
                    .unwrap_or(Value::Null),
                "evidence_count": execution_record
                    .get("emitted_evidence")
                    .and_then(Value::as_array)
                    .map(|items| items.len())
                    .unwrap_or(0),
                "child_execution_count": execution_record
                    .get("child_executions")
                    .and_then(Value::as_array)
                    .map(|items| items.len())
                    .unwrap_or(0),
                "child_execution_uris": execution_record
                    .get("child_executions")
                    .and_then(Value::as_array)
                    .map(|items| {
                        items.iter()
                            .filter_map(|item| item.get("uri").cloned())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_else(Vec::new),
                "child_execution_statuses": execution_record
                    .get("child_executions")
                    .and_then(Value::as_array)
                    .map(|items| {
                        items.iter()
                            .map(|item| {
                                json!({
                                    "uri": item.get("uri").cloned().unwrap_or(Value::Null),
                                    "status": item.get("status").cloned().unwrap_or(Value::Null),
                                    "termination": item.get("termination").cloned().unwrap_or(Value::Null),
                                })
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_else(Vec::new),
                "first_evidence": first_evidence.unwrap_or(Value::Null),
                "granted_capabilities": granted_capabilities_payload(&ctx.granted_capabilities),
            })
            .to_string(),
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

fn describe_resource_json(resource: &ResourceReadResult) -> Result<Value, SkillError> {
    Ok(json!({
        "uri": resource.uri,
        "mime_type": resource.mime_type,
        "sha256": resource.sha256,
        "json": parse_json_bytes(resource, "evidence resource")?,
    }))
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

fn resource_kind_label(kind: &ResourceKind) -> &'static str {
    match kind {
        ResourceKind::Execution => "execution",
        ResourceKind::Object => "object",
        ResourceKind::Query => "query",
    }
}

fn evidence_audience_label(audience: &crate::guild::skill::types::EvidenceAudience) -> &'static str {
    match audience {
        crate::guild::skill::types::EvidenceAudience::User => "user",
        crate::guild::skill::types::EvidenceAudience::Assistant => "assistant",
        crate::guild::skill::types::EvidenceAudience::Internal => "internal",
    }
}

fn redaction_label(redaction: &crate::guild::skill::types::RedactionClass) -> &'static str {
    match redaction {
        crate::guild::skill::types::RedactionClass::None => "none",
        crate::guild::skill::types::RedactionClass::SecretsRemoved => "secrets-removed",
        crate::guild::skill::types::RedactionClass::PiiRemoved => "pii-removed",
        crate::guild::skill::types::RedactionClass::TenantSensitive => "tenant-sensitive",
    }
}

fn severity_label(severity: &Severity) -> &'static str {
    match severity {
        Severity::Info => "info",
        Severity::Warn => "warn",
        Severity::Error => "error",
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

#[allow(dead_code)]
fn execution_mode_label(mode: &ExecutionMode) -> &'static str {
    match mode {
        ExecutionMode::Inspect => "inspect",
        ExecutionMode::Plan => "plan",
        ExecutionMode::Apply => "apply",
    }
}

#[allow(dead_code)]
fn resolved_skill_identity(skill: &ResolvedSkillRef) -> Value {
    json!({
        "key": {
            "namespace": skill.key.namespace,
            "name": skill.key.name,
        },
        "version": skill.version,
        "digest": skill.digest,
    })
}

export!(ExplainExecution with_types_in self);
