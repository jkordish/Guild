use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};

use futures_util::TryStreamExt;
use guild_manifest::{SkillManifest, SourceSkillManifest};
use guild_types::{InstalledVerificationState, LocalTrustTier, ResolvedSkillRef};
use http::HeaderValue;
use oci_client::client::ClientConfig;
use oci_client::client::ClientProtocol;
use oci_client::secrets::RegistryAuth;
use oci_client::{Client, Reference, RegistryOperation};
use serde::{Deserialize, Serialize};

use super::{
    ImportPreviewReport, InstalledSkill, InstalledSkillBundle, InstalledTrustMetadata,
    LocalRegistry, OciRegistryAuth, OciRegistryReference, OciRegistryTarget,
    OciRegistryTransportOptions, PublishedOciArtifact, RegistryError, SignedBundlePayload,
    VERIFICATION_FILENAME, ValidatedBundleSkill, bundle_file_relative_from_str, bundle_index_path,
    bundle_install_dir_relative_from_str, bundle_signature_path, ensure_registry_layout,
    json_bytes, maybe_resolve_local_file, open_existing_registry_root, parse_bundle_index,
    parse_bundle_signature_bytes, path_string, preview_bundle_import, resolve_local_file,
    sha256_bytes, validate_bundle_file_set_alignment, validate_bundle_index_shape,
    validate_bundle_root_and_dependency_closure, validate_import_targets,
    validate_installed_manifest, write_bytes, write_json,
};

const OCI_IMAGE_LAYOUT_VERSION: &str = "1.0.0";
const OCI_IMAGE_INDEX_MEDIA_TYPE: &str = "application/vnd.oci.image.index.v1+json";
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OciLayoutMetadata {
    #[serde(rename = "imageLayoutVersion")]
    image_layout_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OciImageIndex {
    schema_version: u32,
    manifests: Vec<OciDescriptor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone)]
struct OciBlob {
    descriptor: OciDescriptor,
    bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
struct BuiltOciArtifact {
    index_bytes: Vec<u8>,
    manifest_bytes: Vec<u8>,
    root_descriptor: OciDescriptor,
    config: OciBlob,
    layers: Vec<OciBlob>,
}

#[derive(Debug)]
struct DecodedOciArtifact {
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
    bytes: Vec<u8>,
}

pub(super) fn export_oci_layout(
    payload: &SignedBundlePayload,
    layout_root: impl AsRef<Path>,
) -> Result<(), RegistryError> {
    let artifact = build_oci_artifact(payload)?;
    let layout_root = prepare_oci_layout_root(layout_root)?;
    write_json(
        &layout_root.join("oci-layout"),
        &OciLayoutMetadata {
            image_layout_version: OCI_IMAGE_LAYOUT_VERSION.into(),
        },
    )?;
    write_oci_blob_at_digest(
        &layout_root,
        &artifact.config.descriptor.digest,
        &artifact.config.bytes,
    )?;
    for layer in &artifact.layers {
        write_oci_blob_at_digest(&layout_root, &layer.descriptor.digest, &layer.bytes)?;
    }
    write_oci_blob_at_digest(
        &layout_root,
        &artifact.root_descriptor.digest,
        &artifact.manifest_bytes,
    )?;
    write_bytes(&layout_root.join("index.json"), &artifact.index_bytes)?;

    Ok(())
}

pub(super) fn import_oci_layout(
    root: &Path,
    layout_root: &Path,
) -> Result<Vec<InstalledSkill>, RegistryError> {
    let layout_root = open_oci_layout_root(layout_root)?;
    validate_layout_metadata(&layout_root)?;
    let index_bytes = read_layout_index_bytes(&layout_root)?;
    let validated_manifest = load_validated_root_manifest(&index_bytes, &|descriptor| {
        read_oci_blob_bytes(&layout_root, descriptor)
    })?;
    let decoded = decode_oci_artifact(
        &validated_manifest.root_descriptor,
        &validated_manifest.manifest,
        &|descriptor| read_oci_blob_bytes(&layout_root, descriptor),
    )?;
    import_decoded_oci_artifact(root, &decoded, ".oci-layout-import-staging", "oci-layout")
}

pub(super) fn preview_import_oci_layout(
    root: &Path,
    layout_root: &Path,
) -> Result<ImportPreviewReport, RegistryError> {
    let root = open_existing_registry_root(root)?;
    let layout_root = open_oci_layout_root(layout_root)?;
    validate_layout_metadata(&layout_root)?;
    let index_bytes = read_layout_index_bytes(&layout_root)?;
    let validated_manifest = load_validated_root_manifest(&index_bytes, &|descriptor| {
        read_oci_blob_bytes(&layout_root, descriptor)
    })?;
    let decoded = decode_oci_artifact(
        &validated_manifest.root_descriptor,
        &validated_manifest.manifest,
        &|descriptor| read_oci_blob_bytes(&layout_root, descriptor),
    )?;

    preview_decoded_oci_artifact(&root, &decoded)
}

pub(super) fn push_oci_registry(
    payload: &SignedBundlePayload,
    reference: &OciRegistryReference,
    options: &OciRegistryTransportOptions,
) -> Result<PublishedOciArtifact, RegistryError> {
    if matches!(reference.target, OciRegistryTarget::Digest(_)) {
        return Err(RegistryError::new(
            "oci-registry-push-reference-invalid",
            "OCI registry push targets must use a tag reference",
        )
        .with_detail(registry_reference_string(reference)));
    }

    let artifact = build_oci_artifact(payload)?;
    let expected_manifest_digest = format!("sha256:{}", sha256_bytes(&artifact.index_bytes));
    let reference = reference.clone();
    let options = options.clone();
    let bundle = payload.bundle.clone();
    run_registry_future(async move {
        let client_reference = parse_registry_reference(&reference)?;
        let client = build_registry_client(&reference, &options);
        let auth = registry_auth(&options.auth);
        authenticate_registry(&client, &client_reference, &auth, RegistryOperation::Push).await?;

        upload_registry_blob(&client, &client_reference, &artifact.config).await?;
        for layer in &artifact.layers {
            upload_registry_blob(&client, &client_reference, layer).await?;
        }

        client
            .push_manifest_raw(
                &client_reference,
                artifact.manifest_bytes.clone(),
                HeaderValue::from_static(OCI_IMAGE_MANIFEST_MEDIA_TYPE),
            )
            .await
            .map_err(|error| {
                RegistryError::new(
                    "oci-registry-push-manifest-failed",
                    "failed to push the Guild OCI image manifest to the registry",
                )
                .with_detail(error.to_string())
            })?;

        client
            .push_manifest_raw(
                &client_reference,
                artifact.index_bytes.clone(),
                HeaderValue::from_static(OCI_IMAGE_INDEX_MEDIA_TYPE),
            )
            .await
            .map_err(|error| {
                RegistryError::new(
                    "oci-registry-push-index-failed",
                    "failed to push the Guild OCI image index to the registry",
                )
                .with_detail(error.to_string())
            })?;

        let published_digest = client
            .fetch_manifest_digest(&client_reference, &auth)
            .await
            .map_err(|error| {
                RegistryError::new(
                    "oci-registry-push-verify-failed",
                    "failed to read back the published OCI manifest digest from the registry",
                )
                .with_detail(error.to_string())
            })?;
        if published_digest != expected_manifest_digest {
            return Err(RegistryError::new(
                "oci-registry-push-digest-mismatch",
                "registry-published OCI index digest did not match the locally assembled artifact",
            )
            .with_detail(serde_json::json!({
                "expected": expected_manifest_digest,
                "actual": published_digest,
                "reference": registry_reference_string(&reference),
            })));
        }

        Ok(PublishedOciArtifact {
            reference,
            manifest_digest: published_digest,
            bundle,
        })
    })
}

pub(super) fn pull_oci_registry(
    root: &Path,
    reference: &OciRegistryReference,
    options: &OciRegistryTransportOptions,
) -> Result<Vec<InstalledSkill>, RegistryError> {
    let reference = reference.clone();
    let options = options.clone();
    let decoded = run_registry_future(async move {
        let client_reference = parse_registry_reference(&reference)?;
        let client = build_registry_client(&reference, &options);
        let auth = registry_auth(&options.auth);
        authenticate_registry(&client, &client_reference, &auth, RegistryOperation::Pull).await?;

        let (index_bytes, _) = client
            .pull_manifest_raw(&client_reference, &auth, &[OCI_IMAGE_INDEX_MEDIA_TYPE])
            .await
            .map_err(|error| {
                RegistryError::new(
                    "oci-registry-index-fetch-failed",
                    "failed to pull the OCI image index for the requested Guild artifact",
                )
                .with_detail(serde_json::json!({
                    "reference": registry_reference_string(&reference),
                    "cause": error.to_string(),
                }))
            })?;
        let root_descriptor = parse_and_validate_root_index(&index_bytes)?;
        let manifest_reference = parse_registry_reference(&reference_with_digest(
            &reference,
            root_descriptor.digest.clone(),
        ))?;
        let (manifest_bytes, _) = client
            .pull_manifest_raw(&manifest_reference, &auth, &[OCI_IMAGE_MANIFEST_MEDIA_TYPE])
            .await
            .map_err(|error| {
                RegistryError::new(
                    "oci-registry-manifest-fetch-failed",
                    "failed to pull the OCI image manifest for the requested Guild artifact",
                )
                .with_detail(serde_json::json!({
                    "reference": registry_reference_string(&reference),
                    "manifest_digest": root_descriptor.digest,
                    "cause": error.to_string(),
                }))
            })?;
        validate_registry_bytes(&manifest_bytes, &root_descriptor)?;
        let manifest = parse_and_validate_root_manifest(&manifest_bytes)?;
        let blob_bytes = fetch_registry_blobs(&client, &client_reference, &manifest).await?;

        decode_oci_artifact(&root_descriptor, &manifest, &|descriptor| {
            blob_bytes.get(&descriptor.digest).cloned().ok_or_else(|| {
                RegistryError::new(
                    "oci-registry-blob-missing",
                    "pulled OCI registry content was missing a required blob",
                )
                .with_detail(descriptor.digest.clone())
            })
        })
    })?;

    import_decoded_oci_artifact(root, &decoded, ".oci-registry-pull-staging", "oci-registry")
}

pub(super) fn preview_pull_oci_registry(
    root: &Path,
    reference: &OciRegistryReference,
    options: &OciRegistryTransportOptions,
) -> Result<ImportPreviewReport, RegistryError> {
    let root = open_existing_registry_root(root)?;
    let reference = reference.clone();
    let options = options.clone();
    let decoded = run_registry_future(async move {
        let client_reference = parse_registry_reference(&reference)?;
        let client = build_registry_client(&reference, &options);
        let auth = registry_auth(&options.auth);
        authenticate_registry(&client, &client_reference, &auth, RegistryOperation::Pull).await?;

        let (index_bytes, _) = client
            .pull_manifest_raw(&client_reference, &auth, &[OCI_IMAGE_INDEX_MEDIA_TYPE])
            .await
            .map_err(|error| {
                RegistryError::new(
                    "oci-registry-index-fetch-failed",
                    "failed to pull the OCI image index for the requested Guild artifact",
                )
                .with_detail(serde_json::json!({
                    "reference": registry_reference_string(&reference),
                    "cause": error.to_string(),
                }))
            })?;
        let root_descriptor = parse_and_validate_root_index(&index_bytes)?;
        let manifest_reference = parse_registry_reference(&reference_with_digest(
            &reference,
            root_descriptor.digest.clone(),
        ))?;
        let (manifest_bytes, _) = client
            .pull_manifest_raw(&manifest_reference, &auth, &[OCI_IMAGE_MANIFEST_MEDIA_TYPE])
            .await
            .map_err(|error| {
                RegistryError::new(
                    "oci-registry-manifest-fetch-failed",
                    "failed to pull the OCI image manifest for the requested Guild artifact",
                )
                .with_detail(serde_json::json!({
                    "reference": registry_reference_string(&reference),
                    "manifest_digest": root_descriptor.digest,
                    "cause": error.to_string(),
                }))
            })?;
        validate_registry_bytes(&manifest_bytes, &root_descriptor)?;
        let manifest = parse_and_validate_root_manifest(&manifest_bytes)?;
        let blob_bytes = fetch_registry_blobs(&client, &client_reference, &manifest).await?;

        decode_oci_artifact(&root_descriptor, &manifest, &|descriptor| {
            blob_bytes.get(&descriptor.digest).cloned().ok_or_else(|| {
                RegistryError::new(
                    "oci-registry-blob-missing",
                    "pulled OCI registry content was missing a required blob",
                )
                .with_detail(descriptor.digest.clone())
            })
        })
    })?;

    preview_decoded_oci_artifact(&root, &decoded)
}

fn build_oci_artifact(payload: &SignedBundlePayload) -> Result<BuiltOciArtifact, RegistryError> {
    let config = OciBlob {
        descriptor: descriptor_for_blob(
            GUILD_BUNDLE_CONFIG_MEDIA_TYPE,
            &payload.bundle_bytes,
            Some(single_annotation(OCI_TITLE_ANNOTATION, "bundle.json")),
        )?,
        bytes: payload.bundle_bytes.clone(),
    };
    let signature_bytes = json_bytes(&payload.signature)?;
    let mut layers = Vec::with_capacity(payload.files.len() + 1);
    layers.push(OciBlob {
        descriptor: descriptor_for_blob(
            GUILD_BUNDLE_SIGNATURE_MEDIA_TYPE,
            &signature_bytes,
            Some(single_annotation(
                OCI_TITLE_ANNOTATION,
                "bundle.signature.json",
            )),
        )?,
        bytes: signature_bytes,
    });

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
        let descriptor = descriptor_for_blob(
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
        layers.push(OciBlob { descriptor, bytes });
    }

    let manifest = OciImageManifest {
        schema_version: 2,
        artifact_type: Some(GUILD_OCI_ARTIFACT_TYPE.into()),
        config: config.descriptor.clone(),
        layers: layers
            .iter()
            .map(|layer| layer.descriptor.clone())
            .collect(),
    };
    let manifest_bytes = json_bytes(&manifest)?;
    let mut root_descriptor =
        descriptor_for_blob(OCI_IMAGE_MANIFEST_MEDIA_TYPE, &manifest_bytes, None)?;
    root_descriptor.annotations = Some(root_descriptor_annotations(&payload.bundle));
    let index = OciImageIndex {
        schema_version: 2,
        manifests: vec![root_descriptor.clone()],
    };

    Ok(BuiltOciArtifact {
        index_bytes: json_bytes(&index)?,
        manifest_bytes,
        root_descriptor,
        config,
        layers,
    })
}

fn decode_oci_artifact(
    root_descriptor: &OciDescriptor,
    manifest: &OciImageManifest,
    load_blob: &impl Fn(&OciDescriptor) -> Result<Vec<u8>, RegistryError>,
) -> Result<DecodedOciArtifact, RegistryError> {
    let decoded_bundle = decode_bundle_metadata(root_descriptor, manifest, load_blob)?;
    let files = decode_file_layers(&decoded_bundle.bundle, manifest, load_blob)?;

    Ok(DecodedOciArtifact {
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

fn load_validated_root_manifest(
    index_bytes: &[u8],
    load_manifest: &impl Fn(&OciDescriptor) -> Result<Vec<u8>, RegistryError>,
) -> Result<ValidatedOciManifest, RegistryError> {
    let root_descriptor = parse_and_validate_root_index(index_bytes)?;
    let manifest_bytes = load_manifest(&root_descriptor)?;
    let manifest = parse_and_validate_root_manifest(&manifest_bytes)?;

    Ok(ValidatedOciManifest {
        root_descriptor,
        manifest,
    })
}

fn parse_and_validate_root_index(index_bytes: &[u8]) -> Result<OciDescriptor, RegistryError> {
    let index: OciImageIndex = serde_json::from_slice(index_bytes).map_err(|error| {
        RegistryError::new(
            "oci-layout-index-parse-failed",
            "failed to parse OCI image index",
        )
        .with_detail(error.to_string())
    })?;
    if index.schema_version != 2 {
        return Err(RegistryError::new(
            "oci-layout-index-invalid",
            "OCI image index used an unsupported schema version",
        )
        .with_detail(index.schema_version));
    }
    if index.manifests.len() != 1 {
        return Err(RegistryError::new(
            "oci-layout-index-invalid",
            "OCI image index must contain exactly one root manifest descriptor",
        )
        .with_detail(index.manifests.len()));
    }

    let root_descriptor = index.manifests.into_iter().next().expect("single manifest");
    if root_descriptor.media_type != OCI_IMAGE_MANIFEST_MEDIA_TYPE {
        return Err(RegistryError::new(
            "oci-layout-index-invalid",
            "OCI image index root descriptor must reference an OCI image manifest",
        )
        .with_detail(root_descriptor.media_type.as_str()));
    }

    Ok(root_descriptor)
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
        .with_detail(manifest.config.media_type.as_str()));
    }

    Ok(manifest)
}

fn decode_bundle_metadata(
    root_descriptor: &OciDescriptor,
    manifest: &OciImageManifest,
    load_blob: &impl Fn(&OciDescriptor) -> Result<Vec<u8>, RegistryError>,
) -> Result<DecodedBundleMetadata, RegistryError> {
    let bundle_bytes = load_blob(&manifest.config)?;
    let bundle = parse_bundle_index(&bundle_bytes)?;
    validate_root_descriptor_annotations(root_descriptor, &bundle)?;

    let signature_descriptor = find_signature_descriptor(manifest)?;
    let signature_bytes = load_blob(signature_descriptor)?;
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
    bundle: &InstalledSkillBundle,
    manifest: &OciImageManifest,
    load_blob: &impl Fn(&OciDescriptor) -> Result<Vec<u8>, RegistryError>,
) -> Result<Vec<DecodedOciFile>, RegistryError> {
    let mut files_by_path = indexed_file_layers(manifest)?;
    let mut files = Vec::with_capacity(bundle.files.len());
    for file in &bundle.files {
        let descriptor = files_by_path.remove(&file.path).ok_or_else(|| {
            RegistryError::new(
                "oci-layout-file-missing",
                "OCI artifact omitted a file listed by the signed bundle index",
            )
            .with_detail(file.path.clone())
        })?;
        let bytes = load_blob(&descriptor)?;
        let actual_digest = format!("sha256:{}", sha256_bytes(&bytes));
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
        files.push(DecodedOciFile {
            relative_path: file.path.clone(),
            bytes,
        });
    }

    if let Some(extra) = files_by_path.into_keys().next() {
        return Err(RegistryError::new(
            "oci-layout-unexpected-content",
            "OCI artifact included a bundled file that was not listed in the signed bundle index",
        )
        .with_detail(extra));
    }

    Ok(files)
}

fn indexed_file_layers(
    manifest: &OciImageManifest,
) -> Result<BTreeMap<String, OciDescriptor>, RegistryError> {
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
        files_by_path.insert(relative, layer.clone());
    }

    Ok(files_by_path)
}

fn import_decoded_oci_artifact(
    root: &Path,
    decoded: &DecodedOciArtifact,
    staging_dir_name: &str,
    code_prefix: &str,
) -> Result<Vec<InstalledSkill>, RegistryError> {
    let root = ensure_registry_layout(root)?;
    let staging_root = root.join(staging_dir_name);
    if staging_root.exists() {
        fs::remove_dir_all(&staging_root).map_err(|error| {
            RegistryError::new(
                format!("{code_prefix}-import-staging-cleanup-failed"),
                "failed to remove previous OCI import staging directory",
            )
            .with_detail(error.to_string())
        })?;
    }
    fs::create_dir_all(&staging_root).map_err(|error| {
        RegistryError::new(
            format!("{code_prefix}-import-staging-create-failed"),
            "failed to create OCI import staging directory",
        )
        .with_detail(error.to_string())
    })?;

    let staged_result = (|| -> Result<Vec<InstalledSkill>, RegistryError> {
        let bundle_root = staging_root.join("bundle");
        fs::create_dir_all(&bundle_root).map_err(|error| {
            RegistryError::new(
                format!("{code_prefix}-import-staging-create-failed"),
                "failed to create reconstructed bundle staging directory",
            )
            .with_detail(error.to_string())
        })?;
        write_bytes(&bundle_index_path(&bundle_root), &decoded.bundle_bytes)?;
        write_bytes(
            &bundle_signature_path(&bundle_root),
            &decoded.signature_bytes,
        )?;
        write_decoded_files(&bundle_root, &decoded.files, code_prefix)?;
        LocalRegistry::import_bundle(&root, &bundle_root)
    })();

    let cleanup_result = if staging_root.exists() {
        fs::remove_dir_all(&staging_root).map_err(|error| {
            RegistryError::new(
                format!("{code_prefix}-import-staging-cleanup-failed"),
                "failed to clean OCI import staging directory",
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

fn preview_decoded_oci_artifact(
    root: &Path,
    decoded: &DecodedOciArtifact,
) -> Result<ImportPreviewReport, RegistryError> {
    let bundle = parse_bundle_index(&decoded.bundle_bytes)?;
    let signature = parse_bundle_signature_bytes(&decoded.signature_bytes)?;

    preview_bundle_import(
        root,
        bundle,
        decoded.bundle_bytes.clone(),
        signature,
        |bundle, verification| {
            let validated = validate_decoded_bundle(bundle, &decoded.files)?;
            validate_import_targets(root, &validated, verification)
        },
    )
}

fn validate_decoded_bundle(
    bundle: &InstalledSkillBundle,
    files: &[DecodedOciFile],
) -> Result<Vec<ValidatedBundleSkill>, RegistryError> {
    validate_bundle_index_shape(bundle)?;
    let files_by_path = decoded_files_by_path(files)?;
    let validated = validate_decoded_bundle_skill_entries(bundle, &files_by_path)?;
    validate_bundle_root_and_dependency_closure(bundle, &validated)?;
    let listed_paths = validate_decoded_listed_bundle_files(bundle, &files_by_path)?;
    let actual_files = collect_decoded_bundle_files(&validated, &files_by_path)?;
    validate_bundle_file_set_alignment(&listed_paths, &actual_files)?;
    Ok(validated)
}

fn decoded_files_by_path(
    files: &[DecodedOciFile],
) -> Result<BTreeMap<String, &[u8]>, RegistryError> {
    let mut indexed = BTreeMap::new();
    for file in files {
        let relative = bundle_file_relative_from_str(&file.relative_path)?;
        let key = path_string(&relative)?;
        if indexed.insert(key.clone(), file.bytes.as_slice()).is_some() {
            return Err(RegistryError::new(
                "bundle-index-invalid",
                "bundle.json declared the same bundled file more than once",
            )
            .with_detail(key));
        }
    }
    Ok(indexed)
}

fn validate_decoded_bundle_skill_entries(
    bundle: &InstalledSkillBundle,
    files_by_path: &BTreeMap<String, &[u8]>,
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

        let install_dir = bundle_install_dir_relative_from_str(&entry.install_dir)?;
        let install_dir_string = path_string(&install_dir)?;
        if !seen_dirs.insert(install_dir_string.clone()) {
            return Err(RegistryError::new(
                "bundle-index-invalid",
                "bundle.json declared the same install directory more than once",
            )
            .with_detail(install_dir_string));
        }

        let manifest_path = install_dir.join("manifest.json");
        let manifest = decoded_installed_manifest(&manifest_path, files_by_path)?;
        validate_installed_manifest(&manifest)?;
        let artifact_path = resolve_local_file(&install_dir, &manifest.package.artifact_uri)
            .map_err(|error| {
                RegistryError::new(
                    "artifact-uri-invalid",
                    "local registry only supports relative artifact paths",
                )
                .with_detail(error.to_string())
            })?;
        let artifact_bytes =
            decoded_file_bytes(files_by_path, &artifact_path).ok_or_else(|| {
                RegistryError::new("artifact-missing", "artifact file does not exist")
                    .with_detail(path_string(&artifact_path).expect("decoded OCI paths stay utf-8"))
            })?;
        let digest = format!("sha256:{}", sha256_bytes(artifact_bytes));
        if digest != manifest.package.artifact_digest {
            return Err(RegistryError::new(
                "artifact-digest-mismatch",
                "artifact digest does not match manifest",
            )
            .with_detail(serde_json::json!({
                "expected": manifest.package.artifact_digest,
                "actual": digest,
                "artifact_path": path_string(&artifact_path)?,
            })));
        }
        validate_decoded_support_files(&install_dir, &manifest, files_by_path)?;

        let resolved_ref = ResolvedSkillRef {
            key: manifest.key.clone(),
            version: manifest.version.clone(),
            digest: digest.clone(),
        };
        if resolved_ref != entry.resolved_ref {
            return Err(RegistryError::new(
                "bundle-entry-mismatch",
                "bundled installed manifest did not match its declared resolved skill reference",
            )
            .with_detail(serde_json::json!({
                "expected": entry.resolved_ref,
                "actual": resolved_ref,
                "install_dir": entry.install_dir,
            })));
        }

        if manifest.publisher.id != bundle.publisher.id {
            return Err(RegistryError::new(
                "bundle-publisher-mismatch",
                "bundled installed manifest publisher did not match the bundle publisher",
            )
            .with_detail(serde_json::json!({
                "bundle_publisher": bundle.publisher,
                "manifest_publisher": manifest.publisher,
                "resolved_ref": resolved_ref,
            })));
        }

        validated.push(ValidatedBundleSkill {
            entry: entry.clone(),
            install_dir: install_dir.clone(),
            installed: InstalledSkill {
                manifest,
                resolved_ref,
                manifest_path,
                artifact_path,
                root_dir: install_dir,
                verification: None,
                trust: InstalledTrustMetadata {
                    verification_state: InstalledVerificationState::LocalSource,
                    trust_tier: LocalTrustTier::LocalDev,
                },
            },
        });
    }

    Ok(validated)
}

fn decoded_installed_manifest(
    manifest_path: &Path,
    files_by_path: &BTreeMap<String, &[u8]>,
) -> Result<SkillManifest, RegistryError> {
    let Some(bytes) = decoded_file_bytes(files_by_path, manifest_path) else {
        return Err(RegistryError::new(
            "bundle-content-missing",
            "bundle.json referenced an installed skill directory that did not exist",
        )
        .with_detail(serde_json::json!({
            "path": path_string(manifest_path)?,
        })));
    };

    match serde_json::from_slice(bytes) {
        Ok(manifest) => Ok(manifest),
        Err(error) => {
            if serde_json::from_slice::<SourceSkillManifest>(bytes).is_ok() {
                Err(RegistryError::new(
                    "source-skill-not-installed",
                    "source manifests are not executable; install the skill into a local registry first",
                )
                .with_detail(path_string(manifest_path)?))
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

fn validate_decoded_support_files(
    install_dir: &Path,
    manifest: &SkillManifest,
    files_by_path: &BTreeMap<String, &[u8]>,
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
        let Some(path) = maybe_resolve_local_file(install_dir, uri).map_err(|error| {
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
        if decoded_file_bytes(files_by_path, &path).is_none() {
            return Err(RegistryError::new(
                "staged-file-missing",
                "installed manifest referenced a staged support file that did not exist",
            )
            .with_detail(path_string(&path)?));
        }
    }

    Ok(())
}

fn validate_decoded_listed_bundle_files(
    bundle: &InstalledSkillBundle,
    files_by_path: &BTreeMap<String, &[u8]>,
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
        let bytes = decoded_file_bytes(files_by_path, &relative).ok_or_else(|| {
            RegistryError::new(
                "bundle-content-missing",
                "bundle.json referenced a bundled file that did not exist",
            )
            .with_detail(serde_json::json!({
                "path": file.path,
                "file_path": path_string(&relative).expect("decoded OCI paths stay utf-8"),
            }))
        })?;

        let digest = format!("sha256:{}", sha256_bytes(bytes));
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

fn collect_decoded_bundle_files(
    validated: &[ValidatedBundleSkill],
    files_by_path: &BTreeMap<String, &[u8]>,
) -> Result<HashSet<String>, RegistryError> {
    let mut actual_files = HashSet::new();

    for skill in validated {
        for relative in files_by_path.keys() {
            let relative_path = Path::new(relative);
            if !relative_path.starts_with(&skill.install_dir) {
                continue;
            }

            if relative_path.file_name().and_then(|name| name.to_str())
                == Some(VERIFICATION_FILENAME)
            {
                return Err(RegistryError::new(
                    "bundle-content-invalid",
                    "bundled installed skill directories must not contain local verification metadata",
                )
                .with_detail(relative.clone()));
            }

            actual_files.insert(relative.clone());
        }
    }

    Ok(actual_files)
}

fn decoded_file_bytes<'a>(
    files_by_path: &'a BTreeMap<String, &'a [u8]>,
    path: &Path,
) -> Option<&'a [u8]> {
    let key = normalized_decoded_file_key(path).ok()?;
    files_by_path.get(&key).copied()
}

fn normalized_decoded_file_key(path: &Path) -> Result<String, RegistryError> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::Normal(part) => normalized.push(part),
            std::path::Component::ParentDir
            | std::path::Component::RootDir
            | std::path::Component::Prefix(_) => {
                return Err(RegistryError::new(
                    "bundle-file-path-invalid",
                    "bundle.json file paths must stay under the installed/ subtree",
                )
                .with_detail(path.display().to_string()));
            }
        }
    }

    path_string(&normalized)
}

fn write_decoded_files(
    bundle_root: &Path,
    files: &[DecodedOciFile],
    code_prefix: &str,
) -> Result<(), RegistryError> {
    for file in files {
        let relative = bundle_file_relative_from_str(&file.relative_path)?;
        let destination = bundle_root.join(relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                RegistryError::new(
                    format!("{code_prefix}-import-file-create-failed"),
                    "failed to create parent directory while reconstructing bundled files",
                )
                .with_detail(error.to_string())
            })?;
        }
        write_bytes(&destination, &file.bytes)?;
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

fn read_layout_index_bytes(layout_root: &Path) -> Result<Vec<u8>, RegistryError> {
    fs::read(layout_root.join("index.json")).map_err(|error| {
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
    })
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
    validate_layout_bytes(&bytes, descriptor)?;
    Ok(bytes)
}

fn validate_layout_bytes(bytes: &[u8], descriptor: &OciDescriptor) -> Result<(), RegistryError> {
    validate_descriptor_bytes(
        bytes,
        descriptor,
        "oci-layout-blob-size-invalid",
        "oci-layout-blob-size-mismatch",
        "oci-layout-blob-digest-mismatch",
        "OCI layout",
    )
}

fn validate_registry_bytes(bytes: &[u8], descriptor: &OciDescriptor) -> Result<(), RegistryError> {
    validate_descriptor_bytes(
        bytes,
        descriptor,
        "oci-registry-blob-size-invalid",
        "oci-registry-blob-size-mismatch",
        "oci-registry-blob-digest-mismatch",
        "OCI registry",
    )
}

fn validate_descriptor_bytes(
    bytes: &[u8],
    descriptor: &OciDescriptor,
    size_invalid_code: &str,
    size_mismatch_code: &str,
    digest_mismatch_code: &str,
    artifact_label: &str,
) -> Result<(), RegistryError> {
    let actual_size = u64::try_from(bytes.len()).map_err(|_| {
        RegistryError::new(
            size_invalid_code,
            format!("{artifact_label} blob size exceeded the supported descriptor range"),
        )
    })?;
    if actual_size != descriptor.size {
        return Err(RegistryError::new(
            size_mismatch_code,
            format!("{artifact_label} blob size did not match its descriptor"),
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
            digest_mismatch_code,
            format!("{artifact_label} blob digest did not match its descriptor"),
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
            "OCI descriptors must use sha256 digests",
        )
        .with_detail(digest.to_owned()));
    };
    if hex.len() != 64 || !hex.chars().all(|character| character.is_ascii_hexdigit()) {
        return Err(RegistryError::new(
            "oci-layout-digest-invalid",
            "OCI descriptor digests must contain 64 hexadecimal sha256 characters",
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
            "OCI root descriptor was missing Guild root annotations",
        )
    })?;
    let expected = root_descriptor_annotations(bundle);
    for (key, value) in expected {
        let actual = annotations.get(&key).ok_or_else(|| {
            RegistryError::new(
                "oci-layout-index-invalid",
                "OCI root descriptor was missing a required Guild annotation",
            )
            .with_detail(key.clone())
        })?;
        if actual != &value {
            return Err(RegistryError::new(
                "oci-layout-index-invalid",
                "OCI root descriptor annotation did not match the signed bundle root metadata",
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

fn registry_reference_string(reference: &OciRegistryReference) -> String {
    reference.to_string()
}

fn reference_with_digest(reference: &OciRegistryReference, digest: String) -> OciRegistryReference {
    OciRegistryReference {
        registry: reference.registry.clone(),
        repository: reference.repository.clone(),
        target: OciRegistryTarget::Digest(digest),
    }
}

fn parse_registry_reference(reference: &OciRegistryReference) -> Result<Reference, RegistryError> {
    reference.to_string().parse::<Reference>().map_err(|error| {
        RegistryError::new(
            "oci-registry-reference-invalid",
            "failed to parse the OCI registry reference",
        )
        .with_detail(serde_json::json!({
            "reference": reference.to_string(),
            "cause": error.to_string(),
        }))
    })
}

fn build_registry_client(
    reference: &OciRegistryReference,
    options: &OciRegistryTransportOptions,
) -> Client {
    let protocol = if options.allow_http {
        ClientProtocol::HttpsExcept(vec![reference.registry.clone()])
    } else {
        ClientProtocol::Https
    };
    Client::new(ClientConfig {
        protocol,
        ..ClientConfig::default()
    })
}

fn registry_auth(auth: &OciRegistryAuth) -> RegistryAuth {
    match auth {
        OciRegistryAuth::Anonymous => RegistryAuth::Anonymous,
        OciRegistryAuth::Basic { username, password } => {
            RegistryAuth::Basic(username.clone(), password.clone())
        }
        OciRegistryAuth::Bearer { token } => RegistryAuth::Bearer(token.clone()),
    }
}

async fn authenticate_registry(
    client: &Client,
    reference: &Reference,
    auth: &RegistryAuth,
    operation: RegistryOperation,
) -> Result<(), RegistryError> {
    client
        .auth(reference, auth, operation)
        .await
        .map_err(|error| {
            RegistryError::new(
                "oci-registry-auth-failed",
                "failed to authenticate against the OCI registry",
            )
            .with_detail(error.to_string())
        })?;
    Ok(())
}

async fn upload_registry_blob(
    client: &Client,
    reference: &Reference,
    blob: &OciBlob,
) -> Result<(), RegistryError> {
    client
        .push_blob(reference, blob.bytes.clone(), &blob.descriptor.digest)
        .await
        .map_err(|error| {
            RegistryError::new(
                "oci-registry-push-blob-failed",
                "failed to push an OCI blob to the registry",
            )
            .with_detail(serde_json::json!({
                "digest": blob.descriptor.digest,
                "cause": error.to_string(),
            }))
        })?;
    Ok(())
}

async fn fetch_registry_blobs(
    client: &Client,
    reference: &Reference,
    manifest: &OciImageManifest,
) -> Result<BTreeMap<String, Vec<u8>>, RegistryError> {
    let mut blobs = BTreeMap::new();
    let mut descriptors = Vec::with_capacity(manifest.layers.len() + 1);
    descriptors.push(manifest.config.clone());
    descriptors.extend(manifest.layers.iter().cloned());

    for descriptor in descriptors {
        let mut stream = client
            .pull_blob_stream(reference, descriptor.digest.as_str())
            .await
            .map_err(|error| {
                RegistryError::new(
                    "oci-registry-blob-fetch-failed",
                    "failed to pull an OCI blob from the registry",
                )
                .with_detail(serde_json::json!({
                    "digest": descriptor.digest,
                    "cause": error.to_string(),
                }))
            })?;
        let mut bytes = Vec::new();
        while let Some(chunk) = stream.try_next().await.map_err(|error| {
            RegistryError::new(
                "oci-registry-blob-read-failed",
                "failed to stream an OCI blob from the registry",
            )
            .with_detail(serde_json::json!({
                "digest": descriptor.digest,
                "cause": error.to_string(),
            }))
        })? {
            bytes.extend_from_slice(&chunk);
        }
        validate_registry_bytes(&bytes, &descriptor)?;
        blobs.insert(descriptor.digest.clone(), bytes);
    }

    Ok(blobs)
}

fn run_registry_future<T>(
    future: impl Future<Output = Result<T, RegistryError>> + Send + 'static,
) -> Result<T, RegistryError>
where
    T: Send + 'static,
{
    let handle = std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| {
                RegistryError::new(
                    "oci-registry-runtime-create-failed",
                    "failed to create a Tokio runtime for OCI registry transport",
                )
                .with_detail(error.to_string())
            })?;
        runtime.block_on(future)
    });

    handle.join().unwrap_or_else(|_| {
        Err(RegistryError::new(
            "oci-registry-runtime-panicked",
            "OCI registry transport worker panicked while processing the request",
        ))
    })
}
