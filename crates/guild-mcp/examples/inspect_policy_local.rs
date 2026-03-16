use std::path::{Path, PathBuf};

use guild_mcp::{GuildMcpFacade, InspectRequest};
use guild_registry::{LocalRegistry, LocalSourceInstaller};
use guild_runner::WasmtimeRuntimeAdapter;
use guild_types::{
    CapabilityAccess, CapabilityConstraints, CapabilityGrantSet, CapabilityId,
    EmitEvidenceConstraints, EvidenceAudience, GrantedCapability, HttpMethod,
    HttpRequestConstraints, HttpScheme, LocalPolicyConfig, PolicyRule, PolicyRuleEffect,
    RedactionClass, RequestedSkillRef, SkillKey, VersionRequirement,
};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root exists")
}

fn example_source_dir() -> PathBuf {
    repo_root().join("examples/skills/hello-inspect")
}

fn local_registry_root() -> PathBuf {
    repo_root().join("target/dev-local-registry/inspect-policy-local")
}

fn reset_registry_root(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if path.exists() {
        std::fs::remove_dir_all(path)?;
    }
    Ok(())
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

fn extra_http_grant() -> GrantedCapability {
    GrantedCapability {
        id: CapabilityId::HttpRequest,
        access: CapabilityAccess::Read,
        constraints: CapabilityConstraints::HttpRequest(HttpRequestConstraints {
            allowed_schemes: Some(vec![HttpScheme::Http]),
            allowed_hosts: Some(vec!["example.com".into()]),
            allowed_ports: Some(vec![80]),
            allowed_methods: Some(vec![HttpMethod::Get]),
            allowed_path_prefixes: Some(vec!["/".into()]),
            max_timeout_ms: Some(1_000),
            max_response_bytes: Some(1_024),
        }),
    }
}

fn write_policy(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(root)?;
    let policy = LocalPolicyConfig {
        rules: vec![PolicyRule {
            name: Some("deny-hello-evidence-for-blocked-actor".into()),
            actor_ids: Some(vec!["actor-blocked".into()]),
            tenant_ids: None,
            skills: Some(vec![SkillKey {
                namespace: "example".into(),
                name: "hello-inspect".into(),
            }]),
            publisher_ids: None,
            effect: PolicyRuleEffect::Deny,
            capabilities: CapabilityGrantSet {
                grants: vec![GrantedCapability {
                    id: CapabilityId::EmitEvidence,
                    access: CapabilityAccess::Write,
                    constraints: CapabilityConstraints::EmitEvidence(EmitEvidenceConstraints {
                        max_bytes: None,
                        audiences: None,
                        redactions: None,
                    }),
                }],
            },
        }],
        ..LocalPolicyConfig::default()
    };
    std::fs::write(
        root.join("policy.json"),
        serde_json::to_vec_pretty(&policy)?,
    )?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let registry_root = local_registry_root();
    reset_registry_root(&registry_root)?;
    write_policy(&registry_root)?;

    let source_installer = LocalSourceInstaller::new(&registry_root)?;
    let installed_skill = source_installer.install(example_source_dir())?;

    let registry = LocalRegistry::load(&registry_root)?;
    let facade = GuildMcpFacade::new(registry, WasmtimeRuntimeAdapter::new()?);

    let reduced = facade.inspect(InspectRequest::new(
        RequestedSkillRef {
            key: SkillKey {
                namespace: "example".into(),
                name: "hello-inspect".into(),
            },
            version_req: VersionRequirement::parse("^0.1")?,
        },
        serde_json::json!({ "name": "Ada" }),
        "tenant-dev",
        "actor-allowed",
        CapabilityGrantSet {
            grants: vec![emit_evidence_grant(), extra_http_grant()],
        },
    ))?;

    println!("installed {}", installed_skill.resolved_ref.digest);
    println!("reduced policy outcome: {}", reduced.summary);
    println!(
        "{}",
        serde_json::to_string_pretty(&reduced.structured_content)?
    );

    let denied = facade
        .inspect(InspectRequest::new(
            RequestedSkillRef {
                key: SkillKey {
                    namespace: "example".into(),
                    name: "hello-inspect".into(),
                },
                version_req: VersionRequirement::parse("^0.1")?,
            },
            serde_json::json!({ "name": "Ada" }),
            "tenant-dev",
            "actor-blocked",
            CapabilityGrantSet {
                grants: vec![emit_evidence_grant()],
            },
        ))
        .unwrap_err();

    println!("denied: {} {}", denied.code, denied.message);
    let receipt = denied
        .receipt
        .expect("policy denial persists a host-owned receipt");
    let denied_record = facade.read_resource(&receipt.uri)?;
    println!("denied execution resource: {}", denied_record.uri);
    println!("{}", String::from_utf8(denied_record.bytes)?);

    Ok(())
}
