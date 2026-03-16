use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{
    bundle_file_relative_from_str, bundle_index_path, bundle_signature_path,
    ensure_registry_layout, json_bytes, parse_bundle_index, parse_bundle_signature_bytes,
    sha256_bytes, sha256_file, write_bytes, write_json, InstalledSkill, InstalledSkillBundle,
    LocalRegistry, RegistryError, SignedBundlePayload,
};

const OCI_IMAGE_LAYOUT_VERSION: &str = "1.0.0";
const OCI_IMAGE_MANIFEST_MEDIA_TYPE: &str = "application/vnd.oci.image.manifest.v1+json";
const GUILD_OCI_ARTIFACT_TYPE: &str = "application/vnd.guild.installed-bundle.oci.v1";
const GUILD_BUNDLE_CONFIG_MEDIA_TYPE: &str = "application/vnd.guild.installed-bundle.v2+json";
const GUILD_BUNDLE_SIGNATURE_MEDIA_TYPE: &str =
    "application/vnd.guild.installed-bundle.signature.v1+json";
const GUILD_BUNDLE_FILE_MEDIA_TYPE: &str = "application/vnd.guild.installed-bundle.file.v1";
const OCI_TITLE_ANNOTATION: &str = "org.opencontainers.image.title";
const GUILD_ROOT_KEY_ANNOTATION: &str = "dev.guild.root-skill.key";
const GUILD_ROOT_VERSION_ANNOTATION: &str = "dev.guild.root-skill.version";
const GUILD_ROOT_DIGEST_ANNOTATION: &str = "dev.guild.root-skill.digest";
const GUILD_PUBLISHER_ID_ANNOTATION: &str = "dev.guild.publisher.id";
const GUILD_CLOSURE_ANNOTATION: &str = "dev.guild.includes-dependency-closure";

#[derive(Debug, Serialize, Deserialize)]
struct OciLayoutMetadata {
    #[serde(rename = "imageLayoutVersion")]
    image_layout_version: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OciImageIndex {
    schema_version: u32,
    manifests: Vec<OciDescriptor>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OciImageManifest {
    schema_version: u32,
    artifact_type: Option<String>,
    config: OciDescriptor,
    layers: Vec<OciDescriptor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OciDescriptor {
    media_type: String,
    digest: String,
    size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    annotations: Option<BTreeMap<String, String>>,
}

#[derive(Debug)]
struct DecodedOciLayout {
    bundle_bytes: Vec<u8>,
    signature_bytes: Vec<u8>,
    files: Vec<DecodedOciFile>,
}

#[derive(Debug)]
struct ValidatedOciManifest {
    root_descriptor: OciDescriptor,
    manifest: OciImageManifest,
}

#[derive(Debug)]
struct DecodedBundleMetadata {
    bundle: InstalledSkillBundle,
    bundle_bytes: Vec<u8>,
    signature_bytes: Vec<u8>,
}

#[derive(Debug)]
struct DecodedOciFile {
    relative_path: String,
    blob_path: PathBuf,
}

pub(super) fn export_oci_layout(
    payload: &SignedBundlePayload,
    layout_root: impl AsRef<Path>,
) -> Result<(), RegistryError> {
    let layout_root = prepare_oci_layout_root(layout_root)?;
    write_json(
        &layout_root.join("oci-layout"),
        &OciLayoutMetadata {
            image_layout_version: OCI_IMAGE_LAYOUT_VERSION.into(),
        },
    )?;

    let config_descriptor = write_oci_blob(
        &layout_root,
        GUILD_BUNDLE_CONFIG_MEDIA_TYPE,
        &payload.bundle_bytes,
        Some(single_annotation(OCI_TITLE_ANNOTATION, "bundle.json")),
    )?;
    let signature_bytes = json_bytes(&payload.signature)?;
    let signature_descriptor = write_oci_blob(
        &layout_root,
        GUILD_BUNDLE_SIGNATURE_MEDIA_TYPE,
        &signature_bytes,
        Some(single_annotation(
            OCI_TITLE_ANNOTATION,
            "bundle.signature.json",
        )),
    )?;

    let mut layers = Vec::with_capacity(payload.files.len() + 1);
    layers.push(signature_descriptor);
    for file in &payload.files {
        let bytes = fs::read(&file.source_path).map_err(|error| {
            RegistryError::new(
                "oci-layout-file-read-failed",
                "failed to read installed content while exporting OCI layout",
            )
            .with_detail(serde_json::json!({
                "path": file.source_path.display().to_string(),
                "cause": error.to_string(),
            }))
        })?;
        let descriptor = write_oci_blob(
            &layout_root,
            GUILD_BUNDLE_FILE_MEDIA_TYPE,
            &bytes,
            Some(single_annotation(
                OCI_TITLE_ANNOTATION,
                file.relative_path.as_str(),
            )),
        )?;
        if descriptor.digest != file.sha256 {
            return Err(RegistryError::new(
                "oci-layout-file-digest-mismatch",
                "OCI layout file blob digest did not match the signed bundle file digest",
            )
            .with_detail(serde_json::json!({
                "path": file.relative_path,
                "expected": file.sha256,
                "actual": descriptor.digest,
            })));
        }
        layers.push(descriptor);
    }

    let manifest = OciImageManifest {
        schema_version: 2,
        artifact_type: Some(GUILD_OCI_ARTIFACT_TYPE.into()),
        config: config_descriptor,
        layers,
    };
    let manifest_bytes = json_bytes(&manifest)?;
    let mut root_descriptor =
        descriptor_for_blob(OCI_IMAGE_MANIFEST_MEDIA_TYPE, &manifest_bytes, None)?;
    root_descriptor.annotations = Some(root_descriptor_annotations(&payload.bundle));
    write_oci_blob_at_digest(&layout_root, &root_descriptor.digest, &manifest_bytes)?;

    write_json(
        &layout_root.join("index.json"),
        &OciImageIndex {
            schema_version: 2,
            manifests: vec![root_descriptor],
        },
    )?;

    Ok(())
}

pub(super) fn import_oci_layout(
    root: &Path,
    layout_root: &Path,
) -> Result<Vec<InstalledSkill>, RegistryError> {
    let root = ensure_registry_layout(root)?;
    let layout_root = open_oci_layout_root(layout_root)?;
    let decoded = decode_oci_layout(&layout_root)?;
    let staging_root = oci_import_staging_root(&root);
    if staging_root.exists() {
        fs::remove_dir_all(&staging_root).map_err(|error| {
            RegistryError::new(
                "oci-layout-import-staging-cleanup-failed",
                "failed to remove previous OCI layout import staging directory",
            )
            .with_detail(error.to_string())
        })?;
    }
    fs::create_dir_all(&staging_root).map_err(|error| {
        RegistryError::new(
            "oci-layout-import-staging-create-failed",
            "failed to create OCI layout import staging directory",
        )
        .with_detail(error.to_string())
    })?;

    let staged_result = (|| -> Result<Vec<InstalledSkill>, RegistryError> {
        let bundle_root = staging_root.join("bundle");
        fs::create_dir_all(&bundle_root).map_err(|error| {
            RegistryError::new(
                "oci-layout-import-staging-create-failed",
                "failed to create reconstructed bundle staging directory",
            )
            .with_detail(error.to_string())
        })?;
        write_bytes(&bundle_index_path(&bundle_root), &decoded.bundle_bytes)?;
        write_bytes(
            &bundle_signature_path(&bundle_root),
            &decoded.signature_bytes,
        )?;
        write_decoded_files(&bundle_root, &decoded.files)?;
        LocalRegistry::import_bundle(&root, &bundle_root)
    })();

    let cleanup_result = if staging_root.exists() {
        fs::remove_dir_all(&staging_root).map_err(|error| {
            RegistryError::new(
                "oci-layout-import-staging-cleanup-failed",
                "failed to clean OCI layout import staging directory",
            )
            .with_detail(error.to_string())
        })
    } else {
        Ok(())
    };

    match (staged_result, cleanup_result) {
        (Ok(imported), Ok(())) => Ok(imported),
        (Err(error), _) | (Ok(_), Err(error)) => Err(error),
    }
}

fn decode_oci_layout(layout_root: &Path) -> Result<DecodedOciLayout, RegistryError> {
    validate_layout_metadata(layout_root)?;
    let validated_manifest = load_validated_root_manifest(layout_root)?;
    let decoded_bundle = decode_bundle_metadata(
        layout_root,
        &validated_manifest.root_descriptor,
        &validated_manifest.manifest,
    )?;
    let files = decode_file_layers(
        layout_root,
        &decoded_bundle.bundle,
        &validated_manifest.manifest,
    )?;

    Ok(DecodedOciLayout {
        bundle_bytes: decoded_bundle.bundle_bytes,
        signature_bytes: decoded_bundle.signature_bytes,
        files,
    })
}

fn validate_layout_metadata(layout_root: &Path) -> Result<(), RegistryError> {
    let metadata = read_layout_metadata(layout_root)?;
    if metadata.image_layout_version != OCI_IMAGE_LAYOUT_VERSION {
        return Err(RegistryError::new(
            "oci-layout-format-unsupported",
            "OCI image layout version is unsupported",
        )
        .with_detail(serde_json::json!({
            "expected": OCI_IMAGE_LAYOUT_VERSION,
            "actual": metadata.image_layout_version,
        })));
    }

    Ok(())
}

fn load_validated_root_manifest(layout_root: &Path) -> Result<ValidatedOciManifest, RegistryError> {
    let index = read_layout_index(layout_root)?;
    if index.schema_version != 2 {
        return Err(RegistryError::new(
            "oci-layout-index-invalid",
            "OCI layout index used an unsupported schema version",
        )
        .with_detail(index.schema_version));
    }
    if index.manifests.len() != 1 {
        return Err(RegistryError::new(
            "oci-layout-index-invalid",
            "OCI layout index must contain exactly one root manifest descriptor",
        )
        .with_detail(index.manifests.len()));
    }

    let root_descriptor = index.manifests.into_iter().next().expect("single manifest");
    if root_descriptor.media_type != OCI_IMAGE_MANIFEST_MEDIA_TYPE {
        return Err(RegistryError::new(
            "oci-layout-index-invalid",
            "OCI layout root descriptor must reference an OCI image manifest",
        )
        .with_detail(root_descriptor.media_type.clone()));
    }

    let manifest_bytes = read_oci_blob_bytes(layout_root, &root_descriptor)?;
    let manifest = parse_and_validate_root_manifest(&manifest_bytes)?;
    Ok(ValidatedOciManifest {
        root_descriptor,
        manifest,
    })
}

fn parse_and_validate_root_manifest(
    manifest_bytes: &[u8],
) -> Result<OciImageManifest, RegistryError> {
    let manifest: OciImageManifest = serde_json::from_slice(manifest_bytes).map_err(|error| {
        RegistryError::new(
            "oci-layout-manifest-parse-failed",
            "failed to parse OCI root manifest",
        )
        .with_detail(error.to_string())
    })?;
    if manifest.schema_version != 2 {
        return Err(RegistryError::new(
            "oci-layout-manifest-invalid",
            "OCI root manifest used an unsupported schema version",
        )
        .with_detail(manifest.schema_version));
    }
    if manifest.artifact_type.as_deref() != Some(GUILD_OCI_ARTIFACT_TYPE) {
        return Err(RegistryError::new(
            "oci-layout-manifest-invalid",
            "OCI root manifest used an unsupported Guild artifact type",
        )
        .with_detail(manifest.artifact_type));
    }
    if manifest.config.media_type != GUILD_BUNDLE_CONFIG_MEDIA_TYPE {
        return Err(RegistryError::new(
            "oci-layout-manifest-invalid",
            "OCI root manifest config did not carry a Guild bundle index",
        )
        .with_detail(manifest.config.media_type.clone()));
    }

    Ok(manifest)
}

fn decode_bundle_metadata(
    layout_root: &Path,
    root_descriptor: &OciDescriptor,
    manifest: &OciImageManifest,
) -> Result<DecodedBundleMetadata, RegistryError> {
    let bundle_bytes = read_oci_blob_bytes(layout_root, &manifest.config)?;
    let bundle = parse_bundle_index(&bundle_bytes)?;
    validate_root_descriptor_annotations(root_descriptor, &bundle)?;

    let signature_descriptor = find_signature_descriptor(manifest)?;
    let signature_bytes = read_oci_blob_bytes(layout_root, signature_descriptor)?;
    let signature = parse_bundle_signature_bytes(&signature_bytes)?;
    if manifest.config.digest != signature.bundle_sha256 {
        return Err(RegistryError::new(
            "oci-layout-config-digest-mismatch",
            "OCI bundle config digest did not match the signed bundle digest",
        )
        .with_detail(serde_json::json!({
            "config_digest": manifest.config.digest,
            "bundle_sha256": signature.bundle_sha256,
        })));
    }

    Ok(DecodedBundleMetadata {
        bundle,
        bundle_bytes,
        signature_bytes,
    })
}

fn find_signature_descriptor(manifest: &OciImageManifest) -> Result<&OciDescriptor, RegistryError> {
    let signature_layers = manifest
        .layers
        .iter()
        .filter(|layer| layer.media_type == GUILD_BUNDLE_SIGNATURE_MEDIA_TYPE)
        .collect::<Vec<_>>();
    if signature_layers.is_empty() {
        return Err(RegistryError::new(
            "oci-layout-signature-missing",
            "OCI root manifest did not include a Guild bundle signature layer",
        ));
    }
    if signature_layers.len() != 1 {
        return Err(RegistryError::new(
            "oci-layout-signature-duplicate",
            "OCI root manifest included multiple Guild bundle signature layers",
        )
        .with_detail(signature_layers.len()));
    }

    Ok(signature_layers[0])
}

fn decode_file_layers(
    layout_root: &Path,
    bundle: &InstalledSkillBundle,
    manifest: &OciImageManifest,
) -> Result<Vec<DecodedOciFile>, RegistryError> {
    let mut files_by_path = indexed_file_layers(layout_root, manifest)?;
    let mut files = Vec::with_capacity(bundle.files.len());
    for file in &bundle.files {
        let decoded = files_by_path.remove(&file.path).ok_or_else(|| {
            RegistryError::new(
                "oci-layout-file-missing",
                "OCI layout omitted a file listed by the signed bundle index",
            )
            .with_detail(file.path.clone())
        })?;
        let actual_digest = sha256_file(&decoded.blob_path)?;
        if actual_digest != file.sha256 {
            return Err(RegistryError::new(
                "oci-layout-file-digest-mismatch",
                "OCI file layer digest did not match the signed bundle file digest",
            )
            .with_detail(serde_json::json!({
                "path": file.path,
                "expected": file.sha256,
                "actual": actual_digest,
            })));
        }
        files.push(decoded);
    }

    if let Some(extra) = files_by_path.into_keys().next() {
        return Err(RegistryError::new(
            "oci-layout-unexpected-content",
            "OCI layout included a bundled file that was not listed in the signed bundle index",
        )
        .with_detail(extra));
    }

    Ok(files)
}

fn indexed_file_layers(
    layout_root: &Path,
    manifest: &OciImageManifest,
) -> Result<BTreeMap<String, DecodedOciFile>, RegistryError> {
    let mut files_by_path = BTreeMap::new();
    for layer in &manifest.layers {
        if layer.media_type == GUILD_BUNDLE_SIGNATURE_MEDIA_TYPE {
            continue;
        }
        if layer.media_type != GUILD_BUNDLE_FILE_MEDIA_TYPE {
            return Err(RegistryError::new(
                "oci-layout-manifest-invalid",
                "OCI root manifest included an unsupported file layer media type",
            )
            .with_detail(layer.media_type.clone()));
        }
        let title = descriptor_annotation(layer, OCI_TITLE_ANNOTATION).ok_or_else(|| {
            RegistryError::new(
                "oci-layout-file-annotation-missing",
                "OCI file layer was missing its bundle-relative path annotation",
            )
            .with_detail(layer.digest.clone())
        })?;
        let relative = title.to_owned();
        bundle_file_relative_from_str(&relative)?;
        if files_by_path.contains_key(&relative) {
            return Err(RegistryError::new(
                "oci-layout-file-duplicate",
                "OCI root manifest declared the same bundled file more than once",
            )
            .with_detail(relative));
        }
        let blob_path = validate_oci_blob_file(layout_root, layer)?;
        files_by_path.insert(
            relative.clone(),
            DecodedOciFile {
                relative_path: relative,
                blob_path,
            },
        );
    }

    Ok(files_by_path)
}

fn write_decoded_files(bundle_root: &Path, files: &[DecodedOciFile]) -> Result<(), RegistryError> {
    for file in files {
        let relative = bundle_file_relative_from_str(&file.relative_path)?;
        let destination = bundle_root.join(relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                RegistryError::new(
                    "oci-layout-import-file-create-failed",
                    "failed to create parent directory while reconstructing bundled files",
                )
                .with_detail(error.to_string())
            })?;
        }
        fs::copy(&file.blob_path, &destination).map_err(|error| {
            RegistryError::new(
                "oci-layout-import-file-copy-failed",
                "failed to reconstruct a bundled file from an OCI blob",
            )
            .with_detail(serde_json::json!({
                "source": file.blob_path.display().to_string(),
                "destination": destination.display().to_string(),
                "cause": error.to_string(),
            }))
        })?;
    }

    Ok(())
}

fn prepare_oci_layout_root(path: impl AsRef<Path>) -> Result<PathBuf, RegistryError> {
    let path = path.as_ref();
    if path.exists() {
        if !path.is_dir() {
            return Err(RegistryError::new(
                "oci-layout-root-invalid",
                "OCI layout export target must be a directory",
            )
            .with_detail(path.display().to_string()));
        }

        let mut entries = fs::read_dir(path).map_err(|error| {
            RegistryError::new(
                "oci-layout-root-read-failed",
                "failed to inspect OCI layout export target directory",
            )
            .with_detail(error.to_string())
        })?;
        if entries.next().is_some() {
            return Err(RegistryError::new(
                "oci-layout-root-not-empty",
                "OCI layout export target directory must be empty",
            )
            .with_detail(path.display().to_string()));
        }
    } else {
        fs::create_dir_all(path).map_err(|error| {
            RegistryError::new(
                "oci-layout-root-create-failed",
                "failed to create OCI layout export target directory",
            )
            .with_detail(error.to_string())
        })?;
    }

    path.canonicalize().map_err(|error| {
        RegistryError::new(
            "oci-layout-root-open-failed",
            "failed to canonicalize OCI layout export target directory",
        )
        .with_detail(error.to_string())
    })
}

fn open_oci_layout_root(path: impl AsRef<Path>) -> Result<PathBuf, RegistryError> {
    let path = path.as_ref();
    if !path.exists() {
        return Err(RegistryError::new(
            "oci-layout-root-missing",
            "OCI layout directory does not exist",
        )
        .with_detail(path.display().to_string()));
    }
    if !path.is_dir() {
        return Err(RegistryError::new(
            "oci-layout-root-invalid",
            "OCI layout path must be a directory",
        )
        .with_detail(path.display().to_string()));
    }

    path.canonicalize().map_err(|error| {
        RegistryError::new(
            "oci-layout-root-open-failed",
            "failed to open OCI layout directory",
        )
        .with_detail(error.to_string())
    })
}

fn read_layout_metadata(layout_root: &Path) -> Result<OciLayoutMetadata, RegistryError> {
    let bytes = fs::read(layout_root.join("oci-layout")).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            RegistryError::new(
                "oci-layout-metadata-missing",
                "oci-layout file was not found in the OCI layout directory",
            )
        } else {
            RegistryError::new(
                "oci-layout-metadata-read-failed",
                "failed to read OCI layout metadata",
            )
            .with_detail(error.to_string())
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        RegistryError::new(
            "oci-layout-metadata-parse-failed",
            "failed to parse OCI layout metadata",
        )
        .with_detail(error.to_string())
    })
}

fn read_layout_index(layout_root: &Path) -> Result<OciImageIndex, RegistryError> {
    let bytes = fs::read(layout_root.join("index.json")).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            RegistryError::new(
                "oci-layout-index-missing",
                "index.json was not found in the OCI layout directory",
            )
        } else {
            RegistryError::new(
                "oci-layout-index-read-failed",
                "failed to read OCI layout index",
            )
            .with_detail(error.to_string())
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        RegistryError::new(
            "oci-layout-index-parse-failed",
            "failed to parse OCI layout index",
        )
        .with_detail(error.to_string())
    })
}

fn write_oci_blob(
    layout_root: &Path,
    media_type: &str,
    bytes: &[u8],
    annotations: Option<BTreeMap<String, String>>,
) -> Result<OciDescriptor, RegistryError> {
    let descriptor = descriptor_for_blob(media_type, bytes, annotations)?;
    write_oci_blob_at_digest(layout_root, &descriptor.digest, bytes)?;
    Ok(descriptor)
}

fn descriptor_for_blob(
    media_type: &str,
    bytes: &[u8],
    annotations: Option<BTreeMap<String, String>>,
) -> Result<OciDescriptor, RegistryError> {
    let size = u64::try_from(bytes.len()).map_err(|_| {
        RegistryError::new(
            "oci-layout-blob-size-invalid",
            "OCI layout blob size exceeded the supported descriptor range",
        )
    })?;
    Ok(OciDescriptor {
        media_type: media_type.into(),
        digest: format!("sha256:{}", sha256_bytes(bytes)),
        size,
        annotations,
    })
}

fn write_oci_blob_at_digest(
    layout_root: &Path,
    digest: &str,
    bytes: &[u8],
) -> Result<(), RegistryError> {
    let path = blob_path_for_digest(layout_root, digest)?;
    write_bytes(&path, bytes)
}

fn read_oci_blob_bytes(
    layout_root: &Path,
    descriptor: &OciDescriptor,
) -> Result<Vec<u8>, RegistryError> {
    let blob_path = blob_path_for_digest(layout_root, &descriptor.digest)?;
    let bytes = fs::read(&blob_path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            RegistryError::new(
                "oci-layout-blob-missing",
                "OCI layout descriptor referenced a missing blob",
            )
            .with_detail(serde_json::json!({
                "digest": descriptor.digest,
                "path": blob_path.display().to_string(),
            }))
        } else {
            RegistryError::new(
                "oci-layout-blob-read-failed",
                "failed to read an OCI layout blob",
            )
            .with_detail(error.to_string())
        }
    })?;
    validate_blob_bytes(&bytes, descriptor)?;
    Ok(bytes)
}

fn validate_oci_blob_file(
    layout_root: &Path,
    descriptor: &OciDescriptor,
) -> Result<PathBuf, RegistryError> {
    let blob_path = blob_path_for_digest(layout_root, &descriptor.digest)?;
    if !blob_path.exists() {
        return Err(RegistryError::new(
            "oci-layout-blob-missing",
            "OCI layout descriptor referenced a missing blob",
        )
        .with_detail(serde_json::json!({
            "digest": descriptor.digest,
            "path": blob_path.display().to_string(),
        })));
    }
    let metadata = fs::metadata(&blob_path).map_err(|error| {
        RegistryError::new(
            "oci-layout-blob-read-failed",
            "failed to inspect an OCI layout blob",
        )
        .with_detail(error.to_string())
    })?;
    if metadata.len() != descriptor.size {
        return Err(RegistryError::new(
            "oci-layout-blob-size-mismatch",
            "OCI layout blob size did not match its descriptor",
        )
        .with_detail(serde_json::json!({
            "digest": descriptor.digest,
            "expected": descriptor.size,
            "actual": metadata.len(),
        })));
    }
    let actual = sha256_file(&blob_path)?;
    if actual != descriptor.digest {
        return Err(RegistryError::new(
            "oci-layout-blob-digest-mismatch",
            "OCI layout blob digest did not match its descriptor",
        )
        .with_detail(serde_json::json!({
            "expected": descriptor.digest,
            "actual": actual,
        })));
    }

    Ok(blob_path)
}

fn validate_blob_bytes(bytes: &[u8], descriptor: &OciDescriptor) -> Result<(), RegistryError> {
    let actual_size = u64::try_from(bytes.len()).map_err(|_| {
        RegistryError::new(
            "oci-layout-blob-size-invalid",
            "OCI layout blob size exceeded the supported descriptor range",
        )
    })?;
    if actual_size != descriptor.size {
        return Err(RegistryError::new(
            "oci-layout-blob-size-mismatch",
            "OCI layout blob size did not match its descriptor",
        )
        .with_detail(serde_json::json!({
            "digest": descriptor.digest,
            "expected": descriptor.size,
            "actual": actual_size,
        })));
    }

    let actual_digest = format!("sha256:{}", sha256_bytes(bytes));
    if actual_digest != descriptor.digest {
        return Err(RegistryError::new(
            "oci-layout-blob-digest-mismatch",
            "OCI layout blob digest did not match its descriptor",
        )
        .with_detail(serde_json::json!({
            "expected": descriptor.digest,
            "actual": actual_digest,
        })));
    }

    Ok(())
}

fn blob_path_for_digest(layout_root: &Path, digest: &str) -> Result<PathBuf, RegistryError> {
    let hex = sha256_hex_from_digest(digest)?;
    Ok(layout_root.join("blobs").join("sha256").join(hex))
}

fn sha256_hex_from_digest(digest: &str) -> Result<&str, RegistryError> {
    let Some(hex) = digest.strip_prefix("sha256:") else {
        return Err(RegistryError::new(
            "oci-layout-digest-invalid",
            "OCI layout descriptors must use sha256 digests",
        )
        .with_detail(digest.to_owned()));
    };
    if hex.len() != 64 || !hex.chars().all(|character| character.is_ascii_hexdigit()) {
        return Err(RegistryError::new(
            "oci-layout-digest-invalid",
            "OCI layout digest must contain 64 hexadecimal sha256 characters",
        )
        .with_detail(digest.to_owned()));
    }

    Ok(hex)
}

fn root_descriptor_annotations(bundle: &InstalledSkillBundle) -> BTreeMap<String, String> {
    BTreeMap::from([
        (OCI_TITLE_ANNOTATION.into(), root_skill_title(bundle)),
        (GUILD_ROOT_KEY_ANNOTATION.into(), root_skill_key(bundle)),
        (
            GUILD_ROOT_VERSION_ANNOTATION.into(),
            bundle.root_skill.version.to_string(),
        ),
        (
            GUILD_ROOT_DIGEST_ANNOTATION.into(),
            bundle.root_skill.digest.clone(),
        ),
        (
            GUILD_PUBLISHER_ID_ANNOTATION.into(),
            bundle.publisher.id.clone(),
        ),
        (
            GUILD_CLOSURE_ANNOTATION.into(),
            bundle.includes_dependency_closure.to_string(),
        ),
    ])
}

fn validate_root_descriptor_annotations(
    descriptor: &OciDescriptor,
    bundle: &InstalledSkillBundle,
) -> Result<(), RegistryError> {
    let annotations = descriptor.annotations.as_ref().ok_or_else(|| {
        RegistryError::new(
            "oci-layout-index-invalid",
            "OCI layout root descriptor was missing Guild root annotations",
        )
    })?;
    let expected = root_descriptor_annotations(bundle);
    for (key, value) in expected {
        let actual = annotations.get(&key).ok_or_else(|| {
            RegistryError::new(
                "oci-layout-index-invalid",
                "OCI layout root descriptor was missing a required Guild annotation",
            )
            .with_detail(key.clone())
        })?;
        if actual != &value {
            return Err(RegistryError::new(
                "oci-layout-index-invalid",
                "OCI layout root descriptor annotation did not match the signed bundle root metadata",
            )
            .with_detail(serde_json::json!({
                "annotation": key,
                "expected": value,
                "actual": actual,
            })));
        }
    }

    Ok(())
}

fn root_skill_title(bundle: &InstalledSkillBundle) -> String {
    format!("{}:{}", root_skill_key(bundle), bundle.root_skill.version)
}

fn root_skill_key(bundle: &InstalledSkillBundle) -> String {
    format!(
        "{}/{}",
        bundle.root_skill.key.namespace, bundle.root_skill.key.name
    )
}

fn descriptor_annotation<'a>(descriptor: &'a OciDescriptor, key: &str) -> Option<&'a str> {
    descriptor
        .annotations
        .as_ref()
        .and_then(|annotations| annotations.get(key))
        .map(String::as_str)
}

fn single_annotation(key: &str, value: &str) -> BTreeMap<String, String> {
    BTreeMap::from([(key.to_owned(), value.to_owned())])
}

fn oci_import_staging_root(root: &Path) -> PathBuf {
    root.join(".oci-layout-import-staging")
}
