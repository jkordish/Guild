use std::path::{Path, PathBuf};

use guild_mcp::{GuildMcpFacade, InspectRequest};
use guild_registry::{LocalPublisherIdentity, LocalRegistry, LocalSourceInstaller};
use guild_runner::WasmtimeRuntimeAdapter;
use guild_types::{
    CapabilityAccess, CapabilityConstraints, CapabilityGrantSet, CapabilityId, GrantedCapability,
    HttpMethod, HttpRequestConstraints, HttpScheme, InstalledVerificationState, LocalPolicyConfig,
    LocalTrustTier, PolicyProfile, PolicyProfileBinding, PolicyRule, PolicyRuleEffect,
    PolicyRuleTarget, RequestedSkillRef, SkillKey, VersionRequirement,
};

#[path = "../../../test-support/http_test_server.rs"]
mod http_test_server;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root exists")
}

fn http_source_dir() -> PathBuf {
    repo_root().join("examples/skills/inspect-http-json")
}

fn explain_source_dir() -> PathBuf {
    repo_root().join("examples/skills/explain-execution")
}

fn policy_demo_root() -> PathBuf {
    repo_root().join("target/dev-local-registry/inspect-policy-local")
}

fn reset_registry_root(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if path.exists() {
        std::fs::remove_dir_all(path)?;
    }
    Ok(())
}

fn http_grant(host: &str, port: u16, path: &str) -> GrantedCapability {
    GrantedCapability {
        id: CapabilityId::HttpRequest,
        access: CapabilityAccess::Read,
        constraints: CapabilityConstraints::HttpRequest(HttpRequestConstraints {
            allowed_schemes: Some(vec![HttpScheme::Http]),
            allowed_hosts: Some(vec![host.into()]),
            allowed_ports: Some(vec![port]),
            allowed_methods: Some(vec![HttpMethod::Get]),
            allowed_path_prefixes: Some(vec![path.into()]),
            max_timeout_ms: Some(2_000),
            max_response_bytes: Some(8_192),
        }),
    }
}

fn explain_grant() -> GrantedCapability {
    GrantedCapability {
        id: CapabilityId::ReadResource,
        access: CapabilityAccess::Read,
        constraints: CapabilityConstraints::ReadResource(guild_types::ReadResourceConstraints {
            uri_prefixes: Some(vec![
                "guild://executions/".into(),
                "guild://objects/records/".into(),
            ]),
            resource_kinds: Some(vec![
                guild_types::ResourceKind::Execution,
                guild_types::ResourceKind::Object,
            ]),
        }),
    }
}

fn publisher_identity(
    installed: &guild_registry::InstalledSkill,
    path: &Path,
) -> LocalPublisherIdentity {
    let identity = LocalPublisherIdentity::generate(installed.manifest.publisher.clone()).unwrap();
    identity.save(path).unwrap();
    LocalPublisherIdentity::load(path).unwrap()
}

fn write_policy(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(root)?;
    let policy = LocalPolicyConfig {
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
                    skills: Some(vec![SkillKey {
                        namespace: "example".into(),
                        name: "inspect-http-json".into(),
                    }]),
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
    };
    std::fs::write(
        root.join("policy.json"),
        serde_json::to_vec_pretty(&policy)?,
    )?;
    Ok(())
}

struct PolicyDemoPaths {
    demo_root: PathBuf,
    registry_a: PathBuf,
    registry_trusted: PathBuf,
    registry_restricted: PathBuf,
    bundle_root: PathBuf,
    identity_path: PathBuf,
}

impl PolicyDemoPaths {
    fn new() -> Self {
        let demo_root = policy_demo_root();
        Self {
            registry_a: demo_root.join("registry-a"),
            registry_trusted: demo_root.join("registry-trusted"),
            registry_restricted: demo_root.join("registry-restricted"),
            bundle_root: demo_root.join("bundle"),
            identity_path: demo_root.join("publisher.json"),
            demo_root,
        }
    }
}

fn prepare_policy_demo(
    paths: &PolicyDemoPaths,
) -> Result<guild_registry::InstalledSkill, Box<dyn std::error::Error>> {
    let source_installer = LocalSourceInstaller::new(&paths.registry_a)?;
    let installed_skill = source_installer.install(http_source_dir())?;
    let identity = publisher_identity(&installed_skill, &paths.identity_path);
    let registry = LocalRegistry::load(&paths.registry_a)?;
    registry.export_bundle(
        &installed_skill.resolved_ref,
        false,
        &paths.bundle_root,
        &identity,
    )?;

    LocalRegistry::trust_publisher(
        &paths.registry_trusted,
        &identity.trusted_record_with_tier(LocalTrustTier::TrustedImported),
    )?;
    LocalRegistry::trust_publisher(
        &paths.registry_restricted,
        &identity.trusted_record_with_tier(LocalTrustTier::Restricted),
    )?;
    LocalRegistry::import_bundle(&paths.registry_trusted, &paths.bundle_root)?;
    LocalRegistry::import_bundle(&paths.registry_restricted, &paths.bundle_root)?;
    write_policy(&paths.registry_trusted)?;
    write_policy(&paths.registry_restricted)?;

    Ok(installed_skill)
}

fn inspect_http_request(
    url: &str,
    tenant_id: &str,
    actor_id: &str,
    port: u16,
) -> Result<InspectRequest, Box<dyn std::error::Error>> {
    Ok(InspectRequest::new(
        RequestedSkillRef {
            key: SkillKey {
                namespace: "example".into(),
                name: "inspect-http-json".into(),
            },
            version_req: VersionRequirement::parse("^0.1")?,
        },
        serde_json::json!({
            "url": url,
            "method": "get",
            "json_pointers": ["/message"],
        }),
        tenant_id,
        actor_id,
        CapabilityGrantSet {
            grants: vec![http_grant(
                http_test_server::HttpTestServer::host(),
                port,
                "/json",
            )],
        },
    ))
}

fn explain_execution_request(
    execution_uri: &str,
    tenant_id: &str,
    actor_id: &str,
) -> Result<InspectRequest, Box<dyn std::error::Error>> {
    Ok(InspectRequest::new(
        RequestedSkillRef {
            key: SkillKey {
                namespace: "example".into(),
                name: "explain-execution".into(),
            },
            version_req: VersionRequirement::parse("^0.1")?,
        },
        serde_json::json!({
            "execution_uri": execution_uri,
            "include_first_evidence": false,
        }),
        tenant_id,
        actor_id,
        CapabilityGrantSet {
            grants: vec![explain_grant()],
        },
    ))
}

fn print_pretty_json(value: &impl serde::Serialize) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = http_test_server::HttpTestServer::start();
    let paths = PolicyDemoPaths::new();

    reset_registry_root(&paths.demo_root)?;
    let installed_skill = prepare_policy_demo(&paths)?;

    let trusted_facade = GuildMcpFacade::new(
        LocalRegistry::load(&paths.registry_trusted)?,
        WasmtimeRuntimeAdapter::new()?,
    );
    let restricted_facade = GuildMcpFacade::new(
        LocalRegistry::load(&paths.registry_restricted)?,
        WasmtimeRuntimeAdapter::new()?,
    );

    let trusted = trusted_facade.inspect(inspect_http_request(
        &server.json_url(),
        "tenant-trusted",
        "actor-demo",
        server.port(),
    )?)?;

    println!(
        "trusted imported digest: {}",
        installed_skill.resolved_ref.digest
    );
    println!("trusted outcome: {}", trusted.summary);
    print_pretty_json(&trusted.structured_content)?;

    let denied = restricted_facade
        .inspect(inspect_http_request(
            &server.json_url(),
            "tenant-restricted",
            "actor-demo",
            server.port(),
        )?)
        .unwrap_err();

    println!("denied: {} {}", denied.code, denied.message);
    let receipt = denied
        .receipt
        .expect("policy denial persists a host-owned receipt");
    let denied_record = restricted_facade.read_resource(&receipt.uri)?;
    println!("denied execution resource: {}", denied_record.uri);
    println!("{}", String::from_utf8(denied_record.bytes)?);

    LocalSourceInstaller::new(&paths.registry_restricted)?.install(explain_source_dir())?;
    let restricted_facade = GuildMcpFacade::new(
        LocalRegistry::load(&paths.registry_restricted)?,
        WasmtimeRuntimeAdapter::new()?,
    );
    let explanation = restricted_facade.inspect(explain_execution_request(
        &denied_record.uri,
        "tenant-restricted",
        "actor-demo",
    )?)?;

    println!("denied explanation: {}", explanation.summary);
    print_pretty_json(&explanation.structured_content)?;

    Ok(())
}
