use guild_types::{
    Budget, CapabilityAccess, CapabilityConstraints, CapabilityGrantSet, CapabilityId,
    ExecutionContext, ExecutionMode, ExecutionQueryResource, FilesystemConstraints,
    FilesystemOperation, FilesystemRoot, GrantedCapability, GuildResourceScope, GuildResourceUri,
    HttpMethod, HttpRequest, HttpRequestConstraints, HttpResponse, HttpScheme,
    InstalledVerificationState, LocalPolicyConfig, LocalTrustTier, PolicyDecision,
    PolicyDecisionOutcome, PolicyProfile, PolicyProfileBinding, PolicyReason, PolicyRule,
    PolicyRuleEffect, PolicyRuleTarget, ReadResourceConstraints, ResolvedSkillRef, ResourceKind,
    SkillKey, SkillVersion, VersionRequirement,
};

#[test]
fn skill_version_serializes_as_a_string() {
    let version = SkillVersion::parse("1.2.3").unwrap();
    let encoded = serde_json::to_string(&version).unwrap();
    assert_eq!(encoded, "\"1.2.3\"");
}

#[test]
fn version_requirement_serializes_as_a_string() {
    let version_req = VersionRequirement::parse("^1.2").unwrap();
    let encoded = serde_json::to_string(&version_req).unwrap();
    assert_eq!(encoded, "\"^1.2\"");
}

#[test]
fn execution_context_roundtrips_grants() {
    let ctx = ExecutionContext {
        execution_id: "exec-1".into(),
        trace_id: "trace-1".into(),
        tenant_id: "tenant-1".into(),
        skill: ResolvedSkillRef {
            key: SkillKey {
                namespace: "example".into(),
                name: "hello-inspect".into(),
            },
            version: SkillVersion::parse("0.1.0").unwrap(),
            digest: "sha256:e7d2594a0927fb4bf08e7aae9ec8168aedfdd6a4bec54f2831184041d3dd8fba"
                .into(),
        },
        mode: ExecutionMode::Inspect,
        input_sha256: "sha256:abc".into(),
        now_utc: Some("2026-03-14T00:00:00Z".into()),
        budget: Budget::default(),
        granted_capabilities: CapabilityGrantSet {
            grants: vec![GrantedCapability {
                id: CapabilityId::ReadResource,
                access: CapabilityAccess::Read,
                constraints: CapabilityConstraints::ReadResource(ReadResourceConstraints {
                    uri_prefixes: Some(vec!["guild://executions/".into()]),
                    resource_kinds: Some(vec![ResourceKind::Execution]),
                }),
            }],
        },
    };

    let encoded = serde_json::to_value(&ctx).unwrap();
    assert_eq!(encoded["skill"]["key"]["name"], "hello-inspect");
    assert_eq!(
        encoded["granted_capabilities"]["grants"][0]["id"],
        "read-resource"
    );
}

#[test]
fn guild_resource_scopes_must_be_canonical_roots() {
    assert_eq!(
        GuildResourceScope::parse("guild://executions/")
            .unwrap()
            .kind(),
        ResourceKind::Execution
    );
    assert_eq!(
        GuildResourceScope::parse("guild://objects/records/")
            .unwrap()
            .kind(),
        ResourceKind::Object
    );
    assert_eq!(
        GuildResourceScope::parse("guild://queries/executions/")
            .unwrap()
            .kind(),
        ResourceKind::Query
    );
    assert!(GuildResourceScope::parse("guild://executions").is_err());
    assert!(GuildResourceScope::parse("guild://objects/").is_err());
    assert!(GuildResourceScope::parse("guild://exec").is_err());
}

#[test]
fn guild_resource_uris_parse_canonically() {
    assert_eq!(
        GuildResourceUri::parse("guild://executions/exec-1")
            .unwrap()
            .kind(),
        ResourceKind::Execution
    );
    assert_eq!(
        GuildResourceUri::parse("guild://objects/records/record-1")
            .unwrap()
            .kind(),
        ResourceKind::Object
    );
    assert_eq!(
        GuildResourceUri::parse(
            "guild://objects/sha256/0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        )
        .unwrap()
        .kind(),
        ResourceKind::Object
    );
    assert_eq!(
        GuildResourceUri::parse("guild://queries/executions/failures/recent/10")
            .unwrap()
            .kind(),
        ResourceKind::Query
    );
    assert!(GuildResourceUri::parse("guild://objects/sha256/ABCDEF").is_err());
    assert!(GuildResourceUri::parse("guild://executions/%GG").is_err());
}

#[test]
fn execution_query_resources_roundtrip_canonical_uris() {
    let query = ExecutionQueryResource::BySkill {
        namespace: "example".into(),
        name: "summarize-execution-query".into(),
        limit: 7,
    };
    let uri = query.canonical_uri();

    assert_eq!(ExecutionQueryResource::parse_uri(&uri).unwrap(), query);
    assert_eq!(
        GuildResourceUri::parse(&uri).unwrap(),
        GuildResourceUri::ExecutionQuery { query }
    );
}

#[test]
fn malformed_execution_query_resources_fail_closed() {
    assert!(ExecutionQueryResource::parse_uri("guild://queries/executions/recent/0").is_err());
    assert!(
        ExecutionQueryResource::parse_uri("guild://queries/executions/by-status/not-a-status/5")
            .is_err()
    );
    assert!(
        ExecutionQueryResource::parse_uri("guild://queries/executions/by-skill/example/skill/99")
            .is_err()
    );
    assert!(GuildResourceUri::parse("guild://queries/executions/unknown/5").is_err());
}

#[test]
fn http_capability_constraints_roundtrip_in_execution_context() {
    let ctx = ExecutionContext {
        execution_id: "exec-http-1".into(),
        trace_id: "trace-http-1".into(),
        tenant_id: "tenant-1".into(),
        skill: ResolvedSkillRef {
            key: SkillKey {
                namespace: "example".into(),
                name: "inspect-http-json".into(),
            },
            version: SkillVersion::parse("0.1.0").unwrap(),
            digest: "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                .into(),
        },
        mode: ExecutionMode::Inspect,
        input_sha256: "sha256:def".into(),
        now_utc: Some("2026-03-16T00:00:00Z".into()),
        budget: Budget::default(),
        granted_capabilities: CapabilityGrantSet {
            grants: vec![GrantedCapability {
                id: CapabilityId::HttpRequest,
                access: CapabilityAccess::Read,
                constraints: CapabilityConstraints::HttpRequest(HttpRequestConstraints {
                    allowed_schemes: Some(vec![HttpScheme::Http]),
                    allowed_hosts: Some(vec!["127.0.0.1".into()]),
                    allowed_host_suffixes: Some(vec!["example.com".into()]),
                    allowed_ports: Some(vec![8080]),
                    allowed_methods: Some(vec![HttpMethod::Get]),
                    allowed_path_prefixes: Some(vec!["/json".into()]),
                    max_timeout_ms: Some(2_000),
                    max_response_bytes: Some(4_096),
                    follow_redirects: Some(true),
                    max_redirects: Some(2),
                    allow_loopback: Some(true),
                    allow_link_local: None,
                    allow_private_networks: None,
                    allow_ip_literals: Some(true),
                }),
            }],
        },
    };

    let encoded = serde_json::to_value(&ctx).unwrap();
    assert_eq!(
        encoded["granted_capabilities"]["grants"][0]["id"],
        "http-request"
    );
    assert_eq!(
        encoded["granted_capabilities"]["grants"][0]["constraints"]["allowed_hosts"][0],
        "127.0.0.1"
    );
    assert_eq!(
        encoded["granted_capabilities"]["grants"][0]["constraints"]["allowed_host_suffixes"][0],
        "example.com"
    );
    assert_eq!(
        encoded["granted_capabilities"]["grants"][0]["constraints"]["follow_redirects"],
        true
    );
    assert_eq!(
        encoded["granted_capabilities"]["grants"][0]["constraints"]["allow_ip_literals"],
        true
    );
}

#[test]
fn filesystem_capability_contract_roundtrips_in_execution_context() {
    let ctx = ExecutionContext {
        execution_id: "exec-fs-1".into(),
        trace_id: "trace-fs-1".into(),
        tenant_id: "tenant-1".into(),
        skill: ResolvedSkillRef {
            key: SkillKey {
                namespace: "example".into(),
                name: "hello-inspect".into(),
            },
            version: SkillVersion::parse("0.1.0").unwrap(),
            digest: "sha256:abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
                .into(),
        },
        mode: ExecutionMode::Inspect,
        input_sha256: "sha256:abc".into(),
        now_utc: Some("2026-03-17T00:00:00Z".into()),
        budget: Budget::default(),
        granted_capabilities: CapabilityGrantSet {
            grants: vec![GrantedCapability {
                id: CapabilityId::Filesystem,
                access: CapabilityAccess::Read,
                constraints: CapabilityConstraints::Filesystem(FilesystemConstraints {
                    preopened_roots: vec![FilesystemRoot {
                        name: "workspace".into(),
                        guest_path_prefix: "/workspace".into(),
                        host_path: "/var/lib/guild/workspace".into(),
                        operations: vec![FilesystemOperation::Read],
                    }],
                }),
            }],
        },
    };

    let encoded = serde_json::to_value(&ctx).unwrap();
    assert_eq!(
        encoded["granted_capabilities"]["grants"][0]["id"],
        "filesystem"
    );
    assert_eq!(
        encoded["granted_capabilities"]["grants"][0]["constraints"]["preopened_roots"][0]["name"],
        "workspace"
    );
    assert_eq!(
        encoded["granted_capabilities"]["grants"][0]["constraints"]["preopened_roots"][0]["operations"]
            [0],
        "read"
    );
}

#[test]
fn http_request_and_response_roundtrip() {
    let request = HttpRequest {
        method: HttpMethod::Get,
        url: "http://127.0.0.1:8080/json".into(),
        timeout_ms: Some(500),
    };
    let response = HttpResponse {
        url: request.url.clone(),
        status: 200,
        content_type: Some("application/json".into()),
        body: br#"{"message":"deterministic"}"#.to_vec(),
    };

    let request_json = serde_json::to_string(&request).unwrap();
    let response_json = serde_json::to_string(&response).unwrap();

    assert!(request_json.contains("\"method\":\"get\""));
    assert!(response_json.contains("\"status\":200"));
    assert_eq!(
        serde_json::from_str::<HttpRequest>(&request_json).unwrap(),
        request
    );
    assert_eq!(
        serde_json::from_str::<HttpResponse>(&response_json).unwrap(),
        response
    );
}

#[test]
fn local_policy_config_roundtrips_with_rules() {
    let config = LocalPolicyConfig {
        format_version: guild_types::LocalPolicyFormatVersion::GuildLocalPolicyV2,
        default_profile: "trusted-networked".into(),
        profiles: vec![
            PolicyProfile {
                name: "trusted-networked".into(),
                default_action: guild_types::LocalPolicyDefaultAction::AllowRequestedDeclared,
                rules: vec![PolicyRule {
                    name: Some("deny-restricted-http".into()),
                    skills: Some(vec![SkillKey {
                        namespace: "example".into(),
                        name: "inspect-http-json".into(),
                    }]),
                    publisher_ids: Some(vec!["local.example".into()]),
                    trust_tiers: Some(vec![LocalTrustTier::Restricted]),
                    verification_states: Some(vec![InstalledVerificationState::VerifiedImport]),
                    applies_to: PolicyRuleTarget::Any,
                    effect: PolicyRuleEffect::Deny,
                    capabilities: CapabilityGrantSet {
                        grants: vec![GrantedCapability {
                            id: CapabilityId::HttpRequest,
                            access: CapabilityAccess::Read,
                            constraints: CapabilityConstraints::HttpRequest(
                                HttpRequestConstraints {
                                    allowed_schemes: Some(vec![HttpScheme::Http]),
                                    allowed_hosts: Some(vec!["127.0.0.1".into()]),
                                    allowed_host_suffixes: Some(vec!["example.com".into()]),
                                    allowed_ports: Some(vec![8080]),
                                    allowed_methods: Some(vec![HttpMethod::Get]),
                                    allowed_path_prefixes: Some(vec!["/json".into()]),
                                    max_timeout_ms: Some(2_000),
                                    max_response_bytes: Some(4_096),
                                    follow_redirects: Some(true),
                                    max_redirects: Some(2),
                                    allow_loopback: Some(true),
                                    allow_link_local: None,
                                    allow_private_networks: None,
                                    allow_ip_literals: Some(true),
                                },
                            ),
                        }],
                    },
                }],
            },
            PolicyProfile {
                name: "strict".into(),
                default_action: guild_types::LocalPolicyDefaultAction::AllowRequestedDeclared,
                rules: Vec::new(),
            },
        ],
        bindings: vec![PolicyProfileBinding {
            name: Some("prod-tenant".into()),
            actor_ids: Some(vec!["actor-1".into()]),
            tenant_ids: Some(vec!["tenant-1".into()]),
            profile: "strict".into(),
        }],
    };

    let encoded = serde_json::to_string(&config).unwrap();
    let decoded: LocalPolicyConfig = serde_json::from_str(&encoded).unwrap();

    assert_eq!(decoded, config);
    assert!(decoded.validate().is_empty());
}

#[test]
fn filesystem_constraints_require_explicit_roots_and_matching_access() {
    let vague = GrantedCapability {
        id: CapabilityId::Filesystem,
        access: CapabilityAccess::Read,
        constraints: CapabilityConstraints::none(),
    };
    let vague_errors = vague.validate();
    assert!(
        vague_errors
            .iter()
            .any(|message| message.contains("explicit filesystem constraints"))
    );

    let invalid_write = GrantedCapability {
        id: CapabilityId::Filesystem,
        access: CapabilityAccess::Write,
        constraints: CapabilityConstraints::Filesystem(FilesystemConstraints {
            preopened_roots: vec![FilesystemRoot {
                name: "workspace".into(),
                guest_path_prefix: "/workspace".into(),
                host_path: "/var/lib/guild/workspace".into(),
                operations: vec![FilesystemOperation::Read],
            }],
        }),
    };
    let invalid_write_errors = invalid_write.validate();
    assert!(
        invalid_write_errors
            .iter()
            .any(|message| message.contains("must not contain `read` when access is `write`"))
    );
}

#[test]
fn policy_decision_serializes_reasons() {
    let decision = PolicyDecision {
        outcome: PolicyDecisionOutcome::Reduced,
        summary: "local policy reduced requested capabilities".into(),
        profile_name: "trusted-networked".into(),
        trust_tier: LocalTrustTier::Restricted,
        verification_state: InstalledVerificationState::VerifiedImport,
        reasons: vec![PolicyReason {
            code: "policy-requested-capability-reduced".into(),
            message: "requested capability was narrowed to the declared surface".into(),
            detail: Some(serde_json::json!({ "id": "http-request" })),
        }],
        detail: None,
    };

    let encoded = serde_json::to_value(&decision).unwrap();

    assert_eq!(encoded["outcome"], "reduced");
    assert_eq!(encoded["profile_name"], "trusted-networked");
    assert_eq!(encoded["trust_tier"], "restricted");
    assert_eq!(encoded["verification_state"], "verified-import");
    assert_eq!(
        encoded["reasons"][0]["code"],
        "policy-requested-capability-reduced"
    );
}

#[test]
fn http_request_constraints_validate_redirect_and_host_shape() {
    let invalid = HttpRequestConstraints {
        allowed_schemes: Some(vec![HttpScheme::Http]),
        allowed_hosts: Some(vec!["".into(), "http://example.com".into()]),
        allowed_host_suffixes: Some(vec!["127.0.0.1".into(), ".example.com".into()]),
        allowed_ports: Some(vec![8080]),
        allowed_methods: Some(vec![HttpMethod::Get]),
        allowed_path_prefixes: Some(vec!["/json".into()]),
        max_timeout_ms: Some(2_000),
        max_response_bytes: Some(4_096),
        follow_redirects: Some(false),
        max_redirects: Some(2),
        allow_loopback: None,
        allow_link_local: None,
        allow_private_networks: None,
        allow_ip_literals: None,
    };

    let errors = invalid.validate();
    assert!(
        errors
            .iter()
            .any(|message| message.contains("allowed_hosts must not"))
    );
    assert!(errors.iter().any(|message| {
        message.contains("allowed_host_suffixes entries must not use raw IP literals")
    }));
    assert!(
        errors
            .iter()
            .any(|message| message.contains("max_redirects requires follow_redirects"))
    );
}
