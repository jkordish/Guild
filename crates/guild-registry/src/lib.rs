//! Registry model for publishing and resolving Guild skills.

use std::collections::{HashMap, HashSet};
use std::env;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use base64::Engine as _;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use guild_manifest::{
    InstalledDependencySpec, PublisherRef, SkillManifest, SourceBuildKind, SourceSkillManifest,
};
use guild_types::{
    CapabilityId, EvidenceEmissionRequest, EvidenceRecord, EvidenceRef, ExecutionRecord,
    RequestedSkillRef, ResolvedSkillRef, ResourceReadResult, SkillCategory,
};
use rand_core::OsRng;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

#[derive(Debug, Clone, PartialEq)]
pub struct InstalledSkill {
    pub manifest: SkillManifest,
    pub resolved_ref: ResolvedSkillRef,
    pub manifest_path: PathBuf,
    pub artifact_path: PathBuf,
    pub root_dir: PathBuf,
    pub verification: Option<InstalledVerificationRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct SearchQuery {
    pub query: String,
    pub limit: usize,
    pub category: Option<SkillCategory>,
    pub capability: Option<CapabilityId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct SearchResult {
    pub manifest: SkillManifest,
    pub resolved_ref: ResolvedSkillRef,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct InstalledSkillBundle {
    pub format_version: String,
    pub root_skill: ResolvedSkillRef,
    pub includes_dependency_closure: bool,
    pub publisher: PublisherRef,
    pub skills: Vec<InstalledBundleSkillEntry>,
    pub files: Vec<BundleFileEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct InstalledBundleSkillEntry {
    pub resolved_ref: ResolvedSkillRef,
    pub install_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct BundleFileEntry {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SignatureScheme {
    Ed25519,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct BundleSignatureEnvelope {
    pub format_version: String,
    pub scheme: SignatureScheme,
    pub publisher_id: String,
    pub bundle_sha256: String,
    pub signature_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct LocalPublisherIdentity {
    pub publisher: PublisherRef,
    pub scheme: SignatureScheme,
    pub public_key_base64: String,
    pub secret_key_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct TrustedPublisherRecord {
    pub publisher: PublisherRef,
    pub scheme: SignatureScheme,
    pub public_key_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum VerificationStatus {
    Verified,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct InstalledVerificationRecord {
    pub status: VerificationStatus,
    pub publisher: PublisherRef,
    pub scheme: SignatureScheme,
    pub bundle_sha256: String,
    pub signature: BundleSignatureEnvelope,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct RegistryError {
    pub code: String,
    pub message: String,
    pub detail: Option<Value>,
}

impl RegistryError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            detail: None,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<Value>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for RegistryError {}

impl LocalPublisherIdentity {
    pub fn generate(publisher: PublisherRef) -> Result<Self, RegistryError> {
        let signing_key = SigningKey::generate(&mut OsRng);
        Ok(Self {
            publisher,
            scheme: SignatureScheme::Ed25519,
            public_key_base64: base64_encode(&signing_key.verifying_key().to_bytes()),
            secret_key_base64: base64_encode(&signing_key.to_bytes()),
        })
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, RegistryError> {
        let path = path.as_ref();
        let contents = fs::read_to_string(path).map_err(|error| {
            RegistryError::new(
                "publisher-identity-read-failed",
                "failed to read local publisher identity file",
            )
            .with_detail(error.to_string())
        })?;
        let identity: LocalPublisherIdentity =
            serde_json::from_str(&contents).map_err(|error| {
                RegistryError::new(
                    "publisher-identity-parse-failed",
                    "failed to parse local publisher identity file",
                )
                .with_detail(error.to_string())
            })?;
        identity.signing_key()?;
        Ok(identity)
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), RegistryError> {
        write_json(path.as_ref(), self)
    }

    pub fn trusted_record(&self) -> TrustedPublisherRecord {
        TrustedPublisherRecord {
            publisher: self.publisher.clone(),
            scheme: self.scheme.clone(),
            public_key_base64: self.public_key_base64.clone(),
        }
    }

    fn signing_key(&self) -> Result<SigningKey, RegistryError> {
        match self.scheme {
            SignatureScheme::Ed25519 => {
                let secret = decode_fixed_base64::<32>(
                    &self.secret_key_base64,
                    "publisher-secret-key-invalid",
                    "local publisher secret key was invalid",
                )?;
                let public = decode_fixed_base64::<32>(
                    &self.public_key_base64,
                    "publisher-public-key-invalid",
                    "local publisher public key was invalid",
                )?;
                let signing_key = SigningKey::from_bytes(&secret);
                if signing_key.verifying_key().to_bytes() != public {
                    return Err(RegistryError::new(
                        "publisher-keypair-mismatch",
                        "local publisher secret key did not match the stored public key",
                    )
                    .with_detail(self.publisher.id.clone()));
                }
                Ok(signing_key)
            }
        }
    }
}

pub trait SkillRegistry {
    fn resolve(&self, skill: &RequestedSkillRef) -> Result<InstalledSkill, RegistryError>;

    fn resolve_exact(&self, skill: &ResolvedSkillRef) -> Result<InstalledSkill, RegistryError>;

    fn search(&self, query: &SearchQuery) -> Vec<SearchResult>;

    fn persist_execution_record(&self, record: &ExecutionRecord) -> Result<(), RegistryError>;

    fn load_execution_record(&self, execution_id: &str) -> Result<ExecutionRecord, RegistryError>;

    fn store_evidence(
        &self,
        request: &EvidenceEmissionRequest,
    ) -> Result<EvidenceRef, RegistryError>;

    fn load_evidence_record(&self, uri: &str) -> Result<EvidenceRecord, RegistryError>;

    fn read_resource(&self, uri: &str) -> Result<ResourceReadResult, RegistryError>;
}

#[derive(Debug, Clone)]
pub struct LocalRegistry {
    root: PathBuf,
    installed: Vec<InstalledSkill>,
}

#[derive(Debug, Clone)]
pub struct LocalSourceInstaller {
    root: PathBuf,
}

impl LocalRegistry {
    pub fn load(root: impl AsRef<Path>) -> Result<Self, RegistryError> {
        let root = root.as_ref();
        if let Some(error) = detect_source_skill_root(root)? {
            return Err(error);
        }

        let root = ensure_registry_layout(root)?;
        let mut installed = Vec::new();
        let installed_root = installed_root(&root);

        if installed_root.exists() {
            for entry in WalkDir::new(&installed_root).sort_by_file_name() {
                let entry = entry.map_err(|error| {
                    RegistryError::new(
                        "registry-scan-failed",
                        "failed while scanning registry root",
                    )
                    .with_detail(error.to_string())
                })?;

                if !entry.file_type().is_file() || entry.file_name() != "manifest.json" {
                    continue;
                }

                installed.push(Self::load_manifest(entry.path())?);
            }
        }

        Ok(Self { root, installed })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn installed(&self) -> &[InstalledSkill] {
        &self.installed
    }

    pub fn trust_publisher(
        root: impl AsRef<Path>,
        publisher: &TrustedPublisherRecord,
    ) -> Result<(), RegistryError> {
        ensure_registry_layout(root.as_ref())?;
        trusted_publisher_verifying_key(publisher)?;
        write_json(
            &trusted_publisher_path(root.as_ref(), &publisher.publisher.id),
            publisher,
        )
    }

    pub fn export_bundle(
        &self,
        root: &ResolvedSkillRef,
        include_dependencies: bool,
        bundle_root: impl AsRef<Path>,
        signer: &LocalPublisherIdentity,
    ) -> Result<InstalledSkillBundle, RegistryError> {
        let root = self.resolve_exact(root).map_err(|error| {
            RegistryError::new(
                "bundle-root-skill-not-found",
                "failed to resolve the installed root skill for bundle export",
            )
            .with_detail(serde_json::json!({
                "root_skill": root,
                "cause": {
                    "code": error.code,
                    "message": error.message,
                    "detail": error.detail,
                }
            }))
        })?;
        let bundled_skills = self.collect_bundle_skills(&root, include_dependencies)?;
        ensure_bundle_publisher_matches_signer(&bundled_skills, signer)?;
        let bundle_root = prepare_bundle_root(bundle_root)?;
        let mut entries = Vec::with_capacity(bundled_skills.len());
        let mut files = Vec::new();

        for installed in &bundled_skills {
            let install_dir = installed_relative_dir(&self.root, installed)?;
            let bundle_install_dir = bundle_root.join(&install_dir);
            files.extend(copy_installed_dir_for_bundle(
                &installed.root_dir,
                &bundle_install_dir,
                &install_dir,
            )?);
            entries.push(InstalledBundleSkillEntry {
                resolved_ref: installed.resolved_ref.clone(),
                install_dir: path_string(&install_dir)?,
            });
        }
        files.sort_by(|left, right| left.path.cmp(&right.path));

        let bundle = InstalledSkillBundle {
            format_version: BUNDLE_FORMAT_VERSION.into(),
            root_skill: root.resolved_ref,
            includes_dependency_closure: include_dependencies,
            publisher: signer.publisher.clone(),
            skills: entries,
            files,
        };
        let bundle_bytes = json_bytes(&bundle)?;
        write_bytes(&bundle_index_path(&bundle_root), &bundle_bytes)?;
        let envelope = sign_bundle_payload(signer, &bundle_bytes)?;
        write_json(&bundle_signature_path(&bundle_root), &envelope)?;
        Ok(bundle)
    }

    pub fn import_bundle(
        root: impl AsRef<Path>,
        bundle_root: impl AsRef<Path>,
    ) -> Result<Vec<InstalledSkill>, RegistryError> {
        let root = ensure_registry_layout(root)?;
        let bundle_root = open_bundle_directory(bundle_root)?;
        let bundle_bytes = read_bundle_index_bytes(&bundle_root)?;
        let bundle = parse_bundle_index(&bundle_bytes)?;
        let signature = read_bundle_signature(&bundle_root)?;
        let trusted_publisher = load_trusted_publisher(&root, &signature.publisher_id)?;
        verify_bundle_signature(&bundle_bytes, &bundle, &signature, &trusted_publisher)?;
        let validated = validate_bundle(&bundle_root, &bundle)?;
        let verification = InstalledVerificationRecord {
            status: VerificationStatus::Verified,
            publisher: bundle.publisher.clone(),
            scheme: signature.scheme.clone(),
            bundle_sha256: signature.bundle_sha256.clone(),
            signature: signature.clone(),
        };
        validate_import_targets(&root, &validated, &verification)?;

        let staging_root = import_staging_root(&root);
        if staging_root.exists() {
            fs::remove_dir_all(&staging_root).map_err(|error| {
                RegistryError::new(
                    "bundle-import-staging-cleanup-failed",
                    "failed to remove previous bundle import staging directory",
                )
                .with_detail(error.to_string())
            })?;
        }
        fs::create_dir_all(&staging_root).map_err(|error| {
            RegistryError::new(
                "bundle-import-staging-create-failed",
                "failed to create bundle import staging directory",
            )
            .with_detail(error.to_string())
        })?;

        let staged_result = (|| -> Result<Vec<InstalledSkill>, RegistryError> {
            let mut imported = Vec::with_capacity(bundle.skills.len());

            for entry in &bundle.skills {
                let validated_skill = validated
                    .iter()
                    .find(|candidate| candidate.entry.resolved_ref == entry.resolved_ref)
                    .expect("validated bundle contains every indexed skill");
                let install_dir = bundle_install_dir_relative(&validated_skill.entry)?;
                let target_dir = root.join(&install_dir);
                let staged_dir = staging_root.join(&install_dir);
                copy_dir_recursive(&validated_skill.install_dir, &staged_dir)?;
                if let Some(parent) = target_dir.parent() {
                    fs::create_dir_all(parent).map_err(|error| {
                        RegistryError::new(
                            "bundle-import-target-create-failed",
                            "failed to create target directory for imported skill",
                        )
                        .with_detail(error.to_string())
                    })?;
                }
                if target_dir.exists() {
                    write_json(&installed_verification_path(&target_dir), &verification)?;
                    imported.push(LocalRegistry::load_manifest(
                        &target_dir.join("manifest.json"),
                    )?);
                } else {
                    fs::rename(&staged_dir, &target_dir).map_err(|error| {
                        RegistryError::new(
                            "bundle-import-move-failed",
                            "failed to move validated bundled skill into the target registry",
                        )
                        .with_detail(serde_json::json!({
                            "from": staged_dir.display().to_string(),
                            "to": target_dir.display().to_string(),
                            "cause": error.to_string(),
                        }))
                    })?;
                    write_json(&installed_verification_path(&target_dir), &verification)?;
                    imported.push(LocalRegistry::load_manifest(
                        &target_dir.join("manifest.json"),
                    )?);
                }
            }

            Ok(imported)
        })();

        let cleanup_result = if staging_root.exists() {
            fs::remove_dir_all(&staging_root).map_err(|error| {
                RegistryError::new(
                    "bundle-import-staging-cleanup-failed",
                    "failed to clean bundle import staging directory",
                )
                .with_detail(error.to_string())
            })
        } else {
            Ok(())
        };

        match (staged_result, cleanup_result) {
            (Ok(imported), Ok(())) => Ok(imported),
            (Err(error), _) => Err(error),
            (Ok(_), Err(error)) => Err(error),
        }
    }

    fn collect_bundle_skills(
        &self,
        root: &InstalledSkill,
        include_dependencies: bool,
    ) -> Result<Vec<InstalledSkill>, RegistryError> {
        let mut stack = vec![root.clone()];
        let mut seen = HashSet::new();
        let mut bundled = Vec::new();

        while let Some(installed) = stack.pop() {
            if !seen.insert(installed.resolved_ref.clone()) {
                continue;
            }

            if include_dependencies {
                for dependency in &installed.manifest.dependencies {
                    let child = self.resolve_exact(&dependency.skill).map_err(|error| {
                        RegistryError::new(
                            "bundle-export-dependency-missing",
                            "failed to resolve a declared installed dependency while building a bundle closure",
                        )
                        .with_detail(serde_json::json!({
                            "root_skill": root.resolved_ref,
                            "dependency_alias": dependency.alias,
                            "dependency": dependency.skill,
                            "cause": {
                                "code": error.code,
                                "message": error.message,
                                "detail": error.detail,
                            }
                        }))
                    })?;
                    stack.push(child);
                }
            }

            bundled.push(installed);
        }

        bundled.sort_by_key(bundle_skill_sort_key);

        Ok(bundled)
    }

    fn load_manifest(path: &Path) -> Result<InstalledSkill, RegistryError> {
        let manifest_path = path.to_path_buf();
        let root_dir = manifest_path
            .parent()
            .ok_or_else(|| {
                RegistryError::new(
                    "manifest-path-invalid",
                    "manifest.json must live inside a skill directory",
                )
            })?
            .to_path_buf();

        let manifest = read_installed_manifest(&manifest_path)?;
        validate_installed_manifest(&manifest)?;

        let artifact_path =
            resolve_local_file(&root_dir, &manifest.package.artifact_uri).map_err(|error| {
                RegistryError::new(
                    "artifact-uri-invalid",
                    "local registry only supports relative artifact paths",
                )
                .with_detail(error.to_string())
            })?;

        if !artifact_path.exists() {
            return Err(
                RegistryError::new("artifact-missing", "artifact file does not exist")
                    .with_detail(artifact_path.display().to_string()),
            );
        }

        let digest = sha256_file(&artifact_path)?;
        if digest != manifest.package.artifact_digest {
            return Err(RegistryError::new(
                "artifact-digest-mismatch",
                "artifact digest does not match manifest",
            )
            .with_detail(serde_json::json!({
                "expected": manifest.package.artifact_digest,
                "actual": digest,
                "artifact_path": artifact_path.display().to_string(),
            })));
        }
        validate_staged_support_files(&root_dir, &manifest)?;
        let verification = load_verification_record(&root_dir)?;

        Ok(InstalledSkill {
            resolved_ref: ResolvedSkillRef {
                key: manifest.key.clone(),
                version: manifest.version.clone(),
                digest,
            },
            manifest,
            manifest_path,
            artifact_path,
            root_dir,
            verification,
        })
    }
}

impl SkillRegistry for LocalRegistry {
    fn resolve(&self, skill: &RequestedSkillRef) -> Result<InstalledSkill, RegistryError> {
        let mut matches: Vec<&InstalledSkill> = self
            .installed
            .iter()
            .filter(|installed| installed.manifest.key == skill.key)
            .filter(|installed| {
                skill
                    .version_req
                    .as_semver()
                    .matches(installed.resolved_ref.version.as_semver())
            })
            .collect();

        if matches.is_empty() {
            let has_name_match = self
                .installed
                .iter()
                .any(|installed| installed.manifest.key == skill.key);

            let error = if has_name_match {
                RegistryError::new(
                    "skill-version-not-found",
                    "no installed skill version satisfied the requested version requirement",
                )
            } else {
                RegistryError::new(
                    "skill-not-found",
                    "requested skill was not found in registry",
                )
            };

            return Err(error.with_detail(serde_json::json!({
                "namespace": skill.key.namespace,
                "name": skill.key.name,
                "version_req": skill.version_req.to_string(),
            })));
        }

        matches.sort_by(|left, right| left.resolved_ref.version.cmp(&right.resolved_ref.version));
        Ok(matches
            .last()
            .expect("non-empty version matches")
            .to_owned()
            .clone())
    }

    fn resolve_exact(&self, skill: &ResolvedSkillRef) -> Result<InstalledSkill, RegistryError> {
        if let Some(installed) = self
            .installed
            .iter()
            .find(|installed| installed.resolved_ref == *skill)
        {
            return Ok(installed.clone());
        }

        let has_key_and_version = self.installed.iter().any(|installed| {
            installed.resolved_ref.key == skill.key
                && installed.resolved_ref.version == skill.version
        });

        let error = if has_key_and_version {
            RegistryError::new(
                "skill-digest-not-found",
                "no installed skill matched the requested digest-pinned reference",
            )
        } else {
            RegistryError::new(
                "skill-not-found",
                "requested resolved skill was not found in registry",
            )
        };

        Err(error.with_detail(serde_json::json!({
            "namespace": skill.key.namespace,
            "name": skill.key.name,
            "version": skill.version.to_string(),
            "digest": skill.digest,
        })))
    }

    fn search(&self, query: &SearchQuery) -> Vec<SearchResult> {
        let needle = query.query.to_lowercase();

        self.installed
            .iter()
            .filter(|installed| {
                query
                    .category
                    .as_ref()
                    .is_none_or(|category| &installed.manifest.behavior.category == category)
            })
            .filter(|installed| {
                query.capability.as_ref().is_none_or(|capability| {
                    installed
                        .manifest
                        .capabilities
                        .iter()
                        .any(|requirement| &requirement.id == capability)
                })
            })
            .filter(|installed| {
                needle.is_empty()
                    || installed
                        .manifest
                        .display_name
                        .to_lowercase()
                        .contains(&needle)
                    || installed
                        .manifest
                        .description
                        .to_lowercase()
                        .contains(&needle)
                    || installed
                        .manifest
                        .key
                        .namespace
                        .to_lowercase()
                        .contains(&needle)
                    || installed.manifest.key.name.to_lowercase().contains(&needle)
            })
            .take(query.limit)
            .map(|installed| SearchResult {
                manifest: installed.manifest.clone(),
                resolved_ref: installed.resolved_ref.clone(),
            })
            .collect()
    }

    fn persist_execution_record(&self, record: &ExecutionRecord) -> Result<(), RegistryError> {
        let expected_uri = execution_resource_uri(&record.receipt.execution_id);
        if record.receipt.uri != expected_uri {
            return Err(RegistryError::new(
                "execution-uri-mismatch",
                "execution record URI did not match the host-issued execution URI",
            )
            .with_detail(serde_json::json!({
                "expected": expected_uri,
                "actual": record.receipt.uri,
                "execution_id": record.receipt.execution_id,
            })));
        }

        let path = execution_path(&self.root, &record.receipt.execution_id);
        write_json(&path, record)
    }

    fn load_execution_record(&self, execution_id: &str) -> Result<ExecutionRecord, RegistryError> {
        let path = execution_path(&self.root, execution_id);
        let contents = fs::read_to_string(&path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                RegistryError::new(
                    "execution-not-found",
                    "execution record was not found in the local execution store",
                )
                .with_detail(serde_json::json!({
                    "execution_id": execution_id,
                    "path": path.display().to_string(),
                }))
            } else {
                RegistryError::new("execution-read-failed", "failed to read execution record")
                    .with_detail(error.to_string())
            }
        })?;

        serde_json::from_str(&contents).map_err(|error| {
            RegistryError::new(
                "execution-parse-failed",
                "failed to parse stored execution record JSON",
            )
            .with_detail(error.to_string())
        })
    }

    fn store_evidence(
        &self,
        request: &EvidenceEmissionRequest,
    ) -> Result<EvidenceRef, RegistryError> {
        if request.mime_type.trim().is_empty() {
            return Err(RegistryError::new(
                "object-mime-type-invalid",
                "evidence payloads must declare a non-empty mime type",
            ));
        }

        let digest_hex = sha256_bytes(&request.payload);
        let digest_label = format!("sha256:{digest_hex}");
        let uri = object_resource_uri(&digest_hex);
        let object_dir = object_path(&self.root, &digest_hex);
        let payload_path = object_dir.join("payload");
        let metadata_path = object_dir.join("metadata.json");

        fs::create_dir_all(&object_dir).map_err(|error| {
            RegistryError::new(
                "object-store-create-failed",
                "failed to create object store directory",
            )
            .with_detail(error.to_string())
        })?;

        if payload_path.exists() && metadata_path.exists() {
            let metadata: EvidenceRecord =
                serde_json::from_str(&fs::read_to_string(&metadata_path).map_err(|error| {
                    RegistryError::new(
                        "object-metadata-read-failed",
                        "failed to read stored object metadata",
                    )
                    .with_detail(error.to_string())
                })?)
                .map_err(|error| {
                    RegistryError::new(
                        "object-metadata-parse-failed",
                        "failed to parse stored object metadata",
                    )
                    .with_detail(error.to_string())
                })?;

            if metadata.mime_type != request.mime_type {
                return Err(RegistryError::new(
                    "object-metadata-conflict",
                    "stored object metadata conflicted with a new evidence emission request",
                )
                .with_detail(serde_json::json!({
                    "uri": uri,
                    "existing_mime_type": metadata.mime_type,
                    "requested_mime_type": request.mime_type,
                })));
            }
        } else {
            fs::write(&payload_path, &request.payload).map_err(|error| {
                RegistryError::new(
                    "object-payload-write-failed",
                    "failed to persist evidence payload",
                )
                .with_detail(error.to_string())
            })?;

            write_json(
                &metadata_path,
                &EvidenceRecord {
                    uri: uri.clone(),
                    mime_type: request.mime_type.clone(),
                    sha256: digest_label.clone(),
                    size_bytes: request.payload.len() as u64,
                    title: request.title.clone(),
                    audience: request.audience.clone(),
                    redaction: request.redaction.clone(),
                    freshness: request.freshness.clone(),
                    produced_by_execution: None,
                },
            )?;
        }

        Ok(EvidenceRef {
            uri,
            title: request.title.clone(),
            mime_type: Some(request.mime_type.clone()),
            sha256: Some(digest_label),
            audience: request.audience.clone(),
            redaction: request.redaction.clone(),
            freshness: request.freshness.clone(),
        })
    }

    fn load_evidence_record(&self, uri: &str) -> Result<EvidenceRecord, RegistryError> {
        let GuildUri::ObjectSha256 { digest_hex } = parse_guild_uri(uri)? else {
            return Err(RegistryError::new(
                "resource-kind-mismatch",
                "evidence records are only available for Guild object URIs",
            )
            .with_detail(serde_json::json!({ "uri": uri })));
        };

        let metadata_path = object_path(&self.root, &digest_hex).join("metadata.json");
        let contents = fs::read_to_string(&metadata_path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                RegistryError::new(
                    "object-not-found",
                    "evidence record was not found in the local object store",
                )
                .with_detail(serde_json::json!({
                    "uri": uri,
                    "path": metadata_path.display().to_string(),
                }))
            } else {
                RegistryError::new(
                    "object-metadata-read-failed",
                    "failed to read evidence record metadata",
                )
                .with_detail(error.to_string())
            }
        })?;

        serde_json::from_str(&contents).map_err(|error| {
            RegistryError::new(
                "object-metadata-parse-failed",
                "failed to parse evidence record metadata",
            )
            .with_detail(error.to_string())
        })
    }

    fn read_resource(&self, uri: &str) -> Result<ResourceReadResult, RegistryError> {
        match parse_guild_uri(uri)? {
            GuildUri::Execution { execution_id } => {
                let record = self.load_execution_record(&execution_id)?;
                let bytes = serde_json::to_vec_pretty(&record).map_err(|error| {
                    RegistryError::new(
                        "execution-serialize-failed",
                        "failed to serialize stored execution record",
                    )
                    .with_detail(error.to_string())
                })?;
                Ok(ResourceReadResult {
                    uri: uri.to_owned(),
                    mime_type: "application/json".into(),
                    sha256: Some(format!("sha256:{}", sha256_bytes(&bytes))),
                    bytes,
                })
            }
            GuildUri::ObjectSha256 { digest_hex } => {
                let object_dir = object_path(&self.root, &digest_hex);
                let payload_path = object_dir.join("payload");
                let metadata_path = object_dir.join("metadata.json");

                let bytes = fs::read(&payload_path).map_err(|error| {
                    if error.kind() == std::io::ErrorKind::NotFound {
                        RegistryError::new(
                            "object-not-found",
                            "evidence object was not found in the local object store",
                        )
                        .with_detail(serde_json::json!({
                            "uri": uri,
                            "path": payload_path.display().to_string(),
                        }))
                    } else {
                        RegistryError::new(
                            "object-read-failed",
                            "failed to read evidence object payload",
                        )
                        .with_detail(error.to_string())
                    }
                })?;

                let metadata: EvidenceRecord =
                    serde_json::from_str(&fs::read_to_string(&metadata_path).map_err(|error| {
                        RegistryError::new(
                            "object-metadata-read-failed",
                            "failed to read evidence object metadata",
                        )
                        .with_detail(error.to_string())
                    })?)
                    .map_err(|error| {
                        RegistryError::new(
                            "object-metadata-parse-failed",
                            "failed to parse evidence object metadata",
                        )
                        .with_detail(error.to_string())
                    })?;

                Ok(ResourceReadResult {
                    uri: uri.to_owned(),
                    mime_type: metadata.mime_type,
                    sha256: Some(metadata.sha256),
                    bytes,
                })
            }
        }
    }
}

impl LocalSourceInstaller {
    pub fn new(root: impl AsRef<Path>) -> Result<Self, RegistryError> {
        Ok(Self {
            root: ensure_registry_layout(root)?,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn install(&self, source_dir: impl AsRef<Path>) -> Result<InstalledSkill, RegistryError> {
        let source_dir = open_existing_directory(source_dir)?;
        let source_manifest_path = source_dir.join("manifest.json");
        let source_manifest = read_source_manifest(&source_manifest_path)?;
        validate_source_manifest(&source_manifest)?;
        let dependency_registry = LocalRegistry::load(&self.root)?;
        let installed_dependencies =
            resolve_source_dependencies(&dependency_registry, &source_manifest)?;

        let built_artifact = build_source_artifact(&source_dir, &source_manifest)?;
        let digest = sha256_file(&built_artifact)?;
        let install_root = install_root_for(&self.root, &source_manifest);

        if install_root.exists() {
            fs::remove_dir_all(&install_root).map_err(|error| {
                RegistryError::new(
                    "install-root-cleanup-failed",
                    "failed to remove previous install for skill version",
                )
                .with_detail(error.to_string())
            })?;
        }

        let install_dir = install_root.join(digest_dir(&digest));
        fs::create_dir_all(&install_dir).map_err(|error| {
            RegistryError::new(
                "install-root-create-failed",
                "failed to create install directory",
            )
            .with_detail(error.to_string())
        })?;

        let staged_artifact = install_dir.join("component.wasm");
        fs::copy(&built_artifact, &staged_artifact).map_err(|error| {
            RegistryError::new("artifact-stage-failed", "failed to stage built artifact")
                .with_detail(error.to_string())
        })?;

        stage_support_files(&source_dir, &install_dir, &source_manifest)?;

        let installed_manifest = source_manifest.into_installed(
            "./component.wasm",
            digest.clone(),
            installed_dependencies,
        );
        let installed_manifest_path = install_dir.join("manifest.json");
        write_json(&installed_manifest_path, &installed_manifest)?;

        LocalRegistry::load_manifest(&installed_manifest_path)
    }
}

fn resolve_source_dependencies(
    registry: &LocalRegistry,
    manifest: &SourceSkillManifest,
) -> Result<Vec<InstalledDependencySpec>, RegistryError> {
    manifest
        .dependencies
        .iter()
        .map(|dependency| {
            registry
                .resolve(&dependency.skill)
                .map(|installed| InstalledDependencySpec {
                    alias: dependency.alias.clone(),
                    skill: installed.resolved_ref,
                })
                .map_err(|error| {
                    RegistryError::new(
                        "dependency-resolution-failed",
                        "failed to resolve declared dependency during install",
                    )
                    .with_detail(serde_json::json!({
                        "alias": dependency.alias,
                        "dependency": dependency.skill,
                        "cause": {
                            "code": error.code,
                            "message": error.message,
                            "detail": error.detail,
                        }
                    }))
                })
        })
        .collect()
}

fn detect_source_skill_root(root: &Path) -> Result<Option<RegistryError>, RegistryError> {
    let manifest_path = root.join("manifest.json");
    if !manifest_path.exists() {
        return Ok(None);
    }

    let contents = fs::read_to_string(&manifest_path).map_err(|error| {
        RegistryError::new("manifest-read-failed", "failed to read manifest file")
            .with_detail(error.to_string())
    })?;

    if serde_json::from_str::<SourceSkillManifest>(&contents).is_ok() {
        return Ok(Some(
            RegistryError::new(
                "source-skill-not-installed",
                "source manifests are not executable; install the skill into a local registry first",
            )
            .with_detail(manifest_path.display().to_string()),
        ));
    }

    Ok(None)
}

fn ensure_registry_layout(path: impl AsRef<Path>) -> Result<PathBuf, RegistryError> {
    let path = path.as_ref();
    fs::create_dir_all(path).map_err(|error| {
        RegistryError::new("directory-create-failed", "failed to create directory")
            .with_detail(error.to_string())
    })?;

    let root = path.canonicalize().map_err(|error| {
        RegistryError::new("directory-open-failed", "failed to open directory")
            .with_detail(error.to_string())
    })?;

    for subdir in [
        installed_root(&root),
        executions_root(&root),
        objects_root(&root),
        trusted_publishers_root(&root),
    ] {
        fs::create_dir_all(&subdir).map_err(|error| {
            RegistryError::new(
                "directory-create-failed",
                "failed to create local registry storage directory",
            )
            .with_detail(error.to_string())
        })?;
    }

    Ok(root)
}

fn open_existing_directory(path: impl AsRef<Path>) -> Result<PathBuf, RegistryError> {
    let path = path.as_ref();

    if !path.exists() {
        return Err(RegistryError::new(
            "source-root-missing",
            "source skill directory does not exist",
        )
        .with_detail(path.display().to_string()));
    }

    path.canonicalize().map_err(|error| {
        RegistryError::new(
            "source-root-open-failed",
            "failed to open source skill directory",
        )
        .with_detail(error.to_string())
    })
}

fn install_root_for(root: &Path, manifest: &SourceSkillManifest) -> PathBuf {
    installed_root(root)
        .join(&manifest.key.namespace)
        .join(&manifest.key.name)
        .join(manifest.version.to_string())
}

fn digest_dir(digest: &str) -> String {
    digest.replace(':', "-")
}

fn stage_support_files(
    source_root: &Path,
    install_root: &Path,
    manifest: &SourceSkillManifest,
) -> Result<(), RegistryError> {
    copy_relative_file(
        source_root,
        install_root,
        &manifest.interface.input_schema_uri,
    )?;
    copy_relative_file(
        source_root,
        install_root,
        &manifest.interface.output_schema_uri,
    )?;

    if let Some(examples_uri) = &manifest.interface.examples_uri {
        copy_relative_file(source_root, install_root, examples_uri)?;
    }

    if let Some(sbom_uri) = &manifest.package.sbom_uri {
        copy_optional_relative_file(source_root, install_root, sbom_uri)?;
    }

    if let Some(signature_uri) = &manifest.package.signature_uri {
        copy_optional_relative_file(source_root, install_root, signature_uri)?;
    }

    for test in &manifest.tests {
        copy_relative_file(source_root, install_root, &test.fixtures_uri)?;
        copy_relative_file(source_root, install_root, &test.expected_output_uri)?;
    }

    Ok(())
}

fn copy_relative_file(
    source_root: &Path,
    install_root: &Path,
    uri: &str,
) -> Result<(), RegistryError> {
    let source = resolve_local_file(source_root, uri).map_err(|error| {
        RegistryError::new(
            "source-file-uri-invalid",
            "source manifest referenced an unsupported local file URI",
        )
        .with_detail(error.to_string())
    })?;

    if !source.exists() {
        return Err(RegistryError::new(
            "source-file-missing",
            "source manifest referenced a file that does not exist",
        )
        .with_detail(source.display().to_string()));
    }

    let destination = install_root.join(uri);
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            RegistryError::new(
                "install-support-dir-create-failed",
                "failed to create staged support file directory",
            )
            .with_detail(error.to_string())
        })?;
    }

    fs::copy(source, destination).map_err(|error| {
        RegistryError::new(
            "install-support-file-copy-failed",
            "failed to stage support file",
        )
        .with_detail(error.to_string())
    })?;

    Ok(())
}

fn copy_optional_relative_file(
    source_root: &Path,
    install_root: &Path,
    uri: &str,
) -> Result<(), RegistryError> {
    if maybe_resolve_local_file(source_root, uri)
        .map_err(|error| {
            RegistryError::new(
                "source-file-uri-invalid",
                "source manifest referenced an unsupported local file URI",
            )
            .with_detail(error.to_string())
        })?
        .is_none()
    {
        return Ok(());
    }

    copy_relative_file(source_root, install_root, uri)
}

fn build_source_artifact(
    source_root: &Path,
    manifest: &SourceSkillManifest,
) -> Result<PathBuf, RegistryError> {
    if manifest.build.kind != SourceBuildKind::CargoWasmComponent {
        return Err(RegistryError::new(
            "unsupported-build-kind",
            "local installer only supports cargo-wasm-component builds",
        ));
    }

    let cargo_manifest = resolve_local_file(source_root, &manifest.build.cargo_manifest_path)
        .map_err(|error| {
            RegistryError::new(
                "build-manifest-path-invalid",
                "build.cargo_manifest_path must be a relative local path",
            )
            .with_detail(error.to_string())
        })?;

    if !cargo_manifest.exists() {
        return Err(RegistryError::new(
            "build-manifest-missing",
            "build.cargo_manifest_path does not exist",
        )
        .with_detail(cargo_manifest.display().to_string()));
    }

    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let output = Command::new(cargo)
        .arg("build")
        .arg("--manifest-path")
        .arg(&cargo_manifest)
        .arg("--target")
        .arg(&manifest.build.target)
        .arg("--release")
        .arg("--message-format=json-render-diagnostics")
        .output()
        .map_err(|error| {
            RegistryError::new("build-command-failed", "failed to invoke cargo build")
                .with_detail(error.to_string())
        })?;

    if !output.status.success() {
        return Err(
            RegistryError::new("build-failed", "cargo build failed for the source skill")
                .with_detail(serde_json::json!({
                    "status": output.status.code(),
                    "stdout": String::from_utf8_lossy(&output.stdout),
                    "stderr": String::from_utf8_lossy(&output.stderr),
                })),
        );
    }

    let cargo_manifest = cargo_manifest.canonicalize().map_err(|error| {
        RegistryError::new(
            "build-manifest-canonicalize-failed",
            "failed to canonicalize build manifest path",
        )
        .with_detail(error.to_string())
    })?;

    find_built_wasm_artifact(&cargo_manifest, &output.stdout)
}

fn find_built_wasm_artifact(
    cargo_manifest: &Path,
    stdout: &[u8],
) -> Result<PathBuf, RegistryError> {
    let mut artifact = None;
    let stdout = String::from_utf8_lossy(stdout);

    for line in stdout.lines() {
        let Ok(message) = serde_json::from_str::<Value>(line) else {
            continue;
        };

        if message.get("reason").and_then(Value::as_str) != Some("compiler-artifact") {
            continue;
        }

        if message.get("manifest_path").and_then(Value::as_str) != cargo_manifest.to_str() {
            continue;
        }

        let Some(filenames) = message.get("filenames").and_then(Value::as_array) else {
            continue;
        };

        for filename in filenames {
            let Some(filename) = filename.as_str() else {
                continue;
            };

            if filename.ends_with(".wasm") {
                artifact = Some(PathBuf::from(filename));
            }
        }
    }

    let artifact = artifact.ok_or_else(|| {
        RegistryError::new(
            "build-artifact-missing",
            "cargo build did not report a wasm artifact for the source skill",
        )
        .with_detail(cargo_manifest.display().to_string())
    })?;

    if !artifact.exists() {
        return Err(RegistryError::new(
            "build-artifact-missing",
            "cargo reported a wasm artifact that does not exist",
        )
        .with_detail(artifact.display().to_string()));
    }

    artifact.canonicalize().map_err(|error| {
        RegistryError::new(
            "build-artifact-canonicalize-failed",
            "failed to canonicalize built wasm artifact path",
        )
        .with_detail(error.to_string())
    })
}

fn read_installed_manifest(path: &Path) -> Result<SkillManifest, RegistryError> {
    let contents = fs::read_to_string(path).map_err(|error| {
        RegistryError::new("manifest-read-failed", "failed to read manifest file")
            .with_detail(error.to_string())
    })?;

    match serde_json::from_str(&contents) {
        Ok(manifest) => Ok(manifest),
        Err(error) => {
            if serde_json::from_str::<SourceSkillManifest>(&contents).is_ok() {
                Err(RegistryError::new(
                    "source-skill-not-installed",
                    "source manifests are not executable; install the skill into a local registry first",
                )
                .with_detail(path.display().to_string()))
            } else {
                Err(RegistryError::new(
                    "manifest-parse-failed",
                    "failed to parse installed manifest JSON",
                )
                .with_detail(error.to_string()))
            }
        }
    }
}

fn read_source_manifest(path: &Path) -> Result<SourceSkillManifest, RegistryError> {
    let contents = fs::read_to_string(path).map_err(|error| {
        RegistryError::new(
            "source-manifest-read-failed",
            "failed to read source manifest file",
        )
        .with_detail(error.to_string())
    })?;

    serde_json::from_str(&contents).map_err(|error| {
        RegistryError::new(
            "source-manifest-parse-failed",
            "failed to parse source manifest JSON",
        )
        .with_detail(error.to_string())
    })
}

fn validate_installed_manifest(manifest: &SkillManifest) -> Result<(), RegistryError> {
    manifest.validate().map_err(|errors| {
        RegistryError::new("invalid-manifest", "manifest validation failed")
            .with_detail(serde_json::to_value(errors).expect("validation errors serialize"))
    })
}

fn validate_source_manifest(manifest: &SourceSkillManifest) -> Result<(), RegistryError> {
    manifest.validate().map_err(|errors| {
        RegistryError::new(
            "invalid-source-manifest",
            "source manifest validation failed",
        )
        .with_detail(serde_json::to_value(errors).expect("validation errors serialize"))
    })
}

fn resolve_local_file(root: &Path, uri: &str) -> Result<PathBuf, &'static str> {
    if uri.contains("://") {
        return Err("URI schemes are unsupported in the local registry");
    }

    let path = Path::new(uri);
    if path.is_absolute() {
        return Err("absolute artifact paths are unsupported in the local registry");
    }

    Ok(root.join(path))
}

fn maybe_resolve_local_file(root: &Path, uri: &str) -> Result<Option<PathBuf>, &'static str> {
    if uri.contains("://") {
        return Ok(None);
    }

    let path = Path::new(uri);
    if path.is_absolute() {
        return Err("absolute artifact paths are unsupported in the local registry");
    }

    Ok(Some(root.join(path)))
}

fn sha256_file(path: &Path) -> Result<String, RegistryError> {
    let bytes = fs::read(path).map_err(|error| {
        RegistryError::new("artifact-read-failed", "failed to read artifact file")
            .with_detail(error.to_string())
    })?;

    Ok(format!("sha256:{}", sha256_bytes(&bytes)))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), RegistryError> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        RegistryError::new("json-serialize-failed", "failed to serialize JSON")
            .with_detail(error.to_string())
    })?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            RegistryError::new(
                "json-write-dir-failed",
                "failed to create JSON parent directory",
            )
            .with_detail(error.to_string())
        })?;
    }

    fs::write(path, bytes).map_err(|error| {
        RegistryError::new("json-write-failed", "failed to write JSON file")
            .with_detail(error.to_string())
    })
}

fn installed_root(root: &Path) -> PathBuf {
    root.join("installed")
}

fn executions_root(root: &Path) -> PathBuf {
    root.join("executions")
}

fn objects_root(root: &Path) -> PathBuf {
    root.join("objects").join("sha256")
}

fn import_staging_root(root: &Path) -> PathBuf {
    root.join(".bundle-import-staging")
}

fn trust_root(root: &Path) -> PathBuf {
    root.join("trust")
}

fn trusted_publishers_root(root: &Path) -> PathBuf {
    trust_root(root).join("publishers")
}

fn trusted_publisher_path(root: &Path, publisher_id: &str) -> PathBuf {
    trusted_publishers_root(root).join(format!("{}.json", percent_encode_component(publisher_id)))
}

fn bundle_index_path(bundle_root: &Path) -> PathBuf {
    bundle_root.join("bundle.json")
}

fn bundle_signature_path(bundle_root: &Path) -> PathBuf {
    bundle_root.join("bundle.signature.json")
}

fn installed_verification_path(install_root: &Path) -> PathBuf {
    install_root.join("verification.json")
}

fn execution_path(root: &Path, execution_id: &str) -> PathBuf {
    executions_root(root).join(format!("{}.json", percent_encode_component(execution_id)))
}

pub fn execution_resource_uri(execution_id: &str) -> String {
    format!(
        "guild://executions/{}",
        percent_encode_component(execution_id)
    )
}

fn object_path(root: &Path, digest_hex: &str) -> PathBuf {
    objects_root(root).join(digest_hex)
}

pub fn object_resource_uri(digest_hex: &str) -> String {
    format!("guild://objects/sha256/{digest_hex}")
}

enum GuildUri {
    Execution { execution_id: String },
    ObjectSha256 { digest_hex: String },
}

fn parse_guild_uri(uri: &str) -> Result<GuildUri, RegistryError> {
    if let Some(encoded) = uri.strip_prefix("guild://executions/") {
        let execution_id = percent_decode_component(encoded).map_err(|error| {
            RegistryError::new(
                "resource-uri-invalid",
                "execution resource URI contained invalid percent encoding",
            )
            .with_detail(serde_json::json!({ "uri": uri, "error": error }))
        })?;
        return Ok(GuildUri::Execution { execution_id });
    }

    if let Some(digest_hex) = uri.strip_prefix("guild://objects/sha256/") {
        if digest_hex.is_empty() || !digest_hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
            return Err(RegistryError::new(
                "resource-uri-invalid",
                "object resource URI must contain a lowercase hexadecimal sha256 digest",
            )
            .with_detail(uri.to_owned()));
        }

        return Ok(GuildUri::ObjectSha256 {
            digest_hex: digest_hex.to_owned(),
        });
    }

    Err(RegistryError::new(
        "resource-uri-invalid",
        "resource URI did not match a supported local Guild resource",
    )
    .with_detail(uri.to_owned()))
}

fn percent_encode_component(input: &str) -> String {
    let mut encoded = String::with_capacity(input.len());

    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }

    encoded
}

fn percent_decode_component(input: &str) -> Result<String, &'static str> {
    let mut bytes = Vec::with_capacity(input.len());
    let mut chars = input.as_bytes().iter().copied();

    while let Some(byte) = chars.next() {
        if byte == b'%' {
            let high = chars.next().ok_or("truncated escape sequence")?;
            let low = chars.next().ok_or("truncated escape sequence")?;
            let high = hex_nibble(high).ok_or("invalid escape sequence")?;
            let low = hex_nibble(low).ok_or("invalid escape sequence")?;
            bytes.push((high << 4) | low);
        } else {
            bytes.push(byte);
        }
    }

    String::from_utf8(bytes).map_err(|_| "decoded component was not valid UTF-8")
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

const BUNDLE_FORMAT_VERSION: &str = "guild-installed-bundle-v2";
const BUNDLE_SIGNATURE_FORMAT_VERSION: &str = "guild-installed-bundle-signature-v1";
const VERIFICATION_FILENAME: &str = "verification.json";

#[derive(Debug, Clone)]
struct ValidatedBundleSkill {
    entry: InstalledBundleSkillEntry,
    install_dir: PathBuf,
    installed: InstalledSkill,
}

fn bundle_skill_sort_key(installed: &InstalledSkill) -> (String, String, String, String) {
    (
        installed.resolved_ref.key.namespace.clone(),
        installed.resolved_ref.key.name.clone(),
        installed.resolved_ref.version.to_string(),
        installed.resolved_ref.digest.clone(),
    )
}

fn prepare_bundle_root(path: impl AsRef<Path>) -> Result<PathBuf, RegistryError> {
    let path = path.as_ref();
    if path.exists() {
        if !path.is_dir() {
            return Err(RegistryError::new(
                "bundle-root-invalid",
                "bundle export target must be a directory",
            )
            .with_detail(path.display().to_string()));
        }

        let mut entries = fs::read_dir(path).map_err(|error| {
            RegistryError::new(
                "bundle-root-read-failed",
                "failed to inspect bundle export target directory",
            )
            .with_detail(error.to_string())
        })?;
        if entries.next().is_some() {
            return Err(RegistryError::new(
                "bundle-root-not-empty",
                "bundle export target directory must be empty",
            )
            .with_detail(path.display().to_string()));
        }
    } else {
        fs::create_dir_all(path).map_err(|error| {
            RegistryError::new(
                "bundle-root-create-failed",
                "failed to create bundle export target directory",
            )
            .with_detail(error.to_string())
        })?;
    }

    path.canonicalize().map_err(|error| {
        RegistryError::new(
            "bundle-root-open-failed",
            "failed to canonicalize bundle export target directory",
        )
        .with_detail(error.to_string())
    })
}

fn open_bundle_directory(path: impl AsRef<Path>) -> Result<PathBuf, RegistryError> {
    let path = path.as_ref();
    if !path.exists() {
        return Err(
            RegistryError::new("bundle-root-missing", "bundle directory does not exist")
                .with_detail(path.display().to_string()),
        );
    }

    if !path.is_dir() {
        return Err(
            RegistryError::new("bundle-root-invalid", "bundle path must be a directory")
                .with_detail(path.display().to_string()),
        );
    }

    path.canonicalize().map_err(|error| {
        RegistryError::new("bundle-root-open-failed", "failed to open bundle directory")
            .with_detail(error.to_string())
    })
}

fn read_bundle_index_bytes(bundle_root: &Path) -> Result<Vec<u8>, RegistryError> {
    let path = bundle_index_path(bundle_root);
    fs::read(&path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            RegistryError::new(
                "bundle-index-missing",
                "bundle.json was not found in the bundle directory",
            )
            .with_detail(path.display().to_string())
        } else {
            RegistryError::new("bundle-index-read-failed", "failed to read bundle.json")
                .with_detail(error.to_string())
        }
    })
}

fn parse_bundle_index(bundle_bytes: &[u8]) -> Result<InstalledSkillBundle, RegistryError> {
    let bundle: InstalledSkillBundle = serde_json::from_slice(bundle_bytes).map_err(|error| {
        RegistryError::new(
            "bundle-index-parse-failed",
            "failed to parse installed skill bundle metadata",
        )
        .with_detail(error.to_string())
    })?;
    validate_bundle_index_shape(&bundle)?;
    Ok(bundle)
}

fn validate_bundle_index_shape(bundle: &InstalledSkillBundle) -> Result<(), RegistryError> {
    if bundle.format_version != BUNDLE_FORMAT_VERSION {
        return Err(RegistryError::new(
            "bundle-format-unsupported",
            "installed skill bundle format version is unsupported",
        )
        .with_detail(serde_json::json!({
            "expected": BUNDLE_FORMAT_VERSION,
            "actual": bundle.format_version,
        })));
    }

    if bundle.skills.is_empty() {
        return Err(RegistryError::new(
            "bundle-index-invalid",
            "installed skill bundle must include at least one skill",
        ));
    }

    if bundle.publisher.id.trim().is_empty() {
        return Err(RegistryError::new(
            "bundle-index-invalid",
            "installed skill bundle publisher id must not be empty",
        ));
    }

    if bundle.files.is_empty() {
        return Err(RegistryError::new(
            "bundle-index-invalid",
            "installed skill bundle must include at least one bundled file digest",
        ));
    }

    Ok(())
}

fn validate_bundle(
    bundle_root: &Path,
    bundle: &InstalledSkillBundle,
) -> Result<Vec<ValidatedBundleSkill>, RegistryError> {
    validate_bundle_index_shape(bundle)?;
    let mut seen_refs = HashSet::new();
    let mut seen_dirs = HashSet::new();
    let mut validated = Vec::with_capacity(bundle.skills.len());

    for entry in &bundle.skills {
        if !seen_refs.insert(entry.resolved_ref.clone()) {
            return Err(RegistryError::new(
                "bundle-index-invalid",
                "bundle.json declared the same resolved skill more than once",
            )
            .with_detail(serde_json::json!({ "resolved_ref": entry.resolved_ref })));
        }

        let install_dir = resolve_bundle_install_dir(bundle_root, &entry.install_dir)?;
        let install_dir_string = path_string(&bundle_install_dir_relative_from_path(
            bundle_root,
            &install_dir,
        )?)?;
        if !seen_dirs.insert(install_dir_string.clone()) {
            return Err(RegistryError::new(
                "bundle-index-invalid",
                "bundle.json declared the same install directory more than once",
            )
            .with_detail(install_dir_string));
        }

        let installed = LocalRegistry::load_manifest(&install_dir.join("manifest.json"))?;
        if installed.resolved_ref != entry.resolved_ref {
            return Err(RegistryError::new(
                "bundle-entry-mismatch",
                "bundled installed manifest did not match its declared resolved skill reference",
            )
            .with_detail(serde_json::json!({
                "expected": entry.resolved_ref,
                "actual": installed.resolved_ref,
                "install_dir": entry.install_dir,
            })));
        }

        if installed.manifest.publisher.id != bundle.publisher.id {
            return Err(RegistryError::new(
                "bundle-publisher-mismatch",
                "bundled installed manifest publisher did not match the bundle publisher",
            )
            .with_detail(serde_json::json!({
                "bundle_publisher": bundle.publisher,
                "manifest_publisher": installed.manifest.publisher,
                "resolved_ref": installed.resolved_ref,
            })));
        }

        validated.push(ValidatedBundleSkill {
            entry: entry.clone(),
            install_dir,
            installed,
        });
    }

    let mut by_ref = HashMap::new();
    for skill in &validated {
        by_ref.insert(skill.installed.resolved_ref.clone(), skill);
    }

    if !by_ref.contains_key(&bundle.root_skill) {
        return Err(RegistryError::new(
            "bundle-root-missing",
            "bundle.json root_skill was not included in the bundled installed skills",
        )
        .with_detail(serde_json::json!({ "root_skill": bundle.root_skill })));
    }

    if bundle.includes_dependency_closure {
        let mut stack = vec![bundle.root_skill.clone()];
        let mut walked = HashSet::new();

        while let Some(skill_ref) = stack.pop() {
            if !walked.insert(skill_ref.clone()) {
                continue;
            }

            let installed = by_ref.get(&skill_ref).expect("root presence was checked");
            for dependency in &installed.installed.manifest.dependencies {
                if !by_ref.contains_key(&dependency.skill) {
                    return Err(RegistryError::new(
                        "bundle-closure-incomplete",
                        "bundle declared dependency closure but omitted a required installed dependency",
                    )
                    .with_detail(serde_json::json!({
                        "root_skill": bundle.root_skill,
                        "dependency_alias": dependency.alias,
                        "missing_dependency": dependency.skill,
                    })));
                }
                stack.push(dependency.skill.clone());
            }
        }
    }

    let mut listed_files = HashMap::new();
    for file in &bundle.files {
        if listed_files.insert(file.path.clone(), file).is_some() {
            return Err(RegistryError::new(
                "bundle-index-invalid",
                "bundle.json declared the same bundled file more than once",
            )
            .with_detail(file.path.clone()));
        }

        let relative = bundle_file_relative_from_str(&file.path)?;
        let file_path = bundle_root.join(&relative);
        if !file_path.exists() {
            return Err(RegistryError::new(
                "bundle-content-missing",
                "bundle.json referenced a bundled file that did not exist",
            )
            .with_detail(serde_json::json!({
                "path": file.path,
                "file_path": file_path.display().to_string(),
            })));
        }

        if !file_path.is_file() {
            return Err(RegistryError::new(
                "bundle-content-invalid",
                "bundle.json referenced a bundled path that was not a file",
            )
            .with_detail(serde_json::json!({
                "path": file.path,
                "file_path": file_path.display().to_string(),
            })));
        }

        let digest = sha256_file(&file_path)?;
        if digest != file.sha256 {
            return Err(RegistryError::new(
                "bundle-file-digest-mismatch",
                "bundled file digest did not match bundle.json",
            )
            .with_detail(serde_json::json!({
                "path": file.path,
                "expected": file.sha256,
                "actual": digest,
            })));
        }
    }

    let mut actual_files = HashSet::new();
    for skill in &validated {
        for entry in WalkDir::new(&skill.install_dir).sort_by_file_name() {
            let entry = entry.map_err(|error| {
                RegistryError::new(
                    "bundle-content-read-failed",
                    "failed while scanning bundled installed content",
                )
                .with_detail(error.to_string())
            })?;

            if !entry.file_type().is_file() {
                continue;
            }

            if entry.file_name() == VERIFICATION_FILENAME {
                return Err(RegistryError::new(
                    "bundle-content-invalid",
                    "bundled installed skill directories must not contain local verification metadata",
                )
                .with_detail(entry.path().display().to_string()));
            }

            let relative = bundle_install_dir_relative_from_path(bundle_root, entry.path())?;
            actual_files.insert(path_string(&relative)?);
        }
    }

    let listed_paths: HashSet<_> = bundle.files.iter().map(|file| file.path.clone()).collect();
    if let Some(unexpected) = actual_files.difference(&listed_paths).next() {
        return Err(RegistryError::new(
            "bundle-unexpected-content",
            "bundled installed content included a file that was not listed in bundle.json",
        )
        .with_detail(unexpected.clone()));
    }

    if let Some(orphaned) = listed_paths.difference(&actual_files).next() {
        return Err(RegistryError::new(
            "bundle-index-invalid",
            "bundle.json listed a file that did not belong to a bundled installed skill directory",
        )
        .with_detail(orphaned.clone()));
    }

    Ok(validated)
}

fn validate_import_targets(
    root: &Path,
    validated: &[ValidatedBundleSkill],
    _verification: &InstalledVerificationRecord,
) -> Result<(), RegistryError> {
    for skill in validated {
        let install_dir = bundle_install_dir_relative(&skill.entry)?;
        let target_dir = root.join(&install_dir);
        if !target_dir.exists() {
            continue;
        }

        let installed = LocalRegistry::load_manifest(&target_dir.join("manifest.json")).map_err(
            |error| {
                RegistryError::new(
                    "bundle-import-target-invalid",
                    "target registry already contained an invalid installed skill directory at the bundle import path",
                )
                .with_detail(serde_json::json!({
                    "resolved_ref": skill.entry.resolved_ref,
                    "target_dir": target_dir.display().to_string(),
                    "cause": {
                        "code": error.code,
                        "message": error.message,
                        "detail": error.detail,
                    }
                }))
            },
        )?;

        if installed.resolved_ref != skill.installed.resolved_ref
            || installed.manifest != skill.installed.manifest
        {
            return Err(RegistryError::new(
                "bundle-import-conflict",
                "target registry already contained a different installed skill at the import path",
            )
            .with_detail(serde_json::json!({
                "resolved_ref": skill.entry.resolved_ref,
                "target_dir": target_dir.display().to_string(),
            })));
        }
    }

    Ok(())
}

fn resolve_bundle_install_dir(
    bundle_root: &Path,
    install_dir: &str,
) -> Result<PathBuf, RegistryError> {
    let relative = bundle_install_dir_relative_from_str(install_dir)?;
    let path = bundle_root.join(&relative);
    let canonical = path.canonicalize().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            RegistryError::new(
                "bundle-content-missing",
                "bundle.json referenced an installed skill directory that did not exist",
            )
            .with_detail(serde_json::json!({
                "install_dir": install_dir,
                "path": path.display().to_string(),
            }))
        } else {
            RegistryError::new(
                "bundle-content-open-failed",
                "failed to open bundled installed skill directory",
            )
            .with_detail(error.to_string())
        }
    })?;

    if !canonical.starts_with(bundle_root) {
        return Err(RegistryError::new(
            "bundle-entry-path-invalid",
            "bundle.json install_dir escaped the bundle root",
        )
        .with_detail(serde_json::json!({
            "install_dir": install_dir,
            "bundle_root": bundle_root.display().to_string(),
        })));
    }

    if !canonical.is_dir() {
        return Err(RegistryError::new(
            "bundle-entry-path-invalid",
            "bundle.json install_dir must point to a directory",
        )
        .with_detail(canonical.display().to_string()));
    }

    Ok(canonical)
}

fn bundle_install_dir_relative(
    entry: &InstalledBundleSkillEntry,
) -> Result<PathBuf, RegistryError> {
    bundle_install_dir_relative_from_str(&entry.install_dir)
}

fn bundle_file_relative_from_str(file_path: &str) -> Result<PathBuf, RegistryError> {
    let path = Path::new(file_path);
    if path.is_absolute() {
        return Err(RegistryError::new(
            "bundle-file-path-invalid",
            "bundle.json file path must be a relative path",
        )
        .with_detail(file_path.to_owned()));
    }

    let mut components = path.components();
    if components.next().is_none() {
        return Err(RegistryError::new(
            "bundle-file-path-invalid",
            "bundle.json file path must not be empty",
        ));
    }

    if !path.starts_with("installed")
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err(RegistryError::new(
            "bundle-file-path-invalid",
            "bundle.json file paths must stay under the installed/ subtree",
        )
        .with_detail(file_path.to_owned()));
    }

    Ok(path.to_path_buf())
}

fn bundle_install_dir_relative_from_str(install_dir: &str) -> Result<PathBuf, RegistryError> {
    let path = Path::new(install_dir);
    if path.is_absolute() {
        return Err(RegistryError::new(
            "bundle-entry-path-invalid",
            "bundle.json install_dir must be a relative path",
        )
        .with_detail(install_dir.to_owned()));
    }

    let mut components = path.components();
    if components.next().is_none() {
        return Err(RegistryError::new(
            "bundle-entry-path-invalid",
            "bundle.json install_dir must not be empty",
        ));
    }

    if !path.starts_with("installed")
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err(RegistryError::new(
            "bundle-entry-path-invalid",
            "bundle.json install_dir must stay under the installed/ subtree",
        )
        .with_detail(install_dir.to_owned()));
    }

    Ok(path.to_path_buf())
}

fn bundle_install_dir_relative_from_path(
    bundle_root: &Path,
    install_dir: &Path,
) -> Result<PathBuf, RegistryError> {
    install_dir
        .strip_prefix(bundle_root)
        .map(Path::to_path_buf)
        .map_err(|_| {
            RegistryError::new(
                "bundle-entry-path-invalid",
                "bundled installed skill directory did not live under the bundle root",
            )
            .with_detail(serde_json::json!({
                "bundle_root": bundle_root.display().to_string(),
                "install_dir": install_dir.display().to_string(),
            }))
        })
}

fn installed_relative_dir(
    root: &Path,
    installed: &InstalledSkill,
) -> Result<PathBuf, RegistryError> {
    installed
        .root_dir
        .strip_prefix(root)
        .map(Path::to_path_buf)
        .map_err(|_| {
            RegistryError::new(
                "install-dir-invalid",
                "installed skill directory did not live under the registry root",
            )
            .with_detail(serde_json::json!({
                "registry_root": root.display().to_string(),
                "install_dir": installed.root_dir.display().to_string(),
            }))
        })
}

fn path_string(path: &Path) -> Result<String, RegistryError> {
    path.to_str().map(|value| value.to_owned()).ok_or_else(|| {
        RegistryError::new(
            "path-not-utf8",
            "path could not be represented as UTF-8 in the local bundle format",
        )
        .with_detail(path.display().to_string())
    })
}

fn copy_dir_recursive(source: &Path, destination: &Path) -> Result<(), RegistryError> {
    fs::create_dir_all(destination).map_err(|error| {
        RegistryError::new(
            "directory-create-failed",
            "failed to create directory while copying bundle contents",
        )
        .with_detail(error.to_string())
    })?;

    for entry in fs::read_dir(source).map_err(|error| {
        RegistryError::new(
            "directory-read-failed",
            "failed to read directory while copying bundle contents",
        )
        .with_detail(error.to_string())
    })? {
        let entry = entry.map_err(|error| {
            RegistryError::new(
                "directory-read-failed",
                "failed to read directory entry while copying bundle contents",
            )
            .with_detail(error.to_string())
        })?;
        let file_type = entry.file_type().map_err(|error| {
            RegistryError::new(
                "file-type-read-failed",
                "failed to inspect file type while copying bundle contents",
            )
            .with_detail(error.to_string())
        })?;
        let destination_path = destination.join(entry.file_name());

        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &destination_path)?;
        } else {
            fs::copy(entry.path(), &destination_path).map_err(|error| {
                RegistryError::new(
                    "file-copy-failed",
                    "failed to copy a file while copying bundle contents",
                )
                .with_detail(serde_json::json!({
                    "source": entry.path().display().to_string(),
                    "destination": destination_path.display().to_string(),
                    "cause": error.to_string(),
                }))
            })?;
        }
    }

    Ok(())
}

fn copy_installed_dir_for_bundle(
    source: &Path,
    destination: &Path,
    install_dir_relative: &Path,
) -> Result<Vec<BundleFileEntry>, RegistryError> {
    fs::create_dir_all(destination).map_err(|error| {
        RegistryError::new(
            "bundle-dir-create-failed",
            "failed to create bundled installed directory",
        )
        .with_detail(error.to_string())
    })?;

    let mut files = Vec::new();
    for entry in WalkDir::new(source).sort_by_file_name() {
        let entry = entry.map_err(|error| {
            RegistryError::new(
                "bundle-export-read-failed",
                "failed while scanning installed content for bundle export",
            )
            .with_detail(error.to_string())
        })?;
        let path = entry.path();
        let relative = path.strip_prefix(source).map_err(|_| {
            RegistryError::new(
                "bundle-export-path-invalid",
                "installed content path did not stay under the installed root",
            )
            .with_detail(path.display().to_string())
        })?;

        if relative.as_os_str().is_empty() {
            continue;
        }

        let destination_path = destination.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&destination_path).map_err(|error| {
                RegistryError::new(
                    "bundle-dir-create-failed",
                    "failed to create bundled installed subdirectory",
                )
                .with_detail(error.to_string())
            })?;
            continue;
        }

        if entry.file_name() == VERIFICATION_FILENAME {
            continue;
        }

        if let Some(parent) = destination_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                RegistryError::new(
                    "bundle-dir-create-failed",
                    "failed to create parent directory for bundled file",
                )
                .with_detail(error.to_string())
            })?;
        }

        fs::copy(path, &destination_path).map_err(|error| {
            RegistryError::new(
                "bundle-file-copy-failed",
                "failed to copy installed content into the portable bundle",
            )
            .with_detail(serde_json::json!({
                "source": path.display().to_string(),
                "destination": destination_path.display().to_string(),
                "cause": error.to_string(),
            }))
        })?;

        let relative_bundle_path = install_dir_relative.join(relative);
        files.push(BundleFileEntry {
            path: path_string(&relative_bundle_path)?,
            sha256: sha256_file(path)?,
        });
    }

    Ok(files)
}

fn ensure_bundle_publisher_matches_signer(
    bundled_skills: &[InstalledSkill],
    signer: &LocalPublisherIdentity,
) -> Result<(), RegistryError> {
    for installed in bundled_skills {
        if installed.manifest.publisher != signer.publisher {
            return Err(RegistryError::new(
                "bundle-publisher-mismatch",
                "signed portable bundles may only include installed skills from the signing publisher in this milestone",
            )
            .with_detail(serde_json::json!({
                "resolved_ref": installed.resolved_ref,
                "manifest_publisher": installed.manifest.publisher,
                "signer_publisher": signer.publisher,
            })));
        }
    }

    Ok(())
}

fn read_bundle_signature(bundle_root: &Path) -> Result<BundleSignatureEnvelope, RegistryError> {
    let path = bundle_signature_path(bundle_root);
    let contents = fs::read_to_string(&path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            RegistryError::new(
                "bundle-signature-missing",
                "bundle.signature.json was not found in the bundle directory",
            )
            .with_detail(path.display().to_string())
        } else {
            RegistryError::new(
                "bundle-signature-read-failed",
                "failed to read bundle.signature.json",
            )
            .with_detail(error.to_string())
        }
    })?;
    let signature: BundleSignatureEnvelope = serde_json::from_str(&contents).map_err(|error| {
        RegistryError::new(
            "bundle-signature-parse-failed",
            "failed to parse bundle signature metadata",
        )
        .with_detail(error.to_string())
    })?;

    if signature.format_version != BUNDLE_SIGNATURE_FORMAT_VERSION {
        return Err(RegistryError::new(
            "bundle-signature-format-unsupported",
            "bundle signature format version is unsupported",
        )
        .with_detail(serde_json::json!({
            "expected": BUNDLE_SIGNATURE_FORMAT_VERSION,
            "actual": signature.format_version,
        })));
    }

    Ok(signature)
}

fn load_trusted_publisher(
    root: &Path,
    publisher_id: &str,
) -> Result<TrustedPublisherRecord, RegistryError> {
    let path = trusted_publisher_path(root, publisher_id);
    let contents = fs::read_to_string(&path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            RegistryError::new(
                "bundle-publisher-untrusted",
                "signed bundle publisher was not trusted by the target Guild root",
            )
            .with_detail(publisher_id.to_owned())
        } else {
            RegistryError::new(
                "trusted-publisher-read-failed",
                "failed to read trusted publisher record",
            )
            .with_detail(error.to_string())
        }
    })?;
    let publisher: TrustedPublisherRecord = serde_json::from_str(&contents).map_err(|error| {
        RegistryError::new(
            "trusted-publisher-parse-failed",
            "failed to parse trusted publisher record",
        )
        .with_detail(error.to_string())
    })?;
    trusted_publisher_verifying_key(&publisher)?;
    Ok(publisher)
}

fn verify_bundle_signature(
    bundle_bytes: &[u8],
    bundle: &InstalledSkillBundle,
    signature: &BundleSignatureEnvelope,
    trusted_publisher: &TrustedPublisherRecord,
) -> Result<(), RegistryError> {
    if signature.publisher_id != bundle.publisher.id {
        return Err(RegistryError::new(
            "bundle-signature-publisher-mismatch",
            "bundle signature publisher id did not match the signed bundle publisher",
        )
        .with_detail(serde_json::json!({
            "bundle_publisher_id": bundle.publisher.id,
            "signature_publisher_id": signature.publisher_id,
        })));
    }

    if trusted_publisher.publisher.id != signature.publisher_id {
        return Err(RegistryError::new(
            "bundle-signature-publisher-mismatch",
            "trusted publisher record did not match the bundle signature publisher",
        )
        .with_detail(serde_json::json!({
            "trusted_publisher_id": trusted_publisher.publisher.id,
            "signature_publisher_id": signature.publisher_id,
        })));
    }

    if signature.scheme != trusted_publisher.scheme {
        return Err(RegistryError::new(
            "bundle-signature-scheme-mismatch",
            "trusted publisher record used a different signature scheme than the signed bundle",
        ));
    }

    let bundle_sha256 = format!("sha256:{}", sha256_bytes(bundle_bytes));
    if signature.bundle_sha256 != bundle_sha256 {
        return Err(RegistryError::new(
            "bundle-signature-digest-mismatch",
            "bundle signature metadata did not match the signed bundle bytes",
        )
        .with_detail(serde_json::json!({
            "expected": bundle_sha256,
            "actual": signature.bundle_sha256,
        })));
    }

    let verifying_key = trusted_publisher_verifying_key(trusted_publisher)?;
    let signature_bytes = decode_fixed_base64::<64>(
        &signature.signature_base64,
        "bundle-signature-invalid",
        "bundle signature bytes were invalid",
    )?;
    let signature = Signature::from_bytes(&signature_bytes);
    verifying_key
        .verify(bundle_bytes, &signature)
        .map_err(|error| {
            RegistryError::new(
                "bundle-signature-invalid",
                "bundle signature verification failed",
            )
            .with_detail(error.to_string())
        })?;

    Ok(())
}

fn trusted_publisher_verifying_key(
    publisher: &TrustedPublisherRecord,
) -> Result<VerifyingKey, RegistryError> {
    match publisher.scheme {
        SignatureScheme::Ed25519 => {
            let public = decode_fixed_base64::<32>(
                &publisher.public_key_base64,
                "trusted-publisher-key-invalid",
                "trusted publisher public key was invalid",
            )?;
            VerifyingKey::from_bytes(&public).map_err(|error| {
                RegistryError::new(
                    "trusted-publisher-key-invalid",
                    "trusted publisher public key was invalid",
                )
                .with_detail(error.to_string())
            })
        }
    }
}

fn sign_bundle_payload(
    signer: &LocalPublisherIdentity,
    bundle_bytes: &[u8],
) -> Result<BundleSignatureEnvelope, RegistryError> {
    let signing_key = signer.signing_key()?;
    let signature = signing_key.sign(bundle_bytes);
    Ok(BundleSignatureEnvelope {
        format_version: BUNDLE_SIGNATURE_FORMAT_VERSION.into(),
        scheme: signer.scheme.clone(),
        publisher_id: signer.publisher.id.clone(),
        bundle_sha256: format!("sha256:{}", sha256_bytes(bundle_bytes)),
        signature_base64: base64_encode(&signature.to_bytes()),
    })
}

fn json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, RegistryError> {
    serde_json::to_vec_pretty(value).map_err(|error| {
        RegistryError::new("json-serialize-failed", "failed to serialize JSON")
            .with_detail(error.to_string())
    })
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), RegistryError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            RegistryError::new(
                "file-write-dir-failed",
                "failed to create parent directory for file write",
            )
            .with_detail(error.to_string())
        })?;
    }

    fs::write(path, bytes).map_err(|error| {
        RegistryError::new("file-write-failed", "failed to write file")
            .with_detail(error.to_string())
    })
}

fn base64_encode(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn decode_fixed_base64<const N: usize>(
    value: &str,
    code: &str,
    message: &str,
) -> Result<[u8; N], RegistryError> {
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(value)
        .map_err(|error| RegistryError::new(code, message).with_detail(error.to_string()))?;
    decoded.try_into().map_err(|decoded: Vec<u8>| {
        RegistryError::new(code, message).with_detail(format!(
            "expected {} decoded bytes but found {}",
            N,
            decoded.len()
        ))
    })
}

fn load_verification_record(
    install_root: &Path,
) -> Result<Option<InstalledVerificationRecord>, RegistryError> {
    let path = installed_verification_path(install_root);
    if !path.exists() {
        return Ok(None);
    }

    let contents = fs::read_to_string(&path).map_err(|error| {
        RegistryError::new(
            "verification-read-failed",
            "failed to read installed verification metadata",
        )
        .with_detail(error.to_string())
    })?;
    let verification: InstalledVerificationRecord =
        serde_json::from_str(&contents).map_err(|error| {
            RegistryError::new(
                "verification-parse-failed",
                "failed to parse installed verification metadata",
            )
            .with_detail(error.to_string())
        })?;

    if verification.status != VerificationStatus::Verified {
        return Err(RegistryError::new(
            "verification-invalid",
            "installed verification metadata used an unsupported verification status",
        ));
    }

    if verification.signature.format_version != BUNDLE_SIGNATURE_FORMAT_VERSION {
        return Err(RegistryError::new(
            "verification-invalid",
            "installed verification metadata used an unsupported signature format version",
        )
        .with_detail(verification.signature.format_version.clone()));
    }

    if verification.scheme != verification.signature.scheme {
        return Err(RegistryError::new(
            "verification-invalid",
            "installed verification metadata used mismatched signature schemes",
        ));
    }

    if verification.publisher.id != verification.signature.publisher_id {
        return Err(RegistryError::new(
            "verification-invalid",
            "installed verification metadata publisher did not match the signature publisher",
        ));
    }

    if verification.bundle_sha256 != verification.signature.bundle_sha256 {
        return Err(RegistryError::new(
            "verification-invalid",
            "installed verification metadata bundle digest did not match the signature metadata",
        ));
    }

    Ok(Some(verification))
}

fn validate_staged_support_files(
    root_dir: &Path,
    manifest: &SkillManifest,
) -> Result<(), RegistryError> {
    let mut uris = vec![
        manifest.interface.input_schema_uri.as_str(),
        manifest.interface.output_schema_uri.as_str(),
    ];
    if let Some(examples_uri) = &manifest.interface.examples_uri {
        uris.push(examples_uri);
    }
    if let Some(sbom_uri) = &manifest.package.sbom_uri {
        uris.push(sbom_uri);
    }
    if let Some(signature_uri) = &manifest.package.signature_uri {
        uris.push(signature_uri);
    }
    for test in &manifest.tests {
        uris.push(&test.fixtures_uri);
        uris.push(&test.expected_output_uri);
    }

    for uri in uris {
        let Some(path) = maybe_resolve_local_file(root_dir, uri).map_err(|error| {
            RegistryError::new(
                "staged-file-uri-invalid",
                "installed manifest referenced an unsupported local file URI",
            )
            .with_detail(serde_json::json!({
                "uri": uri,
                "error": error,
            }))
        })?
        else {
            continue;
        };

        if !path.exists() {
            return Err(RegistryError::new(
                "staged-file-missing",
                "installed manifest referenced a staged support file that did not exist",
            )
            .with_detail(serde_json::json!({
                "uri": uri,
                "path": path.display().to_string(),
            })));
        }
    }

    Ok(())
}
