use serde_json::{json, Value};
use wit_bindgen::generate;

generate!({
    path: "../../../../wit",
    world: "guild-skill",
});

use crate::exports::guild::skill::skill::{ExecutionContext, Guest, Json, SkillError, SkillOutput};
use crate::guild::skill::host;
use crate::guild::skill::types::{
    CapabilityAccess, CapabilityConstraints, CapabilityId, EvidenceAudience, ExecutionMode,
    GrantedCapability, HttpMethod, HttpRequestMessage, HttpScheme, RedactionClass,
    ResolvedSkillRef, ResourceKind, Severity,
};

struct InspectHttpJson;

impl Guest for InspectHttpJson {
    fn run(ctx: ExecutionContext, input: Json) -> Result<SkillOutput, SkillError> {
        let parsed_input: Value = serde_json::from_str(&input).map_err(invalid_input_error)?;
        let url = parsed_input
            .get("url")
            .and_then(Value::as_str)
            .filter(|url| !url.is_empty())
            .ok_or_else(|| SkillError {
                code: "missing-url".into(),
                message: "url must be a non-empty string".into(),
                retryable: false,
                detail: None,
            })?;
        let method = parse_method(parsed_input.get("method"))?;
        let timeout_ms = parsed_input.get("timeout_ms").and_then(Value::as_u64);
        let json_pointers = parse_json_pointers(parsed_input.get("json_pointers"))?;

        let response = host::http_request(&HttpRequestMessage {
            method: method.clone(),
            url: url.to_owned(),
            timeout_ms,
        })
        .map_err(|message| host_http_error(url, message))?;

        let selected_fields;
        let json_summary;
        if matches!(method, HttpMethod::Get) {
            if !content_type_is_json(response.content_type.as_deref()) {
                return Err(SkillError {
                    code: "http-response-not-json".into(),
                    message: "inspect-http-json requires an application/json response".into(),
                    retryable: false,
                    detail: Some(
                        json!({
                            "url": response.url,
                            "content_type": response.content_type,
                            "status": response.status,
                        })
                        .to_string(),
                    ),
                });
            }

            let json_body: Value =
                serde_json::from_slice(&response.body).map_err(|error| SkillError {
                    code: "http-response-invalid-json".into(),
                    message: "HTTP response body was not valid JSON".into(),
                    retryable: false,
                    detail: Some(
                        json!({
                            "url": response.url,
                            "status": response.status,
                            "error": error.to_string(),
                        })
                        .to_string(),
                    ),
                })?;
            selected_fields = select_json_fields(&json_body, &json_pointers);
            json_summary = summarize_json(&json_body);
        } else {
            selected_fields = Vec::new();
            json_summary = json!({
                "root_kind": "none",
                "reason": "HEAD responses do not include a JSON body",
            });
        }

        Ok(SkillOutput {
            summary: format!("Fetched JSON from {url} with HTTP {}.", response.status),
            structured: json!({
                "requested_url": url,
                "final_url": response.url,
                "method": http_method_label(&method),
                "status": response.status,
                "content_type": response.content_type,
                "response_bytes": response.body.len(),
                "selected_fields": selected_fields,
                "json_summary": json_summary,
                "granted_capabilities": granted_capabilities_payload(&ctx.granted_capabilities),
                "skill": resolved_skill_identity(&ctx.skill),
                "mode": execution_mode_label(&ctx.mode),
            })
            .to_string(),
            diagnostics: Vec::new(),
            effects: Vec::new(),
            evidence: Vec::new(),
        })
    }
}

fn invalid_input_error(error: serde_json::Error) -> SkillError {
    SkillError {
        code: "invalid-input".into(),
        message: "input JSON could not be parsed".into(),
        retryable: false,
        detail: Some(json!({ "error": error.to_string() }).to_string()),
    }
}

fn parse_method(method: Option<&Value>) -> Result<HttpMethod, SkillError> {
    match method.and_then(Value::as_str).unwrap_or("get") {
        "get" => Ok(HttpMethod::Get),
        "head" => Ok(HttpMethod::Head),
        other => Err(SkillError {
            code: "invalid-method".into(),
            message: "method must be one of get or head".into(),
            retryable: false,
            detail: Some(json!({ "method": other }).to_string()),
        }),
    }
}

fn parse_json_pointers(value: Option<&Value>) -> Result<Vec<String>, SkillError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let pointers = value.as_array().ok_or_else(|| SkillError {
        code: "invalid-json-pointers".into(),
        message: "json_pointers must be an array of JSON Pointer strings".into(),
        retryable: false,
        detail: None,
    })?;

    let mut parsed = Vec::with_capacity(pointers.len());
    for pointer in pointers {
        let pointer = pointer.as_str().ok_or_else(|| SkillError {
            code: "invalid-json-pointers".into(),
            message: "json_pointers must contain only strings".into(),
            retryable: false,
            detail: None,
        })?;
        if !pointer.is_empty() && !pointer.starts_with('/') {
            return Err(SkillError {
                code: "invalid-json-pointer".into(),
                message: "json_pointers must use JSON Pointer syntax".into(),
                retryable: false,
                detail: Some(json!({ "pointer": pointer }).to_string()),
            });
        }
        parsed.push(pointer.to_owned());
    }

    Ok(parsed)
}

fn content_type_is_json(content_type: Option<&str>) -> bool {
    let Some(content_type) = content_type else {
        return false;
    };
    let media_type = content_type
        .split(';')
        .next()
        .unwrap_or(content_type)
        .trim()
        .to_ascii_lowercase();
    media_type == "application/json" || media_type.ends_with("+json")
}

fn host_http_error(url: &str, message: String) -> SkillError {
    let (code, detail_message) = message
        .split_once(": ")
        .map_or(("http-request-failed", message.as_str()), |(code, detail)| {
            (code, detail)
        });

    SkillError {
        code: code.into(),
        message: detail_message.into(),
        retryable: code == "http-request-timeout",
        detail: Some(json!({ "url": url, "error": message }).to_string()),
    }
}

fn select_json_fields(body: &Value, pointers: &[String]) -> Vec<Value> {
    pointers
        .iter()
        .map(|pointer| {
            let value = body.pointer(pointer).cloned();
            json!({
                "pointer": pointer,
                "found": value.is_some(),
                "value": value.unwrap_or(Value::Null),
            })
        })
        .collect()
}

fn summarize_json(body: &Value) -> Value {
    match body {
        Value::Object(map) => {
            let mut keys = map.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            json!({
                "root_kind": "object",
                "object_keys": keys,
            })
        }
        Value::Array(items) => json!({
            "root_kind": "array",
            "array_length": items.len(),
        }),
        Value::String(_) => json!({ "root_kind": "string" }),
        Value::Number(_) => json!({ "root_kind": "number" }),
        Value::Bool(_) => json!({ "root_kind": "boolean" }),
        Value::Null => json!({ "root_kind": "null" }),
    }
}

fn execution_mode_label(mode: &ExecutionMode) -> &'static str {
    match mode {
        ExecutionMode::Inspect => "inspect",
        ExecutionMode::Plan => "plan",
        ExecutionMode::Apply => "apply",
    }
}

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

export!(InspectHttpJson with_types_in self);
