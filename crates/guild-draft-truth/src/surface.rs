use std::collections::BTreeSet;

use anyhow::{Context, Result, bail};
use guild_runner::active_wasm_inspect_capability_surface;
use guild_types::{
    CONTRACT_SURFACE_V1_CORE_EXECUTION_QUERY_PATTERNS,
    CONTRACT_SURFACE_V1_CORE_EXECUTION_QUERY_STATUS_SEGMENTS,
    CONTRACT_SURFACE_V1_CORE_HOST_MINTED_EXECUTION_FIELDS,
    CONTRACT_SURFACE_V1_CORE_NON_AUTHORITATIVE_CORRELATION_FIELDS,
    CONTRACT_SURFACE_V1_CORE_REQUESTED_SKILL_REF_FIELDS,
    CONTRACT_SURFACE_V1_CORE_RESOLVED_SKILL_REF_FIELDS, CONTRACT_SURFACE_V1_CORE_RESOURCE_ROOTS,
    CapabilityId, GuildResourceScope, LINKAGE_STATUS_NOT_MEASURED_ON_REAL_PATH,
    LINKAGE_STATUS_PROOF_LINKED, LINKAGE_STATUS_UNLINKED, NEGATIVE_CLAIM_STATUS_COVERAGE_LIMITED,
    NEGATIVE_CLAIM_STATUS_COVERAGE_LIMITED_OR_UNVERIFIABLE, NEGATIVE_CLAIM_STATUS_NOT_PROVABLE,
    NEGATIVE_CLAIM_STATUS_UNVERIFIABLE, PRESENTATION_STATUS_LINKED,
    PRESENTATION_STATUS_PROOF_BACKED, PRESENTATION_STATUS_REFUSED, PRESENTATION_STATUS_UNLINKED,
    PRESENTATION_STATUS_UPPER_BOUND, SUPPORT_STATUS_BOUNDED, SUPPORT_STATUS_NOT_PROVEN,
    SUPPORT_STATUS_PARTIAL, SUPPORT_STATUS_SUPPORTED, SUPPORT_STATUS_UNSUPPORTED,
    TOKEN_LINKAGE_STATUS_PROOF_BACKED, TOKEN_LINKAGE_STATUS_UPPER_BOUND_FALLBACK,
};

use crate::util::{draft_v1_dir, json_array, read_json, read_to_string, repo_root};

pub use guild_types::{
    LINKAGE_STATUS_NOT_MEASURED_ON_REAL_PATH as LINKAGE_NOT_MEASURED_ON_REAL_PATH,
    LINKAGE_STATUS_PROOF_LINKED as LINKAGE_PROOF_LINKED,
    LINKAGE_STATUS_UNLINKED as LINKAGE_UNLINKED, LINKED_PATH_FALLBACK_UNLINKED,
    LINKED_PATH_PROOF_LINKED, LINKED_PATH_PROOF_ONLY, SUPPORT_STATUS_BOUNDED as STATUS_BOUNDED,
    SUPPORT_STATUS_NOT_PROVEN as STATUS_NOT_PROVEN, SUPPORT_STATUS_PARTIAL as STATUS_PARTIAL,
    SUPPORT_STATUS_SUPPORTED as STATUS_SUPPORTED, SUPPORT_STATUS_UNSUPPORTED as STATUS_UNSUPPORTED,
    TOKEN_LINKAGE_STATUS_PROOF_BACKED as TOKEN_LINKAGE_PROOF_BACKED,
    TOKEN_LINKAGE_STATUS_UPPER_BOUND_FALLBACK as TOKEN_LINKAGE_UPPER_BOUND_FALLBACK,
};

pub fn active_runtime_families() -> Vec<CapabilityId> {
    let mut families = Vec::new();
    for (capability_id, _) in active_wasm_inspect_capability_surface() {
        if !families.iter().any(|existing| existing == capability_id) {
            families.push(capability_id.clone());
        }
    }
    families
}

pub fn active_runtime_family_names() -> Vec<String> {
    active_runtime_families()
        .into_iter()
        .map(|family| family.as_str().to_owned())
        .collect()
}

pub fn immutable_read_resource_live_proof_roots() -> Vec<String> {
    vec![
        GuildResourceScope::Execution.canonical_prefix().to_owned(),
        GuildResourceScope::ObjectRecord
            .canonical_prefix()
            .to_owned(),
    ]
}

pub fn ensure_allowed_value(value: &str, allowed: &[&str], context: &str) -> Result<()> {
    if allowed.contains(&value) {
        return Ok(());
    }
    bail!(
        "{context} used unsupported vocabulary value `{value}`; allowed values are: {}",
        allowed.join(", ")
    )
}

pub fn ensure_exact_string_set(
    actual: impl IntoIterator<Item = String>,
    expected: impl IntoIterator<Item = String>,
    context: &str,
) -> Result<()> {
    let actual = actual.into_iter().collect::<BTreeSet<_>>();
    let expected = expected.into_iter().collect::<BTreeSet<_>>();
    if actual == expected {
        return Ok(());
    }
    bail!(
        "{context} drifted from the canonical set; expected {:?}, found {:?}",
        expected,
        actual
    )
}

pub fn verify_active_runtime_example_alignment() -> Result<()> {
    let runtime = read_json(&draft_v1_dir().join("examples/wasmtime-strict.runtime.json"))?;
    let actual = json_array(
        runtime
            .get("supported_canonical_families")
            .context("wasmtime-strict runtime example missing supported_canonical_families")?,
        "wasmtime-strict.runtime.supported_canonical_families",
    )?
    .iter()
    .map(|value| {
        value
            .as_str()
            .map(str::to_owned)
            .context("supported_canonical_families entries must be strings")
    })
    .collect::<Result<Vec<_>>>()?;
    ensure_exact_string_set(
        actual,
        active_runtime_family_names(),
        "docs/schemas/draft-v1/examples/wasmtime-strict.runtime.json supported_canonical_families",
    )
}

pub fn verify_removed_truth_entrypoints() -> Result<()> {
    for relative_path in [
        "docs/schemas/draft-v1/validate_examples.py",
        "docs/schemas/draft-v1/compatibility_check.py",
        "docs/schemas/draft-v1/benchmark_real_path.py",
    ] {
        if repo_root().join(relative_path).exists() {
            bail!(
                "{relative_path} should not exist; the Rust-native truth path must remain the only repo-truth implementation"
            );
        }
    }
    Ok(())
}

pub fn verify_doc_truth_markers() -> Result<()> {
    for check in doc_truth_checks() {
        let text = read_to_string(&repo_root().join(check.path))?;
        for required in check.required {
            if !text.contains(required) {
                bail!(
                    "{} is missing required truth-surface marker `{required}`",
                    check.path
                );
            }
        }
        for forbidden in check.forbidden {
            if text.contains(forbidden) {
                bail!(
                    "{} still contains stale truth-surface marker `{forbidden}`",
                    check.path
                );
            }
        }
    }
    Ok(())
}

pub fn verify_contract_surface_v1_spec_markers() -> Result<()> {
    let specs = read_to_string(&repo_root().join("SPECS.md"))?;
    verify_marker_block(
        &specs,
        "contract-surface-v1-core:uri-roots",
        &expected_uri_roots_block(),
    )?;
    verify_marker_block(
        &specs,
        "contract-surface-v1-core:families",
        &expected_family_block(),
    )?;
    verify_marker_block(
        &specs,
        "contract-surface-v1-core:status-vocabulary",
        &expected_status_vocabulary_block(),
    )?;
    verify_marker_block(
        &specs,
        "contract-surface-v1-core:identity",
        &expected_identity_block(),
    )
}

struct DocTruthCheck {
    path: &'static str,
    required: &'static [&'static str],
    forbidden: &'static [&'static str],
}

fn doc_truth_checks() -> &'static [DocTruthCheck] {
    &[
        DocTruthCheck {
            path: "README.md",
            required: &[
                "Normative runtime sources live in `SPECS.md` section \"Source Of Truth\", `wit/guild-skill-v1.wit`, and the core Rust runtime/types.",
                "Generated support, compatibility, and benchmark artifacts remain checked outputs, not primary contract definitions.",
                "For the frozen core runtime-contract surfaces in this milestone, see `SPECS.md` section \"Contract Surface v1 (core)\".",
            ],
            forbidden: &[],
        },
        DocTruthCheck {
            path: "docs/testing.md",
            required: &[
                "The source-of-truth declaration lives in `SPECS.md` section \"Source Of Truth\".",
                "The checked JSON and Markdown artifacts remain outputs of that Rust-native path; they do not become runtime-contract sources just because they are checked into the repo.",
                "For the frozen runtime-contract surfaces in this milestone, use `SPECS.md` section \"Contract Surface v1 (core)\" rather than treating this testing guide as a parallel source.",
            ],
            forbidden: &[],
        },
        DocTruthCheck {
            path: "docs/schemas/draft-v1/README.md",
            required: &[
                "This bundle is normative only for the draft proof/control-plane harness under `docs/schemas/draft-v1/`.",
                "For runtime-contract truth, use `SPECS.md` section \"Source Of Truth\", `wit/guild-skill-v1.wit`, and the core Rust runtime/types.",
            ],
            forbidden: &[],
        },
        DocTruthCheck {
            path: "docs/command-language.md",
            required: &[
                "This document is the source of truth for Guild's public command and URI grammar only.",
                "It is not the runtime-contract source of truth; see `SPECS.md` section \"Source Of Truth\".",
                "For the frozen runtime URI roots and support vocabulary in this milestone, see `SPECS.md` section \"Contract Surface v1 (core)\".",
            ],
            forbidden: &[],
        },
        DocTruthCheck {
            path: "docs/contracts.md",
            required: &["This file is a compatibility wrapper for older links."],
            forbidden: &[
                "Current contract highlights worth knowing before you follow older notes:",
            ],
        },
        DocTruthCheck {
            path: "docs/architecture.md",
            required: &["This file is a compatibility wrapper for older links."],
            forbidden: &[
                "Current architecture highlights worth knowing before you follow older notes:",
            ],
        },
        DocTruthCheck {
            path: "docs/adr/README.md",
            required: &[
                "ADRs record rationale and accepted decisions. They are not the current normative contract surface.",
                "For the frozen core runtime-contract surfaces in this milestone, see `SPECS.md` section \"Contract Surface v1 (core)\".",
            ],
            forbidden: &[],
        },
        DocTruthCheck {
            path: "docs/adr/0002-skill-output-and-execution-record.md",
            required: &[
                "For the current normative runtime contract, see `SPECS.md`, `wit/guild-skill-v1.wit`, and the core Rust runtime/types.",
            ],
            forbidden: &[],
        },
        DocTruthCheck {
            path: "docs/adr/0003-guest-abi-vs-host-record-boundary.md",
            required: &[
                "For the current normative runtime contract, see `SPECS.md`, `wit/guild-skill-v1.wit`, and the core Rust runtime/types.",
                "For the frozen core runtime-contract surfaces in this milestone, see `SPECS.md` section \"Contract Surface v1 (core)\".",
            ],
            forbidden: &[],
        },
        DocTruthCheck {
            path: "docs/adr/0005-capability-schema-and-active-inspect-profile.md",
            required: &[
                "For the current normative runtime contract, see `SPECS.md`, `wit/guild-skill-v1.wit`, and the core Rust runtime/types.",
                "The frozen active-family registry now lives in `SPECS.md` section \"Contract Surface v1 (core)\".",
            ],
            forbidden: &[],
        },
        DocTruthCheck {
            path: "docs/adr/0011-bounded-artifact-query-resources.md",
            required: &[
                "For the frozen runtime URI and query-resource contract in this milestone, see `SPECS.md` section \"Contract Surface v1 (core)\".",
            ],
            forbidden: &[],
        },
        DocTruthCheck {
            path: "docs/adr/0012-capability-policy-layering-model.md",
            required: &[
                "For the current normative runtime contract, see `SPECS.md`, `wit/guild-skill-v1.wit`, and the core Rust runtime/types.",
                "The frozen active-family registry now lives in `SPECS.md` section \"Contract Surface v1 (core)\".",
            ],
            forbidden: &[],
        },
        DocTruthCheck {
            path: "docs/adr/0013-read-resource-policy-family.md",
            required: &[
                "For the frozen runtime URI and scope-root contract in this milestone, see `SPECS.md` section \"Contract Surface v1 (core)\".",
            ],
            forbidden: &[],
        },
        DocTruthCheck {
            path: "docs/adr/0019-thin-guild-cli.md",
            required: &[
                "The public CLI remains an operator surface, not a separate normative runtime-contract source.",
            ],
            forbidden: &[],
        },
    ]
}

fn verify_marker_block(specs: &str, marker: &str, expected: &str) -> Result<()> {
    let start_marker = format!("<!-- {marker}:start -->");
    let end_marker = format!("<!-- {marker}:end -->");
    let (_, after_start) = specs
        .split_once(&start_marker)
        .with_context(|| format!("SPECS.md is missing marker `{start_marker}`"))?;
    let (actual, _) = after_start
        .split_once(&end_marker)
        .with_context(|| format!("SPECS.md is missing marker `{end_marker}`"))?;
    if actual.trim_matches('\n') == expected.trim_matches('\n') {
        return Ok(());
    }
    bail!("SPECS.md marker block `{marker}` drifted from the canonical contract-surface truth")
}

fn expected_uri_roots_block() -> String {
    let mut lines = Vec::new();
    lines.push("Canonical runtime resource roots:".to_owned());
    lines.extend(
        CONTRACT_SURFACE_V1_CORE_RESOURCE_ROOTS
            .into_iter()
            .map(|root| format!("- `{root}`")),
    );
    lines.push(String::new());
    lines.push("Accepted execution-query forms:".to_owned());
    for (index, pattern) in CONTRACT_SURFACE_V1_CORE_EXECUTION_QUERY_PATTERNS
        .into_iter()
        .enumerate()
    {
        if index == 2 {
            let statuses = CONTRACT_SURFACE_V1_CORE_EXECUTION_QUERY_STATUS_SEGMENTS
                .into_iter()
                .map(|status| format!("`{status}`"))
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!(
                "- `{pattern}` where `{{status}}` is one of {statuses}"
            ));
        } else {
            lines.push(format!("- `{pattern}`"));
        }
    }
    lines.join("\n")
}

fn expected_family_block() -> String {
    let mut lines = vec!["Frozen active live runtime families:".to_owned()];
    lines.extend(
        active_runtime_family_names()
            .into_iter()
            .map(|family| format!("- `{family}`")),
    );
    lines.join("\n")
}

fn expected_status_vocabulary_block() -> String {
    [
        "Frozen support status spellings:",
        &format!("- `{SUPPORT_STATUS_SUPPORTED}`"),
        &format!("- `{SUPPORT_STATUS_BOUNDED}`"),
        &format!("- `{SUPPORT_STATUS_PARTIAL}`"),
        &format!("- `{SUPPORT_STATUS_UNSUPPORTED}`"),
        &format!("- `{SUPPORT_STATUS_NOT_PROVEN}`"),
        "",
        "Frozen linkage and presentation spellings:",
        &format!(
            "- `{TOKEN_LINKAGE_STATUS_PROOF_BACKED}` -> CLI `{PRESENTATION_STATUS_PROOF_BACKED}`"
        ),
        &format!(
            "- `{TOKEN_LINKAGE_STATUS_UPPER_BOUND_FALLBACK}` -> CLI `{PRESENTATION_STATUS_UPPER_BOUND}`"
        ),
        &format!("- `{LINKAGE_STATUS_PROOF_LINKED}` -> CLI `{PRESENTATION_STATUS_LINKED}`"),
        &format!("- `{LINKAGE_STATUS_UNLINKED}` -> CLI `{PRESENTATION_STATUS_UNLINKED}`"),
        &format!("- `{PRESENTATION_STATUS_REFUSED}` -> CLI `{PRESENTATION_STATUS_REFUSED}`"),
        "",
        "Explicit checked-output residual terms:",
        &format!("- `{LINKAGE_STATUS_NOT_MEASURED_ON_REAL_PATH}`"),
        &format!("- `{LINKED_PATH_FALLBACK_UNLINKED}`"),
        &format!("- `{LINKED_PATH_PROOF_ONLY}`"),
        &format!("- `{NEGATIVE_CLAIM_STATUS_COVERAGE_LIMITED}`"),
        &format!("- `{NEGATIVE_CLAIM_STATUS_UNVERIFIABLE}`"),
        &format!("- `{NEGATIVE_CLAIM_STATUS_NOT_PROVABLE}`"),
        &format!("- `{NEGATIVE_CLAIM_STATUS_COVERAGE_LIMITED_OR_UNVERIFIABLE}`"),
    ]
    .join("\n")
}

fn expected_identity_block() -> String {
    [
        "Frozen executable identity terms:",
        &format!(
            "- requested identity: `RequestedSkillRef` fields `{}`",
            CONTRACT_SURFACE_V1_CORE_REQUESTED_SKILL_REF_FIELDS.join("`, `")
        ),
        &format!(
            "- resolved identity: `ResolvedSkillRef` fields `{}`",
            CONTRACT_SURFACE_V1_CORE_RESOLVED_SKILL_REF_FIELDS.join("`, `")
        ),
        &format!(
            "- host-minted durable execution identity field: `{}`",
            CONTRACT_SURFACE_V1_CORE_HOST_MINTED_EXECUTION_FIELDS.join("`, `")
        ),
        &format!(
            "- non-authoritative caller correlation fields: `{}`",
            CONTRACT_SURFACE_V1_CORE_NON_AUTHORITATIVE_CORRELATION_FIELDS.join("`, `")
        ),
    ]
    .join("\n")
}
