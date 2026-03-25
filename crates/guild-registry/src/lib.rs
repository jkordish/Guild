#![warn(clippy::all, clippy::pedantic, clippy::cargo, clippy::perf)]
#![allow(clippy::multiple_crate_versions)]

//! Registry model for publishing and resolving Guild skills.

use std::cmp::{Ordering, Reverse};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fmt::{self, Write as _};
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use guild_manifest::{
    InstalledDependencySpec, PublisherRef, SkillManifest, SourceBuildKind, SourceSkillManifest,
};
use guild_types::{
    CapabilityId, EvidenceBlobRecord, EvidenceEmissionRequest, EvidenceRecord, EvidenceRef,
    ExecutionQueryMatch, ExecutionQueryResource, ExecutionQueryResult, ExecutionRecord,
    ExecutionStatus, GUILD_EXECUTION_URI_PREFIX, GUILD_OBJECT_BLOB_URI_PREFIX,
    GUILD_OBJECT_RECORD_METADATA_URI_SUFFIX, GUILD_OBJECT_RECORD_URI_PREFIX, GuildResourceUri,
    InstalledVerificationState, LocalPolicyConfig, LocalTrustTier, RequestedSkillRef,
    ResolvedSkillRef, ResourceReadResult, SkillCategory,
    local_object_store_evidence_sink_descriptor, mint_host_evidence_record_id,
};
use rand_core::OsRng;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use walkdir::WalkDir;

mod oci_layout;

#[derive(Debug, Clone, PartialEq)]
pub struct InstalledSkill {
    pub manifest: SkillManifest,
    pub resolved_ref: ResolvedSkillRef,
    pub manifest_path: PathBuf,
    pub artifact_path: PathBuf,
    pub root_dir: PathBuf,
    pub verification: Option<InstalledVerificationRecord>,
    pub trust: InstalledTrustMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillResolutionExplanation {
    pub matching_versions: Vec<String>,
    pub selected_version: String,
    pub selected_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct InstalledTrustMetadata {
    pub verification_state: InstalledVerificationState,
    pub trust_tier: LocalTrustTier,
}

#[derive(Debug, Clone)]
struct PortableBundleFile {
    relative_path: String,
    source_path: PathBuf,
    sha256: String,
}

#[derive(Debug, Clone)]
struct SignedBundlePayload {
    bundle: InstalledSkillBundle,
    bundle_bytes: Vec<u8>,
    signature: BundleSignatureEnvelope,
    files: Vec<PortableBundleFile>,
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
pub struct StructuredDigest {
    pub algorithm: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ExecutionPlanSignatureEnvelope {
    pub format_version: String,
    pub scheme: SignatureScheme,
    pub publisher_id: String,
    pub signed_digest: StructuredDigest,
    pub signature_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ExecutionPlanVerification {
    pub publisher: PublisherRef,
    pub scheme: SignatureScheme,
    pub signed_digest: StructuredDigest,
    pub trust_tier: LocalTrustTier,
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
    #[serde(default = "default_trusted_publisher_trust_tier")]
    pub trust_tier: LocalTrustTier,
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct OciRegistryReference {
    pub registry: String,
    pub repository: String,
    pub target: OciRegistryTarget,
}

impl fmt::Display for OciRegistryReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.target {
            OciRegistryTarget::Tag(tag) => write!(f, "{}/{}:{tag}", self.registry, self.repository),
            OciRegistryTarget::Digest(digest) => {
                write!(f, "{}/{}@{digest}", self.registry, self.repository)
            }
        }
    }
}

impl FromStr for OciRegistryReference {
    type Err = RegistryError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let input = s.trim();
        if input.is_empty() {
            return Err(RegistryError::new(
                "oci-registry-reference-invalid",
                "OCI registry reference cannot be empty",
            ));
        }

        input.parse::<oci_client::Reference>().map_err(|error| {
            RegistryError::new(
                "oci-registry-reference-invalid",
                "failed to parse the OCI registry reference",
            )
            .with_detail(serde_json::json!({
                "reference": input,
                "cause": error.to_string(),
            }))
        })?;

        let (name, target) = if let Some((name, digest)) = input.rsplit_once('@') {
            if digest.is_empty() {
                return Err(RegistryError::new(
                    "oci-registry-reference-invalid",
                    "OCI registry digest reference was missing the digest suffix",
                )
                .with_detail(serde_json::json!({ "reference": input })));
            }
            (name, OciRegistryTarget::Digest(digest.to_owned()))
        } else {
            let slash = input.rfind('/').ok_or_else(|| {
                RegistryError::new(
                    "oci-registry-reference-invalid",
                    "OCI registry reference must include a registry host and repository path",
                )
                .with_detail(serde_json::json!({ "reference": input }))
            })?;
            let target_separator = input[(slash + 1)..]
                .rfind(':')
                .map(|index| slash + 1 + index);
            let colon = target_separator.ok_or_else(|| {
                RegistryError::new(
                    "oci-registry-reference-invalid",
                    "OCI registry reference must include either a tag or digest",
                )
                .with_detail(serde_json::json!({ "reference": input }))
            })?;
            let tag = &input[(colon + 1)..];
            if tag.is_empty() {
                return Err(RegistryError::new(
                    "oci-registry-reference-invalid",
                    "OCI registry tag reference was missing the tag suffix",
                )
                .with_detail(serde_json::json!({ "reference": input })));
            }
            (&input[..colon], OciRegistryTarget::Tag(tag.to_owned()))
        };

        let (registry, repository) = name.split_once('/').ok_or_else(|| {
            RegistryError::new(
                "oci-registry-reference-invalid",
                "OCI registry reference must include a registry host and repository path",
            )
            .with_detail(serde_json::json!({ "reference": input }))
        })?;

        if registry.is_empty() || repository.is_empty() {
            return Err(RegistryError::new(
                "oci-registry-reference-invalid",
                "OCI registry reference must include a registry host and repository path",
            )
            .with_detail(serde_json::json!({ "reference": input })));
        }

        Ok(Self {
            registry: registry.to_owned(),
            repository: repository.to_owned(),
            target,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum OciRegistryTarget {
    Tag(String),
    Digest(String),
}

#[derive(Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum OciRegistryAuth {
    #[default]
    Anonymous,
    Basic {
        username: String,
        password: String,
    },
    Bearer {
        token: String,
    },
}

#[derive(Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct OciRegistryTransportOptions {
    #[serde(default)]
    pub auth: OciRegistryAuth,
    #[serde(default)]
    pub allow_http: bool,
}

impl Default for OciRegistryTransportOptions {
    fn default() -> Self {
        Self {
            auth: OciRegistryAuth::Anonymous,
            allow_http: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct PublishedOciArtifact {
    pub reference: OciRegistryReference,
    pub manifest_digest: String,
    pub bundle: InstalledSkillBundle,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ImportPreviewDecision {
    WouldImport,
    WouldRefuse,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ImportPreviewReport {
    pub bundle: InstalledSkillBundle,
    pub signature: BundleSignatureEnvelope,
    pub verified: bool,
    pub verification_error: Option<RegistryError>,
    pub trust_tier: Option<LocalTrustTier>,
    pub decision: ImportPreviewDecision,
    pub refusal: Option<RegistryError>,
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

    #[must_use]
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
    /// Generate a fresh local signing identity for a publisher.
    ///
    /// # Errors
    ///
    /// Returns an error if local signing material cannot be generated.
    pub fn generate(publisher: PublisherRef) -> Result<Self, RegistryError> {
        let signing_key = SigningKey::generate(&mut OsRng);
        Ok(Self {
            publisher,
            scheme: SignatureScheme::Ed25519,
            public_key_base64: base64_encode(&signing_key.verifying_key().to_bytes()),
            secret_key_base64: base64_encode(&signing_key.to_bytes()),
        })
    }

    /// Load a local publisher identity from disk and validate its keypair.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read, parsed, or contains an
    /// invalid signing keypair.
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

    /// Persist a local publisher identity to disk.
    ///
    /// # Errors
    ///
    /// Returns an error if the identity cannot be serialized or written.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), RegistryError> {
        write_json(path.as_ref(), self)
    }

    #[must_use]
    pub fn trusted_record(&self) -> TrustedPublisherRecord {
        self.trusted_record_with_tier(LocalTrustTier::TrustedImported)
    }

    #[must_use]
    pub fn trusted_record_with_tier(&self, trust_tier: LocalTrustTier) -> TrustedPublisherRecord {
        TrustedPublisherRecord {
            publisher: self.publisher.clone(),
            scheme: self.scheme.clone(),
            public_key_base64: self.public_key_base64.clone(),
            trust_tier,
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

/// Sign an execution-plan JSON value using an existing local publisher identity.
///
/// The signed payload is the canonical JSON form of the execution plan with the
/// top-level `plan_signature` field removed. The input plan must already be an
/// unsigned `guild.execution_plan` object.
///
/// # Errors
///
/// Returns an error if the plan is not a valid execution-plan JSON object, the
/// input already contains a non-null signature, the signing identity is invalid,
/// or the signed plan cannot be serialized.
pub fn sign_execution_plan(
    plan: &Value,
    signer: &LocalPublisherIdentity,
) -> Result<Value, RegistryError> {
    let mut unsigned_plan = unsigned_execution_plan_payload(plan)?;
    if plan_signature_field(plan).is_some() {
        return Err(RegistryError::new(
            "execution-plan-already-signed",
            "execution plan already contained a non-null plan signature",
        ));
    }

    let payload_bytes = canonical_json_bytes(&unsigned_plan);
    let signature = sign_execution_plan_payload(signer, &payload_bytes)?;
    let object = unsigned_plan.as_object_mut().ok_or_else(|| {
        RegistryError::new(
            "execution-plan-invalid",
            "execution plan must be a top-level JSON object",
        )
    })?;
    object.insert(
        "plan_signature".into(),
        serde_json::to_value(signature).map_err(|error| {
            RegistryError::new(
                "json-serialize-failed",
                "failed to serialize the execution plan signature",
            )
            .with_detail(error.to_string())
        })?,
    );
    Ok(unsigned_plan)
}

/// Verify a signed execution-plan JSON value against the local Guild trust store.
///
/// The signed payload is the canonical JSON form of the execution plan with the
/// top-level `plan_signature` field removed.
///
/// # Errors
///
/// Returns an error if the plan is malformed or unsigned, the signing metadata
/// does not match the trusted publisher record, the trusted publisher is absent,
/// or the signature does not verify.
pub fn verify_execution_plan(
    root: impl AsRef<Path>,
    plan: &Value,
) -> Result<ExecutionPlanVerification, RegistryError> {
    let signature = execution_plan_signature(plan)?;
    let trusted_publisher = load_trusted_publisher_for_subject(
        root.as_ref(),
        &signature.publisher_id,
        "execution-plan-publisher-untrusted",
        "execution-plan publisher was not trusted by the target Guild root",
        signature.publisher_id.clone(),
    )?;
    verify_execution_plan_with_trusted_publisher(plan, &signature, &trusted_publisher)
}

pub trait SkillRegistry {
    /// Resolve a human-facing requested skill reference to installed executable state.
    ///
    /// # Errors
    ///
    /// Returns an error if the requested skill cannot be resolved from installed
    /// local state.
    fn resolve(&self, skill: &RequestedSkillRef) -> Result<InstalledSkill, RegistryError>;

    /// Resolve an exact digest-pinned skill reference to installed executable state.
    ///
    /// # Errors
    ///
    /// Returns an error if the exact resolved skill does not exist in installed
    /// local state.
    fn resolve_exact(&self, skill: &ResolvedSkillRef) -> Result<InstalledSkill, RegistryError>;

    fn search(&self, query: &SearchQuery) -> Vec<SearchResult>;

    /// Persist a host-owned execution record.
    ///
    /// # Errors
    ///
    /// Returns an error if the durable execution record cannot be written.
    fn persist_execution_record(&self, record: &ExecutionRecord) -> Result<(), RegistryError>;

    /// Load a host-owned execution record by durable execution identifier.
    ///
    /// # Errors
    ///
    /// Returns an error if the execution record does not exist or cannot be read.
    fn load_execution_record(&self, execution_id: &str) -> Result<ExecutionRecord, RegistryError>;

    /// Persist an emitted evidence payload and return its host-issued evidence reference.
    ///
    /// # Errors
    ///
    /// Returns an error if the evidence blob or evidence record cannot be stored.
    fn store_evidence(
        &self,
        produced_by_execution: &str,
        request: &EvidenceEmissionRequest,
    ) -> Result<EvidenceRef, RegistryError>;

    /// Load an evidence record by its host-issued Guild URI.
    ///
    /// # Errors
    ///
    /// Returns an error if the evidence record URI is invalid or the record cannot
    /// be loaded.
    fn load_evidence_record(&self, uri: &str) -> Result<EvidenceRecord, RegistryError>;

    /// Read a Guild execution or evidence resource through the local resource backend.
    ///
    /// # Errors
    ///
    /// Returns an error if the resource URI is invalid, missing, or cannot be
    /// materialized.
    fn read_resource(&self, uri: &str) -> Result<ResourceReadResult, RegistryError>;

    /// Load the local host policy configuration for this Guild root.
    ///
    /// # Errors
    ///
    /// Returns an error if the local policy file cannot be read, parsed, or
    /// validated.
    fn load_policy_config(&self) -> Result<LocalPolicyConfig, RegistryError>;
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
    /// Load the local registry view rooted at a Guild registry directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the registry layout is invalid or installed manifests
    /// cannot be loaded and validated.
    pub fn load(root: impl AsRef<Path>) -> Result<Self, RegistryError> {
        let root = root.as_ref();
        if let Some(error) = detect_source_skill_root(root)? {
            return Err(error);
        }

        let root = ensure_registry_layout(root)?;
        Self::load_from_canonical_root(root)
    }

    /// Load an existing local registry view without creating missing layout
    /// directories.
    ///
    /// # Errors
    ///
    /// Returns an error if the registry root does not exist, points at a source
    /// skill directory, or installed manifests cannot be loaded and validated.
    pub fn load_existing(root: impl AsRef<Path>) -> Result<Self, RegistryError> {
        let root = root.as_ref();
        if let Some(error) = detect_source_skill_root(root)? {
            return Err(error);
        }

        let root = open_existing_registry_root(root)?;
        Self::load_from_canonical_root(root)
    }

    fn load_from_canonical_root(root: PathBuf) -> Result<Self, RegistryError> {
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

                installed.push(Self::load_manifest(&root, entry.path())?);
            }
        }

        Ok(Self { root, installed })
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn installed(&self) -> &[InstalledSkill] {
        &self.installed
    }

    /// List recent persisted execution records from the local execution store.
    ///
    /// # Errors
    ///
    /// Returns an error if the execution store cannot be listed or parsed.
    pub fn list_recent_execution_records(
        &self,
        limit: usize,
    ) -> Result<Vec<ExecutionRecord>, RegistryError> {
        Ok(load_execution_records_sorted(&self.root)?
            .into_iter()
            .take(limit)
            .collect())
    }

    /// List recent evidence-record metadata from the local object store.
    ///
    /// # Errors
    ///
    /// Returns an error if the evidence-record store cannot be scanned or a
    /// persisted metadata record cannot be read and parsed.
    pub fn list_recent_evidence_records(
        &self,
        limit: usize,
    ) -> Result<Vec<EvidenceRecord>, RegistryError> {
        Ok(load_evidence_records_sorted(&self.root)?
            .into_iter()
            .take(limit)
            .collect())
    }

    /// List stored content-addressed object-blob metadata from the local object
    /// store.
    ///
    /// # Errors
    ///
    /// Returns an error if the object-blob store cannot be scanned or a blob
    /// metadata record cannot be read and parsed.
    pub fn list_object_blobs(
        &self,
        limit: usize,
    ) -> Result<Vec<EvidenceBlobRecord>, RegistryError> {
        Ok(load_object_blobs_sorted(&self.root)?
            .into_iter()
            .take(limit)
            .collect())
    }

    /// Execute a bounded local query over persisted execution records.
    ///
    /// # Errors
    ///
    /// Returns an error if the execution store cannot be scanned or a persisted
    /// record cannot be parsed.
    pub fn query_execution_records(
        &self,
        query: &ExecutionQueryResource,
    ) -> Result<ExecutionQueryResult, RegistryError> {
        query_execution_records_from_root(&self.root, query)
    }

    /// Trust a publisher record in the local Guild trust store.
    ///
    /// # Errors
    ///
    /// Returns an error if the trust store cannot be prepared, the publisher
    /// record is invalid, or the trusted record cannot be written.
    pub fn trust_publisher(
        root: impl AsRef<Path>,
        publisher: &TrustedPublisherRecord,
    ) -> Result<(), RegistryError> {
        ensure_registry_layout(root.as_ref())?;
        validate_trusted_publisher_record(publisher)?;
        write_json(
            &trusted_publisher_path(root.as_ref(), &publisher.publisher.id),
            publisher,
        )
    }

    /// List trusted publisher records stored under a local Guild root.
    ///
    /// # Errors
    ///
    /// Returns an error if the trust store cannot be scanned or any trusted
    /// publisher record cannot be read or validated.
    pub fn list_trusted_publishers(
        root: impl AsRef<Path>,
    ) -> Result<Vec<TrustedPublisherRecord>, RegistryError> {
        let root = ensure_registry_layout(root)?;
        let publishers_root = trusted_publishers_root(&root);
        if !publishers_root.exists() {
            return Ok(Vec::new());
        }

        let mut publishers = Vec::new();
        for entry in WalkDir::new(&publishers_root).min_depth(1).max_depth(1) {
            let entry = entry.map_err(|error| {
                RegistryError::new(
                    "trusted-publisher-scan-failed",
                    "failed while scanning trusted publisher records",
                )
                .with_detail(error.to_string())
            })?;

            if !entry.file_type().is_file() {
                continue;
            }

            publishers.push(read_trusted_publisher_record(entry.path())?);
        }

        publishers.sort_by(|left, right| left.publisher.id.cmp(&right.publisher.id));
        Ok(publishers)
    }

    /// Read one trusted publisher record stored under a local Guild root.
    ///
    /// # Errors
    ///
    /// Returns an error if the trust store cannot be prepared or the trusted
    /// publisher record cannot be read or validated.
    pub fn read_trusted_publisher(
        root: impl AsRef<Path>,
        publisher_id: &str,
    ) -> Result<TrustedPublisherRecord, RegistryError> {
        let root = ensure_registry_layout(root)?;
        let path = trusted_publisher_path(&root, publisher_id);
        read_trusted_publisher_record_with_not_found_detail(
            &path,
            "trusted-publisher-missing",
            "trusted publisher record was not found",
            publisher_id.to_owned(),
        )
    }

    /// Remove a trusted publisher record from a local Guild root.
    ///
    /// # Errors
    ///
    /// Returns an error if the trust store cannot be prepared or the record
    /// cannot be removed.
    pub fn remove_trusted_publisher(
        root: impl AsRef<Path>,
        publisher_id: &str,
    ) -> Result<bool, RegistryError> {
        let root = ensure_registry_layout(root)?;
        let path = trusted_publisher_path(&root, publisher_id);
        match fs::remove_file(path) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(RegistryError::new(
                "trusted-publisher-remove-failed",
                "failed to remove trusted publisher record",
            )
            .with_detail(error.to_string())),
        }
    }

    /// Export an installed skill, and optionally its dependency closure, as a signed bundle.
    ///
    /// # Errors
    ///
    /// Returns an error if the root skill cannot be resolved, bundled files cannot
    /// be copied, or the bundle cannot be signed and written.
    pub fn export_bundle(
        &self,
        root: &ResolvedSkillRef,
        include_dependencies: bool,
        bundle_root: impl AsRef<Path>,
        signer: &LocalPublisherIdentity,
    ) -> Result<InstalledSkillBundle, RegistryError> {
        let payload = self.build_signed_bundle_payload(root, include_dependencies, signer)?;
        let bundle_root = prepare_bundle_root(bundle_root)?;
        write_portable_bundle_files(&bundle_root, &payload.files)?;
        write_bytes(&bundle_index_path(&bundle_root), &payload.bundle_bytes)?;
        write_json(&bundle_signature_path(&bundle_root), &payload.signature)?;
        Ok(payload.bundle)
    }

    /// Export an installed skill, and optionally its dependency closure, as a local OCI image layout.
    ///
    /// # Errors
    ///
    /// Returns an error if the root skill cannot be resolved, installed files
    /// cannot be staged into the layout, or the OCI layout cannot be written.
    pub fn export_oci_layout(
        &self,
        root: &ResolvedSkillRef,
        include_dependencies: bool,
        layout_root: impl AsRef<Path>,
        signer: &LocalPublisherIdentity,
    ) -> Result<InstalledSkillBundle, RegistryError> {
        let payload = self.build_signed_bundle_payload(root, include_dependencies, signer)?;
        oci_layout::export_oci_layout(&payload, layout_root)?;
        Ok(payload.bundle)
    }

    /// Publish an installed skill, and optionally its dependency closure, to an OCI registry.
    ///
    /// # Errors
    ///
    /// Returns an error if the root skill cannot be resolved, installed files
    /// cannot be staged into the OCI artifact, or the registry publish fails.
    pub fn push_oci_registry(
        &self,
        root: &ResolvedSkillRef,
        include_dependencies: bool,
        reference: &OciRegistryReference,
        options: &OciRegistryTransportOptions,
        signer: &LocalPublisherIdentity,
    ) -> Result<PublishedOciArtifact, RegistryError> {
        let payload = self.build_signed_bundle_payload(root, include_dependencies, signer)?;
        oci_layout::push_oci_registry(&payload, reference, options)
    }

    /// Import a signed installed-skill bundle into a local registry root.
    ///
    /// # Errors
    ///
    /// Returns an error if the bundle fails trust, signature, validation, or file
    /// installation checks.
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
            signature,
        };
        validate_import_targets(&root, &validated, &verification)?;
        let staging_root = reset_import_staging_root(&root)?;

        let staged_result = (|| -> Result<Vec<InstalledSkill>, RegistryError> {
            let mut imported = Vec::with_capacity(bundle.skills.len());

            for entry in &bundle.skills {
                let validated_skill = validated
                    .iter()
                    .find(|candidate| candidate.entry.resolved_ref == entry.resolved_ref)
                    .ok_or_else(|| {
                        RegistryError::new(
                            "bundle-validation-incomplete",
                            "bundle validation did not retain every indexed skill entry",
                        )
                        .with_detail(serde_json::json!({ "resolved_ref": entry.resolved_ref }))
                    })?;
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
                }
                write_json(&installed_verification_path(&target_dir), &verification)?;
                imported.push(load_imported_bundle_skill(&root, &target_dir)?);
            }

            Ok(imported)
        })();

        let cleanup_result = cleanup_import_staging_root(&staging_root);

        match (staged_result, cleanup_result) {
            (Ok(imported), Ok(())) => Ok(imported),
            (Err(error), _) | (Ok(_), Err(error)) => Err(error),
        }
    }

    /// Preview a signed installed-skill bundle import without mutating the selected root.
    ///
    /// # Errors
    ///
    /// Returns an error if the selected root does not already exist, the bundle
    /// cannot be opened, or the preview cannot inspect the signed installed
    /// state.
    pub fn preview_import_bundle(
        root: impl AsRef<Path>,
        bundle_root: impl AsRef<Path>,
    ) -> Result<ImportPreviewReport, RegistryError> {
        let root = open_existing_registry_root(root)?;
        let bundle_root = open_bundle_directory(bundle_root)?;
        let bundle_bytes = read_bundle_index_bytes(&bundle_root)?;
        let bundle = parse_bundle_index(&bundle_bytes)?;
        let signature = read_bundle_signature(&bundle_root)?;

        preview_bundle_import(
            &root,
            bundle,
            &bundle_bytes,
            signature,
            |bundle, verification| {
                let validated = validate_bundle(&bundle_root, bundle)?;
                validate_import_targets(&root, &validated, verification)
            },
        )
    }

    /// Import a local OCI image layout that carries a signed installed-skill bundle.
    ///
    /// # Errors
    ///
    /// Returns an error if the layout is malformed, trust or signature
    /// verification fails, or installation checks fail.
    pub fn import_oci_layout(
        root: impl AsRef<Path>,
        layout_root: impl AsRef<Path>,
    ) -> Result<Vec<InstalledSkill>, RegistryError> {
        oci_layout::import_oci_layout(root.as_ref(), layout_root.as_ref())
    }

    /// Preview a local OCI image layout import without mutating the selected root.
    ///
    /// # Errors
    ///
    /// Returns an error if the selected root does not already exist, the OCI
    /// layout is malformed, or the preview cannot inspect the carried signed
    /// installed state.
    pub fn preview_import_oci_layout(
        root: impl AsRef<Path>,
        layout_root: impl AsRef<Path>,
    ) -> Result<ImportPreviewReport, RegistryError> {
        oci_layout::preview_import_oci_layout(root.as_ref(), layout_root.as_ref())
    }

    /// Pull and import a signed installed-skill OCI artifact from a registry.
    ///
    /// # Errors
    ///
    /// Returns an error if the remote artifact is malformed, transport integrity
    /// fails, trust or signature verification fails, or installation checks fail.
    pub fn pull_oci_registry(
        root: impl AsRef<Path>,
        reference: &OciRegistryReference,
        options: &OciRegistryTransportOptions,
    ) -> Result<Vec<InstalledSkill>, RegistryError> {
        oci_layout::pull_oci_registry(root.as_ref(), reference, options)
    }

    /// Preview an OCI registry pull without mutating the selected root.
    ///
    /// # Errors
    ///
    /// Returns an error if the selected root does not already exist, the remote
    /// artifact cannot be fetched or decoded, or the preview cannot inspect the
    /// carried signed installed state.
    pub fn preview_pull_oci_registry(
        root: impl AsRef<Path>,
        reference: &OciRegistryReference,
        options: &OciRegistryTransportOptions,
    ) -> Result<ImportPreviewReport, RegistryError> {
        oci_layout::preview_pull_oci_registry(root.as_ref(), reference, options)
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

    fn build_signed_bundle_payload(
        &self,
        root: &ResolvedSkillRef,
        include_dependencies: bool,
        signer: &LocalPublisherIdentity,
    ) -> Result<SignedBundlePayload, RegistryError> {
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
        let mut entries = Vec::with_capacity(bundled_skills.len());
        let mut files = Vec::new();

        for installed in &bundled_skills {
            let install_dir = installed_relative_dir(&self.root, installed)?;
            files.extend(collect_installed_dir_for_bundle(
                &installed.root_dir,
                &install_dir,
            )?);
            entries.push(InstalledBundleSkillEntry {
                resolved_ref: installed.resolved_ref.clone(),
                install_dir: path_string(&install_dir)?,
            });
        }
        files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));

        let bundle = InstalledSkillBundle {
            format_version: BUNDLE_FORMAT_VERSION.into(),
            root_skill: root.resolved_ref,
            includes_dependency_closure: include_dependencies,
            publisher: signer.publisher.clone(),
            skills: entries,
            files: files
                .iter()
                .map(|file| BundleFileEntry {
                    path: file.relative_path.clone(),
                    sha256: file.sha256.clone(),
                })
                .collect(),
        };
        let bundle_bytes = json_bytes(&bundle)?;
        let signature = sign_bundle_payload(signer, &bundle_bytes)?;
        Ok(SignedBundlePayload {
            bundle,
            bundle_bytes,
            signature,
            files,
        })
    }

    fn load_manifest(root: &Path, path: &Path) -> Result<InstalledSkill, RegistryError> {
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
        let trust = derive_installed_trust_metadata(root, verification.as_ref())?;

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
            trust,
        })
    }
}

#[derive(Debug)]
struct ResolutionSelection<'a> {
    installed: &'a InstalledSkill,
    matching_versions: Vec<String>,
}

fn resolve_installed_selection<'a>(
    installed_skills: &'a [InstalledSkill],
    skill: &RequestedSkillRef,
) -> Result<ResolutionSelection<'a>, RegistryError> {
    let mut matches: Vec<&InstalledSkill> = installed_skills
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
        let has_name_match = installed_skills
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
    let mut matching_versions = matches
        .iter()
        .map(|installed| installed.resolved_ref.version.to_string())
        .collect::<Vec<_>>();
    matching_versions.dedup();
    let selected_version = matches
        .last()
        .expect("non-empty version matches")
        .resolved_ref
        .version
        .clone();
    let selected_matches: Vec<_> = matches
        .into_iter()
        .filter(|installed| installed.resolved_ref.version == selected_version)
        .collect();

    if selected_matches.len() > 1 {
        return Err(RegistryError::new(
            "skill-version-ambiguous",
            "multiple installed digests matched the requested skill version",
        )
        .with_detail(serde_json::json!({
            "namespace": skill.key.namespace,
            "name": skill.key.name,
            "version": selected_version.to_string(),
            "version_req": skill.version_req.to_string(),
            "digests": selected_matches
                .iter()
                .map(|installed| installed.resolved_ref.digest.clone())
                .collect::<Vec<_>>(),
        })));
    }

    Ok(ResolutionSelection {
        installed: selected_matches
            .into_iter()
            .next()
            .expect("selected version keeps one installed digest"),
        matching_versions,
    })
}

impl LocalRegistry {
    /// Explain how a requested skill ref matched the currently installed local state.
    ///
    /// # Errors
    ///
    /// Returns an error if the requested skill ref cannot be resolved against the
    /// installed state, including missing matches and ambiguous same-version
    /// multi-digest matches.
    pub fn explain_resolution(
        &self,
        skill: &RequestedSkillRef,
    ) -> Result<SkillResolutionExplanation, RegistryError> {
        let selection = resolve_installed_selection(&self.installed, skill)?;
        Ok(SkillResolutionExplanation {
            matching_versions: selection.matching_versions,
            selected_version: selection.installed.resolved_ref.version.to_string(),
            selected_digest: selection.installed.resolved_ref.digest.clone(),
        })
    }
}

impl SkillRegistry for LocalRegistry {
    fn resolve(&self, skill: &RequestedSkillRef) -> Result<InstalledSkill, RegistryError> {
        Ok(resolve_installed_selection(&self.installed, skill)?
            .installed
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
        write_json_new(
            &path,
            record,
            "execution-record-exists",
            "execution record already exists in the local execution store",
        )
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
        produced_by_execution: &str,
        request: &EvidenceEmissionRequest,
    ) -> Result<EvidenceRef, RegistryError> {
        if request.mime_type.trim().is_empty() {
            return Err(RegistryError::new(
                "object-mime-type-invalid",
                "evidence payloads must declare a non-empty mime type",
            ));
        }

        let (blob_uri, digest_label) = ensure_evidence_blob(&self.root, request)?;

        let evidence_record_id = mint_host_evidence_record_id();
        let record_uri = evidence_record_resource_uri(&evidence_record_id);
        let record_path = evidence_record_path(&self.root, &evidence_record_id);
        let record = EvidenceRecord {
            uri: record_uri.clone(),
            blob_uri,
            mime_type: request.mime_type.clone(),
            sha256: digest_label.clone(),
            size_bytes: request.payload.len() as u64,
            sink: Some(local_object_store_evidence_sink_descriptor()),
            title: request.title.clone(),
            audience: request.audience.clone(),
            redaction: request.redaction.clone(),
            freshness: request.freshness.clone(),
            produced_by_execution: Some(produced_by_execution.to_owned()),
        };

        write_json_new(
            &record_path,
            &record,
            "evidence-record-exists",
            "evidence record already exists in the local evidence store",
        )?;

        Ok(EvidenceRef {
            uri: record_uri,
            title: request.title.clone(),
            mime_type: Some(request.mime_type.clone()),
            sha256: Some(digest_label),
            audience: request.audience.clone(),
            redaction: request.redaction.clone(),
            freshness: request.freshness.clone(),
        })
    }

    fn load_evidence_record(&self, uri: &str) -> Result<EvidenceRecord, RegistryError> {
        load_evidence_record_from_root(&self.root, uri)
    }

    fn read_resource(&self, uri: &str) -> Result<ResourceReadResult, RegistryError> {
        match parse_guild_uri(uri)? {
            GuildResourceUri::Execution { execution_id } => {
                let record = self.load_execution_record(&execution_id)?;
                read_execution_resource(&record, uri)
            }
            GuildResourceUri::ExecutionQuery { query } => {
                let result = self.query_execution_records(&query)?;
                read_execution_query_resource(&result)
            }
            GuildResourceUri::ObjectRecord { .. } => {
                read_record_backed_object_payload(&self.root, uri)
            }
            GuildResourceUri::ObjectRecordMetadata { .. } => {
                read_record_backed_object_metadata(&self.root, uri)
            }
            GuildResourceUri::ObjectBlob { digest_hex } => {
                read_blob_object(&self.root, uri, &digest_hex)
            }
        }
    }

    fn load_policy_config(&self) -> Result<LocalPolicyConfig, RegistryError> {
        let path = policy_path(&self.root);
        if !path.exists() {
            return Ok(LocalPolicyConfig::default());
        }

        let contents = fs::read_to_string(&path).map_err(|error| {
            RegistryError::new(
                "policy-read-failed",
                "failed to read local policy configuration",
            )
            .with_detail(serde_json::json!({
                "path": path.display().to_string(),
                "cause": error.to_string(),
            }))
        })?;

        let config: LocalPolicyConfig = serde_json::from_str(&contents).map_err(|error| {
            RegistryError::new(
                "policy-parse-failed",
                "failed to parse local policy configuration",
            )
            .with_detail(serde_json::json!({
                "path": path.display().to_string(),
                "cause": error.to_string(),
            }))
        })?;

        let validation = config.validate();
        if validation.is_empty() {
            Ok(config)
        } else {
            Err(
                RegistryError::new("policy-invalid", "local policy configuration was invalid")
                    .with_detail(serde_json::json!({
                        "path": path.display().to_string(),
                        "errors": validation,
                    })),
            )
        }
    }
}

const EXECUTION_QUERY_SAMPLE_EVIDENCE_LIMIT: usize = 3;

fn query_execution_records_from_root(
    root: &Path,
    query: &ExecutionQueryResource,
) -> Result<ExecutionQueryResult, RegistryError> {
    let records = load_execution_records_sorted(root)?;
    let filtered = records
        .into_iter()
        .filter(|record| execution_record_matches_query(record, query))
        .collect::<Vec<_>>();
    let total_matches = filtered.len();
    let results = filtered
        .into_iter()
        .take(query.limit())
        .map(|record| ExecutionQueryMatch {
            receipt: record.receipt,
            resolved_skill: record.resolved_skill,
            status: record.status,
            policy_decision: record.policy_decision,
            termination: record.termination,
            parent_execution_id: record.parent_execution_id,
            evidence_count: record.emitted_evidence.len(),
            sample_evidence_record_uris: record
                .emitted_evidence
                .iter()
                .take(EXECUTION_QUERY_SAMPLE_EVIDENCE_LIMIT)
                .map(|evidence| evidence.uri.clone())
                .collect(),
            child_execution_count: record.child_executions.len(),
            started_at_utc: record.provenance.started_at_utc,
            finished_at_utc: record.provenance.finished_at_utc,
        })
        .collect::<Vec<_>>();

    Ok(ExecutionQueryResult {
        query_uri: query.canonical_uri(),
        total_matches,
        returned_matches: results.len(),
        truncated: total_matches > results.len(),
        results,
    })
}

fn load_execution_records_sorted(root: &Path) -> Result<Vec<ExecutionRecord>, RegistryError> {
    let executions_root = executions_root(root);
    if !executions_root.exists() {
        return Ok(Vec::new());
    }

    let mut entries: Vec<_> = fs::read_dir(&executions_root)
        .map_err(|error| {
            RegistryError::new(
                "execution-list-read-failed",
                "failed to read the local execution store",
            )
            .with_detail(error.to_string())
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            RegistryError::new(
                "execution-list-read-failed",
                "failed to read execution store entry",
            )
            .with_detail(error.to_string())
        })?;

    entries.retain(|entry| {
        entry
            .file_type()
            .map(|kind| kind.is_file())
            .unwrap_or(false)
            && entry
                .path()
                .extension()
                .and_then(|extension| extension.to_str())
                == Some("json")
    });
    entries.sort_by_key(|entry| Reverse(entry.file_name()));

    let mut records = entries
        .into_iter()
        .map(|entry| {
            let bytes = fs::read_to_string(entry.path()).map_err(|error| {
                RegistryError::new(
                    "execution-list-entry-read-failed",
                    "failed to read execution record while scanning the local execution store",
                )
                .with_detail(error.to_string())
            })?;

            serde_json::from_str(&bytes).map_err(|error| {
                RegistryError::new(
                    "execution-list-entry-parse-failed",
                    "failed to parse execution record while scanning the local execution store",
                )
                .with_detail(error.to_string())
            })
        })
        .collect::<Result<Vec<ExecutionRecord>, _>>()?;

    records.sort_by(compare_execution_records_for_query);
    Ok(records)
}

fn load_evidence_records_sorted(root: &Path) -> Result<Vec<EvidenceRecord>, RegistryError> {
    let records_root = object_records_root(root);
    if !records_root.exists() {
        return Ok(Vec::new());
    }

    let mut records = Vec::new();
    for entry in WalkDir::new(&records_root).min_depth(1).max_depth(1) {
        let entry = entry.map_err(|error| {
            RegistryError::new(
                "object-record-scan-failed",
                "failed while scanning evidence record metadata",
            )
            .with_detail(error.to_string())
        })?;

        if !entry.file_type().is_file() || !entry.file_name().to_string_lossy().ends_with(".json") {
            continue;
        }

        let metadata = load_evidence_record_from_path(entry.path())?;
        records.push(metadata);
    }

    records.sort_by(compare_evidence_records_desc);
    Ok(records)
}

fn load_object_blobs_sorted(root: &Path) -> Result<Vec<EvidenceBlobRecord>, RegistryError> {
    let blobs_root = object_blobs_root(root);
    if !blobs_root.exists() {
        return Ok(Vec::new());
    }

    let mut records = Vec::new();
    for entry in WalkDir::new(&blobs_root).min_depth(1).max_depth(1) {
        let entry = entry.map_err(|error| {
            RegistryError::new(
                "object-blob-scan-failed",
                "failed while scanning object blobs",
            )
            .with_detail(error.to_string())
        })?;

        if !entry.file_type().is_dir() {
            continue;
        }

        let metadata = read_blob_metadata(&entry.path().join("blob.json"))?;
        records.push(metadata);
    }

    records.sort_by(compare_object_blobs_desc);
    Ok(records)
}

fn execution_record_matches_query(
    record: &ExecutionRecord,
    query: &ExecutionQueryResource,
) -> bool {
    match query {
        ExecutionQueryResource::Recent { .. } => true,
        ExecutionQueryResource::FailuresRecent { .. } => {
            matches!(
                record.status,
                ExecutionStatus::Failed | ExecutionStatus::Rejected
            )
        }
        ExecutionQueryResource::ByStatus { status, .. } => &record.status == status,
        ExecutionQueryResource::BySkill {
            namespace, name, ..
        } => {
            record.resolved_skill.key.namespace == *namespace
                && record.resolved_skill.key.name == *name
        }
    }
}

fn compare_execution_records_for_query(
    left: &ExecutionRecord,
    right: &ExecutionRecord,
) -> Ordering {
    compare_optional_timestamps_desc(
        left.provenance.finished_at_utc.as_deref(),
        right.provenance.finished_at_utc.as_deref(),
    )
    .then_with(|| {
        compare_optional_timestamps_desc(
            left.provenance.started_at_utc.as_deref(),
            right.provenance.started_at_utc.as_deref(),
        )
    })
    .then_with(|| right.receipt.execution_id.cmp(&left.receipt.execution_id))
}

fn compare_optional_timestamps_desc(left: Option<&str>, right: Option<&str>) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => compare_rfc3339_timestamps_desc(left, right),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn compare_evidence_records_desc(left: &EvidenceRecord, right: &EvidenceRecord) -> Ordering {
    right.uri.cmp(&left.uri)
}

fn compare_object_blobs_desc(left: &EvidenceBlobRecord, right: &EvidenceBlobRecord) -> Ordering {
    right.sha256.cmp(&left.sha256)
}

fn compare_rfc3339_timestamps_desc(left: &str, right: &str) -> Ordering {
    match (
        OffsetDateTime::parse(left, &Rfc3339),
        OffsetDateTime::parse(right, &Rfc3339),
    ) {
        (Ok(left), Ok(right)) => right.cmp(&left),
        _ => right.cmp(left),
    }
}

fn ensure_evidence_blob(
    root: &Path,
    request: &EvidenceEmissionRequest,
) -> Result<(String, String), RegistryError> {
    let digest_hex = sha256_bytes(&request.payload);
    let digest_label = format!("sha256:{digest_hex}");
    let blob_uri = object_resource_uri(&digest_hex);
    let blob_dir = object_blob_path(root, &digest_hex);
    let payload_path = blob_dir.join("payload");
    let blob_metadata_path = blob_dir.join("blob.json");

    fs::create_dir_all(&blob_dir).map_err(|error| {
        RegistryError::new(
            "object-store-create-failed",
            "failed to create object store directory",
        )
        .with_detail(error.to_string())
    })?;

    match (payload_path.exists(), blob_metadata_path.exists()) {
        (false, false) => {
            fs::write(&payload_path, &request.payload).map_err(|error| {
                RegistryError::new(
                    "object-payload-write-failed",
                    "failed to persist evidence payload",
                )
                .with_detail(error.to_string())
            })?;

            write_json(
                &blob_metadata_path,
                &EvidenceBlobRecord {
                    uri: blob_uri.clone(),
                    sha256: digest_label.clone(),
                    size_bytes: request.payload.len() as u64,
                },
            )?;
        }
        (true, true) => {
            let metadata = read_blob_metadata(&blob_metadata_path)?;

            if metadata.uri != blob_uri
                || metadata.sha256 != digest_label
                || metadata.size_bytes != request.payload.len() as u64
            {
                return Err(RegistryError::new(
                    "object-metadata-conflict",
                    "stored object metadata conflicted with the evidence payload digest",
                )
                .with_detail(serde_json::json!({
                    "uri": blob_uri,
                    "sha256": digest_label,
                })));
            }
        }
        _ => {
            return Err(RegistryError::new(
                "object-store-invalid",
                "evidence blob storage was partially populated and failed closed",
            )
            .with_detail(serde_json::json!({
                "uri": blob_uri,
                "payload_exists": payload_path.exists(),
                "metadata_exists": blob_metadata_path.exists(),
            })));
        }
    }

    Ok((blob_uri, digest_label))
}

fn read_execution_resource(
    record: &ExecutionRecord,
    uri: &str,
) -> Result<ResourceReadResult, RegistryError> {
    let bytes = serde_json::to_vec_pretty(record).map_err(|error| {
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

fn read_execution_query_resource(
    result: &ExecutionQueryResult,
) -> Result<ResourceReadResult, RegistryError> {
    let bytes = serde_json::to_vec_pretty(result).map_err(|error| {
        RegistryError::new(
            "execution-query-serialize-failed",
            "failed to serialize stored execution query result",
        )
        .with_detail(error.to_string())
    })?;
    Ok(ResourceReadResult {
        uri: result.query_uri.clone(),
        mime_type: "application/json".into(),
        sha256: Some(format!("sha256:{}", sha256_bytes(&bytes))),
        bytes,
    })
}

fn read_record_backed_object_payload(
    root: &Path,
    uri: &str,
) -> Result<ResourceReadResult, RegistryError> {
    let record = load_evidence_record_from_root(root, uri)?;
    let GuildResourceUri::ObjectRecord { evidence_record_id } = parse_guild_uri(uri)? else {
        return Err(RegistryError::new(
            "resource-kind-mismatch",
            "evidence records are only available for Guild evidence-record URIs",
        )
        .with_detail(serde_json::json!({ "uri": uri })));
    };
    let GuildResourceUri::ObjectBlob { digest_hex } = parse_guild_uri(&record.blob_uri)? else {
        return Err(RegistryError::new(
            "object-metadata-invalid",
            "evidence record referenced an invalid blob URI",
        )
        .with_detail(serde_json::json!({
            "uri": uri,
            "blob_uri": record.blob_uri,
            "evidence_record_id": evidence_record_id,
        })));
    };
    let payload_path = object_blob_path(root, &digest_hex).join("payload");
    let bytes = read_object_payload(
        &payload_path,
        "object-not-found",
        serde_json::json!({
            "uri": uri,
            "blob_uri": record.blob_uri,
            "path": payload_path.display().to_string(),
        }),
    )?;

    Ok(ResourceReadResult {
        uri: uri.to_owned(),
        mime_type: record.mime_type,
        sha256: Some(record.sha256),
        bytes,
    })
}

fn read_record_backed_object_metadata(
    root: &Path,
    uri: &str,
) -> Result<ResourceReadResult, RegistryError> {
    let record = load_evidence_record_from_root(root, uri)?;
    let bytes = serde_json::to_vec_pretty(&record).map_err(|error| {
        RegistryError::new(
            "object-metadata-serialize-failed",
            "failed to serialize evidence record metadata",
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

fn read_blob_object(
    root: &Path,
    uri: &str,
    digest_hex: &str,
) -> Result<ResourceReadResult, RegistryError> {
    let object_dir = object_blob_path(root, digest_hex);
    let payload_path = object_dir.join("payload");
    let metadata_path = object_dir.join("blob.json");
    let bytes = read_object_payload(
        &payload_path,
        "object-not-found",
        serde_json::json!({
            "uri": uri,
            "path": payload_path.display().to_string(),
        }),
    )?;
    let metadata = read_blob_metadata(&metadata_path)?;

    Ok(ResourceReadResult {
        uri: uri.to_owned(),
        mime_type: "application/octet-stream".into(),
        sha256: Some(metadata.sha256),
        bytes,
    })
}

fn load_evidence_record_from_root(root: &Path, uri: &str) -> Result<EvidenceRecord, RegistryError> {
    let (GuildResourceUri::ObjectRecord { evidence_record_id }
    | GuildResourceUri::ObjectRecordMetadata { evidence_record_id }) = parse_guild_uri(uri)?
    else {
        return Err(RegistryError::new(
            "resource-kind-mismatch",
            "evidence records are only available for Guild evidence-record URIs",
        )
        .with_detail(serde_json::json!({ "uri": uri })));
    };

    let metadata_path = evidence_record_path(root, &evidence_record_id);
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

    parse_evidence_record_contents(&contents)
}

fn load_evidence_record_from_path(path: &Path) -> Result<EvidenceRecord, RegistryError> {
    let contents = fs::read_to_string(path).map_err(|error| {
        RegistryError::new(
            "object-metadata-read-failed",
            "failed to read evidence record metadata",
        )
        .with_detail(serde_json::json!({
            "path": path.display().to_string(),
            "cause": error.to_string(),
        }))
    })?;
    parse_evidence_record_contents(&contents)
}

fn parse_evidence_record_contents(contents: &str) -> Result<EvidenceRecord, RegistryError> {
    serde_json::from_str(contents).map_err(|error| {
        RegistryError::new(
            "object-metadata-parse-failed",
            "failed to parse evidence record metadata",
        )
        .with_detail(error.to_string())
    })
}

fn read_object_payload(
    payload_path: &Path,
    not_found_code: &'static str,
    not_found_detail: Value,
) -> Result<Vec<u8>, RegistryError> {
    fs::read(payload_path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            RegistryError::new(
                not_found_code,
                "evidence object was not found in the local object store",
            )
            .with_detail(not_found_detail)
        } else {
            RegistryError::new(
                "object-read-failed",
                "failed to read evidence object payload",
            )
            .with_detail(error.to_string())
        }
    })
}

fn read_blob_metadata(metadata_path: &Path) -> Result<EvidenceBlobRecord, RegistryError> {
    serde_json::from_str(&fs::read_to_string(metadata_path).map_err(|error| {
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
    })
}

impl LocalSourceInstaller {
    /// Construct a source installer rooted at a local Guild registry.
    ///
    /// # Errors
    ///
    /// Returns an error if the registry root cannot be created or validated.
    pub fn new(root: impl AsRef<Path>) -> Result<Self, RegistryError> {
        Ok(Self {
            root: ensure_registry_layout(root)?,
        })
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Build, validate, and atomically install a source skill into local installed state.
    ///
    /// # Errors
    ///
    /// Returns an error if the source manifest, dependency resolution, build,
    /// staging, or atomic move into installed state fails.
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
        let install_dir = install_root.join(digest_dir(&digest));
        let staging_dir = create_install_staging_dir(&self.root, &digest)?;
        let staged_result = stage_install_contents(
            &source_dir,
            &staging_dir,
            source_manifest,
            installed_dependencies,
            &built_artifact,
            &digest,
        );

        let staged = match staged_result {
            Ok(staged) => staged,
            Err(error) => {
                let _ = cleanup_install_staging_dir(&staging_dir);
                return Err(error);
            }
        };

        if let Some(existing) = reusable_existing_install(&install_dir, &staged)? {
            let _ = cleanup_install_staging_dir(&staging_dir);
            return Ok(existing);
        }

        ensure_install_parent(&install_dir)?;
        move_staged_install(&staging_dir, &install_dir)?;
        let _ = cleanup_install_staging_dir(&staging_dir);

        LocalRegistry::load_manifest(&self.root, &install_dir.join("manifest.json"))
    }
}

fn create_install_staging_dir(root: &Path, digest: &str) -> Result<PathBuf, RegistryError> {
    let staging_root = source_install_staging_root(root);
    fs::create_dir_all(&staging_root).map_err(|error| {
        RegistryError::new(
            "install-staging-create-failed",
            "failed to create source install staging directory",
        )
        .with_detail(error.to_string())
    })?;

    let staging_dir = staging_root.join(format!(
        "{}-{}",
        digest_dir(digest),
        install_staging_suffix()
    ));
    fs::create_dir_all(&staging_dir).map_err(|error| {
        RegistryError::new(
            "install-staging-create-failed",
            "failed to create staged install directory",
        )
        .with_detail(error.to_string())
    })?;
    Ok(staging_dir)
}

fn stage_install_contents(
    source_dir: &Path,
    staging_dir: &Path,
    source_manifest: SourceSkillManifest,
    installed_dependencies: Vec<InstalledDependencySpec>,
    built_artifact: &Path,
    digest: &str,
) -> Result<InstalledSkill, RegistryError> {
    let staged_artifact = staging_dir.join("component.wasm");
    fs::copy(built_artifact, &staged_artifact).map_err(|error| {
        RegistryError::new("artifact-stage-failed", "failed to stage built artifact")
            .with_detail(error.to_string())
    })?;

    stage_support_files(source_dir, staging_dir, &source_manifest)?;

    let installed_manifest = source_manifest.into_installed(
        "./component.wasm",
        digest.to_owned(),
        installed_dependencies,
    );
    let installed_manifest_path = staging_dir.join("manifest.json");
    write_json(&installed_manifest_path, &installed_manifest)?;

    LocalRegistry::load_manifest(staging_dir, &installed_manifest_path)
}

fn reusable_existing_install(
    install_dir: &Path,
    staged: &InstalledSkill,
) -> Result<Option<InstalledSkill>, RegistryError> {
    if !install_dir.exists() {
        return Ok(None);
    }

    let existing = LocalRegistry::load_manifest(install_dir, &install_dir.join("manifest.json"))
        .map_err(|error| {
            RegistryError::new(
                "install-target-invalid",
                "existing installed digest directory was invalid",
            )
            .with_detail(serde_json::json!({
                "install_dir": install_dir.display().to_string(),
                "cause": {
                    "code": error.code,
                    "message": error.message,
                    "detail": error.detail,
                }
            }))
        })?;

    if existing.resolved_ref == staged.resolved_ref && existing.manifest == staged.manifest {
        return Ok(Some(existing));
    }

    Err(RegistryError::new(
        "install-target-conflict",
        "existing installed digest directory conflicted with the staged install",
    )
    .with_detail(serde_json::json!({
        "install_dir": install_dir.display().to_string(),
        "resolved_ref": staged.resolved_ref,
    })))
}

fn ensure_install_parent(install_dir: &Path) -> Result<(), RegistryError> {
    if let Some(parent) = install_dir.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            RegistryError::new(
                "install-root-create-failed",
                "failed to create install directory",
            )
            .with_detail(error.to_string())
        })?;
    }

    Ok(())
}

fn move_staged_install(staging_dir: &Path, install_dir: &Path) -> Result<(), RegistryError> {
    fs::rename(staging_dir, install_dir).map_err(|error| {
        RegistryError::new(
            "install-move-failed",
            "failed to move the staged install into the local registry",
        )
        .with_detail(serde_json::json!({
            "from": staging_dir.display().to_string(),
            "to": install_dir.display().to_string(),
            "cause": error.to_string(),
        }))
    })
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
        object_blobs_root(&root),
        object_records_root(&root),
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

fn open_existing_registry_root(path: impl AsRef<Path>) -> Result<PathBuf, RegistryError> {
    let path = path.as_ref();

    if !path.exists() {
        return Err(RegistryError::new(
            "registry-root-missing",
            "Guild registry root does not exist yet",
        )
        .with_detail(path.display().to_string()));
    }

    if !path.is_dir() {
        return Err(RegistryError::new(
            "registry-root-invalid",
            "Guild registry root was not a directory",
        )
        .with_detail(path.display().to_string()));
    }

    path.canonicalize().map_err(|error| {
        RegistryError::new(
            "registry-root-open-failed",
            "failed to open Guild registry root",
        )
        .with_detail(error.to_string())
    })
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

            if Path::new(filename)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("wasm"))
            {
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

fn default_trusted_publisher_trust_tier() -> LocalTrustTier {
    LocalTrustTier::TrustedImported
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

fn write_json_new<T: Serialize>(
    path: &Path,
    value: &T,
    exists_code: &'static str,
    exists_message: &'static str,
) -> Result<(), RegistryError> {
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

    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                RegistryError::new(exists_code, exists_message)
                    .with_detail(path.display().to_string())
            } else {
                RegistryError::new("json-write-failed", "failed to write JSON file")
                    .with_detail(error.to_string())
            }
        })?;

    let mut file = file;
    file.write_all(&bytes).map_err(|error| {
        RegistryError::new("json-write-failed", "failed to write JSON file")
            .with_detail(error.to_string())
    })?;
    file.sync_all().map_err(|error| {
        RegistryError::new("json-write-failed", "failed to sync JSON file")
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
    root.join("objects")
}

fn object_blobs_root(root: &Path) -> PathBuf {
    objects_root(root).join("sha256")
}

fn object_records_root(root: &Path) -> PathBuf {
    objects_root(root).join("records")
}

fn import_staging_root(root: &Path) -> PathBuf {
    root.join(".bundle-import-staging")
}

fn reset_import_staging_root(root: &Path) -> Result<PathBuf, RegistryError> {
    let staging_root = import_staging_root(root);
    cleanup_import_staging_root(&staging_root)?;
    fs::create_dir_all(&staging_root).map_err(|error| {
        RegistryError::new(
            "bundle-import-staging-create-failed",
            "failed to create bundle import staging directory",
        )
        .with_detail(error.to_string())
    })?;
    Ok(staging_root)
}

fn cleanup_import_staging_root(staging_root: &Path) -> Result<(), RegistryError> {
    if !staging_root.exists() {
        return Ok(());
    }

    fs::remove_dir_all(staging_root).map_err(|error| {
        RegistryError::new(
            "bundle-import-staging-cleanup-failed",
            "failed to clean bundle import staging directory",
        )
        .with_detail(error.to_string())
    })
}

fn load_imported_bundle_skill(
    root: &Path,
    target_dir: &Path,
) -> Result<InstalledSkill, RegistryError> {
    LocalRegistry::load_manifest(root, &target_dir.join("manifest.json"))
}

fn source_install_staging_root(root: &Path) -> PathBuf {
    root.join(".source-install-staging")
}

fn policy_path(root: &Path) -> PathBuf {
    root.join("policy.json")
}

fn install_staging_suffix() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time is after unix epoch")
        .as_nanos();
    format!("{}-{nanos}", std::process::id())
}

fn cleanup_install_staging_dir(path: &Path) -> Result<(), RegistryError> {
    if !path.exists() {
        return Ok(());
    }

    fs::remove_dir_all(path).map_err(|error| {
        RegistryError::new(
            "install-staging-cleanup-failed",
            "failed to clean staged install directory",
        )
        .with_detail(error.to_string())
    })
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

#[must_use]
pub fn execution_query_resource_uri(query: &ExecutionQueryResource) -> String {
    query.canonical_uri()
}

#[must_use]
pub fn execution_resource_uri(execution_id: &str) -> String {
    format!(
        "{}{}",
        GUILD_EXECUTION_URI_PREFIX,
        percent_encode_component(execution_id)
    )
}

fn object_blob_path(root: &Path, digest_hex: &str) -> PathBuf {
    object_blobs_root(root).join(digest_hex)
}

#[must_use]
pub fn object_resource_uri(digest_hex: &str) -> String {
    format!("{GUILD_OBJECT_BLOB_URI_PREFIX}{digest_hex}")
}

fn evidence_record_path(root: &Path, evidence_record_id: &str) -> PathBuf {
    object_records_root(root).join(format!(
        "{}.json",
        percent_encode_component(evidence_record_id)
    ))
}

#[must_use]
pub fn evidence_record_resource_uri(evidence_record_id: &str) -> String {
    format!(
        "{}{}",
        GUILD_OBJECT_RECORD_URI_PREFIX,
        percent_encode_component(evidence_record_id)
    )
}

#[must_use]
pub fn evidence_record_metadata_resource_uri(evidence_record_id: &str) -> String {
    format!(
        "{}{}{}",
        GUILD_OBJECT_RECORD_URI_PREFIX,
        percent_encode_component(evidence_record_id),
        GUILD_OBJECT_RECORD_METADATA_URI_SUFFIX
    )
}

fn parse_guild_uri(uri: &str) -> Result<GuildResourceUri, RegistryError> {
    GuildResourceUri::parse(uri).map_err(|error| {
        RegistryError::new("resource-uri-invalid", error.to_string()).with_detail(uri.to_owned())
    })
}

fn percent_encode_component(input: &str) -> String {
    let mut encoded = String::with_capacity(input.len());

    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => {
                write!(&mut encoded, "%{byte:02X}").expect("writing into a String cannot fail");
            }
        }
    }

    encoded
}

const BUNDLE_FORMAT_VERSION: &str = "guild-installed-bundle-v2";
const BUNDLE_SIGNATURE_FORMAT_VERSION: &str = "guild-installed-bundle-signature-v1";
const EXECUTION_PLAN_SIGNATURE_FORMAT_VERSION: &str = "guild-execution-plan-signature-v1";
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
    let validated = validate_bundle_skill_entries(bundle_root, bundle)?;
    validate_bundle_root_and_dependency_closure(bundle, &validated)?;
    let listed_paths = validate_listed_bundle_files(bundle_root, bundle)?;
    let actual_files = collect_actual_bundle_files(bundle_root, &validated)?;
    validate_bundle_file_set_alignment(&listed_paths, &actual_files)?;
    Ok(validated)
}

fn validate_bundle_skill_entries(
    bundle_root: &Path,
    bundle: &InstalledSkillBundle,
) -> Result<Vec<ValidatedBundleSkill>, RegistryError> {
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

        let installed =
            LocalRegistry::load_manifest(bundle_root, &install_dir.join("manifest.json"))?;
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

    Ok(validated)
}

fn validate_bundle_root_and_dependency_closure(
    bundle: &InstalledSkillBundle,
    validated: &[ValidatedBundleSkill],
) -> Result<(), RegistryError> {
    let by_ref: HashMap<_, _> = validated
        .iter()
        .map(|skill| (skill.installed.resolved_ref.clone(), skill))
        .collect();

    if !by_ref.contains_key(&bundle.root_skill) {
        return Err(RegistryError::new(
            "bundle-root-missing",
            "bundle.json root_skill was not included in the bundled installed skills",
        )
        .with_detail(serde_json::json!({ "root_skill": bundle.root_skill })));
    }

    if !bundle.includes_dependency_closure {
        return Ok(());
    }

    let mut stack = vec![bundle.root_skill.clone()];
    let mut walked = HashSet::new();
    while let Some(skill_ref) = stack.pop() {
        if !walked.insert(skill_ref.clone()) {
            continue;
        }

        let installed = by_ref.get(&skill_ref).ok_or_else(|| {
            RegistryError::new(
                "bundle-root-missing",
                "bundle.json root_skill was not included in the bundled installed skills",
            )
            .with_detail(serde_json::json!({ "root_skill": bundle.root_skill }))
        })?;
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

    Ok(())
}

fn validate_listed_bundle_files(
    bundle_root: &Path,
    bundle: &InstalledSkillBundle,
) -> Result<HashSet<String>, RegistryError> {
    let mut listed_paths = HashSet::new();

    for file in &bundle.files {
        if !listed_paths.insert(file.path.clone()) {
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

    Ok(listed_paths)
}

fn collect_actual_bundle_files(
    bundle_root: &Path,
    validated: &[ValidatedBundleSkill],
) -> Result<HashSet<String>, RegistryError> {
    let mut actual_files = HashSet::new();

    for skill in validated {
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

    Ok(actual_files)
}

fn validate_bundle_file_set_alignment(
    listed_paths: &HashSet<String>,
    actual_files: &HashSet<String>,
) -> Result<(), RegistryError> {
    if let Some(unexpected) = actual_files.difference(listed_paths).next() {
        return Err(RegistryError::new(
            "bundle-unexpected-content",
            "bundled installed content included a file that was not listed in bundle.json",
        )
        .with_detail(unexpected.clone()));
    }

    if let Some(orphaned) = listed_paths.difference(actual_files).next() {
        return Err(RegistryError::new(
            "bundle-index-invalid",
            "bundle.json listed a file that did not belong to a bundled installed skill directory",
        )
        .with_detail(orphaned.clone()));
    }

    Ok(())
}

fn validate_import_targets(
    root: &Path,
    validated: &[ValidatedBundleSkill],
    _verification: &InstalledVerificationRecord,
) -> Result<(), RegistryError> {
    let existing_registry = LocalRegistry::load_existing(root).map_err(|error| {
        RegistryError::new(
            "bundle-import-target-invalid",
            "failed to load existing installed state while validating bundle import targets",
        )
        .with_detail(serde_json::json!({
            "cause": {
                "code": error.code,
                "message": error.message,
                "detail": error.detail,
            }
        }))
    })?;

    for skill in validated {
        let install_dir = bundle_install_dir_relative(&skill.entry)?;
        let target_dir = root.join(&install_dir);
        if !target_dir.exists() {
            let conflicting = existing_registry
                .installed()
                .iter()
                .filter(|installed| {
                    installed.resolved_ref.key == skill.installed.resolved_ref.key
                        && installed.resolved_ref.version == skill.installed.resolved_ref.version
                        && installed.resolved_ref != skill.installed.resolved_ref
                })
                .map(|installed| installed.resolved_ref.digest.clone())
                .collect::<Vec<_>>();
            if conflicting.is_empty() {
                continue;
            }

            return Err(RegistryError::new(
                "bundle-import-version-ambiguous",
                "bundle import would introduce multiple digests for the same skill version",
            )
            .with_detail(serde_json::json!({
                "resolved_ref": skill.entry.resolved_ref,
                "conflicting_digests": conflicting,
            })));
        }

        let installed = LocalRegistry::load_manifest(root, &target_dir.join("manifest.json")).map_err(
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

        let conflicting = existing_registry
            .installed()
            .iter()
            .filter(|existing| {
                existing.resolved_ref.key == skill.installed.resolved_ref.key
                    && existing.resolved_ref.version == skill.installed.resolved_ref.version
                    && existing.resolved_ref != skill.installed.resolved_ref
            })
            .map(|installed| installed.resolved_ref.digest.clone())
            .collect::<Vec<_>>();
        if !conflicting.is_empty() {
            return Err(RegistryError::new(
                "bundle-import-version-ambiguous",
                "bundle import would introduce multiple digests for the same skill version",
            )
            .with_detail(serde_json::json!({
                "resolved_ref": skill.entry.resolved_ref,
                "target_dir": target_dir.display().to_string(),
                "conflicting_digests": conflicting,
            })));
        }
    }

    Ok(())
}

fn preview_bundle_import(
    root: &Path,
    bundle: InstalledSkillBundle,
    bundle_bytes: &[u8],
    signature: BundleSignatureEnvelope,
    validate_after_signature: impl FnOnce(
        &InstalledSkillBundle,
        &InstalledVerificationRecord,
    ) -> Result<(), RegistryError>,
) -> Result<ImportPreviewReport, RegistryError> {
    let trusted_publisher = match load_trusted_publisher(root, &signature.publisher_id) {
        Ok(publisher) => publisher,
        Err(error) if error.code == "bundle-publisher-untrusted" => {
            return Ok(ImportPreviewReport {
                bundle,
                signature,
                verified: false,
                verification_error: Some(error.clone()),
                trust_tier: None,
                decision: ImportPreviewDecision::WouldRefuse,
                refusal: Some(error),
            });
        }
        Err(error) => return Err(error),
    };

    let trust_tier = Some(trusted_publisher.trust_tier.clone());
    if let Err(error) =
        verify_bundle_signature(bundle_bytes, &bundle, &signature, &trusted_publisher)
    {
        return Ok(ImportPreviewReport {
            bundle,
            signature,
            verified: false,
            verification_error: Some(error.clone()),
            trust_tier,
            decision: ImportPreviewDecision::WouldRefuse,
            refusal: Some(error),
        });
    }

    let verification = InstalledVerificationRecord {
        status: VerificationStatus::Verified,
        publisher: bundle.publisher.clone(),
        scheme: signature.scheme.clone(),
        bundle_sha256: signature.bundle_sha256.clone(),
        signature: signature.clone(),
    };
    if let Err(error) = validate_after_signature(&bundle, &verification) {
        return Ok(ImportPreviewReport {
            bundle,
            signature,
            verified: true,
            verification_error: None,
            trust_tier,
            decision: ImportPreviewDecision::WouldRefuse,
            refusal: Some(error),
        });
    }

    Ok(ImportPreviewReport {
        bundle,
        signature,
        verified: true,
        verification_error: None,
        trust_tier,
        decision: ImportPreviewDecision::WouldImport,
        refusal: None,
    })
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
    path.to_str().map(str::to_owned).ok_or_else(|| {
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

fn collect_installed_dir_for_bundle(
    source: &Path,
    install_dir_relative: &Path,
) -> Result<Vec<PortableBundleFile>, RegistryError> {
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

        if entry.file_type().is_dir() {
            continue;
        }

        if entry.file_name() == VERIFICATION_FILENAME {
            continue;
        }

        let relative_bundle_path = install_dir_relative.join(relative);
        files.push(PortableBundleFile {
            relative_path: path_string(&relative_bundle_path)?,
            source_path: path.to_path_buf(),
            sha256: sha256_file(path)?,
        });
    }

    Ok(files)
}

fn write_portable_bundle_files(
    bundle_root: &Path,
    files: &[PortableBundleFile],
) -> Result<(), RegistryError> {
    for file in files {
        let relative = bundle_file_relative_from_str(&file.relative_path)?;
        let destination = bundle_root.join(relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                RegistryError::new(
                    "bundle-dir-create-failed",
                    "failed to create parent directory for bundled file",
                )
                .with_detail(error.to_string())
            })?;
        }

        fs::copy(&file.source_path, &destination).map_err(|error| {
            RegistryError::new(
                "bundle-file-copy-failed",
                "failed to copy installed content into the portable bundle",
            )
            .with_detail(serde_json::json!({
                "source": file.source_path.display().to_string(),
                "destination": destination.display().to_string(),
                "cause": error.to_string(),
            }))
        })?;
    }

    Ok(())
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
    let bytes = fs::read(&path).map_err(|error| {
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
    parse_bundle_signature_bytes(&bytes)
}

fn plan_signature_field(plan: &Value) -> Option<&Value> {
    plan.as_object()
        .and_then(|object| object.get("plan_signature"))
        .and_then(|value| if value.is_null() { None } else { Some(value) })
}

fn unsigned_execution_plan_payload(plan: &Value) -> Result<Value, RegistryError> {
    let mut payload = plan.clone();
    let object = payload.as_object_mut().ok_or_else(|| {
        RegistryError::new(
            "execution-plan-invalid",
            "execution plan must be a top-level JSON object",
        )
    })?;
    match object.get("kind").and_then(Value::as_str) {
        Some("guild.execution_plan") => {}
        Some(_) | None => {
            return Err(RegistryError::new(
                "execution-plan-invalid",
                "execution plan must declare kind `guild.execution_plan`",
            )
            .with_detail(object.get("kind").cloned().unwrap_or(Value::Null)));
        }
    }
    object.remove("plan_signature");
    Ok(payload)
}

fn parse_bundle_signature_bytes(bytes: &[u8]) -> Result<BundleSignatureEnvelope, RegistryError> {
    let signature: BundleSignatureEnvelope = serde_json::from_slice(bytes).map_err(|error| {
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

fn execution_plan_signature(plan: &Value) -> Result<ExecutionPlanSignatureEnvelope, RegistryError> {
    let signature_value = plan_signature_field(plan).ok_or_else(|| {
        RegistryError::new(
            "execution-plan-signature-missing",
            "execution plan did not contain a plan signature",
        )
    })?;
    let signature: ExecutionPlanSignatureEnvelope = serde_json::from_value(signature_value.clone())
        .map_err(|error| {
            RegistryError::new(
                "execution-plan-signature-parse-failed",
                "failed to parse execution plan signature metadata",
            )
            .with_detail(error.to_string())
        })?;
    if signature.format_version != EXECUTION_PLAN_SIGNATURE_FORMAT_VERSION {
        return Err(RegistryError::new(
            "execution-plan-signature-format-unsupported",
            "execution plan signature format version is unsupported",
        )
        .with_detail(serde_json::json!({
            "expected": EXECUTION_PLAN_SIGNATURE_FORMAT_VERSION,
            "actual": signature.format_version,
        })));
    }
    Ok(signature)
}

fn load_trusted_publisher(
    root: &Path,
    publisher_id: &str,
) -> Result<TrustedPublisherRecord, RegistryError> {
    load_trusted_publisher_for_subject(
        root,
        publisher_id,
        "bundle-publisher-untrusted",
        "signed bundle publisher was not trusted by the target Guild root",
        publisher_id,
    )
}

fn load_trusted_publisher_for_subject(
    root: &Path,
    publisher_id: &str,
    not_found_code: &'static str,
    not_found_message: &'static str,
    not_found_detail: impl Into<Value>,
) -> Result<TrustedPublisherRecord, RegistryError> {
    let path = trusted_publisher_path(root, publisher_id);
    read_trusted_publisher_record_with_not_found_detail(
        &path,
        not_found_code,
        not_found_message,
        not_found_detail,
    )
}

fn read_trusted_publisher_record(path: &Path) -> Result<TrustedPublisherRecord, RegistryError> {
    read_trusted_publisher_record_with_not_found_detail(
        path,
        "trusted-publisher-missing",
        "trusted publisher record was not found",
        path.display().to_string(),
    )
}

fn read_trusted_publisher_record_with_not_found_detail(
    path: &Path,
    not_found_code: &'static str,
    not_found_message: &'static str,
    not_found_detail: impl Into<Value>,
) -> Result<TrustedPublisherRecord, RegistryError> {
    let contents = fs::read_to_string(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            RegistryError::new(not_found_code, not_found_message)
                .with_detail(not_found_detail.into())
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
    validate_trusted_publisher_record(&publisher)?;
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

fn validate_trusted_publisher_record(
    publisher: &TrustedPublisherRecord,
) -> Result<(), RegistryError> {
    if publisher.trust_tier == LocalTrustTier::LocalDev {
        return Err(RegistryError::new(
            "trusted-publisher-tier-invalid",
            "trusted publisher records must use an imported trust tier",
        )
        .with_detail(serde_json::json!({
            "publisher_id": publisher.publisher.id,
            "trust_tier": publisher.trust_tier,
        })));
    }

    trusted_publisher_verifying_key(publisher)?;
    Ok(())
}

fn verify_execution_plan_with_trusted_publisher(
    plan: &Value,
    signature: &ExecutionPlanSignatureEnvelope,
    trusted_publisher: &TrustedPublisherRecord,
) -> Result<ExecutionPlanVerification, RegistryError> {
    let payload = unsigned_execution_plan_payload(plan)?;
    let payload_bytes = canonical_json_bytes(&payload);
    let expected_digest = sha256_structured_digest(&payload_bytes);

    if signature.publisher_id != trusted_publisher.publisher.id {
        return Err(RegistryError::new(
            "execution-plan-signature-publisher-mismatch",
            "execution plan signature publisher id did not match the trusted publisher",
        )
        .with_detail(serde_json::json!({
            "trusted_publisher_id": trusted_publisher.publisher.id,
            "signature_publisher_id": signature.publisher_id,
        })));
    }
    if signature.scheme != trusted_publisher.scheme {
        return Err(RegistryError::new(
            "execution-plan-signature-scheme-mismatch",
            "trusted publisher record used a different signature scheme than the signed execution plan",
        ));
    }
    if signature.signed_digest.algorithm != "sha256" {
        return Err(RegistryError::new(
            "execution-plan-signature-digest-unsupported",
            "execution plan signatures currently require a sha256 signed digest",
        )
        .with_detail(serde_json::json!({
            "actual": signature.signed_digest.algorithm,
        })));
    }
    if signature.signed_digest != expected_digest {
        return Err(RegistryError::new(
            "execution-plan-signature-digest-mismatch",
            "execution plan signature metadata did not match the execution plan bytes",
        )
        .with_detail(serde_json::json!({
            "expected": expected_digest,
            "actual": signature.signed_digest,
        })));
    }

    let verifying_key = trusted_publisher_verifying_key(trusted_publisher)?;
    let signature_bytes = decode_fixed_base64::<64>(
        &signature.signature_base64,
        "execution-plan-signature-invalid",
        "execution plan signature bytes were invalid",
    )?;
    let signature_bytes = Signature::from_bytes(&signature_bytes);
    verifying_key
        .verify(&payload_bytes, &signature_bytes)
        .map_err(|error| {
            RegistryError::new(
                "execution-plan-signature-invalid",
                "execution plan signature verification failed",
            )
            .with_detail(error.to_string())
        })?;

    Ok(ExecutionPlanVerification {
        publisher: trusted_publisher.publisher.clone(),
        scheme: trusted_publisher.scheme.clone(),
        signed_digest: signature.signed_digest.clone(),
        trust_tier: trusted_publisher.trust_tier.clone(),
    })
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

fn sign_execution_plan_payload(
    signer: &LocalPublisherIdentity,
    payload_bytes: &[u8],
) -> Result<ExecutionPlanSignatureEnvelope, RegistryError> {
    let signing_key = signer.signing_key()?;
    let signature = signing_key.sign(payload_bytes);
    Ok(ExecutionPlanSignatureEnvelope {
        format_version: EXECUTION_PLAN_SIGNATURE_FORMAT_VERSION.into(),
        scheme: signer.scheme.clone(),
        publisher_id: signer.publisher.id.clone(),
        signed_digest: sha256_structured_digest(payload_bytes),
        signature_base64: base64_encode(&signature.to_bytes()),
    })
}

fn json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, RegistryError> {
    serde_json::to_vec_pretty(value).map_err(|error| {
        RegistryError::new("json-serialize-failed", "failed to serialize JSON")
            .with_detail(error.to_string())
    })
}

fn canonical_json_bytes(value: &Value) -> Vec<u8> {
    let mut buffer = String::new();
    write_canonical_json(value, &mut buffer);
    buffer.into_bytes()
}

fn write_canonical_json(value: &Value, output: &mut String) {
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {
            output.push_str(&serde_json::to_string(value).expect("primitive JSON serializes"));
        }
        Value::Array(items) => {
            output.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                write_canonical_json(item, output);
            }
            output.push(']');
        }
        Value::Object(map) => {
            let mut entries: Vec<_> = map.iter().collect();
            entries.sort_by(|left, right| left.0.cmp(right.0));
            output.push('{');
            for (index, (key, item)) in entries.into_iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                output.push_str(
                    &serde_json::to_string(key).expect("object keys serialize as strings"),
                );
                output.push(':');
                write_canonical_json(item, output);
            }
            output.push('}');
        }
    }
}

fn sha256_structured_digest(bytes: &[u8]) -> StructuredDigest {
    StructuredDigest {
        algorithm: "sha256".into(),
        value: sha256_bytes(bytes),
    }
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
        .with_detail(verification.signature.format_version.as_str()));
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

fn derive_installed_trust_metadata(
    root: &Path,
    verification: Option<&InstalledVerificationRecord>,
) -> Result<InstalledTrustMetadata, RegistryError> {
    match verification {
        None => Ok(InstalledTrustMetadata {
            verification_state: InstalledVerificationState::LocalSource,
            trust_tier: LocalTrustTier::LocalDev,
        }),
        Some(verification) => match load_trusted_publisher(root, &verification.publisher.id) {
            Ok(publisher) => Ok(InstalledTrustMetadata {
                verification_state: InstalledVerificationState::VerifiedImport,
                trust_tier: publisher.trust_tier,
            }),
            Err(error) if error.code == "bundle-publisher-untrusted" => {
                Ok(InstalledTrustMetadata {
                    verification_state: InstalledVerificationState::VerifiedImport,
                    trust_tier: LocalTrustTier::Restricted,
                })
            }
            Err(error) => Err(error),
        },
    }
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
