use anyhow::{Context, Result, bail};
use guild_types::CapabilityId;
use serde_json::{Map, Value, json};

use crate::ArtifactMode;
use crate::surface::{
    STATUS_BOUNDED, STATUS_NOT_PROVEN, STATUS_PARTIAL, STATUS_SUPPORTED, STATUS_UNSUPPORTED,
    active_runtime_family_names, ensure_allowed_value, ensure_exact_string_set,
    immutable_read_resource_live_proof_roots,
};
use crate::util::{
    draft_v1_dir, ensure_parent_dir, json_array, json_object, read_json, write_json_pretty,
};

const OUTPUT_NAME: &str = "family_support_matrix.json";

pub fn run(mode: ArtifactMode) -> Result<()> {
    match mode {
        ArtifactMode::Check => {
            checked_matrix()?;
            println!("{OUTPUT_NAME} validates cleanly.");
            Ok(())
        }
        ArtifactMode::Write => {
            let path = draft_v1_dir().join(OUTPUT_NAME);
            let generated = build_matrix()?;
            ensure_parent_dir(&path)?;
            write_json_pretty(&path, &generated)?;
            println!("Wrote {}", path.display());
            Ok(())
        }
    }
}

pub fn checked_matrix() -> Result<Value> {
    let path = draft_v1_dir().join(OUTPUT_NAME);
    let existing = read_json(&path)?;
    let generated = build_matrix()?;
    if existing != generated {
        bail!("{OUTPUT_NAME} is out of date with the Rust-native generator");
    }
    Ok(existing)
}

pub fn build_matrix() -> Result<Value> {
    let mut families = Map::new();
    families.insert(
        CapabilityId::HttpRequest.as_str().to_owned(),
        http_request_family(),
    );
    families.insert(
        CapabilityId::ReadResource.as_str().to_owned(),
        read_resource_family(),
    );
    families.insert(
        CapabilityId::InvokeSkill.as_str().to_owned(),
        invoke_skill_family(),
    );
    families.insert(
        CapabilityId::EmitEvidence.as_str().to_owned(),
        emit_evidence_family(),
    );
    families.insert(
        CapabilityId::LogWrite.as_str().to_owned(),
        log_write_family(),
    );

    let matrix = json!({
        "kind": "guild.family_support_matrix",
        "version": "1.0.0",
        "canonical_runtime_vocabulary": true,
        "notes": [
            "The live Rust runtime family vocabulary remains canonical. Draft-v1 statuses here describe only the bounded control-plane and witness bundle under docs/schemas/draft-v1/.",
            "The slice-aware measured source for M8-proper is docs/schemas/draft-v1/benchmark_matrix.json with the paired human report at docs/benchmarking/m8-real-path-benchmark.md. That benchmark keeps supported slices, fallback or refusal paths, and fail-closed walls separate instead of averaging them together.",
            "A family is marked live-proof-supported only where the repository now has a real Rust live proof path with counterfactual execution, a deterministic comparator, conservative search semantics, and proof output that matches the explored search.",
            "A status of bounded means the live proof search is intentionally narrower than general minimality. M8c proves bounded live read-resource shrinking across immutable Guild execution/object-record roots, bounded fixture-backed http-request shrinking only for eight deterministic slices, and two bounded invoke-skill slices: one for exactly one declared alias resolved through the installed dependency snapshot to one exact zero-authority child on guild-skill-inspect-v1 with zero nested child executions, and one for that same declared alias exercised exactly twice in deterministic order under the same zero-authority inspect-only boundary.",
            "M6 token issuance and verification remain a draft-local token layer even when the issuance basis comes from a live-runtime proof. That is not runtime-general enforcement.",
            "M7 witness linkage to proofs is honest and fail-closed: read-resource live linkage is bounded, http-request live linkage is bounded only for the replay-fixtured loopback IP GET and HEAD slices with either an explicit port or the implicit default HTTP port plus the replay-fixtured localhost GET and HEAD slices with either an explicit port or the implicit default HTTP port and deterministic loopback-only resolution bindings, invoke-skill live linkage is bounded only for the exact single-child zero-authority inspect slice and the exact two-child same-alias zero-authority inspect slice, emit-evidence stays unlinked because the tested exact single-emission local-object-store replay still fails closed and the current authority model is too coarse to smuggle sink or payload specifics, and log-write linkage remains a generic draft-layer capability rather than a checked M8-proper real-path benchmarked slice."
        ],
        "layers": [
            "admission_runtime_guarantee_matching",
            "execution_plan_representation",
            "live_minimization_proof",
            "token_issuance_basis",
            "token_verification",
            "witness_generation",
            "witness_verification",
            "positive_claim_verification",
            "negative_claim_verification",
            "plan_proof_token_linkage",
            "proof_witness_linkage"
        ],
        "families": families,
        "compatibility_aliases": {
            "net.connect": {
                "status": STATUS_PARTIAL,
                "reason_codes": ["COMPAT_ALIAS_USED", "COMPAT_ALIAS_DEPRECATED", "VOCABULARY_MAPPING_NARROWING"],
                "maps_to": CapabilityId::HttpRequest.as_str(),
                "notes": "Legacy net.connect stays as an explicit narrowing-only compatibility alias for safe HTTP(S) GET or HEAD scopes."
            },
            "component.invoke": {
                "status": STATUS_PARTIAL,
                "reason_codes": ["COMPAT_ALIAS_USED", "COMPAT_ALIAS_DEPRECATED", "VOCABULARY_MAPPING_NARROWING"],
                "maps_to": CapabilityId::InvokeSkill.as_str(),
                "notes": "Legacy component.invoke stays as an explicit narrowing-only compatibility alias to alias-scoped invoke-skill."
            },
            "net.resolve": {
                "status": STATUS_UNSUPPORTED,
                "reason_codes": ["VOCABULARY_MAPPING_UNSUPPORTED", "CANONICAL_FAMILY_UNSUPPORTED"],
                "maps_to": null,
                "notes": "The live runtime does not expose a standalone DNS-resolution family in the active inspect slice."
            }
        }
    });
    validate_generated_matrix(&matrix)?;
    Ok(matrix)
}

fn validate_generated_matrix(matrix: &Value) -> Result<()> {
    let matrix = json_object(matrix, "family_support_matrix")?;
    let families = json_object(
        matrix
            .get("families")
            .context("family_support_matrix missing families")?,
        "family_support_matrix.families",
    )?;
    ensure_exact_string_set(
        families.keys().cloned(),
        active_runtime_family_names(),
        "family_support_matrix families",
    )?;

    for (family_name, family) in families {
        let family = json_object(
            family,
            &format!("family_support_matrix.families.{family_name}"),
        )?;
        let layers = json_object(
            family
                .get("layers")
                .context("family entry missing layers")?,
            &format!("family_support_matrix.families.{family_name}.layers"),
        )?;
        for (layer_name, layer_value) in layers {
            let layer = json_object(
                layer_value,
                &format!("family_support_matrix.families.{family_name}.layers.{layer_name}"),
            )?;
            let status = layer
                .get("status")
                .and_then(Value::as_str)
                .with_context(|| {
                    format!(
                        "family_support_matrix.families.{family_name}.layers.{layer_name}.status missing"
                    )
                })?;
            ensure_allowed_value(
                status,
                &[
                    STATUS_SUPPORTED,
                    STATUS_BOUNDED,
                    STATUS_NOT_PROVEN,
                    STATUS_UNSUPPORTED,
                ],
                &format!("family_support_matrix layer status for {family_name}.{layer_name}"),
            )?;
        }

        if family_name == CapabilityId::ReadResource.as_str() {
            validate_read_resource_roots(family)?;
        }
    }

    let aliases = json_object(
        matrix
            .get("compatibility_aliases")
            .context("family_support_matrix missing compatibility_aliases")?,
        "family_support_matrix.compatibility_aliases",
    )?;
    for (alias_name, alias) in aliases {
        let alias = json_object(
            alias,
            &format!("family_support_matrix.compatibility_aliases.{alias_name}"),
        )?;
        let status = alias
            .get("status")
            .and_then(Value::as_str)
            .with_context(|| {
                format!("family_support_matrix.compatibility_aliases.{alias_name}.status missing")
            })?;
        ensure_allowed_value(
            status,
            &[STATUS_PARTIAL, STATUS_UNSUPPORTED],
            &format!("compatibility alias status for {alias_name}"),
        )?;
    }
    Ok(())
}

fn validate_read_resource_roots(family: &Map<String, Value>) -> Result<()> {
    let proven_slices = json_array(
        family
            .get("proven_slices")
            .context("read-resource family missing proven_slices")?,
        "family_support_matrix.families.read-resource.proven_slices",
    )?;
    let slice = proven_slices
        .first()
        .context("read-resource family missing immutable proven slice")?;
    let slice = json_object(
        slice,
        "family_support_matrix.families.read-resource.proven_slices[0]",
    )?;
    let request_shape = json_object(
        slice
            .get("request_shape")
            .context("read-resource immutable proven slice missing request_shape")?,
        "family_support_matrix.families.read-resource.proven_slices[0].request_shape",
    )?;
    let roots = json_array(
        request_shape
            .get("uri_prefixes")
            .context("read-resource immutable proven slice missing uri_prefixes")?,
        "family_support_matrix.families.read-resource.proven_slices[0].request_shape.uri_prefixes",
    )?
    .iter()
    .map(|value| {
        value
            .as_str()
            .map(str::to_owned)
            .context("read-resource immutable proven uri_prefixes must be strings")
    })
    .collect::<Result<Vec<_>>>()?;
    ensure_exact_string_set(
        roots,
        immutable_read_resource_live_proof_roots(),
        "family_support_matrix read-resource immutable live-proof roots",
    )
}

fn http_request_family() -> Value {
    json!({
        "scope_shape": {
            "kind": "network",
            "fields": [
                "allowed_schemes",
                "allowed_hosts",
                "allowed_host_suffixes",
                "allowed_ports",
                "allowed_methods",
                "allowed_path_prefixes",
                "max_timeout_ms",
                "max_response_bytes",
                "follow_redirects",
                "max_redirects",
                "allow_loopback",
                "allow_link_local",
                "allow_private_networks",
                "allow_ip_literals"
            ]
        },
        "proven_slices": [
            http_slice("loopback-ip-get-explicit-port", "GET", "loopback-ip-literal", "explicit", None),
            http_slice("loopback-ip-get-default-port", "GET", "loopback-ip-literal", "implicit_default_http_port", None),
            http_slice("loopback-ip-head-explicit-port", "HEAD", "loopback-ip-literal", "explicit", None),
            http_slice("loopback-ip-head-default-port", "HEAD", "loopback-ip-literal", "implicit_default_http_port", None),
            http_slice("localhost-get-explicit-port", "GET", "loopback-hostname-localhost", "explicit", Some("required")),
            http_slice("localhost-get-default-port", "GET", "loopback-hostname-localhost", "implicit_default_http_port", Some("required")),
            http_slice("localhost-head-explicit-port", "HEAD", "loopback-hostname-localhost", "explicit", Some("required")),
            http_slice("localhost-head-default-port", "HEAD", "loopback-hostname-localhost", "implicit_default_http_port", Some("required")),
        ],
        "not_proven_shapes": [
            {
                "shape_id": "hostname-forms-beyond-localhost-get-or-head",
                "reason_codes": [
                    "HTTP_HOST_UNSUPPORTED_FOR_LIVE_PROOF",
                    "HTTP_HOST_RESOLUTION_BINDING_UNAVAILABLE",
                    "HTTP_HOST_RESOLUTION_BINDING_UNSAFE"
                ],
                "notes": "Only exact localhost GET and HEAD with either an explicit port or the implicit default HTTP port are proven among hostname forms, always with deterministic loopback-only resolution bindings. All other hostname forms remain outside the honest replay-backed proof slice."
            },
            {
                "shape_id": "query-or-fragment",
                "reason_codes": ["HTTP_QUERY_UNSUPPORTED"],
                "notes": "Query and fragment components remain not_proven because the bounded live proof slice does not replay or compare them conservatively."
            },
            {
                "shape_id": "redirect-driven-execution",
                "reason_codes": ["HTTP_REDIRECTS_UNSUPPORTED"],
                "notes": "Redirect responses and redirect-following executions remain outside the honest live proof slice."
            },
            {
                "shape_id": "multiple-exercised-requests",
                "reason_codes": ["HTTP_MULTI_REQUEST_UNSUPPORTED"],
                "notes": "Baseline executions with more than one exercised HTTP request remain not_proven."
            },
            {
                "shape_id": "https-loopback",
                "reason_codes": ["HTTP_SCHEME_UNSUPPORTED_FOR_LIVE_PROOF"],
                "notes": "HTTPS remains outside the current replay-backed live proof slice."
            }
        ],
        "layers": {
            "admission_runtime_guarantee_matching": layer(STATUS_SUPPORTED, &["CANONICAL_FAMILY_SUPPORTED"], "Direct canonical http-request grants match the live runtime vocabulary and URL-granularity runtime guarantee checks."),
            "execution_plan_representation": layer(STATUS_SUPPORTED, &["CANONICAL_FAMILY_SUPPORTED"], "M4 plans carry canonical http-request scopes directly."),
            "live_minimization_proof": layer(STATUS_BOUNDED, &["HTTP_LIVE_PROOF_BOUNDED", "LIVE_PROOF_BOUNDED", "LIVE_PROOF_SUPPORTED"], "M8c now has bounded live proof for exactly eight deterministic replay-fixtured http-request slices: exercised GET and HEAD over http to a loopback IP-literal host, each with an explicit port form and an implicit default HTTP port form, plus localhost GET and HEAD with either an explicit port or the implicit default HTTP port when the proof basis carries deterministic loopback-only resolution bindings. All eight remain exact-path, query-free, redirect-free, single-request slices under the normalized inspect-output comparator only. Broader http-request shapes stay not_proven."),
            "token_issuance_basis": layer(STATUS_SUPPORTED, &["CANONICAL_FAMILY_SUPPORTED", "TOKEN_PROOF_BASIS_LIVE"], "M6 still issues direct canonical http-request scopes, and it can now consume the bounded live proof for the replay-fixtured loopback IP and localhost slices. Unsupported http-request shapes still fall back to explicit upper-bound issuance or refusal."),
            "token_verification": layer(STATUS_SUPPORTED, &["CANONICAL_FAMILY_SUPPORTED"], "Canonical METHOD:scheme://host:port/path resource bindings verify directly against http-request scopes."),
            "witness_generation": layer(STATUS_SUPPORTED, &["CANONICAL_FAMILY_SUPPORTED"], "Live exercised and blocked http-request observations still normalize directly into canonical witness effects."),
            "witness_verification": layer(STATUS_SUPPORTED, &["CANONICAL_FAMILY_SUPPORTED"], "Canonical http-request witnesses verify directly against plans and tokens."),
            "positive_claim_verification": layer(STATUS_UNSUPPORTED, &["POSITIVE_CLAIM_VOCABULARY_UNAVAILABLE"], "Draft-v1 still does not carry a fixed positive observed-fact claim vocabulary for canonical http-request facts."),
            "negative_claim_verification": layer(STATUS_SUPPORTED, &["CANONICAL_FAMILY_SUPPORTED"], "Scope-only negative claims stay supported when coverage is complete."),
            "plan_proof_token_linkage": layer(STATUS_BOUNDED, &["HTTP_LIVE_PROOF_BOUNDED", "TOKEN_PROOF_BASIS_LIVE"], "Plan -> proof -> token linkage is bounded to the eight replay-backed live proof slices only."),
            "proof_witness_linkage": layer(STATUS_BOUNDED, &["HTTP_LIVE_PROOF_BOUNDED", "LIVE_PROOF_LINKED_WITNESS"], "Proof -> witness linkage is bounded to the eight replay-backed live proof slices only.")
        }
    })
}

fn read_resource_family() -> Value {
    let immutable_roots = immutable_read_resource_live_proof_roots();
    json!({
        "scope_shape": {
            "kind": "resource",
            "fields": ["uri_prefixes", "resource_kinds"]
        },
        "proven_slices": [
            {
                "slice_id": "immutable-guild-execution-object-record-roots",
                "proof_status": "bounded_minimal",
                "proof_backed_layers": ["live_minimization_proof", "plan_proof_token_linkage", "proof_witness_linkage"],
                "request_shape": {
                    "uri_prefixes": immutable_roots,
                    "resource_kinds": ["execution", "object"]
                },
                "notes": "Bounded live proof exists over immutable Guild execution and object-record roots only."
            }
        ],
        "not_proven_shapes": [
            {
                "shape_id": "execution-query-shrink",
                "reason_codes": ["LIVE_SCOPE_SHRINK_UNSUPPORTED", "READ_RESOURCE_QUERY_UNSUPPORTED"],
                "notes": "Execution-query resources remain outside the honest bounded live shrink envelope."
            }
        ],
        "layers": {
            "admission_runtime_guarantee_matching": layer(STATUS_SUPPORTED, &["CANONICAL_FAMILY_SUPPORTED"], "Direct canonical read-resource grants match the live runtime vocabulary."),
            "execution_plan_representation": layer(STATUS_SUPPORTED, &["CANONICAL_FAMILY_SUPPORTED"], "M4 plans carry canonical read-resource scopes directly."),
            "live_minimization_proof": layer(STATUS_BOUNDED, &["LIVE_PROOF_BOUNDED", "LIVE_PROOF_SUPPORTED"], "Read-resource live proof is bounded to immutable execution/object-record roots."),
            "token_issuance_basis": layer(STATUS_SUPPORTED, &["CANONICAL_FAMILY_SUPPORTED", "TOKEN_PROOF_BASIS_LIVE"], "Proof-backed M6 issuance can consume the bounded read-resource proof."),
            "token_verification": layer(STATUS_SUPPORTED, &["CANONICAL_FAMILY_SUPPORTED"], "Canonical Guild resource bindings verify directly."),
            "witness_generation": layer(STATUS_SUPPORTED, &["CANONICAL_FAMILY_SUPPORTED"], "Live read-resource observations normalize directly into canonical witness effects."),
            "witness_verification": layer(STATUS_SUPPORTED, &["CANONICAL_FAMILY_SUPPORTED"], "Canonical read-resource witnesses verify directly."),
            "positive_claim_verification": layer(STATUS_UNSUPPORTED, &["POSITIVE_CLAIM_VOCABULARY_UNAVAILABLE"], "Positive observed-fact claim vocabulary is still unavailable."),
            "negative_claim_verification": layer(STATUS_SUPPORTED, &["CANONICAL_FAMILY_SUPPORTED"], "Scope-only negative claims stay supported when coverage is complete."),
            "plan_proof_token_linkage": layer(STATUS_BOUNDED, &["TOKEN_PROOF_BASIS_LIVE"], "Plan -> proof -> token linkage is bounded to immutable execution/object-record roots."),
            "proof_witness_linkage": layer(STATUS_BOUNDED, &["LIVE_PROOF_LINKED_WITNESS"], "Proof -> witness linkage is bounded to immutable execution/object-record roots.")
        }
    })
}

fn invoke_skill_family() -> Value {
    json!({
        "scope_shape": {
            "kind": "skill",
            "fields": ["aliases"]
        },
        "proven_slices": [
            {
                "slice_id": "single-child-zero-authority-inspect-child",
                "proof_status": "bounded_minimal",
                "proof_backed_layers": ["live_minimization_proof", "plan_proof_token_linkage", "proof_witness_linkage"],
                "invoke_shape": {
                    "max_exercised_children": 1,
                    "alias_binding": "exact_declared_dependency_alias",
                    "child_identity": "installed_dependency_snapshot_exact_digest",
                    "child_target_world": "guild-skill-inspect-v1",
                    "child_result_comparator": "guild.runner.live-proof.normalized-inspect-single-child-invoke.v1",
                    "child_authority": "forbidden",
                    "nested_child_executions": "forbidden"
                },
                "notes": "Bounded live proof exists only for the exact single-child zero-authority inspect slice."
            },
            {
                "slice_id": "two-child-same-alias-zero-authority-inspect-fan-out",
                "proof_status": "bounded_minimal",
                "proof_backed_layers": ["live_minimization_proof", "plan_proof_token_linkage", "proof_witness_linkage"],
                "invoke_shape": {
                    "max_exercised_children": 2,
                    "alias_binding": "same_exact_declared_dependency_alias_in_order",
                    "child_identity": "installed_dependency_snapshot_exact_digest",
                    "child_target_world": "guild-skill-inspect-v1",
                    "child_result_comparator": "guild.runner.live-proof.normalized-inspect-multi-child-invoke.v1",
                    "child_authority": "forbidden",
                    "nested_child_executions": "forbidden"
                },
                "notes": "Bounded live proof exists only for the exact two-child same-alias zero-authority inspect slice."
            }
        ],
        "not_proven_shapes": [
            not_proven_shape("dynamic-or-broader-resolution", &["INVOKE_SKILL_DYNAMIC_RESOLUTION_UNSUPPORTED"], "Dynamic or broader invoke resolution remains not_proven."),
            not_proven_shape("multi-child-fan-out", &["INVOKE_SKILL_MULTI_CHILD_UNSUPPORTED"], "Multi-child fan-out beyond the exact two-child same-alias zero-authority inspect slice remains not_proven."),
            not_proven_shape("recursive-or-deeper-call-graph", &["INVOKE_SKILL_RECURSION_UNSUPPORTED"], "Recursive or deeper invoke graphs remain not_proven."),
            not_proven_shape("child-authority-use", &["INVOKE_SKILL_CHILD_AUTHORITY_UNSUPPORTED"], "Child-side authority use remains not_proven."),
            not_proven_shape("non-inspect-child-world", &["INVOKE_SKILL_CHILD_WORLD_UNSUPPORTED"], "Non-inspect child targets remain not_proven.")
        ],
        "layers": {
            "admission_runtime_guarantee_matching": layer(STATUS_SUPPORTED, &["CANONICAL_FAMILY_SUPPORTED"], "Direct canonical invoke-skill grants match the active runtime vocabulary."),
            "execution_plan_representation": layer(STATUS_SUPPORTED, &["CANONICAL_FAMILY_SUPPORTED"], "M4 plans carry canonical invoke-skill scopes directly."),
            "live_minimization_proof": layer(STATUS_BOUNDED, &["LIVE_PROOF_BOUNDED", "LIVE_PROOF_SUPPORTED"], "Invoke-skill live proof is bounded to the exact zero-authority inspect slices only."),
            "token_issuance_basis": layer(STATUS_SUPPORTED, &["CANONICAL_FAMILY_SUPPORTED", "TOKEN_PROOF_BASIS_LIVE"], "Proof-backed M6 issuance can consume the bounded invoke-skill proof for the exact single-child and exact two-child same-alias slices."),
            "token_verification": layer(STATUS_SUPPORTED, &["CANONICAL_FAMILY_SUPPORTED"], "Canonical alias bindings verify directly against invoke-skill scopes."),
            "witness_generation": layer(STATUS_SUPPORTED, &["CANONICAL_FAMILY_SUPPORTED"], "Live invoke observations normalize directly into canonical witness effects."),
            "witness_verification": layer(STATUS_SUPPORTED, &["CANONICAL_FAMILY_SUPPORTED"], "Canonical invoke-skill witnesses verify directly."),
            "positive_claim_verification": layer(STATUS_UNSUPPORTED, &["POSITIVE_CLAIM_VOCABULARY_UNAVAILABLE"], "Positive observed-fact claim vocabulary is still unavailable."),
            "negative_claim_verification": layer(STATUS_SUPPORTED, &["CANONICAL_FAMILY_SUPPORTED"], "Scope-only negative claims stay supported when coverage is complete."),
            "plan_proof_token_linkage": layer(STATUS_BOUNDED, &["TOKEN_PROOF_BASIS_LIVE"], "Plan -> proof -> token linkage is bounded to the exact single-child and exact two-child same-alias slices only."),
            "proof_witness_linkage": layer(STATUS_BOUNDED, &["LIVE_PROOF_LINKED_WITNESS"], "Proof -> witness linkage is bounded to the exact single-child and exact two-child same-alias slices only.")
        }
    })
}

fn emit_evidence_family() -> Value {
    json!({
        "scope_shape": {
            "kind": "evidence",
            "fields": ["max_bytes", "audiences", "redactions"]
        },
        "proven_slices": [],
        "not_proven_shapes": [
            not_proven_shape("single-emission-local-object-store-replay-mismatch", &["EMIT_EVIDENCE_REPLAY_UNAVAILABLE"], "The tested exact single-emission local-object-store replay still fails closed."),
            not_proven_shape("dynamic-or-unstable-sink-semantics", &["EMIT_EVIDENCE_LINKAGE_MODEL_UNAVAILABLE"], "Dynamic or unstable sink semantics remain not_proven."),
            not_proven_shape("multiple-emissions-or-fan-out", &["EMIT_EVIDENCE_MULTI_EMISSION_UNSUPPORTED"], "Multiple emissions or fan-out remain not_proven."),
            not_proven_shape("nondeterministic-or-unreadable-payload", &["EMIT_EVIDENCE_PAYLOAD_UNSUPPORTED"], "Nondeterministic or unreadable payloads remain not_proven."),
            not_proven_shape("host-result-error-on-emission", &["EMIT_EVIDENCE_HOST_ERROR_UNSUPPORTED"], "Host-side emission failures remain not_proven.")
        ],
        "layers": {
            "admission_runtime_guarantee_matching": layer(STATUS_SUPPORTED, &["CANONICAL_FAMILY_SUPPORTED"], "Direct canonical emit-evidence grants match the active runtime vocabulary."),
            "execution_plan_representation": layer(STATUS_SUPPORTED, &["CANONICAL_FAMILY_SUPPORTED"], "M4 plans carry canonical emit-evidence scopes directly."),
            "live_minimization_proof": layer(STATUS_NOT_PROVEN, &["EMIT_EVIDENCE_REPLAY_UNAVAILABLE"], "Emit-evidence remains not_proven because the tested exact single-emission replay still fails closed."),
            "token_issuance_basis": layer(STATUS_SUPPORTED, &["CANONICAL_FAMILY_SUPPORTED"], "M6 can still issue upper-bound emit-evidence scopes as a draft-local token layer."),
            "token_verification": layer(STATUS_SUPPORTED, &["CANONICAL_FAMILY_SUPPORTED"], "Canonical emit-evidence bindings verify directly against emit-evidence scopes."),
            "witness_generation": layer(STATUS_SUPPORTED, &["CANONICAL_FAMILY_SUPPORTED"], "Live emit-evidence observations normalize directly into canonical witness effects."),
            "witness_verification": layer(STATUS_SUPPORTED, &["CANONICAL_FAMILY_SUPPORTED"], "Canonical emit-evidence witnesses verify directly."),
            "positive_claim_verification": layer(STATUS_UNSUPPORTED, &["POSITIVE_CLAIM_VOCABULARY_UNAVAILABLE"], "Positive observed-fact claim vocabulary is still unavailable."),
            "negative_claim_verification": layer(STATUS_SUPPORTED, &["CANONICAL_FAMILY_SUPPORTED"], "Scope-only negative claims stay supported when coverage is complete."),
            "plan_proof_token_linkage": layer(STATUS_NOT_PROVEN, &["EMIT_EVIDENCE_REPLAY_UNAVAILABLE"], "Plan -> proof -> token linkage remains fail-closed while emit-evidence is not_proven."),
            "proof_witness_linkage": layer(STATUS_NOT_PROVEN, &["EMIT_EVIDENCE_REPLAY_UNAVAILABLE"], "Proof -> witness linkage remains fail-closed while emit-evidence is not_proven.")
        }
    })
}

fn log_write_family() -> Value {
    json!({
        "scope_shape": {
            "kind": "log",
            "fields": ["levels"]
        },
        "proven_slices": [
            {
                "slice_id": "observed-discrete-levels",
                "proof_status": "exact_minimal",
                "proof_backed_layers": ["live_minimization_proof"],
                "log_shape": {
                    "levels": ["info"]
                },
                "notes": "Log-write live proof exists over the observed discrete level slice."
            }
        ],
        "not_proven_shapes": [],
        "layers": {
            "admission_runtime_guarantee_matching": layer(STATUS_SUPPORTED, &["CANONICAL_FAMILY_SUPPORTED"], "Direct canonical log-write grants match the active runtime vocabulary."),
            "execution_plan_representation": layer(STATUS_SUPPORTED, &["CANONICAL_FAMILY_SUPPORTED"], "M4 plans carry canonical log-write scopes directly."),
            "live_minimization_proof": layer(STATUS_SUPPORTED, &["LIVE_PROOF_SUPPORTED"], "Log-write has real live proof over the observed discrete level slice."),
            "token_issuance_basis": layer(STATUS_SUPPORTED, &["CANONICAL_FAMILY_SUPPORTED"], "M6 can issue direct canonical log-write scopes."),
            "token_verification": layer(STATUS_SUPPORTED, &["CANONICAL_FAMILY_SUPPORTED"], "Canonical log-write bindings verify directly against log-write scopes."),
            "witness_generation": layer(STATUS_SUPPORTED, &["CANONICAL_FAMILY_SUPPORTED"], "Live log observations normalize directly into canonical witness effects."),
            "witness_verification": layer(STATUS_SUPPORTED, &["CANONICAL_FAMILY_SUPPORTED"], "Canonical log-write witnesses verify directly."),
            "positive_claim_verification": layer(STATUS_UNSUPPORTED, &["POSITIVE_CLAIM_VOCABULARY_UNAVAILABLE"], "Positive observed-fact claim vocabulary is still unavailable."),
            "negative_claim_verification": layer(STATUS_SUPPORTED, &["CANONICAL_FAMILY_SUPPORTED"], "Scope-only negative claims stay supported when coverage is complete."),
            "plan_proof_token_linkage": layer(STATUS_UNSUPPORTED, &["BENCHMARK_LINKAGE_NOT_MEASURED"], "The checked benchmark does not currently claim a real-path token linkage slice for log-write."),
            "proof_witness_linkage": layer(STATUS_UNSUPPORTED, &["BENCHMARK_LINKAGE_NOT_MEASURED"], "The checked benchmark does not currently claim a real-path witness linkage slice for log-write.")
        }
    })
}

fn layer(status: &str, reason_codes: &[&str], notes: &str) -> Value {
    json!({
        "status": status,
        "reason_codes": reason_codes,
        "notes": notes,
    })
}

fn not_proven_shape(shape_id: &str, reason_codes: &[&str], notes: &str) -> Value {
    json!({
        "shape_id": shape_id,
        "reason_codes": reason_codes,
        "notes": notes,
    })
}

fn http_slice(
    slice_id: &str,
    method: &str,
    host_form: &str,
    port_form: &str,
    resolution_binding: Option<&str>,
) -> Value {
    let mut request_shape = json!({
        "max_exercised_requests": 1,
        "method": method,
        "scheme": "http",
        "host_form": host_form,
        "port_form": port_form,
        "path_match": "exact_observed_path",
        "query": "forbidden",
        "redirects": "forbidden",
        "comparator": "guild.runner.live-proof.normalized-inspect-output.v1",
        "replay": "required"
    });
    if let Some(binding) = resolution_binding {
        request_shape["resolution_binding"] = json!(binding);
        request_shape["resolution_scope"] =
            json!("literal_host_explicit_port_resolved_addresses_address_families_loopback_only");
    }
    json!({
        "slice_id": slice_id,
        "proof_status": "bounded_minimal",
        "proof_backed_layers": [
            "live_minimization_proof",
            "plan_proof_token_linkage",
            "proof_witness_linkage"
        ],
        "request_shape": request_shape,
        "notes": format!("Bounded live proof is real for the {slice_id} slice only.")
    })
}
