//! Centralized host-to-guest projection for the active inspect ABI.
//!
//! Guild keeps durable execution, policy, and evidence state richer than the
//! guest-visible inspect ABI. This module owns the intentional projection from
//! host-owned inspect execution state into `guild-skill-inspect-v1`.

use guild_types::{
    CapabilityAccess, CapabilityConstraints, CapabilityId, ExecutionContext, ExecutionMode,
    ExecutionPhase, GrantedCapability, HttpRequestConstraints, ResolvedSkillRef,
};
use serde::Serialize;
use serde_json::json;

use super::{ExecutionError, bindings, canonicalize_host_scope, canonicalize_host_suffix_scope};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) enum InspectProjectionCompleteness {
    Full,
    BoundedSubset,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct InspectExecutionContextProjectionContract {
    pub completeness: InspectProjectionCompleteness,
    pub omitted_host_fields: &'static [&'static str],
    pub rationale: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct InspectCapabilityProjectionContract {
    pub capability_id: CapabilityId,
    pub completeness: InspectProjectionCompleteness,
    pub omitted_host_fields: &'static [&'static str],
    pub rationale: &'static str,
}

static EXECUTION_CONTEXT_PROJECTION_CONTRACT: InspectExecutionContextProjectionContract =
    InspectExecutionContextProjectionContract {
        completeness: InspectProjectionCompleteness::BoundedSubset,
        omitted_host_fields: &["ExecutionContext.mode"],
        rationale: "guild-skill-inspect-v1 is inspect-only, so the guest does not receive a redundant mode field",
    };

static ACTIVE_CAPABILITY_PROJECTION_CONTRACTS: [InspectCapabilityProjectionContract; 5] = [
    InspectCapabilityProjectionContract {
        capability_id: CapabilityId::ReadResource,
        completeness: InspectProjectionCompleteness::Full,
        omitted_host_fields: &[],
        rationale: "The guest sees the full active read-resource grant shape; canonical URI parsing and authorization remain host-owned runtime behavior.",
    },
    InspectCapabilityProjectionContract {
        capability_id: CapabilityId::InvokeSkill,
        completeness: InspectProjectionCompleteness::Full,
        omitted_host_fields: &[],
        rationale: "The guest sees the full active invoke-skill alias grant shape; installed dependency pinning, child grant reduction, and child policy decisions remain host-owned.",
    },
    InspectCapabilityProjectionContract {
        capability_id: CapabilityId::EmitEvidence,
        completeness: InspectProjectionCompleteness::Full,
        omitted_host_fields: &[],
        rationale: "The guest sees the full active emit-evidence grant shape; evidence-record identity and persistence metadata remain host-owned.",
    },
    InspectCapabilityProjectionContract {
        capability_id: CapabilityId::LogWrite,
        completeness: InspectProjectionCompleteness::Full,
        omitted_host_fields: &[],
        rationale: "The guest sees the full active log-write grant shape; sink handling and any future durable log storage remain host-owned.",
    },
    InspectCapabilityProjectionContract {
        capability_id: CapabilityId::HttpRequest,
        completeness: InspectProjectionCompleteness::Full,
        omitted_host_fields: &[],
        rationale: "The guest sees the full active HTTP grant shape; URL parsing, risky-destination classification, redirect evaluation, and denial provenance remain host-owned runtime state rather than grant fields.",
    },
];

pub(crate) fn execution_context_projection_contract()
-> &'static InspectExecutionContextProjectionContract {
    &EXECUTION_CONTEXT_PROJECTION_CONTRACT
}

#[cfg(test)]
pub(crate) fn active_capability_projection_contracts()
-> &'static [InspectCapabilityProjectionContract] {
    &ACTIVE_CAPABILITY_PROJECTION_CONTRACTS
}

fn capability_projection_contract(
    id: &CapabilityId,
) -> Option<&'static InspectCapabilityProjectionContract> {
    ACTIVE_CAPABILITY_PROJECTION_CONTRACTS
        .iter()
        .find(|contract| contract.capability_id == *id)
}

pub(crate) fn project_execution_context_to_inspect_abi(
    context: &ExecutionContext,
) -> Result<bindings::guild::skill::inspect_types::ExecutionContext, ExecutionError> {
    if context.mode != ExecutionMode::Inspect {
        return Err(ExecutionError::new(
            "inspect-abi-mode-mismatch",
            "guild-skill-inspect-v1 can only project inspect executions into the guest ABI",
        )
        .with_detail(json!({
            "mode": context.mode,
            "projection_contract": execution_context_projection_contract(),
        }))
        .with_phase(ExecutionPhase::Validation));
    }

    Ok(bindings::guild::skill::inspect_types::ExecutionContext {
        execution_id: context.execution_id.clone(),
        trace_id: context.trace_id.clone(),
        tenant_id: context.tenant_id.clone(),
        skill: project_resolved_skill_ref_to_inspect_abi(&context.skill),
        input_sha256: context.input_sha256.clone(),
        now_utc: context.now_utc.clone(),
        budget: bindings::guild::skill::inspect_types::Budget {
            max_millis: context.budget.max_millis,
            max_memory_bytes: context.budget.max_memory_bytes,
            max_output_bytes: context.budget.max_output_bytes,
            max_network_requests: context.budget.max_network_requests,
            max_child_executions: context.budget.max_child_executions,
        },
        granted_capabilities: context
            .granted_capabilities
            .grants
            .iter()
            .map(project_granted_capability_to_inspect_abi)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn project_resolved_skill_ref_to_inspect_abi(
    skill: &ResolvedSkillRef,
) -> bindings::guild::skill::inspect_types::ResolvedSkillRef {
    bindings::guild::skill::inspect_types::ResolvedSkillRef {
        key: bindings::guild::skill::inspect_types::SkillKey {
            namespace: skill.key.namespace.clone(),
            name: skill.key.name.clone(),
        },
        version: skill.version.to_string(),
        digest: skill.digest.clone(),
    }
}

fn project_granted_capability_to_inspect_abi(
    grant: &GrantedCapability,
) -> Result<bindings::guild::skill::inspect_types::GrantedCapability, ExecutionError> {
    Ok(bindings::guild::skill::inspect_types::GrantedCapability {
        id: project_capability_id_to_inspect_abi(&grant.id)?,
        access: project_capability_access_to_inspect_abi(&grant.access),
        constraints: project_capability_constraints_to_inspect_abi(&grant.id, &grant.constraints)?,
    })
}

fn project_capability_id_to_inspect_abi(
    id: &CapabilityId,
) -> Result<bindings::guild::skill::inspect_types::CapabilityId, ExecutionError> {
    if capability_projection_contract(id).is_some() {
        return Ok(match id {
            CapabilityId::HttpRequest => {
                bindings::guild::skill::inspect_types::CapabilityId::HttpRequest
            }
            CapabilityId::ReadResource => {
                bindings::guild::skill::inspect_types::CapabilityId::ReadResource
            }
            CapabilityId::InvokeSkill => {
                bindings::guild::skill::inspect_types::CapabilityId::InvokeSkill
            }
            CapabilityId::EmitEvidence => {
                bindings::guild::skill::inspect_types::CapabilityId::EmitEvidence
            }
            CapabilityId::LogWrite => bindings::guild::skill::inspect_types::CapabilityId::LogWrite,
            _ => unreachable!("active capability projection contracts must stay in sync"),
        });
    }

    match id {
        CapabilityId::Filesystem => Err(ExecutionError::new(
            "filesystem-runtime-not-supported",
            "filesystem capability contracts are not implemented in the active Wasm inspect slice",
        )
        .with_detail(json!({
            "id": id,
        }))
        .with_phase(ExecutionPhase::Validation)),
        CapabilityId::GetSecret
        | CapabilityId::CacheRead
        | CapabilityId::CacheWrite
        | CapabilityId::MonotonicClock
        | CapabilityId::WallClock => Err(ExecutionError::new(
            "unsupported-runtime-surface",
            "guild-skill-inspect-v1 only projects the active inspect capability families into the guest ABI",
        )
        .with_detail(json!({
            "id": id,
        }))
        .with_phase(ExecutionPhase::Validation)),
        CapabilityId::HttpRequest
        | CapabilityId::ReadResource
        | CapabilityId::InvokeSkill
        | CapabilityId::EmitEvidence
        | CapabilityId::LogWrite => unreachable!(
            "active inspect capability families should have returned through the explicit projection-contract path"
        ),
    }
}

fn project_capability_access_to_inspect_abi(
    access: &CapabilityAccess,
) -> bindings::guild::skill::inspect_types::CapabilityAccess {
    match access {
        CapabilityAccess::Read => bindings::guild::skill::inspect_types::CapabilityAccess::Read,
        CapabilityAccess::Write => bindings::guild::skill::inspect_types::CapabilityAccess::Write,
        CapabilityAccess::Invoke => bindings::guild::skill::inspect_types::CapabilityAccess::Invoke,
    }
}

fn project_capability_constraints_to_inspect_abi(
    id: &CapabilityId,
    constraints: &CapabilityConstraints,
) -> Result<bindings::guild::skill::inspect_types::CapabilityConstraints, ExecutionError> {
    match constraints {
        CapabilityConstraints::None(_) => {
            Ok(bindings::guild::skill::inspect_types::CapabilityConstraints::None)
        }
        CapabilityConstraints::Filesystem(value) => Err(ExecutionError::new(
            "filesystem-runtime-not-supported",
            "filesystem capability contracts are not implemented in the active Wasm inspect slice",
        )
        .with_detail(json!({
            "id": id,
            "constraints": value,
        }))
        .with_phase(ExecutionPhase::Validation)),
        CapabilityConstraints::HttpRequest(value) => {
            Ok(project_http_request_constraints_to_inspect_abi(value))
        }
        CapabilityConstraints::ReadResource(value) => {
            Ok(project_read_resource_constraints_to_inspect_abi(value))
        }
        CapabilityConstraints::InvokeDependency(value) => {
            Ok(project_invoke_dependency_constraints_to_inspect_abi(value))
        }
        CapabilityConstraints::EmitEvidence(value) => {
            Ok(project_emit_evidence_constraints_to_inspect_abi(value))
        }
        CapabilityConstraints::Log(value) => Ok(project_log_constraints_to_inspect_abi(value)),
    }
}

fn project_read_resource_constraints_to_inspect_abi(
    value: &guild_types::ReadResourceConstraints,
) -> bindings::guild::skill::inspect_types::CapabilityConstraints {
    bindings::guild::skill::inspect_types::CapabilityConstraints::ReadResource(
        bindings::guild::skill::inspect_types::ReadResourceConstraints {
            uri_prefixes: value.uri_prefixes.clone(),
            resource_kinds: value.resource_kinds.as_ref().map(|kinds| {
                kinds
                    .iter()
                    .map(project_resource_kind_to_inspect_abi)
                    .collect()
            }),
        },
    )
}

fn project_invoke_dependency_constraints_to_inspect_abi(
    value: &guild_types::InvokeDependencyConstraints,
) -> bindings::guild::skill::inspect_types::CapabilityConstraints {
    bindings::guild::skill::inspect_types::CapabilityConstraints::InvokeDependency(
        bindings::guild::skill::inspect_types::InvokeDependencyConstraints {
            aliases: value.aliases.clone(),
        },
    )
}

fn project_emit_evidence_constraints_to_inspect_abi(
    value: &guild_types::EmitEvidenceConstraints,
) -> bindings::guild::skill::inspect_types::CapabilityConstraints {
    bindings::guild::skill::inspect_types::CapabilityConstraints::EmitEvidence(
        bindings::guild::skill::inspect_types::EmitEvidenceConstraints {
            max_bytes: value.max_bytes,
            audiences: value.audiences.as_ref().map(|audiences| {
                audiences
                    .iter()
                    .map(project_evidence_audience_to_inspect_abi)
                    .collect()
            }),
            redactions: value.redactions.as_ref().map(|redactions| {
                redactions
                    .iter()
                    .map(project_redaction_class_to_inspect_abi)
                    .collect()
            }),
        },
    )
}

fn project_log_constraints_to_inspect_abi(
    value: &guild_types::LogConstraints,
) -> bindings::guild::skill::inspect_types::CapabilityConstraints {
    bindings::guild::skill::inspect_types::CapabilityConstraints::Log(
        bindings::guild::skill::inspect_types::LogConstraints {
            levels: value
                .levels
                .as_ref()
                .map(|levels| levels.iter().map(project_severity_to_inspect_abi).collect()),
        },
    )
}

fn project_http_request_constraints_to_inspect_abi(
    value: &HttpRequestConstraints,
) -> bindings::guild::skill::inspect_types::CapabilityConstraints {
    bindings::guild::skill::inspect_types::CapabilityConstraints::HttpRequest(
        bindings::guild::skill::inspect_types::HttpRequestConstraints {
            allowed_schemes: value.allowed_schemes.as_ref().map(|schemes| {
                schemes
                    .iter()
                    .map(project_http_scheme_to_inspect_abi)
                    .collect()
            }),
            allowed_hosts: value
                .allowed_hosts
                .as_ref()
                .map(|hosts| canonicalize_host_scope(hosts)),
            allowed_host_suffixes: value
                .allowed_host_suffixes
                .as_ref()
                .map(|hosts| canonicalize_host_suffix_scope(hosts)),
            allowed_ports: value.allowed_ports.clone(),
            allowed_methods: value.allowed_methods.as_ref().map(|methods| {
                methods
                    .iter()
                    .map(project_http_method_to_inspect_abi)
                    .collect()
            }),
            allowed_path_prefixes: value.allowed_path_prefixes.clone(),
            max_timeout_ms: value.max_timeout_ms,
            max_response_bytes: value.max_response_bytes,
            follow_redirects: value.follow_redirects,
            max_redirects: value.max_redirects,
            allow_loopback: value.allow_loopback,
            allow_link_local: value.allow_link_local,
            allow_private_networks: value.allow_private_networks,
            allow_ip_literals: value.allow_ip_literals,
        },
    )
}

fn project_http_method_to_inspect_abi(
    method: &guild_types::HttpMethod,
) -> bindings::guild::skill::inspect_types::HttpMethod {
    match method {
        guild_types::HttpMethod::Get => bindings::guild::skill::inspect_types::HttpMethod::Get,
        guild_types::HttpMethod::Head => bindings::guild::skill::inspect_types::HttpMethod::Head,
    }
}

fn project_http_scheme_to_inspect_abi(
    scheme: &guild_types::HttpScheme,
) -> bindings::guild::skill::inspect_types::HttpScheme {
    match scheme {
        guild_types::HttpScheme::Http => bindings::guild::skill::inspect_types::HttpScheme::Http,
        guild_types::HttpScheme::Https => bindings::guild::skill::inspect_types::HttpScheme::Https,
    }
}

fn project_resource_kind_to_inspect_abi(
    kind: &guild_types::ResourceKind,
) -> bindings::guild::skill::inspect_types::ResourceKind {
    match kind {
        guild_types::ResourceKind::Execution => {
            bindings::guild::skill::inspect_types::ResourceKind::Execution
        }
        guild_types::ResourceKind::Object => {
            bindings::guild::skill::inspect_types::ResourceKind::Object
        }
        guild_types::ResourceKind::Query => {
            bindings::guild::skill::inspect_types::ResourceKind::Query
        }
    }
}

fn project_evidence_audience_to_inspect_abi(
    audience: &guild_types::EvidenceAudience,
) -> bindings::guild::skill::inspect_types::EvidenceAudience {
    match audience {
        guild_types::EvidenceAudience::User => {
            bindings::guild::skill::inspect_types::EvidenceAudience::User
        }
        guild_types::EvidenceAudience::Assistant => {
            bindings::guild::skill::inspect_types::EvidenceAudience::Assistant
        }
        guild_types::EvidenceAudience::Internal => {
            bindings::guild::skill::inspect_types::EvidenceAudience::Internal
        }
    }
}

fn project_redaction_class_to_inspect_abi(
    redaction: &guild_types::RedactionClass,
) -> bindings::guild::skill::inspect_types::RedactionClass {
    match redaction {
        guild_types::RedactionClass::None => {
            bindings::guild::skill::inspect_types::RedactionClass::None
        }
        guild_types::RedactionClass::SecretsRemoved => {
            bindings::guild::skill::inspect_types::RedactionClass::SecretsRemoved
        }
        guild_types::RedactionClass::PiiRemoved => {
            bindings::guild::skill::inspect_types::RedactionClass::PiiRemoved
        }
        guild_types::RedactionClass::TenantSensitive => {
            bindings::guild::skill::inspect_types::RedactionClass::TenantSensitive
        }
    }
}

fn project_severity_to_inspect_abi(
    severity: &guild_types::Severity,
) -> bindings::guild::skill::inspect_types::Severity {
    match severity {
        guild_types::Severity::Info => bindings::guild::skill::inspect_types::Severity::Info,
        guild_types::Severity::Warn => bindings::guild::skill::inspect_types::Severity::Warn,
        guild_types::Severity::Error => bindings::guild::skill::inspect_types::Severity::Error,
    }
}

#[cfg(test)]
mod tests {
    use guild_types::{
        Budget, CapabilityGrantSet, EvidenceAudience, ExecutionContext, HttpMethod,
        HttpRequestConstraints, HttpScheme, InvokeDependencyConstraints, LogConstraints,
        ReadResourceConstraints, RedactionClass, ResolvedSkillRef, ResourceKind, Severity,
        SkillKey, SkillVersion,
    };

    use super::*;

    fn sample_skill() -> ResolvedSkillRef {
        ResolvedSkillRef {
            key: SkillKey {
                namespace: "example".into(),
                name: "projection".into(),
            },
            version: SkillVersion::parse("0.1.0").expect("valid semver"),
            digest: "sha256:projection".into(),
        }
    }

    fn sample_execution_context(
        grants: Vec<GrantedCapability>,
        mode: ExecutionMode,
    ) -> ExecutionContext {
        ExecutionContext {
            execution_id: "exec-projection".into(),
            trace_id: "trace-projection".into(),
            tenant_id: "tenant-projection".into(),
            skill: sample_skill(),
            mode,
            input_sha256: "sha256:input".into(),
            now_utc: Some("2026-03-18T00:00:00Z".into()),
            budget: Budget::default(),
            granted_capabilities: CapabilityGrantSet { grants },
        }
    }

    fn sample_http_grant() -> GrantedCapability {
        GrantedCapability {
            id: CapabilityId::HttpRequest,
            access: CapabilityAccess::Read,
            constraints: CapabilityConstraints::HttpRequest(HttpRequestConstraints {
                allowed_schemes: Some(vec![HttpScheme::Http, HttpScheme::Https]),
                allowed_hosts: Some(vec![
                    "EXAMPLE.com".into(),
                    "127.0.0.1".into(),
                    "example.COM".into(),
                ]),
                allowed_host_suffixes: Some(vec!["Example.COM".into(), "EXAMPLE.com".into()]),
                allowed_ports: Some(vec![80, 443]),
                allowed_methods: Some(vec![HttpMethod::Get, HttpMethod::Head]),
                allowed_path_prefixes: Some(vec!["/inspect".into(), "/health".into()]),
                max_timeout_ms: Some(2_000),
                max_response_bytes: Some(8_192),
                follow_redirects: Some(true),
                max_redirects: Some(2),
                allow_loopback: Some(true),
                allow_link_local: Some(false),
                allow_private_networks: Some(false),
                allow_ip_literals: Some(true),
            }),
        }
    }

    fn sample_read_resource_grant() -> GrantedCapability {
        GrantedCapability {
            id: CapabilityId::ReadResource,
            access: CapabilityAccess::Read,
            constraints: CapabilityConstraints::ReadResource(ReadResourceConstraints {
                uri_prefixes: Some(vec![
                    "guild://executions/".into(),
                    "guild://queries/executions/".into(),
                ]),
                resource_kinds: Some(vec![ResourceKind::Execution, ResourceKind::Query]),
            }),
        }
    }

    fn sample_invoke_dependency_grant() -> GrantedCapability {
        GrantedCapability {
            id: CapabilityId::InvokeSkill,
            access: CapabilityAccess::Invoke,
            constraints: CapabilityConstraints::InvokeDependency(InvokeDependencyConstraints {
                aliases: Some(vec!["child-a".into(), "child-b".into()]),
            }),
        }
    }

    fn sample_emit_evidence_grant() -> GrantedCapability {
        GrantedCapability {
            id: CapabilityId::EmitEvidence,
            access: CapabilityAccess::Write,
            constraints: CapabilityConstraints::EmitEvidence(
                guild_types::EmitEvidenceConstraints {
                    max_bytes: Some(4_096),
                    audiences: Some(vec![EvidenceAudience::User, EvidenceAudience::Internal]),
                    redactions: Some(vec![RedactionClass::None, RedactionClass::TenantSensitive]),
                },
            ),
        }
    }

    fn sample_log_grant() -> GrantedCapability {
        GrantedCapability {
            id: CapabilityId::LogWrite,
            access: CapabilityAccess::Write,
            constraints: CapabilityConstraints::Log(LogConstraints {
                levels: Some(vec![Severity::Info, Severity::Error]),
            }),
        }
    }

    fn assert_projected_read_resource(
        projected: &bindings::guild::skill::inspect_types::GrantedCapability,
    ) {
        match &projected.constraints {
            bindings::guild::skill::inspect_types::CapabilityConstraints::ReadResource(value) => {
                assert_eq!(
                    value.uri_prefixes,
                    Some(vec![
                        "guild://executions/".into(),
                        "guild://queries/executions/".into(),
                    ])
                );
                assert_eq!(
                    value.resource_kinds,
                    Some(vec![
                        bindings::guild::skill::inspect_types::ResourceKind::Execution,
                        bindings::guild::skill::inspect_types::ResourceKind::Query,
                    ])
                );
            }
            other => panic!("unexpected projected read-resource constraints: {other:?}"),
        }
    }

    fn assert_projected_invoke_dependency(
        projected: &bindings::guild::skill::inspect_types::GrantedCapability,
    ) {
        match &projected.constraints {
            bindings::guild::skill::inspect_types::CapabilityConstraints::InvokeDependency(
                value,
            ) => {
                assert_eq!(
                    value.aliases,
                    Some(vec!["child-a".into(), "child-b".into()])
                );
            }
            other => panic!("unexpected projected invoke-skill constraints: {other:?}"),
        }
    }

    fn assert_projected_emit_evidence(
        projected: &bindings::guild::skill::inspect_types::GrantedCapability,
    ) {
        match &projected.constraints {
            bindings::guild::skill::inspect_types::CapabilityConstraints::EmitEvidence(value) => {
                assert_eq!(value.max_bytes, Some(4_096));
                assert_eq!(
                    value.audiences,
                    Some(vec![
                        bindings::guild::skill::inspect_types::EvidenceAudience::User,
                        bindings::guild::skill::inspect_types::EvidenceAudience::Internal,
                    ])
                );
                assert_eq!(
                    value.redactions,
                    Some(vec![
                        bindings::guild::skill::inspect_types::RedactionClass::None,
                        bindings::guild::skill::inspect_types::RedactionClass::TenantSensitive,
                    ])
                );
            }
            other => panic!("unexpected projected emit-evidence constraints: {other:?}"),
        }
    }

    fn assert_projected_log(projected: &bindings::guild::skill::inspect_types::GrantedCapability) {
        match &projected.constraints {
            bindings::guild::skill::inspect_types::CapabilityConstraints::Log(value) => {
                assert_eq!(
                    value.levels,
                    Some(vec![
                        bindings::guild::skill::inspect_types::Severity::Info,
                        bindings::guild::skill::inspect_types::Severity::Error,
                    ])
                );
            }
            other => panic!("unexpected projected log-write constraints: {other:?}"),
        }
    }

    fn assert_projected_http(projected: &bindings::guild::skill::inspect_types::GrantedCapability) {
        match &projected.constraints {
            bindings::guild::skill::inspect_types::CapabilityConstraints::HttpRequest(value) => {
                assert_eq!(
                    value.allowed_schemes,
                    Some(vec![
                        bindings::guild::skill::inspect_types::HttpScheme::Http,
                        bindings::guild::skill::inspect_types::HttpScheme::Https,
                    ])
                );
                assert_eq!(
                    value.allowed_hosts,
                    Some(vec!["example.com".into(), "127.0.0.1".into()])
                );
                assert_eq!(
                    value.allowed_host_suffixes,
                    Some(vec!["example.com".into()])
                );
                assert_eq!(value.allowed_ports, Some(vec![80, 443]));
                assert_eq!(
                    value.allowed_methods,
                    Some(vec![
                        bindings::guild::skill::inspect_types::HttpMethod::Get,
                        bindings::guild::skill::inspect_types::HttpMethod::Head,
                    ])
                );
                assert_eq!(
                    value.allowed_path_prefixes,
                    Some(vec!["/inspect".into(), "/health".into()])
                );
                assert_eq!(value.max_timeout_ms, Some(2_000));
                assert_eq!(value.max_response_bytes, Some(8_192));
                assert_eq!(value.follow_redirects, Some(true));
                assert_eq!(value.max_redirects, Some(2));
                assert_eq!(value.allow_loopback, Some(true));
                assert_eq!(value.allow_link_local, Some(false));
                assert_eq!(value.allow_private_networks, Some(false));
                assert_eq!(value.allow_ip_literals, Some(true));
            }
            other => panic!("unexpected projected http-request constraints: {other:?}"),
        }
    }

    #[test]
    fn execution_context_projection_contract_is_explicitly_bounded() {
        let contract = execution_context_projection_contract();
        assert_eq!(
            contract.completeness,
            InspectProjectionCompleteness::BoundedSubset
        );
        assert_eq!(contract.omitted_host_fields, ["ExecutionContext.mode"]);
    }

    #[test]
    fn active_capability_projection_contracts_cover_each_active_family() {
        let contracts = active_capability_projection_contracts();
        assert_eq!(contracts.len(), 5);
        assert_eq!(
            contracts
                .iter()
                .map(|contract| contract.capability_id.clone())
                .collect::<Vec<_>>(),
            vec![
                CapabilityId::ReadResource,
                CapabilityId::InvokeSkill,
                CapabilityId::EmitEvidence,
                CapabilityId::LogWrite,
                CapabilityId::HttpRequest,
            ]
        );
        assert!(contracts.iter().all(|contract| {
            contract.completeness == InspectProjectionCompleteness::Full
                && contract.omitted_host_fields.is_empty()
        }));
    }

    #[test]
    fn active_family_projection_paths_are_explicit_and_stable() {
        let context = sample_execution_context(
            vec![
                sample_read_resource_grant(),
                sample_invoke_dependency_grant(),
                sample_emit_evidence_grant(),
                sample_log_grant(),
                sample_http_grant(),
            ],
            ExecutionMode::Inspect,
        );

        let projected = project_execution_context_to_inspect_abi(&context)
            .expect("active inspect families project cleanly");

        assert_eq!(projected.granted_capabilities.len(), 5);
        assert_projected_read_resource(&projected.granted_capabilities[0]);
        assert_projected_invoke_dependency(&projected.granted_capabilities[1]);
        assert_projected_emit_evidence(&projected.granted_capabilities[2]);
        assert_projected_log(&projected.granted_capabilities[3]);
        assert_projected_http(&projected.granted_capabilities[4]);
    }

    #[test]
    fn unsupported_families_do_not_project_into_the_active_inspect_abi() {
        let unsupported = GrantedCapability {
            id: CapabilityId::CacheRead,
            access: CapabilityAccess::Read,
            constraints: CapabilityConstraints::none(),
        };
        let unsupported_error = project_granted_capability_to_inspect_abi(&unsupported)
            .expect_err("cache-read must stay outside the inspect ABI");
        assert_eq!(unsupported_error.code, "unsupported-runtime-surface");

        let filesystem = GrantedCapability {
            id: CapabilityId::Filesystem,
            access: CapabilityAccess::Read,
            constraints: CapabilityConstraints::Filesystem(guild_types::FilesystemConstraints {
                preopened_roots: Vec::new(),
            }),
        };
        let filesystem_error = project_granted_capability_to_inspect_abi(&filesystem)
            .expect_err("filesystem must stay outside the active inspect ABI");
        assert_eq!(filesystem_error.code, "filesystem-runtime-not-supported");
    }

    #[test]
    fn execution_context_projection_rejects_non_inspect_modes() {
        let error = project_execution_context_to_inspect_abi(&sample_execution_context(
            vec![sample_emit_evidence_grant()],
            ExecutionMode::Plan,
        ))
        .expect_err("non-inspect modes must not project into guild-skill-inspect-v1");

        assert_eq!(error.code, "inspect-abi-mode-mismatch");
    }
}
