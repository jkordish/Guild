use guild_types::{
    Budget, CapabilityAccess, CapabilityConstraints, CapabilityGrantSet, CapabilityId,
    ExecutionContext, ExecutionMode, GrantedCapability, GuildResourceScope, GuildResourceUri,
    HttpMethod, HttpRequest, HttpRequestConstraints, HttpResponse, HttpScheme,
    ReadResourceConstraints, ResolvedSkillRef, ResourceKind, SkillKey, SkillVersion,
    VersionRequirement,
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
    assert!(GuildResourceUri::parse("guild://objects/sha256/ABCDEF").is_err());
    assert!(GuildResourceUri::parse("guild://executions/%GG").is_err());
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
                    allowed_ports: Some(vec![8080]),
                    allowed_methods: Some(vec![HttpMethod::Get]),
                    allowed_path_prefixes: Some(vec!["/json".into()]),
                    max_timeout_ms: Some(2_000),
                    max_response_bytes: Some(4_096),
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
