use std::path::{Path, PathBuf};

use guild_mcp::{GuildMcpFacade, InspectRequest};
use guild_registry::{
    ImportPreviewDecision, ImportPreviewReport, LocalPublisherIdentity, LocalRegistry,
    LocalSourceInstaller,
};
use guild_runner::WasmtimeRuntimeAdapter;
use guild_types::{
    CapabilityAccess, CapabilityConstraints, CapabilityGrantSet, CapabilityId,
    EmitEvidenceConstraints, EvidenceAudience, ExecutionRecord, GrantedCapability,
    InvokeDependencyConstraints, RedactionClass, RequestedSkillRef, SkillKey, VersionRequirement,
};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root exists")
}

fn primitive_source_dir() -> PathBuf {
    repo_root().join("examples/skills/hello-inspect")
}

fn composite_source_dir() -> PathBuf {
    repo_root().join("examples/skills/hello-composite")
}

fn base_root() -> PathBuf {
    repo_root().join("target/dev-local-registry/export-import-composite-oci-local")
}

fn registry_a_root() -> PathBuf {
    base_root().join("registry-a")
}

fn layout_root() -> PathBuf {
    base_root().join("oci-layout")
}

fn publisher_identity_path() -> PathBuf {
    base_root().join("publisher.json")
}

fn registry_b_root() -> PathBuf {
    base_root().join("registry-b")
}

fn reset_root(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if path.exists() {
        std::fs::remove_dir_all(path)?;
    }
    Ok(())
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

fn preview_decision_label(decision: &ImportPreviewDecision) -> &'static str {
    match decision {
        ImportPreviewDecision::WouldImport => "would-import",
        ImportPreviewDecision::WouldRefuse => "would-refuse",
    }
}

fn print_preview(label: &str, preview: &ImportPreviewReport) {
    let trust_tier = preview
        .trust_tier
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_else(|| "none".to_owned());
    let refusal = preview
        .refusal
        .as_ref()
        .map(|error| error.code.as_str())
        .unwrap_or("none");
    println!(
        "{label}: decision={}, verified={}, trust_tier={trust_tier}, refusal={refusal}",
        preview_decision_label(&preview.decision),
        preview.verified,
    );
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base_root = base_root();
    reset_root(&base_root)?;

    let installer = LocalSourceInstaller::new(registry_a_root())?;
    let primitive = installer.install(primitive_source_dir())?;
    let composite = installer.install(composite_source_dir())?;
    let identity = LocalPublisherIdentity::generate(composite.manifest.publisher.clone())?;
    identity.save(publisher_identity_path())?;
    let identity = LocalPublisherIdentity::load(publisher_identity_path())?;

    let registry_a = LocalRegistry::load(registry_a_root())?;
    let bundle =
        registry_a.export_oci_layout(&composite.resolved_ref, true, layout_root(), &identity)?;
    let _preview_target = LocalRegistry::load(registry_b_root())?;
    let pretrust_preview =
        LocalRegistry::preview_import_oci_layout(registry_b_root(), layout_root())?;
    LocalRegistry::trust_publisher(registry_b_root(), &identity.trusted_record())?;
    let posttrust_preview =
        LocalRegistry::preview_import_oci_layout(registry_b_root(), layout_root())?;
    let imported = LocalRegistry::import_oci_layout(registry_b_root(), layout_root())?;

    let registry_b = LocalRegistry::load(registry_b_root())?;
    let facade = GuildMcpFacade::new(registry_b, WasmtimeRuntimeAdapter::new()?);
    let response = facade.inspect(InspectRequest::new(
        RequestedSkillRef {
            key: SkillKey {
                namespace: "example".into(),
                name: "hello-composite".into(),
            },
            version_req: VersionRequirement::parse("^0.1")?,
        },
        serde_json::json!({ "name": "Ada" }),
        "tenant-dev",
        "actor-dev",
        CapabilityGrantSet {
            grants: vec![invoke_hello_grant(), emit_evidence_grant()],
        },
    ))?;

    println!(
        "exported primitive digest: {}",
        primitive.resolved_ref.digest
    );
    println!(
        "exported composite digest: {}",
        composite.resolved_ref.digest
    );
    println!("publisher: {}", identity.publisher.id);
    println!(
        "publisher identity: {}",
        publisher_identity_path().display()
    );
    println!("oci layout root: {}", layout_root().display());
    println!("bundle skills: {}", bundle.skills.len());
    print_preview("pre-trust preview", &pretrust_preview);
    print_preview("post-trust preview", &posttrust_preview);
    println!("imported skills: {}", imported.len());
    println!("{}", response.summary);
    println!(
        "{}",
        serde_json::to_string_pretty(&response.structured_content)?
    );

    let parent_resource = facade.read_resource(&response.structured_content.receipt.uri)?;
    println!("parent execution resource: {}", parent_resource.uri);
    println!("{}", String::from_utf8(parent_resource.bytes)?);

    if let Some(child_link) = response.structured_content.child_executions.first() {
        let child_resource = facade.read_resource(&child_link.uri)?;
        let child_record: ExecutionRecord = serde_json::from_slice(&child_resource.bytes)?;
        println!("child execution resource: {}", child_resource.uri);
        println!("{}", serde_json::to_string_pretty(&child_record)?);

        if let Some(evidence) = child_record.emitted_evidence.first() {
            let child_evidence = facade.read_resource(&evidence.uri)?;
            println!("child evidence resource: {}", child_evidence.uri);
            if child_evidence.mime_type == "application/json" {
                let json: serde_json::Value = serde_json::from_slice(&child_evidence.bytes)?;
                println!("{}", serde_json::to_string_pretty(&json)?);
            } else {
                println!("{}", String::from_utf8(child_evidence.bytes)?);
            }
        }
    }

    Ok(())
}
