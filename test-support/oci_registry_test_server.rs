#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use registry_testkit::{RegistryConfig, RegistryServer};
use tokio::runtime::Builder;

pub struct OciRegistryTestServer {
    port: u16,
    storage_root: PathBuf,
    shutdown: Option<mpsc::Sender<()>>,
    handle: Option<JoinHandle<()>>,
}

impl OciRegistryTestServer {
    pub fn start(storage_root: impl AsRef<Path>) -> Self {
        let storage_root = storage_root.as_ref().to_path_buf();
        fs::create_dir_all(&storage_root).expect("local OCI registry storage root exists");

        let (ready_tx, ready_rx) = mpsc::channel();
        let (shutdown_tx, shutdown_rx) = mpsc::channel();
        let thread_storage_root = storage_root.clone();
        let handle = thread::spawn(move || {
            let runtime = Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("local OCI registry runtime builds");
            let server = runtime
                .block_on(RegistryServer::new(RegistryConfig::directory(
                    thread_storage_root,
                )))
                .expect("local OCI registry starts");
            ready_tx
                .send(server.port())
                .expect("registry server reports its port");
            let _ = shutdown_rx.recv();
            drop(server);
            runtime.shutdown_timeout(Duration::from_millis(10));
        });

        let port = ready_rx.recv().expect("registry server becomes ready");
        Self {
            port,
            storage_root,
            shutdown: Some(shutdown_tx),
            handle: Some(handle),
        }
    }

    pub fn host() -> &'static str {
        "127.0.0.1"
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn registry(&self) -> String {
        format!("{}:{}", Self::host(), self.port())
    }

    pub fn url(&self) -> String {
        format!("http://{}", self.registry())
    }

    pub fn reference(&self, repository: &str, tag: &str) -> String {
        format!("{}/{}:{tag}", self.registry(), repository)
    }

    pub fn storage_root(&self) -> &Path {
        &self.storage_root
    }

    pub fn blob_path(&self, digest: &str) -> PathBuf {
        self.storage_root
            .join("blobs")
            .join(digest.replace(':', "_"))
    }

    pub fn tag_manifest_path(&self, repository: &str, tag: &str) -> PathBuf {
        self.storage_root
            .join("manifests")
            .join(format!("{}.json", storage_key(repository, tag)))
    }

    pub fn digest_manifest_path(&self, repository: &str, digest: &str) -> PathBuf {
        self.storage_root
            .join("manifests")
            .join(format!("{}.json", storage_key(repository, digest)))
    }

    pub fn tamper_blob(&self, digest: &str, bytes: &[u8]) {
        fs::write(self.blob_path(digest), bytes).expect("blob tamper write succeeds");
    }

    pub fn tamper_tag_manifest(&self, repository: &str, tag: &str, bytes: &[u8]) {
        fs::write(self.tag_manifest_path(repository, tag), bytes)
            .expect("tag manifest tamper write succeeds");
    }

    pub fn tamper_digest_manifest(&self, repository: &str, digest: &str, bytes: &[u8]) {
        fs::write(self.digest_manifest_path(repository, digest), bytes)
            .expect("digest manifest tamper write succeeds");
    }

    pub fn manifest_bytes_for_tag(&self, repository: &str, tag: &str) -> Vec<u8> {
        fs::read(self.tag_manifest_path(repository, tag)).expect("tag manifest exists")
    }
}

impl Drop for OciRegistryTestServer {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn storage_key(repository: &str, reference: &str) -> String {
    format!("{repository}:{reference}").replace(['/', ':'], "_")
}
