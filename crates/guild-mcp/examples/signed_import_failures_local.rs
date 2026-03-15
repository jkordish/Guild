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
    repo_root().join("target/dev-local-registry/signed-import-failures-local")
}

fn registry_a_root() -> PathBuf {
    base_root().join("registry-a")
}

fn bundle_root() -> PathBuf {
    base_root().join("bundle")
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base_root = base_root();
    reset_root(&base_root)?;

    let installer = LocalSourceInstaller::new(registry_a_root())?;
    let installed = installer.install(example_source_dir())?;
    let identity = LocalPublisherIdentity::generate(installed.manifest.publisher.clone())?;
    identity.save(publisher_identity_path())?;
    let identity = LocalPublisherIdentity::load(publisher_identity_path())?;

    let registry_a = LocalRegistry::load(registry_a_root())?;
    let bundle =
        registry_a.export_bundle(&installed.resolved_ref, false, bundle_root(), &identity)?;

    let untrusted_error = LocalRegistry::import_bundle(registry_b_root(), bundle_root())
        .expect_err("untrusted import should fail");
    println!("untrusted failure code: {}", untrusted_error.code);
    println!("untrusted failure message: {}", untrusted_error.message);

    LocalRegistry::trust_publisher(registry_b_root(), &identity.trusted_record())?;

    let component_path = bundle_root()
        .join(&bundle.skills[0].install_dir)
        .join("component.wasm");
    fs::write(&component_path, b"tampered artifact")?;

    let tampered_error = LocalRegistry::import_bundle(registry_b_root(), bundle_root())
        .expect_err("tampered import should fail");
    println!("tampered failure code: {}", tampered_error.code);
    println!("tampered failure message: {}", tampered_error.message);
    println!("publisher: {}", identity.publisher.id);
    println!(
        "publisher identity: {}",
        publisher_identity_path().display()
    );
    println!("bundle root: {}", bundle_root().display());

    Ok(())
}
