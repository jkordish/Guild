use serde_json::{json, Value};
use wit_bindgen::generate;

generate!({
    path: "../../../../wit",
    world: "guild-skill",
});

use crate::exports::guild::skill::skill::{ExecutionContext, Guest, Json, SkillError, SkillOutput};
use crate::guild::skill::host;
use crate::guild::skill::types::{
    CapabilityAccess, CapabilityConstraints, CapabilityId, DependencyInvocationRequest,
    ExecutionMode, GrantedCapability, ResolvedSkillRef, ResourceKind, Severity,
};

struct HelloComposite;

impl Guest for HelloComposite {
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
        let child_alias = parsed_input
            .get("child_alias")
            .and_then(Value::as_str)
            .filter(|alias| !alias.is_empty())
            .unwrap_or("hello");
        let child_emit_log = parsed_input
            .get("child_emit_log")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let child_output = host::invoke_dependency(&DependencyInvocationRequest {
            alias: child_alias.to_owned(),
            input: json!({
                "name": greeted,
                "emit_log": child_emit_log,
            })
            .to_string(),
        })?;
        let child_summary = child_output.summary;
        let child_structured: Value =
            serde_json::from_str(&child_output.structured).unwrap_or_else(|_| {
                json!({
                    "parse_error": "child structured output was not valid JSON"
                })
            });

        Ok(SkillOutput {
            summary: format!("Hello, {greeted}. Composite inspect is working."),
            structured: json!({
                "echoed_input": parsed_input,
                "mode": execution_mode_label(&ctx.mode),
                "skill": resolved_skill_identity(&ctx.skill),
                "granted_capabilities": granted_capabilities_payload(&ctx.granted_capabilities),
                "invoked_alias": child_alias,
                "message": format!("Composite hello for {greeted}"),
                "child": {
                    "summary": child_summary,
                    "structured": child_structured,
                },
            })
            .to_string(),
            diagnostics: Vec::new(),
            effects: Vec::new(),
            evidence: Vec::new(),
        })
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

export!(HelloComposite with_types_in self);
