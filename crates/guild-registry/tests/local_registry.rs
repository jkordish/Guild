use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use guild_registry::{
    BundleSignatureEnvelope, InstalledSkill, InstalledSkillBundle, LocalPublisherIdentity,
    LocalRegistry, LocalSourceInstaller, SkillRegistry, VerificationStatus,
};
use guild_types::{
    EvidenceAudience, EvidenceEmissionRequest, RedactionClass, RequestedSkillRef, SkillKey,
    VersionRequirement,
};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn example_source_dir() -> PathBuf {
    repo_root()
        .join("examples/skills/hello-inspect")
        .canonicalize()
        .unwrap()
}

fn composite_source_dir() -> PathBuf {
    repo_root()
        .join("examples/skills/hello-composite")
        .canonicalize()
        .unwrap()
}

fn bundle_json(bundle_root: &Path) -> InstalledSkillBundle {
    serde_json::from_str(&fs::read_to_string(bundle_root.join("bundle.json")).unwrap()).unwrap()
}

fn bundle_signature(bundle_root: &Path) -> BundleSignatureEnvelope {
    serde_json::from_str(&fs::read_to_string(bundle_root.join("bundle.signature.json")).unwrap())
        .unwrap()
}

fn publisher_identity(installed: &InstalledSkill, path: &Path) -> LocalPublisherIdentity {
    let identity = LocalPublisherIdentity::generate(installed.manifest.publisher.clone()).unwrap();
    identity.save(path).unwrap();
    LocalPublisherIdentity::load(path).unwrap()
}

fn prepared_registry_root() -> &'static PathBuf {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();

    ROOT.get_or_init(|| {
        let root = repo_root().join("target/test-install-registry/local-registry");
        if root.exists() {
            let _ = fs::remove_dir_all(&root);
        }

        LocalSourceInstaller::new(&root)
            .unwrap()
            .install(example_source_dir())
            .unwrap();

        root
    })
}

fn requested_hello_inspect() -> RequestedSkillRef {
    RequestedSkillRef {
        key: SkillKey {
            namespace: "example".into(),
            name: "hello-inspect".into(),
        },
        version_req: VersionRequirement::parse("^0.1").unwrap(),
    }
}

fn requested_hello_composite() -> RequestedSkillRef {
    RequestedSkillRef {
        key: SkillKey {
            namespace: "example".into(),
            name: "hello-composite".into(),
        },
        version_req: VersionRequirement::parse("^0.1").unwrap(),
    }
}

struct TempFixtureDir {
    path: PathBuf,
}

impl TempFixtureDir {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("guild-local-registry-{unique}"));
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

#[test]
fn primitive_bundle_export_contains_expected_installed_record() {
    let temp = TempFixtureDir::new();
    let registry_root = temp.path().join("registry-a");
    let bundle_root = temp.path().join("bundle");
    let installer = LocalSourceInstaller::new(&registry_root).unwrap();
    let installed = installer.install(example_source_dir()).unwrap();
    let identity = publisher_identity(&installed, &temp.path().join("publisher.json"));
    let registry = LocalRegistry::load(&registry_root).unwrap();

    let bundle = registry
        .export_bundle(&installed.resolved_ref, false, &bundle_root, &identity)
        .unwrap();
    let stored_bundle = bundle_json(&bundle_root);

    assert_eq!(bundle, stored_bundle);
    assert_eq!(bundle.format_version, "guild-installed-bundle-v2");
    assert_eq!(bundle.root_skill, installed.resolved_ref);
    assert!(!bundle.includes_dependency_closure);
    assert_eq!(bundle.publisher, installed.manifest.publisher);
    assert_eq!(bundle.skills.len(), 1);
    assert_eq!(bundle.skills[0].resolved_ref, bundle.root_skill);
    assert!(bundle_root.join(&bundle.skills[0].install_dir).exists());
    assert!(bundle_root
        .join(&bundle.skills[0].install_dir)
        .join("component.wasm")
        .exists());
    assert!(bundle_root
        .join(&bundle.skills[0].install_dir)
        .join("input.schema.json")
        .exists());
    assert!(bundle_root.join("bundle.signature.json").exists());
    assert!(bundle
        .files
        .iter()
        .any(|entry| entry.path.ends_with("/component.wasm")));
}

#[test]
fn signed_bundle_export_verifies_against_local_publisher_identity() {
    let temp = TempFixtureDir::new();
    let registry_root = temp.path().join("registry-a");
    let bundle_root = temp.path().join("bundle");
    let installer = LocalSourceInstaller::new(&registry_root).unwrap();
    let installed = installer.install(example_source_dir()).unwrap();
    let identity = publisher_identity(&installed, &temp.path().join("publisher.json"));
    let registry = LocalRegistry::load(&registry_root).unwrap();

    registry
        .export_bundle(&installed.resolved_ref, false, &bundle_root, &identity)
        .unwrap();

    let bundle_bytes = fs::read(bundle_root.join("bundle.json")).unwrap();
    let envelope = bundle_signature(&bundle_root);
    let public_key: [u8; 32] = base64::engine::general_purpose::STANDARD
        .decode(&identity.public_key_base64)
        .unwrap()
        .try_into()
        .unwrap();
    let signature_bytes: [u8; 64] = base64::engine::general_purpose::STANDARD
        .decode(&envelope.signature_base64)
        .unwrap()
        .try_into()
        .unwrap();
    let verifying_key = VerifyingKey::from_bytes(&public_key).unwrap();
    let signature = Signature::from_bytes(&signature_bytes);
    verifying_key.verify(&bundle_bytes, &signature).unwrap();
    assert_eq!(envelope.publisher_id, identity.publisher.id);
}

#[test]
fn primitive_bundle_import_resolves_digest_pinned_skill_in_fresh_registry() {
    let temp = TempFixtureDir::new();
    let registry_a = temp.path().join("registry-a");
    let bundle_root = temp.path().join("bundle");
    let registry_b = temp.path().join("registry-b");
    let installer = LocalSourceInstaller::new(&registry_a).unwrap();
    let installed = installer.install(example_source_dir()).unwrap();
    let identity = publisher_identity(&installed, &temp.path().join("publisher.json"));
    let registry = LocalRegistry::load(&registry_a).unwrap();

    registry
        .export_bundle(&installed.resolved_ref, false, &bundle_root, &identity)
        .unwrap();
    LocalRegistry::trust_publisher(&registry_b, &identity.trusted_record()).unwrap();
    LocalRegistry::import_bundle(&registry_b, &bundle_root).unwrap();

    let imported = LocalRegistry::load(&registry_b)
        .unwrap()
        .resolve(&requested_hello_inspect())
        .unwrap();
    assert_eq!(imported.resolved_ref, installed.resolved_ref);
    assert_eq!(
        imported.manifest.package.artifact_digest,
        installed.manifest.package.artifact_digest
    );
    assert!(imported.artifact_path.exists());
    let verification = imported
        .verification
        .expect("imported skills carry verification metadata");
    assert_eq!(verification.status, VerificationStatus::Verified);
    assert_eq!(verification.publisher.id, identity.publisher.id);
}

#[test]
fn composite_bundle_export_includes_dependency_closure() {
    let temp = TempFixtureDir::new();
    let registry_root = temp.path().join("registry-a");
    let bundle_root = temp.path().join("bundle");
    let installer = LocalSourceInstaller::new(&registry_root).unwrap();
    let primitive = installer.install(example_source_dir()).unwrap();
    let composite = installer.install(composite_source_dir()).unwrap();
    let identity = publisher_identity(&composite, &temp.path().join("publisher.json"));
    let registry = LocalRegistry::load(&registry_root).unwrap();

    let bundle = registry
        .export_bundle(&composite.resolved_ref, true, &bundle_root, &identity)
        .unwrap();

    assert!(bundle.includes_dependency_closure);
    assert_eq!(bundle.root_skill, composite.resolved_ref);
    assert_eq!(bundle.publisher, composite.manifest.publisher);
    assert_eq!(bundle.skills.len(), 2);
    assert!(bundle
        .skills
        .iter()
        .any(|entry| entry.resolved_ref == composite.resolved_ref));
    assert!(bundle
        .skills
        .iter()
        .any(|entry| entry.resolved_ref == primitive.resolved_ref));
    assert_eq!(composite.manifest.dependencies[0].alias, "hello");
    assert_eq!(
        composite.manifest.dependencies[0].skill,
        primitive.resolved_ref
    );
    assert!(bundle
        .files
        .iter()
        .any(|entry| entry.path.ends_with("/component.wasm")));
}

#[test]
fn composite_bundle_import_preserves_dependency_resolution() {
    let temp = TempFixtureDir::new();
    let registry_a = temp.path().join("registry-a");
    let bundle_root = temp.path().join("bundle");
    let registry_b = temp.path().join("registry-b");
    let installer = LocalSourceInstaller::new(&registry_a).unwrap();
    let primitive = installer.install(example_source_dir()).unwrap();
    let composite = installer.install(composite_source_dir()).unwrap();
    let identity = publisher_identity(&composite, &temp.path().join("publisher.json"));
    let registry = LocalRegistry::load(&registry_a).unwrap();

    registry
        .export_bundle(&composite.resolved_ref, true, &bundle_root, &identity)
        .unwrap();
    LocalRegistry::trust_publisher(&registry_b, &identity.trusted_record()).unwrap();
    LocalRegistry::import_bundle(&registry_b, &bundle_root).unwrap();

    let imported_registry = LocalRegistry::load(&registry_b).unwrap();
    let imported_composite = imported_registry
        .resolve(&requested_hello_composite())
        .unwrap();
    assert_eq!(imported_composite.resolved_ref, composite.resolved_ref);
    assert_eq!(
        imported_composite.manifest.dependencies[0].skill,
        primitive.resolved_ref
    );
    assert_eq!(
        imported_registry
            .resolve_exact(&imported_composite.manifest.dependencies[0].skill)
            .unwrap()
            .resolved_ref,
        primitive.resolved_ref
    );
    assert_eq!(
        imported_composite
            .verification
            .expect("verified import metadata present")
            .publisher
            .id,
        identity.publisher.id
    );
}

#[test]
fn bundle_import_fails_for_untrusted_signed_bundle() {
    let temp = TempFixtureDir::new();
    let registry_a = temp.path().join("registry-a");
    let bundle_root = temp.path().join("bundle");
    let registry_b = temp.path().join("registry-b");
    let installer = LocalSourceInstaller::new(&registry_a).unwrap();
    let installed = installer.install(example_source_dir()).unwrap();
    let identity = publisher_identity(&installed, &temp.path().join("publisher.json"));
    let registry = LocalRegistry::load(&registry_a).unwrap();

    registry
        .export_bundle(&installed.resolved_ref, false, &bundle_root, &identity)
        .unwrap();

    let error = LocalRegistry::import_bundle(&registry_b, &bundle_root).unwrap_err();
    assert_eq!(error.code, "bundle-publisher-untrusted");
    assert!(!registry_b.join("installed").join("example").exists());
}

#[test]
fn bundle_import_fails_on_tampered_content_even_when_signature_is_trusted() {
    let temp = TempFixtureDir::new();
    let registry_a = temp.path().join("registry-a");
    let bundle_root = temp.path().join("bundle");
    let registry_b = temp.path().join("registry-b");
    let installer = LocalSourceInstaller::new(&registry_a).unwrap();
    let installed = installer.install(example_source_dir()).unwrap();
    let identity = publisher_identity(&installed, &temp.path().join("publisher.json"));
    let registry = LocalRegistry::load(&registry_a).unwrap();

    registry
        .export_bundle(&installed.resolved_ref, false, &bundle_root, &identity)
        .unwrap();
    LocalRegistry::trust_publisher(&registry_b, &identity.trusted_record()).unwrap();
    fs::write(
        bundle_root
            .join("installed/example/hello-inspect/0.1.0")
            .join(installed.resolved_ref.digest.replace(':', "-"))
            .join("component.wasm"),
        b"tampered artifact",
    )
    .unwrap();

    let error = LocalRegistry::import_bundle(&registry_b, &bundle_root).unwrap_err();
    assert_eq!(error.code, "artifact-digest-mismatch");
    assert!(!registry_b.join("installed").join("example").exists());
}

#[test]
fn bundle_import_fails_when_required_content_is_missing() {
    let temp = TempFixtureDir::new();
    let registry_a = temp.path().join("registry-a");
    let bundle_root = temp.path().join("bundle");
    let registry_b = temp.path().join("registry-b");
    let installer = LocalSourceInstaller::new(&registry_a).unwrap();
    let installed = installer.install(example_source_dir()).unwrap();
    let identity = publisher_identity(&installed, &temp.path().join("publisher.json"));
    let registry = LocalRegistry::load(&registry_a).unwrap();

    registry
        .export_bundle(&installed.resolved_ref, false, &bundle_root, &identity)
        .unwrap();
    LocalRegistry::trust_publisher(&registry_b, &identity.trusted_record()).unwrap();
    fs::remove_file(
        bundle_root
            .join("installed/example/hello-inspect/0.1.0")
            .join(installed.resolved_ref.digest.replace(':', "-"))
            .join("input.schema.json"),
    )
    .unwrap();

    let error = LocalRegistry::import_bundle(&registry_b, &bundle_root).unwrap_err();
    assert_eq!(error.code, "staged-file-missing");
}

#[test]
fn bundle_import_fails_when_signature_file_is_missing() {
    let temp = TempFixtureDir::new();
    let registry_a = temp.path().join("registry-a");
    let bundle_root = temp.path().join("bundle");
    let registry_b = temp.path().join("registry-b");
    let installer = LocalSourceInstaller::new(&registry_a).unwrap();
    let installed = installer.install(example_source_dir()).unwrap();
    let identity = publisher_identity(&installed, &temp.path().join("publisher.json"));
    let registry = LocalRegistry::load(&registry_a).unwrap();

    registry
        .export_bundle(&installed.resolved_ref, false, &bundle_root, &identity)
        .unwrap();
    LocalRegistry::trust_publisher(&registry_b, &identity.trusted_record()).unwrap();
    fs::remove_file(bundle_root.join("bundle.signature.json")).unwrap();

    let error = LocalRegistry::import_bundle(&registry_b, &bundle_root).unwrap_err();
    assert_eq!(error.code, "bundle-signature-missing");
}

#[test]
fn local_source_install_builds_and_stages_digest_pinned_skill() {
    let registry_root = prepared_registry_root();
    let registry = LocalRegistry::load(registry_root).unwrap();
    let installed = registry.resolve(&requested_hello_inspect()).unwrap();

    assert_eq!(installed.resolved_ref.key.namespace, "example");
    assert_eq!(installed.resolved_ref.key.name, "hello-inspect");
    assert_eq!(installed.resolved_ref.version.to_string(), "0.1.0");
    assert!(installed.resolved_ref.digest.starts_with("sha256:"));
    assert!(installed.artifact_path.ends_with("component.wasm"));
    assert!(installed.artifact_path.exists());
    assert!(installed.manifest_path.exists());
}

#[test]
fn requested_skill_resolves_to_digest_pinned_installed_skill() {
    let registry = LocalRegistry::load(prepared_registry_root()).unwrap();
    let installed = registry.resolve(&requested_hello_inspect()).unwrap();

    assert_eq!(installed.resolved_ref.key.namespace, "example");
    assert_eq!(installed.resolved_ref.key.name, "hello-inspect");
    assert_eq!(installed.resolved_ref.version.to_string(), "0.1.0");
    assert_eq!(
        installed.resolved_ref.digest,
        installed.manifest.package.artifact_digest
    );
    assert!(installed.artifact_path.exists());
}

#[test]
fn reinstall_updates_digest_when_source_changes() {
    let temp = TempFixtureDir::new();
    let workspace_root = temp.path().join("workspace");
    let source_root = workspace_root.join("examples/skills/hello-inspect");
    let registry_root = temp.path().join("registry");

    copy_dir_recursive(&example_source_dir(), &source_root);
    copy_dir_recursive(&repo_root().join("wit"), &workspace_root.join("wit"));

    let installer = LocalSourceInstaller::new(&registry_root).unwrap();
    let first = installer.install(&source_root).unwrap();

    let guest_source = source_root.join("skill-rust/src/lib.rs");
    let guest = fs::read_to_string(&guest_source)
        .unwrap()
        .replace("Guild inspect is working.", "Guild inspect was rebuilt.");
    fs::write(&guest_source, guest).unwrap();

    let second = installer.install(&source_root).unwrap();

    assert_ne!(first.resolved_ref.digest, second.resolved_ref.digest);

    let registry = LocalRegistry::load(&registry_root).unwrap();
    let resolved = registry.resolve(&requested_hello_inspect()).unwrap();
    assert_eq!(resolved.resolved_ref.digest, second.resolved_ref.digest);
}

#[test]
fn missing_staged_artifact_fails_closed() {
    let temp = TempFixtureDir::new();
    let installer = LocalSourceInstaller::new(temp.path()).unwrap();
    let installed = installer.install(example_source_dir()).unwrap();

    fs::remove_file(&installed.artifact_path).unwrap();

    let error = LocalRegistry::load(temp.path()).unwrap_err();
    assert_eq!(error.code, "artifact-missing");
}

#[test]
fn source_only_skill_root_is_not_treated_as_installed_registry() {
    let error = LocalRegistry::load(example_source_dir()).unwrap_err();
    assert_eq!(error.code, "source-skill-not-installed");
}

#[test]
fn composite_install_fails_when_declared_dependency_is_missing() {
    let temp = TempFixtureDir::new();
    let installer = LocalSourceInstaller::new(temp.path()).unwrap();

    let error = installer.install(composite_source_dir()).unwrap_err();
    assert_eq!(error.code, "dependency-resolution-failed");
}

#[test]
fn composite_install_resolves_declared_dependency_to_installed_record() {
    let temp = TempFixtureDir::new();
    let installer = LocalSourceInstaller::new(temp.path()).unwrap();
    let primitive = installer.install(example_source_dir()).unwrap();
    let composite = installer.install(composite_source_dir()).unwrap();

    assert_eq!(composite.manifest.dependencies.len(), 1);
    assert_eq!(composite.manifest.dependencies[0].alias, "hello");
    assert_eq!(
        composite.manifest.dependencies[0].skill,
        primitive.resolved_ref
    );

    let registry = LocalRegistry::load(temp.path()).unwrap();
    let resolved = registry.resolve(&requested_hello_composite()).unwrap();
    assert_eq!(resolved.resolved_ref.key.name, "hello-composite");
    assert_eq!(
        resolved.manifest.dependencies[0].skill,
        primitive.resolved_ref
    );
    assert_eq!(
        registry
            .resolve_exact(&resolved.manifest.dependencies[0].skill)
            .unwrap()
            .resolved_ref,
        primitive.resolved_ref
    );
}

#[test]
fn evidence_objects_are_stored_deduped_and_readable() {
    let registry = LocalRegistry::load(prepared_registry_root()).unwrap();
    let first = registry
        .store_evidence(&EvidenceEmissionRequest {
            payload: br#"{"hello":"world"}"#.to_vec(),
            mime_type: "application/json".into(),
            title: Some("fixture".into()),
            audience: EvidenceAudience::User,
            redaction: RedactionClass::None,
            freshness: Some("deterministic".into()),
        })
        .unwrap();
    let second = registry
        .store_evidence(&EvidenceEmissionRequest {
            payload: br#"{"hello":"world"}"#.to_vec(),
            mime_type: "application/json".into(),
            title: Some("fixture-again".into()),
            audience: EvidenceAudience::Assistant,
            redaction: RedactionClass::None,
            freshness: Some("deterministic".into()),
        })
        .unwrap();

    assert_eq!(first.uri, second.uri);
    assert_eq!(first.sha256, second.sha256);

    let evidence = registry.load_evidence_record(&first.uri).unwrap();
    let stored = registry.read_resource(&first.uri).unwrap();
    assert_eq!(evidence.uri, first.uri);
    assert_eq!(evidence.sha256, first.sha256.clone().unwrap());
    assert_eq!(evidence.mime_type, "application/json");
    assert_eq!(evidence.title.as_deref(), Some("fixture"));
    assert_eq!(stored.mime_type, "application/json");
    assert_eq!(stored.bytes, br#"{"hello":"world"}"#);
    assert_eq!(stored.sha256, first.sha256);
}

#[test]
fn missing_object_resource_fails_closed() {
    let registry = LocalRegistry::load(prepared_registry_root()).unwrap();
    let error = registry
        .read_resource("guild://objects/sha256/deadbeef")
        .unwrap_err();

    assert_eq!(error.code, "object-not-found");
}
