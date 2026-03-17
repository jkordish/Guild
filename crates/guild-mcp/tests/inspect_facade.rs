use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use guild_mcp::{GuildMcpFacade, InspectRequest};
use guild_registry::{
    execution_query_resource_uri, InstalledSkill, LocalPublisherIdentity, LocalRegistry,
    LocalSourceInstaller, OciRegistryAuth, OciRegistryReference, OciRegistryTarget,
    OciRegistryTransportOptions,
};
use guild_runner::WasmtimeRuntimeAdapter;
use guild_types::{
    CapabilityAccess, CapabilityConstraints, CapabilityGrantSet, CapabilityId,
    EmitEvidenceConstraints, EvidenceAudience, ExecutionQueryResource, ExecutionQueryResult,
    ExecutionRecord, ExecutionStatus, GrantedCapability, HttpMethod, HttpRequestConstraints,
    HttpScheme, InstalledVerificationState, InvokeDependencyConstraints, LocalPolicyConfig,
    LocalTrustTier, LogConstraints, PolicyDecisionOutcome, PolicyProfile, PolicyProfileBinding,
    PolicyRule, PolicyRuleEffect, PolicyRuleTarget, ReadResourceConstraints, RedactionClass,
    RequestedSkillRef, ResourceKind, Severity, SkillKey, VersionRequirement,
};

#[path = "../../../test-support/http_test_server.rs"]
mod http_test_server;
#[path = "../../../test-support/oci_registry_test_server.rs"]
mod oci_registry_test_server;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn example_source_dir() -> PathBuf {
    repo_root().join("examples/skills/hello-inspect")
}

fn composite_source_dir() -> PathBuf {
    repo_root().join("examples/skills/hello-composite")
}

fn http_source_dir() -> PathBuf {
    repo_root().join("examples/skills/inspect-http-json")
}

fn explain_source_dir() -> PathBuf {
    repo_root().join("examples/skills/explain-execution")
}

fn summarize_query_source_dir() -> PathBuf {
    repo_root().join("examples/skills/summarize-execution-query")
}

fn wit_dir() -> PathBuf {
    repo_root().join("wit")
}

fn publisher_identity(installed: &InstalledSkill, path: &Path) -> LocalPublisherIdentity {
    let identity = LocalPublisherIdentity::generate(installed.manifest.publisher.clone()).unwrap();
    identity.save(path).unwrap();
    LocalPublisherIdentity::load(path).unwrap()
}

fn registry_reference(
    server: &oci_registry_test_server::OciRegistryTestServer,
    repository: &str,
    tag: &str,
) -> OciRegistryReference {
    OciRegistryReference {
        registry: server.registry(),
        repository: repository.into(),
        target: OciRegistryTarget::Tag(tag.into()),
    }
}

fn registry_options() -> OciRegistryTransportOptions {
    OciRegistryTransportOptions {
        auth: OciRegistryAuth::Anonymous,
        allow_http: true,
    }
}

fn prepared_registry_root() -> &'static PathBuf {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();

    ROOT.get_or_init(|| {
        let root = repo_root().join("target/test-install-registry/guild-mcp");
        if root.exists() {
            let _ = std::fs::remove_dir_all(&root);
        }

        LocalSourceInstaller::new(&root)
            .unwrap()
            .install(example_source_dir())
            .unwrap();
        LocalSourceInstaller::new(&root)
            .unwrap()
            .install(composite_source_dir())
            .unwrap();
        LocalSourceInstaller::new(&root)
            .unwrap()
            .install(http_source_dir())
            .unwrap();
        LocalSourceInstaller::new(&root)
            .unwrap()
            .install(explain_source_dir())
            .unwrap();
        LocalSourceInstaller::new(&root)
            .unwrap()
            .install(summarize_query_source_dir())
            .unwrap();

        root
    })
}

fn composite_request() -> InspectRequest {
    InspectRequest::new(
        RequestedSkillRef {
            key: SkillKey {
                namespace: "example".into(),
                name: "hello-composite".into(),
            },
            version_req: VersionRequirement::parse("^0.1").unwrap(),
        },
        serde_json::json!({ "name": "Ada" }),
        "tenant-1",
        "actor-1",
        CapabilityGrantSet {
            grants: vec![invoke_hello_grant(), emit_evidence_grant()],
        },
    )
}

fn build_facade() -> GuildMcpFacade<LocalRegistry, WasmtimeRuntimeAdapter> {
    let registry = LocalRegistry::load(prepared_registry_root()).unwrap();
    GuildMcpFacade::new(registry, WasmtimeRuntimeAdapter::new().unwrap())
}

fn build_facade_for_root(root: &Path) -> GuildMcpFacade<LocalRegistry, WasmtimeRuntimeAdapter> {
    GuildMcpFacade::new(
        LocalRegistry::load(root).unwrap(),
        WasmtimeRuntimeAdapter::new().unwrap(),
    )
}

fn install_query_test_skills(root: &Path) {
    let installer = LocalSourceInstaller::new(root).unwrap();
    installer.install(http_source_dir()).unwrap();
    installer.install(summarize_query_source_dir()).unwrap();
}

fn explain_request(uri: &str) -> InspectRequest {
    InspectRequest::new(
        RequestedSkillRef {
            key: SkillKey {
                namespace: "example".into(),
                name: "explain-execution".into(),
            },
            version_req: VersionRequirement::parse("^0.1").unwrap(),
        },
        serde_json::json!({
            "execution_uri": uri,
            "include_first_evidence": true,
        }),
        "tenant-1",
        "actor-1",
        CapabilityGrantSet {
            grants: vec![read_resource_grant()],
        },
    )
}

fn invoke_hello_grant() -> GrantedCapability {
    GrantedCapability {
        id: CapabilityId::InvokeSkill,
        access: CapabilityAccess::Invoke,
        constraints: CapabilityConstraints::InvokeDependency(InvokeDependencyConstraints {
            aliases: Some(vec!["hello".into()]),
        }),
    }
}

fn emit_evidence_grant() -> GrantedCapability {
    GrantedCapability {
        id: CapabilityId::EmitEvidence,
        access: CapabilityAccess::Write,
        constraints: CapabilityConstraints::EmitEvidence(EmitEvidenceConstraints {
            max_bytes: Some(65_536),
            audiences: Some(vec![EvidenceAudience::User]),
            redactions: Some(vec![RedactionClass::None]),
        }),
    }
}

fn read_resource_grant() -> GrantedCapability {
    GrantedCapability {
        id: CapabilityId::ReadResource,
        access: CapabilityAccess::Read,
        constraints: CapabilityConstraints::ReadResource(ReadResourceConstraints {
            uri_prefixes: Some(vec![
                "guild://executions/".into(),
                "guild://objects/records/".into(),
            ]),
            resource_kinds: Some(vec![ResourceKind::Execution, ResourceKind::Object]),
        }),
    }
}

fn log_info_grant() -> GrantedCapability {
    GrantedCapability {
        id: CapabilityId::LogWrite,
        access: CapabilityAccess::Write,
        constraints: CapabilityConstraints::Log(LogConstraints {
            levels: Some(vec![Severity::Info]),
        }),
    }
}

fn query_resource_grant() -> GrantedCapability {
    GrantedCapability {
        id: CapabilityId::ReadResource,
        access: CapabilityAccess::Read,
        constraints: CapabilityConstraints::ReadResource(ReadResourceConstraints {
            uri_prefixes: Some(vec!["guild://queries/executions/".into()]),
            resource_kinds: Some(vec![ResourceKind::Query]),
        }),
    }
}

fn summarize_query_request(query_uri: &str) -> InspectRequest {
    InspectRequest::new(
        RequestedSkillRef {
            key: SkillKey {
                namespace: "example".into(),
                name: "summarize-execution-query".into(),
            },
            version_req: VersionRequirement::parse("^0.1").unwrap(),
        },
        serde_json::json!({
            "query_uri": query_uri,
        }),
        "tenant-1",
        "actor-1",
        CapabilityGrantSet {
            grants: vec![query_resource_grant()],
        },
    )
}

fn http_grant(host: &str, port: u16, path_prefix: &str, method: HttpMethod) -> GrantedCapability {
    GrantedCapability {
        id: CapabilityId::HttpRequest,
        access: CapabilityAccess::Read,
        constraints: CapabilityConstraints::HttpRequest(HttpRequestConstraints {
            allowed_schemes: Some(vec![HttpScheme::Http]),
            allowed_hosts: Some(vec![host.to_owned()]),
            allowed_ports: Some(vec![port]),
            allowed_methods: Some(vec![method]),
            allowed_path_prefixes: Some(vec![path_prefix.to_owned()]),
            max_timeout_ms: Some(2_000),
            max_response_bytes: Some(4_096),
        }),
    }
}

struct TempFixtureDir {
    path: PathBuf,
}

impl TempFixtureDir {
    fn new(prefix: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("{prefix}-{unique}"));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempFixtureDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn copy_dir_recursive(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();

    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let file_type = entry.file_type().unwrap();
        let destination_path = destination.join(entry.file_name());

        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &destination_path);
        } else {
            fs::copy(entry.path(), destination_path).unwrap();
        }
    }
}

fn write_policy(root: &Path, policy: &LocalPolicyConfig) {
    fs::create_dir_all(root).unwrap();
    fs::write(
        root.join("policy.json"),
        serde_json::to_vec_pretty(policy).unwrap(),
    )
    .unwrap();
}

fn deny_emit_evidence_for_actor_policy(actor_id: &str) -> LocalPolicyConfig {
    LocalPolicyConfig {
        profiles: vec![
            PolicyProfile {
                name: "default".into(),
                default_action: guild_types::LocalPolicyDefaultAction::AllowRequestedDeclared,
                rules: Vec::new(),
            },
            PolicyProfile {
                name: "blocked".into(),
                default_action: guild_types::LocalPolicyDefaultAction::AllowRequestedDeclared,
                rules: vec![PolicyRule {
                    name: Some("deny-hello-evidence".into()),
                    skills: Some(vec![SkillKey {
                        namespace: "example".into(),
                        name: "hello-inspect".into(),
                    }]),
                    publisher_ids: None,
                    trust_tiers: None,
                    verification_states: None,
                    applies_to: PolicyRuleTarget::Any,
                    effect: PolicyRuleEffect::Deny,
                    capabilities: CapabilityGrantSet {
                        grants: vec![GrantedCapability {
                            id: CapabilityId::EmitEvidence,
                            access: CapabilityAccess::Write,
                            constraints: CapabilityConstraints::EmitEvidence(
                                EmitEvidenceConstraints {
                                    max_bytes: None,
                                    audiences: None,
                                    redactions: None,
                                },
                            ),
                        }],
                    },
                }],
            },
        ],
        bindings: vec![PolicyProfileBinding {
            name: Some("blocked-actor".into()),
            actor_ids: Some(vec![actor_id.into()]),
            tenant_ids: None,
            profile: "blocked".into(),
        }],
        ..LocalPolicyConfig::default()
    }
}

fn deny_http_for_restricted_imports_policy() -> LocalPolicyConfig {
    LocalPolicyConfig {
        default_profile: "trusted-networked".into(),
        profiles: vec![
            PolicyProfile {
                name: "trusted-networked".into(),
                default_action: guild_types::LocalPolicyDefaultAction::AllowRequestedDeclared,
                rules: Vec::new(),
            },
            PolicyProfile {
                name: "restricted-networked".into(),
                default_action: guild_types::LocalPolicyDefaultAction::AllowRequestedDeclared,
                rules: vec![PolicyRule {
                    name: Some("deny-restricted-http".into()),
                    skills: None,
                    publisher_ids: None,
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
                                    allowed_schemes: None,
                                    allowed_hosts: None,
                                    allowed_ports: None,
                                    allowed_methods: None,
                                    allowed_path_prefixes: None,
                                    max_timeout_ms: None,
                                    max_response_bytes: None,
                                },
                            ),
                        }],
                    },
                }],
            },
        ],
        bindings: vec![PolicyProfileBinding {
            name: Some("restricted-tenant".into()),
            actor_ids: None,
            tenant_ids: Some(vec!["tenant-restricted".into()]),
            profile: "restricted-networked".into(),
        }],
        ..LocalPolicyConfig::default()
    }
}

#[test]
fn guild_inspect_uses_real_registry_and_runner_path() {
    let facade = build_facade();
    let request = InspectRequest::new(
        RequestedSkillRef {
            key: SkillKey {
                namespace: "example".into(),
                name: "hello-inspect".into(),
            },
            version_req: VersionRequirement::parse("^0.1").unwrap(),
        },
        serde_json::json!({ "name": "Ada" }),
        "tenant-1",
        "actor-1",
        CapabilityGrantSet {
            grants: vec![emit_evidence_grant(), log_info_grant()],
        },
    );

    let response = facade.inspect(request).unwrap();
    let stored = facade
        .read_resource(&response.structured_content.receipt.uri)
        .unwrap();
    let output = response.structured_content.output.as_ref().unwrap();

    assert_eq!(response.summary, output.summary);
    assert_eq!(output.structured["mode"], "inspect");
    assert_eq!(
        response
            .structured_content
            .provenance
            .resolved_skill
            .key
            .name,
        "hello-inspect"
    );
    assert_eq!(stored.mime_type, "application/json");
}

#[test]
fn guild_inspect_executes_composite_skill_through_nested_path() {
    let facade = build_facade();
    let response = facade.inspect(composite_request()).unwrap();
    let parent_resource = facade
        .read_resource(&response.structured_content.receipt.uri)
        .unwrap();
    let child_resource = facade
        .read_resource(&response.structured_content.child_executions[0].uri)
        .unwrap();
    let output = response.structured_content.output.as_ref().unwrap();

    assert_eq!(response.summary, output.summary);
    assert_eq!(output.structured["invoked_alias"], "hello");
    assert_eq!(response.structured_content.child_executions.len(), 1);
    assert_eq!(
        response.structured_content.child_executions[0]
            .provenance
            .resolved_skill
            .key
            .name,
        "hello-inspect"
    );
    assert_eq!(parent_resource.mime_type, "application/json");
    assert_eq!(child_resource.mime_type, "application/json");
    let child_record: guild_types::ExecutionRecord =
        serde_json::from_slice(&child_resource.bytes).unwrap();
    let evidence_resource = facade
        .read_resource(&child_record.emitted_evidence[0].uri)
        .unwrap();
    assert_eq!(evidence_resource.mime_type, "application/json");
}

#[test]
fn guild_inspect_executes_http_skill_through_real_host_path() {
    let server = http_test_server::HttpTestServer::start();
    let facade = build_facade();
    let response = facade
        .inspect(InspectRequest::new(
            RequestedSkillRef {
                key: SkillKey {
                    namespace: "example".into(),
                    name: "inspect-http-json".into(),
                },
                version_req: VersionRequirement::parse("^0.1").unwrap(),
            },
            serde_json::json!({
                "url": server.json_url(),
                "method": "get",
                "json_pointers": ["/message", "/nested/count"],
            }),
            "tenant-1",
            "actor-1",
            CapabilityGrantSet {
                grants: vec![http_grant(
                    http_test_server::HttpTestServer::host(),
                    server.port(),
                    "/json",
                    HttpMethod::Get,
                )],
            },
        ))
        .unwrap();
    let output = response.structured_content.output.as_ref().unwrap();

    assert_eq!(
        response.structured_content.status,
        ExecutionStatus::Succeeded
    );
    assert_eq!(response.structured_content.metrics.network_requests, 1);
    assert_eq!(output.structured["status"], 200);
    assert_eq!(
        output.structured["selected_fields"][0]["value"],
        serde_json::json!("deterministic")
    );
}

#[test]
fn local_policy_reduces_requested_capabilities_before_execution() {
    let temp = TempFixtureDir::new("guild-policy-reduced");
    let registry_root = temp.path().join("registry");
    LocalSourceInstaller::new(&registry_root)
        .unwrap()
        .install(example_source_dir())
        .unwrap();

    let facade = build_facade_for_root(&registry_root);
    let response = facade
        .inspect(InspectRequest::new(
            RequestedSkillRef {
                key: SkillKey {
                    namespace: "example".into(),
                    name: "hello-inspect".into(),
                },
                version_req: VersionRequirement::parse("^0.1").unwrap(),
            },
            serde_json::json!({ "name": "Ada" }),
            "tenant-1",
            "actor-1",
            CapabilityGrantSet {
                grants: vec![
                    emit_evidence_grant(),
                    log_info_grant(),
                    http_grant("example.com", 80, "/", HttpMethod::Get),
                ],
            },
        ))
        .unwrap();

    assert_eq!(
        response.structured_content.policy_decision.outcome,
        PolicyDecisionOutcome::Reduced
    );
    assert!(response
        .structured_content
        .granted_capabilities
        .grants
        .iter()
        .all(|grant| grant.id != CapabilityId::HttpRequest));
    assert!(response
        .structured_content
        .request
        .requested_capabilities
        .grants
        .iter()
        .any(|grant| grant.id == CapabilityId::HttpRequest));
}

#[test]
fn local_policy_denial_persists_host_owned_rejection() {
    let temp = TempFixtureDir::new("guild-policy-denied");
    let registry_root = temp.path().join("registry");
    LocalSourceInstaller::new(&registry_root)
        .unwrap()
        .install(example_source_dir())
        .unwrap();
    write_policy(
        &registry_root,
        &deny_emit_evidence_for_actor_policy("actor-blocked"),
    );

    let facade = build_facade_for_root(&registry_root);
    let error = facade
        .inspect(InspectRequest::new(
            RequestedSkillRef {
                key: SkillKey {
                    namespace: "example".into(),
                    name: "hello-inspect".into(),
                },
                version_req: VersionRequirement::parse("^0.1").unwrap(),
            },
            serde_json::json!({ "name": "Ada" }),
            "tenant-1",
            "actor-blocked",
            CapabilityGrantSet {
                grants: vec![emit_evidence_grant()],
            },
        ))
        .unwrap_err();

    assert_eq!(error.code, "policy-denied");
    let receipt = error.receipt.expect("policy denial persists a receipt");
    let record: guild_types::ExecutionRecord =
        serde_json::from_slice(&facade.read_resource(&receipt.uri).unwrap().bytes).unwrap();

    assert_eq!(record.status, ExecutionStatus::Rejected);
    assert_eq!(
        record.policy_decision.outcome,
        PolicyDecisionOutcome::Rejected
    );
    assert_eq!(record.policy_decision.profile_name, "blocked");
    assert_eq!(
        record.policy_decision.verification_state,
        InstalledVerificationState::LocalSource
    );
    assert!(record
        .policy_decision
        .reasons
        .iter()
        .any(|reason| reason.code == "policy-profile-rule-deny"));
    assert!(record
        .policy_decision
        .reasons
        .iter()
        .any(|reason| reason.code == "policy-required-capability-missing"));
    assert!(record.granted_capabilities.grants.is_empty());
}

#[allow(clippy::too_many_lines)]
#[test]
fn local_policy_can_vary_http_by_imported_trust_tier() {
    let server = http_test_server::HttpTestServer::start();
    let temp = TempFixtureDir::new("guild-policy-trust-tier-http");
    let registry_a = temp.path().join("registry-a");
    let registry_trusted = temp.path().join("registry-trusted");
    let registry_restricted = temp.path().join("registry-restricted");
    let bundle_root = temp.path().join("bundle");

    let source_installer = LocalSourceInstaller::new(&registry_a).unwrap();
    let installed_skill = source_installer.install(http_source_dir()).unwrap();
    let identity = publisher_identity(&installed_skill, &temp.path().join("publisher.json"));
    let registry = LocalRegistry::load(&registry_a).unwrap();
    registry
        .export_bundle(
            &installed_skill.resolved_ref,
            false,
            &bundle_root,
            &identity,
        )
        .unwrap();

    LocalRegistry::trust_publisher(
        &registry_trusted,
        &identity.trusted_record_with_tier(LocalTrustTier::TrustedImported),
    )
    .unwrap();
    LocalRegistry::trust_publisher(
        &registry_restricted,
        &identity.trusted_record_with_tier(LocalTrustTier::Restricted),
    )
    .unwrap();
    LocalRegistry::import_bundle(&registry_trusted, &bundle_root).unwrap();
    LocalRegistry::import_bundle(&registry_restricted, &bundle_root).unwrap();

    let policy = deny_http_for_restricted_imports_policy();
    write_policy(&registry_trusted, &policy);
    write_policy(&registry_restricted, &policy);

    let trusted_facade = build_facade_for_root(&registry_trusted);
    let trusted = trusted_facade
        .inspect(InspectRequest::new(
            RequestedSkillRef {
                key: SkillKey {
                    namespace: "example".into(),
                    name: "inspect-http-json".into(),
                },
                version_req: VersionRequirement::parse("^0.1").unwrap(),
            },
            serde_json::json!({
                "url": server.json_url(),
                "method": "get",
                "json_pointers": ["/message"],
            }),
            "tenant-1",
            "actor-1",
            CapabilityGrantSet {
                grants: vec![http_grant(
                    http_test_server::HttpTestServer::host(),
                    server.port(),
                    "/json",
                    HttpMethod::Get,
                )],
            },
        ))
        .unwrap();
    assert_eq!(
        trusted.structured_content.policy_decision.outcome,
        PolicyDecisionOutcome::Allowed
    );
    assert_eq!(
        trusted.structured_content.policy_decision.profile_name,
        "trusted-networked"
    );
    assert_eq!(
        trusted.structured_content.policy_decision.trust_tier,
        LocalTrustTier::TrustedImported
    );
    assert_eq!(
        trusted
            .structured_content
            .policy_decision
            .verification_state,
        InstalledVerificationState::VerifiedImport
    );

    let restricted_facade = build_facade_for_root(&registry_restricted);
    let denied = restricted_facade
        .inspect(InspectRequest::new(
            RequestedSkillRef {
                key: SkillKey {
                    namespace: "example".into(),
                    name: "inspect-http-json".into(),
                },
                version_req: VersionRequirement::parse("^0.1").unwrap(),
            },
            serde_json::json!({
                "url": server.json_url(),
                "method": "get",
                "json_pointers": ["/message"],
            }),
            "tenant-restricted",
            "actor-1",
            CapabilityGrantSet {
                grants: vec![http_grant(
                    http_test_server::HttpTestServer::host(),
                    server.port(),
                    "/json",
                    HttpMethod::Get,
                )],
            },
        ))
        .unwrap_err();

    assert_eq!(denied.code, "policy-denied");
    let receipt = denied
        .receipt
        .expect("trust-tier-aware policy denial persists a receipt");
    let record: guild_types::ExecutionRecord =
        serde_json::from_slice(&restricted_facade.read_resource(&receipt.uri).unwrap().bytes)
            .unwrap();
    assert_eq!(record.policy_decision.profile_name, "restricted-networked");
    assert_eq!(
        record.policy_decision.trust_tier,
        LocalTrustTier::Restricted
    );
    assert_eq!(
        record.policy_decision.verification_state,
        InstalledVerificationState::VerifiedImport
    );
    assert!(record
        .policy_decision
        .reasons
        .iter()
        .any(|reason| reason.code == "policy-profile-rule-deny"));
}

#[test]
fn missing_resource_read_fails_closed() {
    let facade = build_facade();
    let error = facade
        .read_resource("guild://executions/does-not-exist")
        .unwrap_err();

    assert_eq!(error.code, "execution-not-found");
}

#[test]
fn explain_skill_reads_the_same_resources_mcp_exposes() {
    let facade = build_facade();
    let primitive = facade
        .inspect(InspectRequest::new(
            RequestedSkillRef {
                key: SkillKey {
                    namespace: "example".into(),
                    name: "hello-inspect".into(),
                },
                version_req: VersionRequirement::parse("^0.1").unwrap(),
            },
            serde_json::json!({ "name": "Ada" }),
            "tenant-1",
            "actor-1",
            CapabilityGrantSet {
                grants: vec![emit_evidence_grant()],
            },
        ))
        .unwrap();

    let execution_resource = facade
        .read_resource(&primitive.structured_content.receipt.uri)
        .unwrap();
    let evidence_resource = facade
        .read_resource(&primitive.structured_content.emitted_evidence[0].uri)
        .unwrap();
    let explained = facade
        .inspect(explain_request(&primitive.structured_content.receipt.uri))
        .unwrap();
    let explained_output = explained.structured_content.output.as_ref().unwrap();
    let primitive_output = primitive.structured_content.output.as_ref().unwrap();

    assert_eq!(
        explained_output.structured["target_execution_uri"],
        primitive.structured_content.receipt.uri
    );
    assert_eq!(
        explained_output.structured["execution_resource"]["sha256"],
        execution_resource.sha256.unwrap()
    );
    assert_eq!(
        explained_output.structured["first_evidence"]["sha256"],
        evidence_resource.sha256.unwrap()
    );
    assert_eq!(
        explained_output.structured["stored_summary"],
        primitive_output.summary
    );
}

#[test]
fn explain_skill_can_summarize_persisted_http_denials() {
    let server = http_test_server::HttpTestServer::start();
    let facade = build_facade();
    let denied = facade
        .inspect(InspectRequest::new(
            RequestedSkillRef {
                key: SkillKey {
                    namespace: "example".into(),
                    name: "inspect-http-json".into(),
                },
                version_req: VersionRequirement::parse("^0.1").unwrap(),
            },
            serde_json::json!({
                "url": server.json_url(),
                "method": "get",
                "json_pointers": ["/message"],
            }),
            "tenant-1",
            "actor-1",
            CapabilityGrantSet {
                grants: vec![http_grant(
                    "localhost",
                    server.port(),
                    "/json",
                    HttpMethod::Get,
                )],
            },
        ))
        .unwrap_err();

    assert_eq!(denied.code, "http-request-host-not-granted");
    let receipt = denied.receipt.expect("HTTP denial persists a receipt");
    let explained = facade.inspect(explain_request(&receipt.uri)).unwrap();
    let explained_output = explained.structured_content.output.as_ref().unwrap();

    assert_eq!(explained_output.structured["target_status"], "rejected");
    assert_eq!(
        explained_output.structured["termination"]["code"],
        "http-request-host-not-granted"
    );
}

#[test]
fn explain_skill_can_summarize_persisted_policy_denials() {
    let temp = TempFixtureDir::new("guild-policy-explain-denial");
    let registry_root = temp.path().join("registry");
    LocalSourceInstaller::new(&registry_root)
        .unwrap()
        .install(example_source_dir())
        .unwrap();
    LocalSourceInstaller::new(&registry_root)
        .unwrap()
        .install(explain_source_dir())
        .unwrap();
    write_policy(
        &registry_root,
        &deny_emit_evidence_for_actor_policy("actor-blocked"),
    );

    let facade = build_facade_for_root(&registry_root);
    let denied = facade
        .inspect(InspectRequest::new(
            RequestedSkillRef {
                key: SkillKey {
                    namespace: "example".into(),
                    name: "hello-inspect".into(),
                },
                version_req: VersionRequirement::parse("^0.1").unwrap(),
            },
            serde_json::json!({ "name": "Ada" }),
            "tenant-1",
            "actor-blocked",
            CapabilityGrantSet {
                grants: vec![emit_evidence_grant()],
            },
        ))
        .unwrap_err();

    let receipt = denied.receipt.expect("policy denial persists a receipt");
    let explained = facade.inspect(explain_request(&receipt.uri)).unwrap();
    let explained_output = explained.structured_content.output.as_ref().unwrap();

    assert_eq!(explained_output.structured["target_status"], "rejected");
    assert_eq!(
        explained_output.structured["termination"]["code"],
        "policy-denied"
    );
    assert_eq!(
        explained_output.structured["policy_decision"]["outcome"],
        "rejected"
    );
}

#[test]
fn local_policy_can_further_reduce_child_grants_without_widening() {
    let temp = TempFixtureDir::new("guild-policy-child");
    let registry_root = temp.path().join("registry");
    LocalSourceInstaller::new(&registry_root)
        .unwrap()
        .install(example_source_dir())
        .unwrap();
    LocalSourceInstaller::new(&registry_root)
        .unwrap()
        .install(composite_source_dir())
        .unwrap();
    write_policy(
        &registry_root,
        &deny_emit_evidence_for_actor_policy("skill"),
    );

    let facade = build_facade_for_root(&registry_root);
    let error = facade.inspect(composite_request()).unwrap_err();

    assert_eq!(error.code, "child-invocation-failed");
    let receipt = error.receipt.expect("composite failure persists a receipt");
    let parent: guild_types::ExecutionRecord =
        serde_json::from_slice(&facade.read_resource(&receipt.uri).unwrap().bytes).unwrap();

    assert_eq!(parent.status, ExecutionStatus::Failed);
    assert_eq!(parent.child_executions.len(), 1);
    assert!(parent
        .granted_capabilities
        .grants
        .iter()
        .any(|grant| grant.id == CapabilityId::EmitEvidence));
    assert_eq!(
        parent.child_executions[0].policy_decision.outcome,
        PolicyDecisionOutcome::Rejected
    );
    assert!(parent.child_executions[0]
        .policy_decision
        .reasons
        .iter()
        .any(|reason| reason.code == "policy-profile-rule-deny"));
    assert!(parent.child_executions[0]
        .granted_capabilities
        .grants
        .iter()
        .all(|grant| grant.id != CapabilityId::EmitEvidence));
}

#[test]
fn mcp_can_read_persisted_rejected_execution_resources() {
    let facade = build_facade();
    let error = facade
        .inspect(InspectRequest::new(
            RequestedSkillRef {
                key: SkillKey {
                    namespace: "example".into(),
                    name: "explain-execution".into(),
                },
                version_req: VersionRequirement::parse("^0.1").unwrap(),
            },
            serde_json::json!({
                "execution_uri": "guild://executions/does-not-matter",
                "include_first_evidence": false,
            }),
            "tenant-1",
            "actor-1",
            CapabilityGrantSet::default(),
        ))
        .unwrap_err();

    assert_eq!(error.code, "policy-denied");
    let receipt = error.receipt.expect("rejected execution exposes a receipt");
    let resource = facade.read_resource(&receipt.uri).unwrap();
    let record: guild_types::ExecutionRecord = serde_json::from_slice(&resource.bytes).unwrap();
    assert_eq!(record.status, ExecutionStatus::Rejected);
    assert!(record.output.is_none());
}

#[test]
fn mcp_can_read_persisted_failed_execution_resources() {
    let facade = build_facade();
    let error = facade
        .inspect(InspectRequest::new(
            RequestedSkillRef {
                key: SkillKey {
                    namespace: "example".into(),
                    name: "hello-inspect".into(),
                },
                version_req: VersionRequirement::parse("^0.1").unwrap(),
            },
            serde_json::json!({ "name": "Ada", "emit_log": true }),
            "tenant-1",
            "actor-1",
            CapabilityGrantSet {
                grants: vec![emit_evidence_grant()],
            },
        ))
        .unwrap_err();

    assert_eq!(error.code, "log-write-not-granted");
    let receipt = error.receipt.expect("failed execution exposes a receipt");
    let resource = facade.read_resource(&receipt.uri).unwrap();
    let record: guild_types::ExecutionRecord = serde_json::from_slice(&resource.bytes).unwrap();
    assert_eq!(record.status, ExecutionStatus::Rejected);
    assert!(record.output.is_none());
}

#[test]
fn mcp_can_read_bounded_execution_query_resources() {
    let temp = TempFixtureDir::new("guild-query-read");
    install_query_test_skills(temp.path());
    let server = http_test_server::HttpTestServer::start();
    let facade = build_facade_for_root(temp.path());

    facade
        .inspect(InspectRequest::new(
            RequestedSkillRef {
                key: SkillKey {
                    namespace: "example".into(),
                    name: "inspect-http-json".into(),
                },
                version_req: VersionRequirement::parse("^0.1").unwrap(),
            },
            serde_json::json!({
                "url": server.json_url(),
                "method": "get",
            }),
            "tenant-1",
            "actor-1",
            CapabilityGrantSet {
                grants: vec![http_grant(
                    http_test_server::HttpTestServer::host(),
                    server.port(),
                    "/json",
                    HttpMethod::Get,
                )],
            },
        ))
        .unwrap();

    let failed = facade
        .inspect(InspectRequest::new(
            RequestedSkillRef {
                key: SkillKey {
                    namespace: "example".into(),
                    name: "inspect-http-json".into(),
                },
                version_req: VersionRequirement::parse("^0.1").unwrap(),
            },
            serde_json::json!({
                "url": server.json_url(),
                "method": "post",
            }),
            "tenant-1",
            "actor-1",
            CapabilityGrantSet {
                grants: vec![http_grant(
                    http_test_server::HttpTestServer::host(),
                    server.port(),
                    "/json",
                    HttpMethod::Get,
                )],
            },
        ))
        .unwrap_err();
    let rejected = facade
        .inspect(InspectRequest::new(
            RequestedSkillRef {
                key: SkillKey {
                    namespace: "example".into(),
                    name: "inspect-http-json".into(),
                },
                version_req: VersionRequirement::parse("^0.1").unwrap(),
            },
            serde_json::json!({
                "url": server.json_url(),
                "method": "get",
            }),
            "tenant-1",
            "actor-1",
            CapabilityGrantSet::default(),
        ))
        .unwrap_err();

    let query = ExecutionQueryResource::FailuresRecent { limit: 10 };
    let resource = facade
        .read_resource(execution_query_resource_uri(&query))
        .unwrap();
    let result: ExecutionQueryResult = serde_json::from_slice(&resource.bytes).unwrap();

    assert_eq!(result.query_uri, execution_query_resource_uri(&query));
    assert_eq!(result.total_matches, 2);
    assert_eq!(result.returned_matches, 2);
    assert_eq!(result.results[0].status, ExecutionStatus::Rejected);
    assert_eq!(result.results[1].status, ExecutionStatus::Failed);
    assert_eq!(
        result.results[0].receipt.uri,
        rejected.receipt.as_ref().unwrap().uri
    );
    assert_eq!(
        result.results[1].receipt.uri,
        failed.receipt.as_ref().unwrap().uri
    );
}

#[test]
fn summarize_query_skill_uses_the_same_query_backend_as_direct_reads() {
    let temp = TempFixtureDir::new("guild-query-skill");
    install_query_test_skills(temp.path());
    let server = http_test_server::HttpTestServer::start();
    let facade = build_facade_for_root(temp.path());

    facade
        .inspect(InspectRequest::new(
            RequestedSkillRef {
                key: SkillKey {
                    namespace: "example".into(),
                    name: "inspect-http-json".into(),
                },
                version_req: VersionRequirement::parse("^0.1").unwrap(),
            },
            serde_json::json!({
                "url": server.json_url(),
                "method": "post",
            }),
            "tenant-1",
            "actor-1",
            CapabilityGrantSet {
                grants: vec![http_grant(
                    http_test_server::HttpTestServer::host(),
                    server.port(),
                    "/json",
                    HttpMethod::Get,
                )],
            },
        ))
        .unwrap_err();
    facade
        .inspect(InspectRequest::new(
            RequestedSkillRef {
                key: SkillKey {
                    namespace: "example".into(),
                    name: "inspect-http-json".into(),
                },
                version_req: VersionRequirement::parse("^0.1").unwrap(),
            },
            serde_json::json!({
                "url": server.json_url(),
                "method": "get",
            }),
            "tenant-1",
            "actor-1",
            CapabilityGrantSet::default(),
        ))
        .unwrap_err();

    let query = ExecutionQueryResource::FailuresRecent { limit: 10 };
    let query_uri = execution_query_resource_uri(&query);
    let direct: ExecutionQueryResult =
        serde_json::from_slice(&facade.read_resource(&query_uri).unwrap().bytes).unwrap();

    let response = facade.inspect(summarize_query_request(&query_uri)).unwrap();
    let report = &response
        .structured_content
        .output
        .as_ref()
        .unwrap()
        .structured;

    assert_eq!(report["query_uri"], query_uri);
    assert_eq!(report["total_matches"], direct.total_matches);
    assert_eq!(report["returned_matches"], direct.returned_matches);
    assert_eq!(report["truncated"], direct.truncated);
    assert_eq!(report["status_counts"][0]["count"], 1);
    assert_eq!(report["status_counts"][1]["count"], 1);
    assert_eq!(
        report["notable_execution_uris"][0],
        direct.results[0].receipt.uri
    );
}

#[test]
fn query_resource_reads_require_query_scope() {
    let temp = TempFixtureDir::new("guild-query-auth");
    install_query_test_skills(temp.path());
    let query_uri = execution_query_resource_uri(&ExecutionQueryResource::Recent { limit: 10 });
    let facade = build_facade_for_root(temp.path());

    let error = facade
        .inspect(InspectRequest::new(
            RequestedSkillRef {
                key: SkillKey {
                    namespace: "example".into(),
                    name: "summarize-execution-query".into(),
                },
                version_req: VersionRequirement::parse("^0.1").unwrap(),
            },
            serde_json::json!({
                "query_uri": query_uri,
            }),
            "tenant-1",
            "actor-1",
            CapabilityGrantSet {
                grants: vec![read_resource_grant()],
            },
        ))
        .unwrap_err();

    assert_eq!(error.code, "policy-denied");
    let receipt = error
        .receipt
        .expect("missing query scope should still persist a rejected execution");
    let resource = facade.read_resource(&receipt.uri).unwrap();
    let record: ExecutionRecord = serde_json::from_slice(&resource.bytes).unwrap();
    assert_eq!(record.status, ExecutionStatus::Rejected);
    assert_eq!(record.termination.as_ref().unwrap().code, "policy-denied");
    assert!(record
        .policy_decision
        .reasons
        .iter()
        .any(|reason| reason.code == "policy-required-capability-missing"));
}

#[test]
fn imported_primitive_bundle_executes_without_source_workspace() {
    let temp = TempFixtureDir::new("guild-portable-primitive");
    let workspace_root = temp.path().join("workspace");
    let source_root = workspace_root.join("examples/skills/hello-inspect");
    let registry_a = temp.path().join("registry-a");
    let bundle_root = temp.path().join("bundle");
    let registry_b = temp.path().join("registry-b");

    copy_dir_recursive(&example_source_dir(), &source_root);
    copy_dir_recursive(&wit_dir(), &workspace_root.join("wit"));

    let source_installer = LocalSourceInstaller::new(&registry_a).unwrap();
    let installed_skill = source_installer.install(&source_root).unwrap();
    let identity = publisher_identity(&installed_skill, &temp.path().join("publisher.json"));
    let registry = LocalRegistry::load(&registry_a).unwrap();
    registry
        .export_bundle(
            &installed_skill.resolved_ref,
            false,
            &bundle_root,
            &identity,
        )
        .unwrap();

    fs::remove_dir_all(&workspace_root).unwrap();

    LocalRegistry::trust_publisher(&registry_b, &identity.trusted_record()).unwrap();
    LocalRegistry::import_bundle(&registry_b, &bundle_root).unwrap();
    let facade = GuildMcpFacade::new(
        LocalRegistry::load(&registry_b).unwrap(),
        WasmtimeRuntimeAdapter::new().unwrap(),
    );
    let response = facade
        .inspect(InspectRequest::new(
            RequestedSkillRef {
                key: SkillKey {
                    namespace: "example".into(),
                    name: "hello-inspect".into(),
                },
                version_req: VersionRequirement::parse("^0.1").unwrap(),
            },
            serde_json::json!({ "name": "Ada" }),
            "tenant-1",
            "actor-1",
            CapabilityGrantSet {
                grants: vec![emit_evidence_grant()],
            },
        ))
        .unwrap();

    assert_eq!(
        response.structured_content.status,
        ExecutionStatus::Succeeded
    );
    assert_eq!(
        response.structured_content.provenance.resolved_skill.digest,
        installed_skill.resolved_ref.digest
    );
    assert_eq!(
        facade
            .read_resource(&response.structured_content.receipt.uri)
            .unwrap()
            .mime_type,
        "application/json"
    );
}

#[test]
fn imported_primitive_oci_layout_executes_without_source_workspace() {
    let temp = TempFixtureDir::new("guild-portable-primitive-oci");
    let workspace_root = temp.path().join("workspace");
    let source_root = workspace_root.join("examples/skills/hello-inspect");
    let registry_a = temp.path().join("registry-a");
    let layout_root = temp.path().join("oci-layout");
    let registry_b = temp.path().join("registry-b");

    copy_dir_recursive(&example_source_dir(), &source_root);
    copy_dir_recursive(&wit_dir(), &workspace_root.join("wit"));

    let source_installer = LocalSourceInstaller::new(&registry_a).unwrap();
    let installed_skill = source_installer.install(&source_root).unwrap();
    let identity = publisher_identity(&installed_skill, &temp.path().join("publisher.json"));
    let registry = LocalRegistry::load(&registry_a).unwrap();
    registry
        .export_oci_layout(
            &installed_skill.resolved_ref,
            false,
            &layout_root,
            &identity,
        )
        .unwrap();

    fs::remove_dir_all(&workspace_root).unwrap();

    LocalRegistry::trust_publisher(&registry_b, &identity.trusted_record()).unwrap();
    LocalRegistry::import_oci_layout(&registry_b, &layout_root).unwrap();
    let facade = GuildMcpFacade::new(
        LocalRegistry::load(&registry_b).unwrap(),
        WasmtimeRuntimeAdapter::new().unwrap(),
    );
    let response = facade
        .inspect(InspectRequest::new(
            RequestedSkillRef {
                key: SkillKey {
                    namespace: "example".into(),
                    name: "hello-inspect".into(),
                },
                version_req: VersionRequirement::parse("^0.1").unwrap(),
            },
            serde_json::json!({ "name": "Ada" }),
            "tenant-1",
            "actor-1",
            CapabilityGrantSet {
                grants: vec![emit_evidence_grant()],
            },
        ))
        .unwrap();

    assert_eq!(
        response.structured_content.status,
        ExecutionStatus::Succeeded
    );
    assert_eq!(
        response.structured_content.provenance.resolved_skill.digest,
        installed_skill.resolved_ref.digest
    );
    assert_eq!(
        facade
            .read_resource(&response.structured_content.receipt.uri)
            .unwrap()
            .mime_type,
        "application/json"
    );
}

#[test]
fn pulled_primitive_oci_registry_executes_without_source_workspace() {
    let temp = TempFixtureDir::new("guild-portable-primitive-oci-registry");
    let workspace_root = temp.path().join("workspace");
    let source_root = workspace_root.join("examples/skills/hello-inspect");
    let registry_a = temp.path().join("registry-a");
    let registry_b = temp.path().join("registry-b");

    copy_dir_recursive(&example_source_dir(), &source_root);
    copy_dir_recursive(&wit_dir(), &workspace_root.join("wit"));

    let source_installer = LocalSourceInstaller::new(&registry_a).unwrap();
    let installed_skill = source_installer.install(&source_root).unwrap();
    let identity = publisher_identity(&installed_skill, &temp.path().join("publisher.json"));
    let registry = LocalRegistry::load(&registry_a).unwrap();
    let server = oci_registry_test_server::OciRegistryTestServer::start(
        temp.path().join("oci-registry-store"),
    );
    let reference = registry_reference(&server, "guild-example-hello-inspect", "0.1.0");

    registry
        .push_oci_registry(
            &installed_skill.resolved_ref,
            false,
            &reference,
            &registry_options(),
            &identity,
        )
        .unwrap();

    fs::remove_dir_all(&workspace_root).unwrap();
    fs::remove_dir_all(&registry_a).unwrap();

    LocalRegistry::trust_publisher(&registry_b, &identity.trusted_record()).unwrap();
    LocalRegistry::pull_oci_registry(&registry_b, &reference, &registry_options()).unwrap();
    let facade = GuildMcpFacade::new(
        LocalRegistry::load(&registry_b).unwrap(),
        WasmtimeRuntimeAdapter::new().unwrap(),
    );
    let response = facade
        .inspect(InspectRequest::new(
            RequestedSkillRef {
                key: SkillKey {
                    namespace: "example".into(),
                    name: "hello-inspect".into(),
                },
                version_req: VersionRequirement::parse("^0.1").unwrap(),
            },
            serde_json::json!({ "name": "Ada" }),
            "tenant-1",
            "actor-1",
            CapabilityGrantSet {
                grants: vec![emit_evidence_grant()],
            },
        ))
        .unwrap();

    assert_eq!(
        response.structured_content.status,
        ExecutionStatus::Succeeded
    );
    assert_eq!(
        response.structured_content.provenance.resolved_skill.digest,
        installed_skill.resolved_ref.digest
    );
    assert_eq!(
        facade
            .read_resource(&response.structured_content.receipt.uri)
            .unwrap()
            .mime_type,
        "application/json"
    );
}

#[test]
fn imported_composite_bundle_executes_through_normal_nested_path() {
    let temp = TempFixtureDir::new("guild-portable-composite");
    let registry_a = temp.path().join("registry-a");
    let bundle_root = temp.path().join("bundle");
    let registry_b = temp.path().join("registry-b");

    let source_installer = LocalSourceInstaller::new(&registry_a).unwrap();
    let primitive = source_installer.install(example_source_dir()).unwrap();
    let composite = source_installer.install(composite_source_dir()).unwrap();
    let identity = publisher_identity(&composite, &temp.path().join("publisher.json"));
    let registry = LocalRegistry::load(&registry_a).unwrap();
    registry
        .export_bundle(&composite.resolved_ref, true, &bundle_root, &identity)
        .unwrap();

    fs::remove_dir_all(&registry_a).unwrap();

    LocalRegistry::trust_publisher(&registry_b, &identity.trusted_record()).unwrap();
    LocalRegistry::import_bundle(&registry_b, &bundle_root).unwrap();
    let facade = GuildMcpFacade::new(
        LocalRegistry::load(&registry_b).unwrap(),
        WasmtimeRuntimeAdapter::new().unwrap(),
    );
    let response = facade.inspect(composite_request()).unwrap();

    assert_eq!(
        response.structured_content.status,
        ExecutionStatus::Succeeded
    );
    assert_eq!(
        response.structured_content.provenance.resolved_skill.digest,
        composite.resolved_ref.digest
    );
    assert_eq!(response.structured_content.child_executions.len(), 1);
    assert_eq!(
        response.structured_content.child_executions[0]
            .provenance
            .resolved_skill
            .digest,
        primitive.resolved_ref.digest
    );
    let child_resource = facade
        .read_resource(&response.structured_content.child_executions[0].uri)
        .unwrap();
    assert_eq!(child_resource.mime_type, "application/json");
}

#[test]
fn imported_composite_oci_layout_executes_through_normal_nested_path() {
    let temp = TempFixtureDir::new("guild-portable-composite-oci");
    let registry_a = temp.path().join("registry-a");
    let layout_root = temp.path().join("oci-layout");
    let registry_b = temp.path().join("registry-b");

    let source_installer = LocalSourceInstaller::new(&registry_a).unwrap();
    let primitive = source_installer.install(example_source_dir()).unwrap();
    let composite = source_installer.install(composite_source_dir()).unwrap();
    let identity = publisher_identity(&composite, &temp.path().join("publisher.json"));
    let registry = LocalRegistry::load(&registry_a).unwrap();
    registry
        .export_oci_layout(&composite.resolved_ref, true, &layout_root, &identity)
        .unwrap();

    fs::remove_dir_all(&registry_a).unwrap();

    LocalRegistry::trust_publisher(&registry_b, &identity.trusted_record()).unwrap();
    LocalRegistry::import_oci_layout(&registry_b, &layout_root).unwrap();
    let facade = GuildMcpFacade::new(
        LocalRegistry::load(&registry_b).unwrap(),
        WasmtimeRuntimeAdapter::new().unwrap(),
    );
    let response = facade.inspect(composite_request()).unwrap();

    assert_eq!(
        response.structured_content.status,
        ExecutionStatus::Succeeded
    );
    assert_eq!(
        response.structured_content.provenance.resolved_skill.digest,
        composite.resolved_ref.digest
    );
    assert_eq!(response.structured_content.child_executions.len(), 1);
    assert_eq!(
        response.structured_content.child_executions[0]
            .provenance
            .resolved_skill
            .digest,
        primitive.resolved_ref.digest
    );
    let child_resource = facade
        .read_resource(&response.structured_content.child_executions[0].uri)
        .unwrap();
    assert_eq!(child_resource.mime_type, "application/json");
}

#[test]
fn pulled_composite_oci_registry_executes_through_normal_nested_path() {
    let temp = TempFixtureDir::new("guild-portable-composite-oci-registry");
    let registry_a = temp.path().join("registry-a");
    let registry_store = temp.path().join("oci-registry-store");
    let registry_b = temp.path().join("registry-b");

    let source_installer = LocalSourceInstaller::new(&registry_a).unwrap();
    let primitive = source_installer.install(example_source_dir()).unwrap();
    let composite = source_installer.install(composite_source_dir()).unwrap();
    let identity = publisher_identity(&composite, &temp.path().join("publisher.json"));
    let registry = LocalRegistry::load(&registry_a).unwrap();
    let server = oci_registry_test_server::OciRegistryTestServer::start(&registry_store);
    let reference = registry_reference(&server, "guild-example-hello-composite", "0.1.0");

    registry
        .push_oci_registry(
            &composite.resolved_ref,
            true,
            &reference,
            &registry_options(),
            &identity,
        )
        .unwrap();

    fs::remove_dir_all(&registry_a).unwrap();

    LocalRegistry::trust_publisher(&registry_b, &identity.trusted_record()).unwrap();
    LocalRegistry::pull_oci_registry(&registry_b, &reference, &registry_options()).unwrap();
    let facade = GuildMcpFacade::new(
        LocalRegistry::load(&registry_b).unwrap(),
        WasmtimeRuntimeAdapter::new().unwrap(),
    );
    let response = facade.inspect(composite_request()).unwrap();

    assert_eq!(
        response.structured_content.status,
        ExecutionStatus::Succeeded
    );
    assert_eq!(
        response.structured_content.provenance.resolved_skill.digest,
        composite.resolved_ref.digest
    );
    assert_eq!(response.structured_content.child_executions.len(), 1);
    assert_eq!(
        response.structured_content.child_executions[0]
            .provenance
            .resolved_skill
            .digest,
        primitive.resolved_ref.digest
    );
    let child_resource = facade
        .read_resource(&response.structured_content.child_executions[0].uri)
        .unwrap();
    assert_eq!(child_resource.mime_type, "application/json");
}
