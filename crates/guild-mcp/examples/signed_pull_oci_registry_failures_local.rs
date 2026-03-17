use std::fs;
use std::path::{Path, PathBuf};

use guild_registry::{
    LocalPublisherIdentity, LocalRegistry, LocalSourceInstaller, OciRegistryAuth,
    OciRegistryReference, OciRegistryTarget, OciRegistryTransportOptions,
};

#[path = "../../../test-support/oci_registry_test_server.rs"]
mod oci_registry_test_server;

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
    repo_root().join("target/dev-local-registry/signed-pull-oci-registry-failures-local")
}

fn registry_a_root() -> PathBuf {
    base_root().join("registry-a")
}

fn registry_store_root() -> PathBuf {
    base_root().join("oci-registry-store")
}

fn registry_b_root() -> PathBuf {
    base_root().join("registry-b")
}

fn publisher_identity_path() -> PathBuf {
    base_root().join("publisher.json")
}

fn registry_options() -> OciRegistryTransportOptions {
    OciRegistryTransportOptions {
        auth: OciRegistryAuth::Anonymous,
        allow_http: true,
    }
}

fn registry_reference(
    server: &oci_registry_test_server::OciRegistryTestServer,
) -> OciRegistryReference {
    OciRegistryReference {
        registry: server.registry(),
        repository: "guild-example-hello-inspect".into(),
        target: OciRegistryTarget::Tag("0.1.0".into()),
    }
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

    let source_installer = LocalSourceInstaller::new(registry_a_root())?;
    let installed_skill = source_installer.install(example_source_dir())?;
    let identity = LocalPublisherIdentity::generate(installed_skill.manifest.publisher.clone())?;
    identity.save(publisher_identity_path())?;
    let identity = LocalPublisherIdentity::load(publisher_identity_path())?;
    let registry_server =
        oci_registry_test_server::OciRegistryTestServer::start(registry_store_root());
    let reference = registry_reference(&registry_server);

    let registry_a = LocalRegistry::load(registry_a_root())?;
    registry_a.push_oci_registry(
        &installed_skill.resolved_ref,
        false,
        &reference,
        &registry_options(),
        &identity,
    )?;

    let untrusted_error =
        LocalRegistry::pull_oci_registry(registry_b_root(), &reference, &registry_options())
            .expect_err("untrusted pull should fail");
    println!("untrusted failure code: {}", untrusted_error.code);
    println!("untrusted failure message: {}", untrusted_error.message);

    LocalRegistry::trust_publisher(registry_b_root(), &identity.trusted_record())?;
    registry_server.tamper_blob(
        &installed_skill.manifest.package.artifact_digest,
        b"tampered artifact",
    );

    let tampered_error =
        LocalRegistry::pull_oci_registry(registry_b_root(), &reference, &registry_options())
            .expect_err("tampered pull should fail");
    println!("tampered failure code: {}", tampered_error.code);
    println!("tampered failure message: {}", tampered_error.message);
    println!("publisher: {}", identity.publisher.id);
    println!(
        "publisher identity: {}",
        publisher_identity_path().display()
    );
    println!("registry: {}", registry_server.url());
    println!(
        "artifact reference: {}/{}:0.1.0",
        reference.registry, reference.repository
    );

    Ok(())
}
