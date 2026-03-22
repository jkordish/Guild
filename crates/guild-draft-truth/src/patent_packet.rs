use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::benchmark;
use crate::support_matrix;
use crate::surface::{
    LINKED_PATH_PROOF_LINKED, LINKED_PATH_PROOF_ONLY, STATUS_SUPPORTED, ensure_allowed_value,
    ensure_exact_string_set,
};
use crate::util::{get_required_str, json_array, json_object, read_to_string, repo_root};

const MANIFEST_PATH: &str = "docs/patent/m9-packet-manifest.json";
const PACKET_KIND: &str = "guild.m9_patent_packet";
const STATUS_MEASURED_SUPPORTED_NOW: &str = "measured_supported_now";
const STATUS_ARCHITECTURE_SUPPORTED_BUT_NOT_YET_MEASURED: &str =
    "architecture_supported_but_not_yet_measured";
const STATUS_NOT_CLAIMABLE_YET: &str = "not_claimable_yet";

#[derive(Debug, Deserialize)]
struct PatentPacketManifest {
    kind: String,
    version: String,
    packet_id: String,
    source_of_truth: SourceOfTruth,
    documents: Vec<DocumentSpec>,
    measured_frontier: MeasuredFrontier,
    claim_concepts: Vec<ClaimConcept>,
}

#[derive(Debug, Deserialize)]
struct SourceOfTruth {
    benchmark_matrix: String,
    benchmark_report: String,
    support_matrix: String,
    specs: String,
}

#[derive(Debug, Deserialize)]
struct DocumentSpec {
    id: String,
    path: String,
    required_terms: Vec<String>,
    forbidden_terms: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct MeasuredFrontier {
    supported_proof_linked_slice_ids: Vec<String>,
    supported_proof_only_slice_ids: Vec<String>,
    unsupported_or_not_proven_slice_ids: Vec<String>,
    fail_closed_wall_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ClaimConcept {
    id: String,
    status: String,
    measured_supported_slice_ids: Vec<String>,
    proof_only_slice_ids: Vec<String>,
    unsupported_or_not_proven_slice_ids: Vec<String>,
    fail_closed_wall_ids: Vec<String>,
    support_matrix_refs: Vec<SupportMatrixRef>,
    test_refs: Vec<NameRefGroup>,
    scenario_refs: Vec<NameRefGroup>,
    codepaths: Vec<String>,
    required_documents: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct SupportMatrixRef {
    family: String,
    kind: String,
    name: String,
    expected_status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct NameRefGroup {
    path: String,
    names: Vec<String>,
}

struct LoadedDocument {
    path: PathBuf,
    text: String,
}

pub fn check() -> Result<()> {
    let manifest = load_manifest()?;
    validate_manifest_shape(&manifest)?;
    validate_source_paths(&manifest)?;
    let documents = validate_documents(&manifest)?;

    let benchmark = benchmark::checked_matrix()?;
    let support = support_matrix::checked_matrix()?;

    validate_frontier(&manifest, &benchmark)?;
    validate_claim_concepts(&manifest, &benchmark, &support, &documents)?;

    println!("patent packet validates cleanly.");
    Ok(())
}

fn load_manifest() -> Result<PatentPacketManifest> {
    let text = read_to_string(&repo_root().join(MANIFEST_PATH))?;
    serde_json::from_str(&text).context("failed to parse patent packet manifest JSON")
}

fn validate_manifest_shape(manifest: &PatentPacketManifest) -> Result<()> {
    if manifest.kind != PACKET_KIND {
        bail!(
            "patent packet manifest kind drifted: expected `{PACKET_KIND}`, found `{}`",
            manifest.kind
        );
    }
    if manifest.version.trim().is_empty() {
        bail!("patent packet manifest version must not be empty");
    }
    if manifest.packet_id.trim().is_empty() {
        bail!("patent packet manifest packet_id must not be empty");
    }
    if manifest.documents.is_empty() {
        bail!("patent packet manifest must list at least one document");
    }
    if manifest.claim_concepts.is_empty() {
        bail!("patent packet manifest must list at least one claim concept");
    }
    Ok(())
}

fn validate_source_paths(manifest: &PatentPacketManifest) -> Result<()> {
    for relative_path in [
        &manifest.source_of_truth.benchmark_matrix,
        &manifest.source_of_truth.benchmark_report,
        &manifest.source_of_truth.support_matrix,
        &manifest.source_of_truth.specs,
    ] {
        let path = repo_root().join(relative_path);
        if !path.exists() {
            bail!(
                "patent packet source-of-truth path `{relative_path}` does not exist at {}",
                path.display()
            );
        }
    }
    Ok(())
}

fn validate_documents(manifest: &PatentPacketManifest) -> Result<BTreeMap<String, LoadedDocument>> {
    let mut documents = BTreeMap::new();
    for spec in &manifest.documents {
        let path = repo_root().join(&spec.path);
        if !path.exists() {
            bail!(
                "patent packet document `{}` is missing at {}",
                spec.id,
                path.display()
            );
        }
        let text = read_to_string(&path)?;
        for required_term in &spec.required_terms {
            if !contains_case_insensitive(&text, required_term) {
                bail!(
                    "patent packet document `{}` is missing required term `{required_term}`",
                    spec.id
                );
            }
        }
        for forbidden_term in &spec.forbidden_terms {
            if contains_case_insensitive(&text, forbidden_term) {
                bail!(
                    "patent packet document `{}` contains forbidden term `{forbidden_term}`",
                    spec.id
                );
            }
        }
        validate_markdown_links(&path, &text)?;
        documents.insert(spec.id.clone(), LoadedDocument { path, text });
    }
    ensure_exact_string_set(
        documents.keys().cloned(),
        manifest
            .documents
            .iter()
            .map(|document| document.id.clone()),
        "patent packet document ids",
    )?;
    Ok(documents)
}

fn validate_markdown_links(document_path: &Path, text: &str) -> Result<()> {
    for link in extract_markdown_links(text) {
        let Some(target) = resolve_local_markdown_link(document_path, &link) else {
            continue;
        };
        if !target.exists() {
            bail!(
                "patent packet markdown link `{link}` in {} does not resolve to a real file",
                document_path.display()
            );
        }
    }
    Ok(())
}

fn validate_frontier(manifest: &PatentPacketManifest, benchmark_matrix: &Value) -> Result<()> {
    let slices = json_array(
        benchmark_matrix
            .get("slices")
            .context("benchmark matrix missing slices")?,
        "benchmark_matrix.slices",
    )?;
    let mut supported_proof_linked = Vec::new();
    let mut supported_proof_only = Vec::new();
    let mut unsupported_or_not_proven = Vec::new();
    for slice in slices {
        let slice = json_object(slice, "benchmark slice")?;
        let slice_id = get_required_str(slice, "slice_id", "benchmark slice")?;
        let support_status = get_required_str(slice, "support_status", "benchmark slice")?;
        let linked_path = get_required_str(slice, "linked_path", "benchmark slice")?;
        if support_status == STATUS_SUPPORTED && linked_path == LINKED_PATH_PROOF_LINKED {
            supported_proof_linked.push(slice_id.to_owned());
        } else if support_status == STATUS_SUPPORTED && linked_path == LINKED_PATH_PROOF_ONLY {
            supported_proof_only.push(slice_id.to_owned());
        } else if support_status != STATUS_SUPPORTED {
            unsupported_or_not_proven.push(slice_id.to_owned());
        }
    }

    let walls = json_array(
        benchmark_matrix
            .get("checked_fail_closed_walls")
            .context("benchmark matrix missing checked_fail_closed_walls")?,
        "benchmark_matrix.checked_fail_closed_walls",
    )?;
    let wall_ids = walls
        .iter()
        .map(|wall| {
            let wall = json_object(wall, "benchmark fail-closed wall")?;
            Ok(get_required_str(wall, "wall_id", "benchmark fail-closed wall")?.to_owned())
        })
        .collect::<Result<Vec<_>>>()?;

    ensure_exact_string_set(
        supported_proof_linked,
        manifest
            .measured_frontier
            .supported_proof_linked_slice_ids
            .clone(),
        "patent packet supported proof-linked frontier",
    )?;
    ensure_exact_string_set(
        supported_proof_only,
        manifest
            .measured_frontier
            .supported_proof_only_slice_ids
            .clone(),
        "patent packet supported proof-only frontier",
    )?;
    ensure_exact_string_set(
        unsupported_or_not_proven,
        manifest
            .measured_frontier
            .unsupported_or_not_proven_slice_ids
            .clone(),
        "patent packet unsupported or not_proven frontier",
    )?;
    ensure_exact_string_set(
        wall_ids,
        manifest.measured_frontier.fail_closed_wall_ids.clone(),
        "patent packet fail-closed wall frontier",
    )?;
    Ok(())
}

fn validate_claim_concepts(
    manifest: &PatentPacketManifest,
    benchmark_matrix: &Value,
    support_matrix: &Value,
    documents: &BTreeMap<String, LoadedDocument>,
) -> Result<()> {
    let benchmark_slices = benchmark_slice_map(benchmark_matrix)?;
    let benchmark_walls = benchmark_wall_ids(benchmark_matrix)?;
    let support_families = json_object(
        support_matrix
            .get("families")
            .context("support matrix missing families")?,
        "family_support_matrix.families",
    )?;

    for concept in &manifest.claim_concepts {
        ensure_allowed_value(
            &concept.status,
            &[
                STATUS_MEASURED_SUPPORTED_NOW,
                STATUS_ARCHITECTURE_SUPPORTED_BUT_NOT_YET_MEASURED,
                STATUS_NOT_CLAIMABLE_YET,
            ],
            &format!("patent packet claim concept status for {}", concept.id),
        )?;

        if concept.status == STATUS_NOT_CLAIMABLE_YET
            && (!concept.measured_supported_slice_ids.is_empty()
                || !concept.proof_only_slice_ids.is_empty())
        {
            bail!(
                "not-claimable-yet claim concept `{}` cannot point at measured supported slices",
                concept.id
            );
        }

        if concept.measured_supported_slice_ids.is_empty()
            && concept.proof_only_slice_ids.is_empty()
            && concept.unsupported_or_not_proven_slice_ids.is_empty()
            && concept.fail_closed_wall_ids.is_empty()
            && concept.support_matrix_refs.is_empty()
        {
            bail!(
                "claim concept `{}` must carry at least one measured or bounded evidence reference",
                concept.id
            );
        }

        for slice_id in &concept.measured_supported_slice_ids {
            let slice = benchmark_slices.get(slice_id).with_context(|| {
                format!(
                    "claim concept `{}` referenced unknown benchmark slice `{slice_id}`",
                    concept.id
                )
            })?;
            let support_status = get_required_str(slice, "support_status", "benchmark slice")?;
            let linked_path = get_required_str(slice, "linked_path", "benchmark slice")?;
            if support_status != STATUS_SUPPORTED || linked_path != LINKED_PATH_PROOF_LINKED {
                bail!(
                    "claim concept `{}` treated `{slice_id}` as measured proof-linked support, but the benchmark records `{support_status}` and `{linked_path}`",
                    concept.id
                );
            }
        }

        for slice_id in &concept.proof_only_slice_ids {
            let slice = benchmark_slices.get(slice_id).with_context(|| {
                format!(
                    "claim concept `{}` referenced unknown proof-only benchmark slice `{slice_id}`",
                    concept.id
                )
            })?;
            let linked_path = get_required_str(slice, "linked_path", "benchmark slice")?;
            if linked_path != LINKED_PATH_PROOF_ONLY {
                bail!(
                    "claim concept `{}` treated `{slice_id}` as proof-only, but the benchmark records `{linked_path}`",
                    concept.id
                );
            }
        }

        for slice_id in &concept.unsupported_or_not_proven_slice_ids {
            let slice = benchmark_slices.get(slice_id).with_context(|| {
                format!(
                    "claim concept `{}` referenced unknown unsupported slice `{slice_id}`",
                    concept.id
                )
            })?;
            let support_status = get_required_str(slice, "support_status", "benchmark slice")?;
            if support_status == STATUS_SUPPORTED {
                bail!(
                    "claim concept `{}` treated `{slice_id}` as unsupported or not_proven, but the benchmark marks it supported",
                    concept.id
                );
            }
        }

        for wall_id in &concept.fail_closed_wall_ids {
            if !benchmark_walls.contains(wall_id) {
                bail!(
                    "claim concept `{}` referenced unknown fail-closed wall `{wall_id}`",
                    concept.id
                );
            }
        }

        for support_ref in &concept.support_matrix_refs {
            validate_support_matrix_ref(concept, support_ref, support_families)?;
        }

        for group in &concept.test_refs {
            validate_name_ref_group(
                group,
                &format!("claim concept `{}` test reference", concept.id),
            )?;
        }
        for group in &concept.scenario_refs {
            validate_name_ref_group(
                group,
                &format!("claim concept `{}` scenario reference", concept.id),
            )?;
        }
        for codepath in &concept.codepaths {
            let path = repo_root().join(codepath);
            if !path.exists() {
                bail!(
                    "claim concept `{}` referenced missing codepath `{codepath}`",
                    concept.id
                );
            }
        }
        for document_id in &concept.required_documents {
            let document = documents.get(document_id).with_context(|| {
                format!(
                    "claim concept `{}` referenced unknown packet document id `{document_id}`",
                    concept.id
                )
            })?;
            if !document.text.contains(&concept.id) {
                bail!(
                    "claim concept `{}` is not named explicitly in packet document `{document_id}` at {}",
                    concept.id,
                    document.path.display()
                );
            }
        }
    }
    Ok(())
}

fn benchmark_slice_map<'a>(
    benchmark_matrix: &'a Value,
) -> Result<BTreeMap<String, &'a Map<String, Value>>> {
    let slices = json_array(
        benchmark_matrix
            .get("slices")
            .context("benchmark matrix missing slices")?,
        "benchmark_matrix.slices",
    )?;
    let mut map = BTreeMap::new();
    for slice in slices {
        let slice = json_object(slice, "benchmark slice")?;
        let slice_id = get_required_str(slice, "slice_id", "benchmark slice")?;
        map.insert(slice_id.to_owned(), slice);
    }
    Ok(map)
}

fn benchmark_wall_ids(benchmark_matrix: &Value) -> Result<BTreeSet<String>> {
    let walls = json_array(
        benchmark_matrix
            .get("checked_fail_closed_walls")
            .context("benchmark matrix missing checked_fail_closed_walls")?,
        "benchmark_matrix.checked_fail_closed_walls",
    )?;
    walls
        .iter()
        .map(|wall| {
            let wall = json_object(wall, "benchmark fail-closed wall")?;
            Ok(get_required_str(wall, "wall_id", "benchmark fail-closed wall")?.to_owned())
        })
        .collect()
}

fn validate_support_matrix_ref(
    concept: &ClaimConcept,
    support_ref: &SupportMatrixRef,
    support_families: &Map<String, Value>,
) -> Result<()> {
    let family = json_object(
        support_families.get(&support_ref.family).with_context(|| {
            format!(
                "claim concept `{}` referenced unknown support-matrix family `{}`",
                concept.id, support_ref.family
            )
        })?,
        &format!("family_support_matrix.families.{}", support_ref.family),
    )?;

    match support_ref.kind.as_str() {
        "layer" => {
            let layers = json_object(
                family
                    .get("layers")
                    .context("support-matrix family missing layers")?,
                "support-matrix family layers",
            )?;
            let layer = json_object(
                layers.get(&support_ref.name).with_context(|| {
                    format!(
                        "claim concept `{}` referenced unknown support-matrix layer `{}.{}`",
                        concept.id, support_ref.family, support_ref.name
                    )
                })?,
                "support-matrix layer",
            )?;
            if let Some(expected_status) = &support_ref.expected_status {
                let actual_status = get_required_str(layer, "status", "support-matrix layer")?;
                if actual_status != expected_status {
                    bail!(
                        "claim concept `{}` expected support-matrix layer `{}.{}` to have status `{expected_status}`, found `{actual_status}`",
                        concept.id,
                        support_ref.family,
                        support_ref.name
                    );
                }
            }
        }
        "proven_slice" => {
            let proven_slices = json_array(
                family
                    .get("proven_slices")
                    .context("support-matrix family missing proven_slices")?,
                "support-matrix proven_slices",
            )?;
            let proven_slice = proven_slices
                .iter()
                .find_map(|slice| {
                    let slice = slice.as_object()?;
                    let slice_id = slice.get("slice_id")?.as_str()?;
                    (slice_id == support_ref.name).then_some(slice)
                })
                .with_context(|| {
                    format!(
                        "claim concept `{}` referenced unknown support-matrix proven slice `{}.{}`",
                        concept.id, support_ref.family, support_ref.name
                    )
                })?;
            if let Some(expected_status) = &support_ref.expected_status {
                let actual_status =
                    get_required_str(proven_slice, "proof_status", "support-matrix proven slice")?;
                if actual_status != expected_status {
                    bail!(
                        "claim concept `{}` expected support-matrix proven slice `{}.{}` to have proof_status `{expected_status}`, found `{actual_status}`",
                        concept.id,
                        support_ref.family,
                        support_ref.name
                    );
                }
            }
        }
        "not_proven_shape" => {
            let not_proven_shapes = json_array(
                family
                    .get("not_proven_shapes")
                    .context("support-matrix family missing not_proven_shapes")?,
                "support-matrix not_proven_shapes",
            )?;
            let found = not_proven_shapes.iter().any(|shape| {
                shape
                    .as_object()
                    .and_then(|shape| shape.get("shape_id"))
                    .and_then(Value::as_str)
                    == Some(support_ref.name.as_str())
            });
            if !found {
                bail!(
                    "claim concept `{}` referenced unknown support-matrix not_proven shape `{}.{}`",
                    concept.id,
                    support_ref.family,
                    support_ref.name
                );
            }
        }
        other => bail!(
            "claim concept `{}` used unsupported support-matrix ref kind `{other}`",
            concept.id
        ),
    }

    Ok(())
}

fn validate_name_ref_group(group: &NameRefGroup, context: &str) -> Result<()> {
    let path = repo_root().join(&group.path);
    if !path.exists() {
        bail!("{context} points to missing file `{}`", group.path);
    }
    let text = read_to_string(&path)?;
    for name in &group.names {
        if !text.contains(name) {
            bail!(
                "{context} expected to find `{name}` inside `{}`",
                group.path
            );
        }
    }
    Ok(())
}

fn contains_case_insensitive(haystack: &str, needle: &str) -> bool {
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

fn extract_markdown_links(text: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut remaining = text;
    while let Some(start) = remaining.find("](") {
        let after_marker = &remaining[start + 2..];
        let Some(end) = after_marker.find(')') else {
            break;
        };
        links.push(after_marker[..end].to_owned());
        remaining = &after_marker[end + 1..];
    }
    links
}

fn resolve_local_markdown_link(document_path: &Path, link: &str) -> Option<PathBuf> {
    if link.starts_with('#')
        || link.starts_with("http://")
        || link.starts_with("https://")
        || link.starts_with("mailto:")
    {
        return None;
    }
    let path_part = link.split('#').next().unwrap_or(link);
    if path_part.is_empty() {
        return None;
    }
    Some(
        document_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(path_part),
    )
}
