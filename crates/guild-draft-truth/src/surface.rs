use std::collections::BTreeSet;

use anyhow::{Context, Result, bail};
use guild_runner::active_wasm_inspect_capability_surface;
use guild_types::{CapabilityId, GuildResourceScope};

use crate::util::{draft_v1_dir, json_array, read_json, read_to_string, repo_root};

pub const STATUS_SUPPORTED: &str = "supported";
pub const STATUS_BOUNDED: &str = "bounded";
pub const STATUS_NOT_PROVEN: &str = "not_proven";
pub const STATUS_UNSUPPORTED: &str = "unsupported";
pub const STATUS_PARTIAL: &str = "partial";

pub const TOKEN_LINKAGE_PROOF_BACKED: &str = "proof_backed";
pub const TOKEN_LINKAGE_UPPER_BOUND_FALLBACK: &str = "upper_bound_fallback";
pub const LINKAGE_PROOF_LINKED: &str = "proof_linked";
pub const LINKAGE_UNLINKED: &str = "unlinked";
pub const LINKAGE_NOT_MEASURED_ON_REAL_PATH: &str = "not_measured_on_real_path";
pub const LINKED_PATH_PROOF_LINKED: &str = "proof_linked";
pub const LINKED_PATH_FALLBACK_UNLINKED: &str = "fallback_unlinked";
pub const LINKED_PATH_PROOF_ONLY: &str = "proof_only";

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
            ],
            forbidden: &[],
        },
        DocTruthCheck {
            path: "docs/testing.md",
            required: &[
                "The source-of-truth declaration lives in `SPECS.md` section \"Source Of Truth\".",
                "The checked JSON and Markdown artifacts remain outputs of that Rust-native path; they do not become runtime-contract sources just because they are checked into the repo.",
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
            ],
            forbidden: &[],
        },
        DocTruthCheck {
            path: "docs/adr/0005-capability-schema-and-active-inspect-profile.md",
            required: &[
                "For the current normative runtime contract, see `SPECS.md`, `wit/guild-skill-v1.wit`, and the core Rust runtime/types.",
            ],
            forbidden: &[],
        },
        DocTruthCheck {
            path: "docs/adr/0012-capability-policy-layering-model.md",
            required: &[
                "For the current normative runtime contract, see `SPECS.md`, `wit/guild-skill-v1.wit`, and the core Rust runtime/types.",
            ],
            forbidden: &[],
        },
    ]
}
