use std::fs;
use std::path::{Path, PathBuf};

use guild_registry::{LocalPublisherIdentity, LocalRegistry, LocalSourceInstaller};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root exists")
}

fn example_source_dir() -> PathBuf {
    repo_root().join("examples/skills/hello-inspect")
}

fn base_root() -> PathBuf {
    repo_root().join("target/dev-local-registry/signed-import-oci-failures-local")
}

fn registry_a_root() -> PathBuf {
    base_root().join("registry-a")
}

fn layout_root() -> PathBuf {
    base_root().join("oci-layout")
}

fn registry_b_root() -> PathBuf {
    base_root().join("registry-b")
}

fn publisher_identity_path() -> PathBuf {
    base_root().join("publisher.json")
}

fn reset_root(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}

fn oci_blob_path(layout_root: &Path, digest: &str) -> PathBuf {
    layout_root
        .join("blobs/sha256")
        .join(digest.strip_prefix("sha256:").expect("sha256 digest"))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base_root = base_root();
    reset_root(&base_root)?;

    let source_installer = LocalSourceInstaller::new(registry_a_root())?;
    let installed_skill = source_installer.install(example_source_dir())?;
    let identity = LocalPublisherIdentity::generate(installed_skill.manifest.publisher.clone())?;
    identity.save(publisher_identity_path())?;
    let identity = LocalPublisherIdentity::load(publisher_identity_path())?;

    let registry_a = LocalRegistry::load(registry_a_root())?;
    registry_a.export_oci_layout(
        &installed_skill.resolved_ref,
        false,
        layout_root(),
        &identity,
    )?;

    let untrusted_error = LocalRegistry::import_oci_layout(registry_b_root(), layout_root())
        .expect_err("untrusted import should fail");
    println!("untrusted failure code: {}", untrusted_error.code);
    println!("untrusted failure message: {}", untrusted_error.message);

    LocalRegistry::trust_publisher(registry_b_root(), &identity.trusted_record())?;

    let component_blob = oci_blob_path(
        &layout_root(),
        &installed_skill.manifest.package.artifact_digest,
    );
    fs::write(&component_blob, b"tampered artifact")?;

    let tampered_error = LocalRegistry::import_oci_layout(registry_b_root(), layout_root())
        .expect_err("tampered import should fail");
    println!("tampered failure code: {}", tampered_error.code);
    println!("tampered failure message: {}", tampered_error.message);
    println!("publisher: {}", identity.publisher.id);
    println!(
        "publisher identity: {}",
        publisher_identity_path().display()
    );
    println!("oci layout root: {}", layout_root().display());

    Ok(())
}
