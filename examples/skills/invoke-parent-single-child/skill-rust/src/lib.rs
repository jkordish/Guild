use serde_json::{Value, json};
use wit_bindgen::generate;

const _: &str = include_str!("../../../../../wit/guild-skill-v1.wit");

generate!({
    path: "../../../../wit",
    world: "guild-skill-inspect-v1",
});

use crate::exports::guild::skill::inspect_skill::{
    ExecutionContext, Guest, Json, SkillError, SkillOutput,
};
use crate::guild::skill::inspect_host as host;
use crate::guild::skill::inspect_types::{
    CapabilityAccess, CapabilityConstraints, CapabilityId, DependencyInvocationRequest,
    GrantedCapability, ResolvedSkillRef,
};

const CHILD_ALIAS: &str = "child";

struct InvokeParentSingleChild;

impl Guest for InvokeParentSingleChild {
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
        let invoke_twice = parsed_input
            .get("invoke_twice")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let second_greeted = parsed_input
            .get("second_name")
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty())
            .unwrap_or(greeted);

        let child_input = json!({ "name": greeted }).to_string();
        let first = invoke_child(&child_input)?;
        let mut children = vec![first];
        if invoke_twice {
            let second_child_input = json!({ "name": second_greeted }).to_string();
            children.push(invoke_child(&second_child_input)?);
        }

        let invoked_aliases = vec![CHILD_ALIAS.to_owned(); children.len()];
        let invocation_count = children.len();

        Ok(SkillOutput {
            summary: format!(
                "Single-child invoke completed for {greeted} with {invocation_count} child run(s)."
            ),
            structured: json!({
                "echoed_input": parsed_input,
                "mode": "inspect",
                "skill": resolved_skill_identity(&ctx.skill),
                "granted_capabilities": granted_capabilities_payload(&ctx.granted_capabilities),
                "invocation_count": invocation_count,
                "invoked_aliases": invoked_aliases,
                "children": children,
                "message": format!("parent invoked {invocation_count} child run(s) for {greeted}"),
            })
            .to_string(),
            diagnostics: Vec::new(),
            effects: Vec::new(),
            evidence: Vec::new(),
        })
    }
}

fn invoke_child(input: &str) -> Result<Value, SkillError> {
    let child_output = host::invoke_dependency(&DependencyInvocationRequest {
        alias: CHILD_ALIAS.to_owned(),
        input: input.to_owned(),
    })?;
    let child_structured: Value = serde_json::from_str(&child_output.structured).map_err(|error| {
        SkillError {
            code: "child-structured-invalid".into(),
            message: "child structured output was not valid JSON".into(),
            retryable: false,
            detail: Some(json!({ "error": error.to_string() }).to_string()),
        }
    })?;

    Ok(json!({
        "summary": child_output.summary,
        "structured": child_structured,
    }))
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
        CapabilityId::LogWrite => "log-write",
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
        CapabilityConstraints::InvokeDependency(value) => json!({
            "aliases": value.aliases,
        }),
        CapabilityConstraints::HttpRequest(_)
        | CapabilityConstraints::ReadResource(_)
        | CapabilityConstraints::EmitEvidence(_)
        | CapabilityConstraints::Log(_) => json!({ "unsupported": "unexpected in this fixture" }),
    }
}

export!(InvokeParentSingleChild with_types_in self);
