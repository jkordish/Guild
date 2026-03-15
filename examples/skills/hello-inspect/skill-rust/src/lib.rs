use serde_json::{json, Value};
use wit_bindgen::generate;

generate!({
    path: "../../../../wit",
    world: "guild-skill",
});

use crate::exports::guild::skill::skill::{ExecutionContext, Guest, Json, SkillError, SkillOutput};
use crate::guild::skill::host;
use crate::guild::skill::types::{
    CapabilityAccess, CapabilityConstraints, CapabilityId, EvidenceAudience,
    EvidenceEmissionRequest, ExecutionMode, GrantedCapability, RedactionClass, ResolvedSkillRef,
    ResourceKind, Severity,
};

struct HelloInspect;

impl Guest for HelloInspect {
    fn run(ctx: ExecutionContext, input: Json) -> Result<SkillOutput, SkillError> {
        let parsed_input: Value = serde_json::from_str(&input).map_err(|error| SkillError {
            code: "invalid-input".into(),
            message: "input JSON could not be parsed".into(),
            retryable: false,
            detail: Some(json!({ "error": error.to_string() }).to_string()),
        })?;

        let greeted = parsed_input
            .get("name")
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty())
            .unwrap_or("friend");

        if requested_log(&parsed_input) {
            host::log(
                Severity::Info,
                &format!("hello-inspect is running for {}", ctx.execution_id),
            );
        }

        let evidence = emit_execution_evidence(&ctx, &parsed_input)?;

        Ok(SkillOutput {
            summary: format!("Hello, {greeted}. Guild inspect is working."),
            structured: json!({
                "echoed_input": parsed_input,
                "mode": execution_mode_label(&ctx.mode),
                "skill": resolved_skill_identity(&ctx.skill),
                "granted_capabilities": granted_capabilities_payload(&ctx.granted_capabilities),
                "message": format!("Hello, {greeted}"),
            })
            .to_string(),
            diagnostics: Vec::new(),
            effects: Vec::new(),
            evidence: vec![evidence],
        })
    }
}

fn requested_log(input: &Value) -> bool {
    input
        .get("emit_log")
        .and_then(Value::as_bool)
        .unwrap_or(false)
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

fn emit_execution_evidence(
    ctx: &ExecutionContext,
    parsed_input: &Value,
) -> Result<crate::guild::skill::types::EvidenceRef, SkillError> {
    let payload = json!({
        "kind": "hello-inspect-snapshot",
        "echoed_input": parsed_input,
        "mode": execution_mode_label(&ctx.mode),
        "skill": resolved_skill_identity(&ctx.skill),
    });
    let payload = serde_json::to_vec(&payload).map_err(|error| SkillError {
        code: "evidence-payload-invalid".into(),
        message: "evidence payload could not be serialized".into(),
        retryable: false,
        detail: Some(json!({ "error": error.to_string() }).to_string()),
    })?;

    host::emit_evidence(&EvidenceEmissionRequest {
        payload,
        mime_type: "application/json".into(),
        title: Some("hello-inspect snapshot".into()),
        audience: EvidenceAudience::User,
        redaction: RedactionClass::None,
        freshness: Some("deterministic".into()),
    })
    .map_err(|message| SkillError {
        code: "emit-evidence-failed".into(),
        message: "host failed to persist execution evidence".into(),
        retryable: false,
        detail: Some(json!({ "error": message }).to_string()),
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

export!(HelloInspect with_types_in self);
