from __future__ import annotations

from copy import deepcopy
from pathlib import Path
from typing import Any

from admission_core import (
    build_registry,
    canonical_json,
    digest_struct,
    effect_covers,
    effect_is_canonical,
    effect_scope_kind,
    effect_selector,
    load_json,
    normalize_effect,
    path_pattern_covers,
    require_valid,
    stable_unique_dicts,
    stable_unique_strings,
    validate_instance,
)
from minimization_core import HARNESS_ID, HARNESS_VERSION, run_example_harness
from runtime_alignment import (
    LIVE_RUNTIME_SOURCE_KIND,
    MAPPING_EXACT,
    MAPPING_NARROWING,
    MAPPING_PARTIAL,
    MAPPING_UNSUPPORTED,
    observation_bundle_from_execution_record,
)
from token_core import (
    PROOF_SOURCE_LIVE_RUNTIME,
    TOKEN_CANONICALIZATION,
    TOKEN_PROTECTION_MODE,
    attach_protection,
    host_exact_bindings_match_authority,
    load_issuer_secret,
    proof_source_kind,
    protection_for_payload,
    scope_kind_for_effect,
    stable_unique_host_exact_bindings,
    stable_unique_resource_bindings,
    validate_plan_contract_alignment,
    validate_proof_alignment,
    verify_token,
)


WITNESS_KIND = "guild.witness_record"
WITNESS_VERSION = "1.0.0"
VERIFICATION_KIND = "guild.witness_verification_result"
VERIFICATION_VERSION = "1.0.0"
DEFAULT_SOURCE_KIND = "bounded-observation-fixture"
HARNESS_SOURCE_KIND = "draft-example-harness"
SUPPORTED_SOURCE_KINDS = {
    DEFAULT_SOURCE_KIND,
    HARNESS_SOURCE_KIND,
    LIVE_RUNTIME_SOURCE_KIND,
}

UNVERIFIABLE_REASON_CODES = {
    "AUDIENCE_BINDING_MISMATCH",
    "CALL_CHAIN_MISMATCH",
    "HOLDER_BINDING_MISMATCH",
    "OBSERVATION_SOURCE_UNAVAILABLE",
    "OBSERVATION_SOURCE_UNSUPPORTED",
    "PLAN_LINKAGE_MISMATCH",
    "PROOF_LINKAGE_MISMATCH",
    "RUNTIME_BINDING_MISMATCH",
    "TOKEN_LINKAGE_MISMATCH",
    "TOKEN_VERIFICATION_FAILED",
    "WITNESS_ISSUER_UNKNOWN",
    "WITNESS_KEY_ID_UNKNOWN",
    "WITNESS_MAC_INVALID",
    "WITNESS_REASON_CODES_INCONSISTENT",
    "WITNESS_SCHEMA_INVALID",
    "WITNESS_STATUS_INCONSISTENT",
}

HARNESSED_FAMILIES: dict[str, list[tuple[str, str]]] = {
    "urn:guild:contract:local-log-analyzer:v1": [
        ("fs.read", "filesystem"),
        ("clock.read", "system"),
    ],
    "urn:guild:contract:fetch-transform:v1": [
        ("fs.read", "filesystem"),
        ("fs.write", "filesystem"),
        ("secret.read", "secret"),
        ("net.connect", "network"),
        ("clock.read", "system"),
    ],
    "urn:guild:contract:zero-authority:v1": [],
}


class WitnessInputError(RuntimeError):
    """Raised when witness generation or verification inputs are malformed."""


def load_json_if_present(path: str | Path | None) -> dict[str, Any] | None:
    if path is None:
        return None
    return load_json(path)


def digest_or_none(value: Any) -> dict[str, str] | None:
    if value is None:
        return None
    return digest_struct(value)


def stable_unique_blocked_effects(values: list[dict[str, Any]]) -> list[dict[str, Any]]:
    keyed: dict[str, dict[str, Any]] = {}
    for value in values:
        key = canonical_json(
            {
                "effect": normalize_effect(value["effect"]),
                "reason_code": value["reason_code"],
                "message": value["message"],
                "details_digest": value["details_digest"],
            }
        )
        keyed[key] = value
    return [keyed[key] for key in sorted(keyed)]


def coverage_entry_selector(entry: dict[str, Any]) -> str:
    return entry["family"] if entry.get("draft_effect_class") is None else entry["draft_effect_class"]


def sort_coverage_entries(values: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return sorted(
        values,
        key=lambda item: (
            item["family"],
            item["draft_effect_class"] or "",
            item["scope_kind"] or "",
            canonical_json(item["scope_descriptors"]),
        ),
    )


def sort_unmapped_observations(values: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return sorted(
        values,
        key=lambda item: (
            item["family"],
            item["observed_as"],
            item["details_summary"],
        ),
    )


def relation_status_rank(value: str) -> int:
    return {"complete": 0, "partial": 1, "insufficient": 2}[value]


def combine_coverage_status(values: list[str]) -> str:
    if not values:
        return "complete"
    if any(value == "insufficient" for value in values):
        return "insufficient"
    if any(value == "partial" for value in values):
        return "partial"
    return "complete"


def observed_family_status(entry: dict[str, Any]) -> str:
    if entry["mapping_status"] == MAPPING_UNSUPPORTED:
        return "insufficient"
    if entry["mapping_status"] == MAPPING_PARTIAL and entry["status"] == "complete":
        return "partial"
    return entry["status"]


def relation_reason_codes(status: str) -> list[str]:
    if status == "partial":
        return ["OBSERVATION_COVERAGE_PARTIAL"]
    if status == "insufficient":
        return ["OBSERVATION_COVERAGE_INSUFFICIENT"]
    return []


def mapping_reason_codes(mapping_status: str) -> list[str]:
    if mapping_status == MAPPING_NARROWING:
        return ["VOCABULARY_MAPPING_NARROWING"]
    if mapping_status == MAPPING_PARTIAL:
        return ["VOCABULARY_MAPPING_PARTIAL"]
    if mapping_status == MAPPING_UNSUPPORTED:
        return ["VOCABULARY_MAPPING_UNSUPPORTED"]
    return []


def scope_descriptors_for_effect(effect: dict[str, Any]) -> list[str]:
    if effect_is_canonical(effect):
        family = effect_selector(effect)
        scope = effect["scope"]
        if family == "http-request":
            descriptors: list[str] = []
            methods = scope.get("allowed_methods") or ["GET"]
            schemes = scope.get("allowed_schemes") or ["http"]
            hosts = scope.get("allowed_hosts") or scope.get("allowed_host_suffixes") or ["*"]
            ports = scope.get("allowed_ports") or ["*"]
            prefixes = scope.get("allowed_path_prefixes") or ["/"]
            for method in methods:
                for scheme in schemes:
                    for host in hosts:
                        for port in ports:
                            for prefix in prefixes:
                                descriptors.append(f"{method}:{scheme}://{host}:{port}{prefix}")
            return stable_unique_strings(sorted(descriptors))
        if family == "read-resource":
            return stable_unique_strings(sorted(scope.get("uri_prefixes", []) or ["guild://"]))
        if family == "invoke-skill":
            return stable_unique_strings(sorted(scope.get("aliases", []) or ["invoke-skill"]))
        if family == "emit-evidence":
            audiences = scope.get("audiences") or ["*"]
            redactions = scope.get("redactions") or ["*"]
            return stable_unique_strings(
                sorted(f"audience={audience};redaction={redaction}" for audience in audiences for redaction in redactions)
            )
        if family == "log-write":
            return stable_unique_strings(sorted(scope.get("levels", []) or ["log-write"]))
        return [family]

    scope = effect["scope"]
    kind = scope["kind"]
    if kind == "filesystem":
        return stable_unique_strings(sorted(scope["paths"]))
    if kind == "network":
        descriptors: list[str] = []
        for audience in scope["audiences"]:
            host = audience["host"]
            schemes = audience.get("schemes") or ["*"]
            ports = audience.get("ports") or ["*"]
            prefixes = audience.get("path_prefixes") or ["/"]
            methods = audience.get("methods") or ["*"]
            for scheme in schemes:
                for port in ports:
                    for prefix in prefixes:
                        for method in methods:
                            descriptors.append(f"{method}:{scheme}://{host}:{port}{prefix}")
        return stable_unique_strings(sorted(descriptors))
    if kind == "secret":
        return stable_unique_strings(sorted(scope.get("secret_ids", [])))
    if kind == "component":
        return stable_unique_strings(sorted(scope.get("exports", [])))
    if kind == "environment":
        return stable_unique_strings(sorted(scope.get("names", [])))
    if kind == "delegation":
        audiences = scope.get("audiences", [])
        return stable_unique_strings(sorted(audiences or ["delegation"]))
    return [effect_selector(effect)]


def normalize_blocked_effect(value: dict[str, Any]) -> dict[str, Any]:
    details = value.get("details")
    return {
        "effect": normalize_effect(value["effect"]),
        "reason_code": value["reason_code"],
        "message": value["message"],
        "details_digest": digest_or_none(details),
    }


def normalize_unmapped_observation(value: dict[str, Any]) -> dict[str, Any]:
    details = value.get("details")
    coverage_status = value.get("coverage_status", "partial")
    reason_codes = stable_unique_strings(
        sorted(
            value.get("reason_codes", [])
            + relation_reason_codes(coverage_status)
            + mapping_reason_codes(MAPPING_UNSUPPORTED)
        )
    )
    normalized = {
        "family": value["family"],
        "observed_as": value["observed_as"],
        "details_summary": value["details_summary"],
        "details_digest": digest_or_none(details),
        "coverage_status": coverage_status,
        "reason_codes": reason_codes,
    }
    if value.get("notes") is not None:
        normalized["notes"] = value["notes"]
    return normalized


def normalize_coverage_entry(value: dict[str, Any]) -> dict[str, Any]:
    mapping_status = value.get("mapping_status", MAPPING_EXACT)
    normalized_status = observed_family_status(
        {
            "status": value.get("status", "complete"),
            "mapping_status": mapping_status,
        }
    )
    normalized = {
        "family": value["family"],
        "draft_effect_class": value.get("draft_effect_class"),
        "scope_kind": value.get("scope_kind"),
        "status": normalized_status,
        "mapping_status": mapping_status,
        "supports_positive_facts": value.get("supports_positive_facts", True),
        "supports_absence_claims": value.get(
            "supports_absence_claims",
            normalized_status == "complete" and mapping_status in {MAPPING_EXACT, MAPPING_NARROWING},
        ),
        "scope_descriptors": stable_unique_strings(sorted(value.get("scope_descriptors", []))),
        "reason_codes": stable_unique_strings(
            sorted(
                value.get("reason_codes", [])
                + relation_reason_codes(normalized_status)
                + mapping_reason_codes(mapping_status)
            )
        ),
    }
    if value.get("notes") is not None:
        normalized["notes"] = value["notes"]
    return normalized


def coverage_entry_for_family(
    *,
    family: str,
    draft_effect_class: str | None,
    scope_kind: str | None,
    status: str,
    mapping_status: str,
    scope_descriptors: list[str],
    notes: str | None = None,
) -> dict[str, Any]:
    return normalize_coverage_entry(
        {
            "family": family,
            "draft_effect_class": draft_effect_class,
            "scope_kind": scope_kind,
            "status": status,
            "mapping_status": mapping_status,
            "supports_positive_facts": True,
            "supports_absence_claims": status == "complete" and mapping_status in {MAPPING_EXACT, MAPPING_NARROWING},
            "scope_descriptors": scope_descriptors,
            "reason_codes": [],
            "notes": notes,
        }
    )


def coverage_entries_from_harness(
    contract: dict[str, Any],
    authority_plan: dict[str, Any],
    observed_effects: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    families = HARNESSED_FAMILIES.get(contract["contract_id"])
    if families is None:
        return []
    descriptors: dict[str, list[str]] = {}
    for effect_class, _scope_kind in families:
        descriptors[effect_class] = []
    for effect in observed_effects + authority_plan.get("grants", []):
        if effect["effect_class"] not in descriptors:
            continue
        descriptors[effect["effect_class"]].extend(scope_descriptors_for_effect(effect))
    return sort_coverage_entries(
        [
            coverage_entry_for_family(
                family=effect_class,
                draft_effect_class=effect_class,
                scope_kind=scope_kind,
                status="complete",
                mapping_status=MAPPING_EXACT,
                scope_descriptors=stable_unique_strings(sorted(descriptors[effect_class])),
                notes="Bounded draft-v1 example harness coverage. This is not a runtime-general observation claim.",
            )
            for effect_class, scope_kind in families
        ]
    )


def coverage_entries_from_observation(
    observation: dict[str, Any],
    authority_plan: dict[str, Any],
    observed_effects: list[dict[str, Any]],
    blocked_effects: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    provided = observation.get("coverage_families")
    if provided:
        return sort_coverage_entries([normalize_coverage_entry(item) for item in provided])

    families: dict[str, dict[str, Any]] = {}
    for effect in authority_plan.get("grants", []) + observed_effects + [item["effect"] for item in blocked_effects]:
        selector = effect_selector(effect)
        families.setdefault(
            selector,
            {
                "family": selector if effect_is_canonical(effect) else selector,
                "draft_effect_class": None if effect_is_canonical(effect) else selector,
                "scope_kind": effect_scope_kind(effect) if effect_is_canonical(effect) else scope_kind_for_effect(selector),
                "status": "complete",
                "mapping_status": MAPPING_EXACT,
                "scope_descriptors": [],
                "supports_positive_facts": True,
                "supports_absence_claims": True,
                "reason_codes": [],
                "notes": "Explicit bounded observation fixture coverage.",
            },
        )
        families[selector]["scope_descriptors"].extend(scope_descriptors_for_effect(effect))
    return sort_coverage_entries(
        [
            normalize_coverage_entry(
                {
                    **value,
                    "scope_descriptors": stable_unique_strings(sorted(value["scope_descriptors"])),
                }
            )
            for value in families.values()
        ]
    )


def overall_coverage_status(entries: list[dict[str, Any]], unmapped_observations: list[dict[str, Any]]) -> str:
    statuses = [entry["status"] for entry in entries] + [item["coverage_status"] for item in unmapped_observations]
    return combine_coverage_status(statuses)


def authority_summary(effects: list[dict[str, Any]], *, effects_redacted: bool) -> dict[str, Any]:
    return {
        "effects": [] if effects_redacted else stable_unique_dicts([normalize_effect(effect) for effect in effects]),
        "effect_classes": stable_unique_strings(sorted({effect_selector(effect) for effect in effects})),
        "total_effects": len(stable_unique_dicts([normalize_effect(effect) for effect in effects])),
        "effects_redacted": effects_redacted,
    }


def blocked_authority_summary(
    blocked_effects: list[dict[str, Any]],
    *,
    observable: bool,
    effects_redacted: bool,
) -> dict[str, Any]:
    rendered_effects = [] if effects_redacted else stable_unique_blocked_effects(blocked_effects)
    return {
        "observable": observable,
        "effects": rendered_effects,
        "effect_classes": stable_unique_strings(sorted({effect_selector(item["effect"]) for item in blocked_effects})),
        "total_effects": len(stable_unique_blocked_effects(blocked_effects)),
        "effects_redacted": effects_redacted,
    }


def granted_but_unused_authority(
    authority_plan: dict[str, Any],
    exercised_effects: list[dict[str, Any]],
    coverage_entries: list[dict[str, Any]],
    *,
    redacted: bool,
) -> dict[str, Any]:
    grant_families = {effect_selector(grant) for grant in authority_plan.get("grants", [])}
    supports_absence = {
        coverage_entry_selector(entry)
        for entry in coverage_entries
        if entry["supports_absence_claims"]
    }
    derivable_families = grant_families.intersection(supports_absence)

    if not authority_plan.get("grants"):
        return {
            "derivation_status": "complete",
            "fully_unused_grants": [],
            "fully_unused_grants_redacted": redacted,
            "reason_codes": [],
        }

    unused: list[dict[str, Any]] = []
    for grant in authority_plan.get("grants", []):
        if effect_selector(grant) not in derivable_families:
            continue
        if any(effect_covers(grant, effect) for effect in exercised_effects):
            continue
        unused.append(normalize_effect(grant))

    if derivable_families == grant_families:
        derivation_status = "complete"
        reason_codes: list[str] = []
    elif derivable_families:
        derivation_status = "partial"
        reason_codes = ["OBSERVATION_COVERAGE_PARTIAL"]
    else:
        derivation_status = "not_derivable"
        reason_codes = ["OBSERVATION_COVERAGE_INSUFFICIENT"]

    return {
        "derivation_status": derivation_status,
        "fully_unused_grants": [] if redacted else stable_unique_dicts(unused),
        "fully_unused_grants_redacted": redacted,
        "reason_codes": stable_unique_strings(sorted(reason_codes)),
    }


def compare_to_envelope(
    exercised_effects: list[dict[str, Any]],
    authority_plan: dict[str, Any] | None,
) -> dict[str, Any] | None:
    if authority_plan is None:
        return None
    outside = [
        normalize_effect(effect)
        for effect in exercised_effects
        if not any(effect_covers(grant, effect) for grant in authority_plan.get("grants", []))
    ]
    return {
        "status": "outside" if outside else "within",
        "outside_effects": stable_unique_dicts(outside),
        "outside_effect_classes": stable_unique_strings(sorted({effect_selector(effect) for effect in outside})),
    }


def compare_to_required_basis(witness: dict[str, Any]) -> str:
    if witness["observation_source"]["source_kind"] not in SUPPORTED_SOURCE_KINDS:
        return "unverifiable"
    if witness["token_basis"] is not None and (
        witness["token_basis"]["verification_decision"] != "allow" or not witness["token_basis"]["verification_verified"]
    ):
        return "unverifiable"
    if witness["envelope_comparison"]["plan"]["status"] == "outside":
        return "out_of_envelope"
    proof_comparison = witness["envelope_comparison"]["proof"]
    if proof_comparison is not None and proof_comparison["status"] == "outside":
        return "out_of_envelope"
    token_comparison = witness["envelope_comparison"]["token"]
    if token_comparison is not None and token_comparison["status"] == "outside":
        return "out_of_envelope"
    if (
        witness["observation_coverage"]["overall_status"] != "complete"
        or any(
            entry["mapping_status"] not in {MAPPING_EXACT, MAPPING_NARROWING}
            for entry in witness["observation_coverage"]["families"]
        )
        or witness["unmapped_observations"]
    ):
        return "coverage_limited"
    return "within_envelope"


def witness_payload_without_protection(witness: dict[str, Any]) -> dict[str, Any]:
    payload = deepcopy(witness)
    payload.pop("protection", None)
    return payload


def witness_payload_for_redaction_digest(witness: dict[str, Any]) -> dict[str, Any]:
    payload = witness_payload_without_protection(witness)
    payload["redaction"]["redacted_content_digest"] = None
    return payload


def witness_redaction_digest(witness: dict[str, Any]) -> dict[str, str]:
    return digest_struct(witness_payload_for_redaction_digest(witness))


def verify_witness_protection(witness: dict[str, Any], shared_secret: str) -> bool:
    protection = witness.get("protection")
    if protection is None:
        return False
    if protection.get("mode") != TOKEN_PROTECTION_MODE:
        return False
    if protection.get("canonicalization") != TOKEN_CANONICALIZATION:
        return False
    payload = witness_payload_without_protection(witness)
    expected = protection_for_payload(payload, shared_secret)
    return expected == protection


def call_chain_digest(chain_id: str, links: list[str] | None) -> dict[str, str]:
    return digest_struct({"chain_id": chain_id, "links": links})


def observation_bundle(
    *,
    contract: dict[str, Any],
    invocation_input: dict[str, Any] | None,
    authority_plan: dict[str, Any],
    observation: dict[str, Any] | None,
) -> tuple[dict[str, Any], list[str]]:
    reason_codes: list[str] = []

    if observation is None:
        if invocation_input is None:
            return (
                {
                    "source": {
                        "source_id": "urn:guild:observation:none",
                        "source_kind": DEFAULT_SOURCE_KIND,
                        "version": "1.0.0",
                        "notes": "No observation input was supplied.",
                    },
                    "observed_effects": [],
                    "blocked_effects": [],
                    "blocked_attempts_observable": False,
                    "unmapped_observations": [],
                    "coverage_families": [],
                    "overall_status": "insufficient",
                    "raw_trace": None,
                    "started_at": None,
                    "finished_at": None,
                },
                ["OBSERVATION_SOURCE_UNAVAILABLE"],
            )

        run = run_example_harness(contract, invocation_input, authority_plan)
        supported_families = HARNESSED_FAMILIES.get(contract["contract_id"])
        if supported_families is None or run["status"] != "success":
            failure_code = "OBSERVATION_SOURCE_UNSUPPORTED" if supported_families is None else "OBSERVATION_SOURCE_UNAVAILABLE"
            return (
                {
                    "source": {
                        "source_id": HARNESS_ID,
                        "source_kind": HARNESS_SOURCE_KIND,
                        "version": HARNESS_VERSION,
                        "notes": "Harness observation was unavailable for this invocation.",
                    },
                    "observed_effects": [],
                    "blocked_effects": [],
                    "blocked_attempts_observable": False,
                    "unmapped_observations": [],
                    "coverage_families": [],
                    "overall_status": "insufficient",
                    "raw_trace": {
                        "status": run["status"],
                        "error_code": run["error_code"],
                    },
                    "started_at": None,
                    "finished_at": None,
                },
                [failure_code],
            )

        observed_effects = stable_unique_dicts([normalize_effect(effect) for effect in run["observed_effects"]])
        coverage_families = coverage_entries_from_harness(contract, authority_plan, observed_effects)
        return (
            {
                "source": {
                    "source_id": HARNESS_ID,
                    "source_kind": HARNESS_SOURCE_KIND,
                    "version": HARNESS_VERSION,
                    "notes": "Bounded draft-v1 example harness observation. This does not claim runtime-general completeness.",
                },
                "observed_effects": observed_effects,
                "blocked_effects": [],
                "blocked_attempts_observable": False,
                "unmapped_observations": [],
                "coverage_families": coverage_families,
                "overall_status": overall_coverage_status(coverage_families, []),
                "raw_trace": {
                    "status": run["status"],
                    "output": run["output"],
                    "observed_effects": run["observed_effects"],
                },
                "started_at": None,
                "finished_at": None,
            },
            [],
        )

    source_kind = observation.get("source_kind", DEFAULT_SOURCE_KIND)
    if source_kind not in SUPPORTED_SOURCE_KINDS:
        reason_codes.append("OBSERVATION_SOURCE_UNSUPPORTED")
    if source_kind == LIVE_RUNTIME_SOURCE_KIND and observation.get("execution_record") is not None:
        return observation_bundle_from_execution_record(
            observation["execution_record"],
            authority_plan,
        )
    observed_effects = stable_unique_dicts(
        [normalize_effect(effect) for effect in observation.get("observed_effects", [])]
    )
    blocked_effects = stable_unique_blocked_effects(
        [normalize_blocked_effect(item) for item in observation.get("blocked_attempts", [])]
    )
    unmapped_observations = sort_unmapped_observations(
        [normalize_unmapped_observation(item) for item in observation.get("unmapped_observations", [])]
    )
    coverage_families = coverage_entries_from_observation(
        observation,
        authority_plan,
        observed_effects,
        blocked_effects,
    )
    overall_status = observation.get("overall_coverage_status") or overall_coverage_status(
        coverage_families,
        unmapped_observations,
    )
    return (
        {
            "source": {
                "source_id": observation.get("source_id", "urn:guild:observation:fixture:v1"),
                "source_kind": source_kind,
                "version": observation.get("version", "1.0.0"),
                "notes": observation.get("notes"),
            },
            "observed_effects": observed_effects,
            "blocked_effects": blocked_effects,
            "blocked_attempts_observable": observation.get("blocked_attempts_observable", bool(blocked_effects)),
            "unmapped_observations": unmapped_observations,
            "coverage_families": coverage_families,
            "overall_status": overall_status,
            "raw_trace": observation.get("raw_trace"),
            "started_at": observation.get("started_at"),
            "finished_at": observation.get("finished_at"),
        },
        stable_unique_strings(sorted(reason_codes)),
    )


def derive_audience_binding(plan: dict[str, Any], token: dict[str, Any] | None) -> dict[str, Any]:
    if token is not None:
        return {
            "audiences": stable_unique_strings(sorted(token["audience_binding"]["audiences"])),
            "resources": stable_unique_resource_bindings(token["audience_binding"]["resources"]),
        }
    return {
        "audiences": stable_unique_strings(
            sorted(plan["delegation_token_policy_inputs"]["audience_binding_inputs"])
        ),
        "resources": stable_unique_resource_bindings(
            plan["delegation_token_policy_inputs"]["resource_binding_inputs"]
        ),
    }


def derive_holder_binding(token: dict[str, Any] | None) -> dict[str, Any] | None:
    if token is None:
        return None
    return deepcopy(token["holder_binding"])


def derive_call_chain(token: dict[str, Any] | None) -> dict[str, Any] | None:
    if token is None:
        return None
    return deepcopy(token["call_chain"])


def derive_host_exact_bindings(
    proof: dict[str, Any] | None,
    token: dict[str, Any] | None,
) -> list[dict[str, Any]]:
    if token is not None and token.get("issuance_basis") == "m5_proven_subset":
        return stable_unique_host_exact_bindings(token.get("host_exact_bindings", []))
    if proof is not None:
        return stable_unique_host_exact_bindings(proof.get("host_exact_bindings", []))
    return []


def proof_basis_from_proof(proof: dict[str, Any] | None) -> dict[str, Any] | None:
    if proof is None:
        return None
    basis = {
        "proof_id": proof["proof_id"],
        "proof_digest": digest_struct(proof),
        "proof_status": proof["proof_status"],
    }
    if proof_source_kind(proof) == PROOF_SOURCE_LIVE_RUNTIME:
        basis["proof_source_kind"] = PROOF_SOURCE_LIVE_RUNTIME
    return basis


def canonical_reason_codes(values: list[str]) -> list[str]:
    return stable_unique_strings(sorted(values))


def verify_or_bind_token(
    *,
    token: dict[str, Any] | None,
    token_verification_result: dict[str, Any] | None,
    plan: dict[str, Any],
    contract: dict[str, Any],
    proof: dict[str, Any] | None,
    parent_token: dict[str, Any] | None,
    issuer_keys: dict[str, dict[str, str]] | None,
    verification_time: str,
) -> tuple[dict[str, Any] | None, dict[str, Any] | None, dict[str, Any] | None, list[str]]:
    if token is None:
        return None, None, None, []

    registry = build_registry()
    require_valid("delegated_capability_token.schema.json", token, registry, "delegated capability token")
    if parent_token is not None:
        require_valid("delegated_capability_token.schema.json", parent_token, registry, "parent delegated capability token")

    if token_verification_result is not None:
        require_valid("token_verification_result.schema.json", token_verification_result, registry, "token verification result")
        verification_result = deepcopy(token_verification_result)
    else:
        if issuer_keys is None:
            raise WitnessInputError("token verification requires issuer_keys")
        verification_result = verify_token(
            token,
            issuer_keys=issuer_keys,
            verification_time=verification_time,
            expected_holder_id=token["holder_binding"]["holder_id"],
            expected_audiences=token["audience_binding"]["audiences"],
            expected_resources=token["audience_binding"]["resources"],
            expected_runtime_guarantee_id=plan["chosen_runtime"]["runtime_guarantee_id"],
            expected_call_chain_links=token["call_chain"]["links"],
            plan=plan,
            contract=contract,
            proof=proof,
            parent_token=parent_token,
            check_replay=False,
        )

    reason_codes: list[str] = []
    if verification_result["token_id"] != token["token_id"] or verification_result["token_digest"] != digest_struct(token):
        reason_codes.append("TOKEN_LINKAGE_MISMATCH")
    if verification_result["issuer_id"] != token["issuer"]["issuer_id"] or verification_result["key_id"] != token["issuer"]["key_id"]:
        reason_codes.append("TOKEN_LINKAGE_MISMATCH")
    if (
        verification_result["bound_context"]["holder_id"] != token["holder_binding"]["holder_id"]
        or verification_result["bound_context"]["audiences"] != token["audience_binding"]["audiences"]
        or verification_result["bound_context"]["resources"] != token["audience_binding"]["resources"]
        or verification_result["bound_context"]["runtime_guarantee_id"] != plan["chosen_runtime"]["runtime_guarantee_id"]
        or verification_result["bound_context"]["call_chain_digest"] != token["call_chain"]["chain_digest"]
    ):
        reason_codes.append("TOKEN_LINKAGE_MISMATCH")
    if verification_result["decision"] != "allow" or not verification_result["verified"]:
        reason_codes.append("TOKEN_VERIFICATION_FAILED")

    token_exact_bindings = stable_unique_host_exact_bindings(token.get("host_exact_bindings", []))
    if token["issuance_basis"] == "m5_proven_subset":
        proof_exact_bindings = stable_unique_host_exact_bindings(
            proof.get("host_exact_bindings", []) if proof is not None else []
        )
        if token_exact_bindings != proof_exact_bindings:
            reason_codes.append("TOKEN_LINKAGE_MISMATCH")
    elif token_exact_bindings:
        reason_codes.append("TOKEN_LINKAGE_MISMATCH")
    if token_exact_bindings and not host_exact_bindings_match_authority(
        token_exact_bindings,
        token["granted_authority"],
    ):
        reason_codes.append("TOKEN_LINKAGE_MISMATCH")

    token_basis = {
        "token_id": token["token_id"],
        "token_digest": digest_struct(token),
        "issuance_basis": token["issuance_basis"],
        "parent_token_id": token["parent_token"]["token_id"] if token["parent_token"] is not None else None,
        "parent_token_digest": token["parent_token"]["token_digest"] if token["parent_token"] is not None else None,
        "verification_result_digest": digest_struct(verification_result),
        "verification_decision": verification_result["decision"],
        "verification_verified": verification_result["verified"],
        "verification_reason_codes": verification_result["reason_codes"],
    }
    if token.get("proof_source_kind") is not None:
        token_basis["proof_source_kind"] = token["proof_source_kind"]

    trusted_authority = deepcopy(token["granted_authority"]) if not reason_codes else None
    return token_basis, verification_result, trusted_authority, canonical_reason_codes(reason_codes)


def determine_authority_basis(
    *,
    plan: dict[str, Any],
    proof: dict[str, Any] | None,
    token_authority: dict[str, Any] | None,
) -> dict[str, Any]:
    if token_authority is not None:
        return deepcopy(token_authority)
    if proof is not None:
        return deepcopy(proof["proven_authority_plan"])
    return deepcopy(plan["granted_authority"])


def apply_redaction_profile(witness: dict[str, Any], profile: str) -> dict[str, Any]:
    redacted = deepcopy(witness)
    redacted_fields = ["raw_trace"]

    if profile == "summary_only":
        if redacted["call_chain"] is not None:
            redacted["call_chain"]["links"] = None
            redacted_fields.append("call_chain.links")
    elif profile == "counts_only":
        redacted["actual_exercised_authority"]["effects"] = []
        redacted["actual_exercised_authority"]["effects_redacted"] = True
        redacted["blocked_attempted_authority"]["effects"] = []
        redacted["blocked_attempted_authority"]["effects_redacted"] = True
        redacted["granted_but_unused_authority"]["fully_unused_grants"] = []
        redacted["granted_but_unused_authority"]["fully_unused_grants_redacted"] = True
        if redacted["call_chain"] is not None:
            redacted["call_chain"]["links"] = None
        redacted_fields.extend(
            [
                "actual_exercised_authority.effects",
                "blocked_attempted_authority.effects",
                "granted_but_unused_authority.fully_unused_grants",
                "call_chain.links",
            ]
        )
    elif profile != "none":
        raise WitnessInputError(f"unsupported redaction profile {profile!r}")

    redacted["redaction"] = {
        "profile": profile,
        "raw_trace_included": False,
        "raw_trace_digest": redacted["redaction"]["raw_trace_digest"],
        "redacted_fields": stable_unique_strings(sorted(redacted_fields)),
        "redacted_content_digest": None,
    }
    redacted["redaction"]["redacted_content_digest"] = witness_redaction_digest(redacted)
    return redacted


def generate_witness(
    *,
    plan: dict[str, Any],
    contract: dict[str, Any],
    issuer: dict[str, Any],
    issued_at: str,
    invocation_input: dict[str, Any] | None = None,
    proof: dict[str, Any] | None = None,
    required_proof_source_kind: str | None = None,
    token: dict[str, Any] | None = None,
    parent_token: dict[str, Any] | None = None,
    token_verification_result: dict[str, Any] | None = None,
    observation: dict[str, Any] | None = None,
    issuer_keys: dict[str, dict[str, str]] | None = None,
    witness_id: str | None = None,
    redaction_profile: str = "summary_only",
    started_at: str | None = None,
    finished_at: str | None = None,
    notes: str | None = None,
) -> dict[str, Any]:
    registry = build_registry()
    require_valid("execution_plan.schema.json", plan, registry, "execution plan")
    require_valid("skill_contract.schema.json", contract, registry, "skill contract")
    if proof is not None:
        require_valid("proof_record.schema.json", proof, registry, "proof record")

    reason_codes = validate_plan_contract_alignment(plan, contract)
    witness_reason_codes: list[str] = []
    if reason_codes:
        witness_reason_codes.append("PLAN_LINKAGE_MISMATCH")

    proof_basis = proof_basis_from_proof(proof)
    trusted_proof_authority: dict[str, Any] | None = None
    if proof is not None:
        proof_errors = validate_proof_alignment(plan, contract, proof, issued_at)
        if required_proof_source_kind is not None and proof_source_kind(proof) != required_proof_source_kind:
            proof_errors.append("PROOF_LINKAGE_UNAVAILABLE")
        if proof_errors:
            proof_basis = None
            witness_reason_codes.append("PROOF_LINKAGE_MISMATCH")
            if required_proof_source_kind is not None:
                witness_reason_codes.append("WITNESS_PROOF_LINKAGE_UNAVAILABLE")
        else:
            trusted_proof_authority = deepcopy(proof["proven_authority_plan"])
    elif required_proof_source_kind is not None:
        proof_basis = None
        witness_reason_codes.append("PROOF_LINKAGE_UNAVAILABLE")
        witness_reason_codes.append("WITNESS_PROOF_LINKAGE_UNAVAILABLE")

    token_basis, _computed_token_verification_result, trusted_token_authority, token_reason_codes = verify_or_bind_token(
        token=token,
        token_verification_result=token_verification_result,
        plan=plan,
        contract=contract,
        proof=proof,
        parent_token=parent_token,
        issuer_keys=issuer_keys,
        verification_time=issued_at,
    )
    witness_reason_codes.extend(token_reason_codes)

    authority_basis = determine_authority_basis(
        plan=plan,
        proof=proof if trusted_proof_authority is not None else None,
        token_authority=trusted_token_authority,
    )

    observation_data, observation_reason_codes = observation_bundle(
        contract=contract,
        invocation_input=invocation_input,
        authority_plan=authority_basis,
        observation=observation,
    )
    witness_reason_codes.extend(observation_reason_codes)

    exercised_effects = observation_data["observed_effects"]
    blocked_effects = observation_data["blocked_effects"]
    unmapped_observations = observation_data["unmapped_observations"]
    coverage_families = observation_data["coverage_families"]
    overall_status = observation_data["overall_status"]
    for entry in coverage_families:
        witness_reason_codes.extend(entry["reason_codes"])
    for item in unmapped_observations:
        witness_reason_codes.extend(item["reason_codes"])

    plan_comparison = compare_to_envelope(exercised_effects, plan["granted_authority"])
    proof_comparison = compare_to_envelope(exercised_effects, trusted_proof_authority) if proof_basis is not None else None
    token_comparison = compare_to_envelope(exercised_effects, trusted_token_authority) if token_basis is not None and trusted_token_authority is not None else None

    if plan_comparison is not None and plan_comparison["status"] == "outside":
        witness_reason_codes.append("OBSERVED_EFFECT_OUTSIDE_PLAN")
    if proof_comparison is not None and proof_comparison["status"] == "outside":
        witness_reason_codes.append("OBSERVED_EFFECT_OUTSIDE_PROOF")
    if token_comparison is not None and token_comparison["status"] == "outside":
        witness_reason_codes.append("OBSERVED_EFFECT_OUTSIDE_TOKEN")
    if overall_status == "partial":
        witness_reason_codes.append("OBSERVATION_COVERAGE_PARTIAL")
    if overall_status == "insufficient":
        witness_reason_codes.append("OBSERVATION_COVERAGE_INSUFFICIENT")
    if any(entry["mapping_status"] == MAPPING_NARROWING for entry in coverage_families):
        witness_reason_codes.append("VOCABULARY_MAPPING_NARROWING")
    if any(entry["mapping_status"] == MAPPING_PARTIAL for entry in coverage_families):
        witness_reason_codes.append("VOCABULARY_MAPPING_PARTIAL")
    if any(entry["mapping_status"] == MAPPING_UNSUPPORTED for entry in coverage_families) or unmapped_observations:
        witness_reason_codes.append("VOCABULARY_MAPPING_UNSUPPORTED")

    witness = {
        "kind": WITNESS_KIND,
        "version": WITNESS_VERSION,
        "witness_id": witness_id or f"{plan['plan_id']}:witness",
        "issuer": {
            "issuer_id": issuer["issuer_id"],
            "key_id": issuer["key_id"],
            "issuer_epoch": issuer.get("issuer_epoch", 0),
        },
        "issued_at": issued_at,
        "request_id": plan["request_id"],
        "execution_plan": {
            "execution_plan_id": plan["plan_id"],
            "execution_plan_digest": digest_struct(plan),
        },
        "proof_basis": proof_basis,
        "token_basis": token_basis,
        "host_exact_bindings": derive_host_exact_bindings(
            proof if trusted_proof_authority is not None else None,
            token if token_basis is not None else None,
        ),
        "subject": {
            "skill_contract_id": contract["contract_id"],
            "contract_digest": digest_struct(contract),
            "component_digest": deepcopy(contract["component"]["digest"]),
            "export_name": plan["export_name"],
            "input_class_fingerprint": deepcopy(plan["input_class_fingerprint"]),
            "invocation_input_digest": digest_or_none(invocation_input),
        },
        "runtime_binding": deepcopy(plan["chosen_runtime"]),
        "audience_binding": derive_audience_binding(plan, token),
        "holder_binding": derive_holder_binding(token),
        "call_chain": derive_call_chain(token),
        "observation_source": observation_data["source"],
        "observation_coverage": {
            "overall_status": overall_status,
            "families": coverage_families,
        },
        "actual_exercised_authority": authority_summary(exercised_effects, effects_redacted=False),
        "blocked_attempted_authority": blocked_authority_summary(
            blocked_effects,
            observable=observation_data["blocked_attempts_observable"],
            effects_redacted=False,
        ),
        "granted_but_unused_authority": granted_but_unused_authority(
            authority_basis,
            exercised_effects,
            coverage_families,
            redacted=False,
        ),
        "unmapped_observations": unmapped_observations,
        "envelope_comparison": {
            "plan": plan_comparison,
            "proof": proof_comparison,
            "token": token_comparison,
        },
        "execution_window": {
            "started_at": started_at or observation_data["started_at"],
            "finished_at": finished_at or observation_data["finished_at"],
        },
        "witness_status": "within_envelope",
        "reason_codes": canonical_reason_codes(witness_reason_codes),
        "redaction": {
            "profile": redaction_profile,
            "raw_trace_included": False,
            "raw_trace_digest": digest_or_none(observation_data["raw_trace"]),
            "redacted_fields": [],
            "redacted_content_digest": None,
        },
        "protection": {
            "mode": TOKEN_PROTECTION_MODE,
            "canonicalization": TOKEN_CANONICALIZATION,
            "claims_digest": {"algorithm": "sha256", "value": ""},
            "mac_base64": "",
        },
        "notes": notes or "M7 draft-v1 witness over bounded observation. This does not claim runtime-general completeness.",
    }

    witness["witness_status"] = compare_to_required_basis(witness)
    witness = apply_redaction_profile(witness, redaction_profile)
    witness["witness_status"] = compare_to_required_basis(witness)
    witness["reason_codes"] = canonical_reason_codes(witness["reason_codes"])
    witness = attach_protection(witness, issuer["shared_secret"])
    require_valid("witness_record.schema.json", witness, registry, "witness record")
    return witness


def verification_result(
    *,
    verification_time: str,
    witness: dict[str, Any] | None,
    verified: bool,
    witness_status: str,
    reason_codes: list[str],
    claim_evaluation: dict[str, Any] | None = None,
    notes: str | None = None,
) -> dict[str, Any]:
    witness_id = witness.get("witness_id") if witness is not None else None
    issuer_id = witness.get("issuer", {}).get("issuer_id") if witness is not None else None
    key_id = witness.get("issuer", {}).get("key_id") if witness is not None else None
    return {
        "kind": VERIFICATION_KIND,
        "version": VERIFICATION_VERSION,
        "verification_time": verification_time,
        "witness_id": witness_id,
        "witness_digest": digest_or_none(witness),
        "issuer_id": issuer_id,
        "key_id": key_id,
        "verified": verified,
        "witness_status": witness_status,
        "reason_codes": canonical_reason_codes(reason_codes),
        "claim_evaluation": claim_evaluation,
        **({"notes": notes} if notes is not None else {}),
    }


def minimal_consistency_reason_codes(witness: dict[str, Any]) -> list[str]:
    reason_codes: list[str] = []
    if witness["observation_coverage"]["overall_status"] == "partial" and "OBSERVATION_COVERAGE_PARTIAL" not in witness["reason_codes"]:
        reason_codes.append("WITNESS_REASON_CODES_INCONSISTENT")
    if witness["observation_coverage"]["overall_status"] == "insufficient" and "OBSERVATION_COVERAGE_INSUFFICIENT" not in witness["reason_codes"]:
        reason_codes.append("WITNESS_REASON_CODES_INCONSISTENT")
    if any(entry["mapping_status"] == MAPPING_NARROWING for entry in witness["observation_coverage"]["families"]) and "VOCABULARY_MAPPING_NARROWING" not in witness["reason_codes"]:
        reason_codes.append("WITNESS_REASON_CODES_INCONSISTENT")
    if (
        any(entry["mapping_status"] == MAPPING_PARTIAL for entry in witness["observation_coverage"]["families"])
        and "VOCABULARY_MAPPING_PARTIAL" not in witness["reason_codes"]
    ):
        reason_codes.append("WITNESS_REASON_CODES_INCONSISTENT")
    if (
        any(entry["mapping_status"] == MAPPING_UNSUPPORTED for entry in witness["observation_coverage"]["families"])
        or witness["unmapped_observations"]
    ) and "VOCABULARY_MAPPING_UNSUPPORTED" not in witness["reason_codes"]:
        reason_codes.append("WITNESS_REASON_CODES_INCONSISTENT")
    if witness["envelope_comparison"]["plan"]["status"] == "outside" and "OBSERVED_EFFECT_OUTSIDE_PLAN" not in witness["reason_codes"]:
        reason_codes.append("WITNESS_REASON_CODES_INCONSISTENT")
    proof_comparison = witness["envelope_comparison"]["proof"]
    if proof_comparison is not None and proof_comparison["status"] == "outside" and "OBSERVED_EFFECT_OUTSIDE_PROOF" not in witness["reason_codes"]:
        reason_codes.append("WITNESS_REASON_CODES_INCONSISTENT")
    token_comparison = witness["envelope_comparison"]["token"]
    if token_comparison is not None and token_comparison["status"] == "outside" and "OBSERVED_EFFECT_OUTSIDE_TOKEN" not in witness["reason_codes"]:
        reason_codes.append("WITNESS_REASON_CODES_INCONSISTENT")
    if witness["token_basis"] is not None and (
        witness["token_basis"]["verification_decision"] != "allow" or not witness["token_basis"]["verification_verified"]
    ) and "TOKEN_VERIFICATION_FAILED" not in witness["reason_codes"]:
        reason_codes.append("WITNESS_REASON_CODES_INCONSISTENT")
    return canonical_reason_codes(reason_codes)


def verify_witness(
    witness: dict[str, Any],
    *,
    issuer_keys: dict[str, dict[str, str]],
    verification_time: str,
    plan: dict[str, Any] | None = None,
    contract: dict[str, Any] | None = None,
    proof: dict[str, Any] | None = None,
    token: dict[str, Any] | None = None,
    parent_token: dict[str, Any] | None = None,
    raw_trace: Any | None = None,
) -> dict[str, Any]:
    registry = build_registry()
    schema_errors = validate_instance("witness_record.schema.json", witness, registry)
    if schema_errors:
        return verification_result(
            verification_time=verification_time,
            witness=witness,
            verified=False,
            witness_status="unverifiable",
            reason_codes=["WITNESS_SCHEMA_INVALID"],
            notes="; ".join(schema_errors),
        )

    reason_codes: list[str] = []
    shared_secret, issuer_errors = load_issuer_secret(
        issuer_keys,
        witness["issuer"]["issuer_id"],
        witness["issuer"]["key_id"],
    )
    if shared_secret is None:
        if "ISSUER_UNKNOWN" in issuer_errors:
            reason_codes.append("WITNESS_ISSUER_UNKNOWN")
        if "KEY_ID_UNKNOWN" in issuer_errors:
            reason_codes.append("WITNESS_KEY_ID_UNKNOWN")
    elif not verify_witness_protection(witness, shared_secret):
        reason_codes.append("WITNESS_MAC_INVALID")

    if witness["observation_source"]["source_kind"] not in SUPPORTED_SOURCE_KINDS:
        reason_codes.append("OBSERVATION_SOURCE_UNSUPPORTED")

    if witness["redaction"]["redacted_content_digest"] != witness_redaction_digest(witness):
        reason_codes.append("REDACTION_HASH_MISMATCH")
    if raw_trace is not None and witness["redaction"]["raw_trace_digest"] != digest_struct(raw_trace):
        reason_codes.append("REDACTION_HASH_MISMATCH")

    if plan is None or contract is None:
        reason_codes.append("PLAN_LINKAGE_MISMATCH")
    else:
        if validate_plan_contract_alignment(plan, contract):
            reason_codes.append("PLAN_LINKAGE_MISMATCH")
        if witness["request_id"] != plan["request_id"]:
            reason_codes.append("PLAN_LINKAGE_MISMATCH")
        if witness["execution_plan"]["execution_plan_id"] != plan["plan_id"] or witness["execution_plan"]["execution_plan_digest"] != digest_struct(plan):
            reason_codes.append("PLAN_LINKAGE_MISMATCH")
        if witness["subject"]["skill_contract_id"] != contract["contract_id"] or witness["subject"]["contract_digest"] != digest_struct(contract):
            reason_codes.append("PLAN_LINKAGE_MISMATCH")
        if witness["subject"]["component_digest"] != contract["component"]["digest"]:
            reason_codes.append("PLAN_LINKAGE_MISMATCH")
        if witness["subject"]["export_name"] != plan["export_name"]:
            reason_codes.append("PLAN_LINKAGE_MISMATCH")
        if witness["subject"]["input_class_fingerprint"] != plan["input_class_fingerprint"]:
            reason_codes.append("PLAN_LINKAGE_MISMATCH")
        if witness["runtime_binding"] != plan["chosen_runtime"]:
            reason_codes.append("RUNTIME_BINDING_MISMATCH")

    if witness["proof_basis"] is not None:
        if proof is None or plan is None or contract is None:
            reason_codes.append("PROOF_LINKAGE_MISMATCH")
        else:
            if validate_proof_alignment(plan, contract, proof, verification_time):
                reason_codes.append("PROOF_LINKAGE_MISMATCH")
            if (
                witness["proof_basis"]["proof_id"] != proof["proof_id"]
                or witness["proof_basis"]["proof_digest"] != digest_struct(proof)
                or witness["proof_basis"]["proof_status"] != proof["proof_status"]
            ):
                reason_codes.append("PROOF_LINKAGE_MISMATCH")
            proof_basis_source_kind = witness["proof_basis"].get("proof_source_kind")
            if proof_basis_source_kind is not None and proof_basis_source_kind != proof_source_kind(proof):
                reason_codes.append("PROOF_LINKAGE_MISMATCH")
            if stable_unique_host_exact_bindings(witness.get("host_exact_bindings", [])) != stable_unique_host_exact_bindings(
                proof.get("host_exact_bindings", [])
            ):
                reason_codes.append("PROOF_LINKAGE_MISMATCH")

    if witness["token_basis"] is not None:
        if token is None or plan is None or contract is None:
            reason_codes.append("TOKEN_LINKAGE_MISMATCH")
        else:
            verification = verify_token(
                token,
                issuer_keys=issuer_keys,
                verification_time=witness["issued_at"],
                expected_holder_id=token["holder_binding"]["holder_id"],
                expected_audiences=token["audience_binding"]["audiences"],
                expected_resources=token["audience_binding"]["resources"],
                expected_runtime_guarantee_id=plan["chosen_runtime"]["runtime_guarantee_id"],
                expected_call_chain_links=token["call_chain"]["links"],
                plan=plan,
                contract=contract,
                proof=proof,
                parent_token=parent_token,
                check_replay=False,
            )
            if verification["decision"] != "allow" or not verification["verified"]:
                reason_codes.append("TOKEN_VERIFICATION_FAILED")
            token_basis = witness["token_basis"]
            if (
                token_basis["token_id"] != token["token_id"]
                or token_basis["token_digest"] != digest_struct(token)
                or token_basis["issuance_basis"] != token["issuance_basis"]
                or token_basis.get("proof_source_kind") != token.get("proof_source_kind")
                or token_basis["parent_token_id"] != (token["parent_token"]["token_id"] if token["parent_token"] is not None else None)
                or token_basis["parent_token_digest"] != (token["parent_token"]["token_digest"] if token["parent_token"] is not None else None)
                or token_basis["verification_result_digest"] != digest_struct(verification)
                or token_basis["verification_decision"] != verification["decision"]
                or token_basis["verification_verified"] != verification["verified"]
                or token_basis["verification_reason_codes"] != verification["reason_codes"]
            ):
                reason_codes.append("TOKEN_LINKAGE_MISMATCH")
            if stable_unique_host_exact_bindings(witness.get("host_exact_bindings", [])) != stable_unique_host_exact_bindings(
                token.get("host_exact_bindings", [])
            ):
                reason_codes.append("TOKEN_LINKAGE_MISMATCH")
            if stable_unique_host_exact_bindings(witness.get("host_exact_bindings", [])) and not host_exact_bindings_match_authority(
                stable_unique_host_exact_bindings(witness.get("host_exact_bindings", [])),
                token["granted_authority"],
            ):
                reason_codes.append("TOKEN_LINKAGE_MISMATCH")
            if witness["audience_binding"] != token["audience_binding"]:
                reason_codes.append("AUDIENCE_BINDING_MISMATCH")
            if witness["holder_binding"] != token["holder_binding"]:
                reason_codes.append("HOLDER_BINDING_MISMATCH")
            if (
                witness["call_chain"] is None
                or witness["call_chain"]["chain_id"] != token["call_chain"]["chain_id"]
                or witness["call_chain"]["chain_digest"] != token["call_chain"]["chain_digest"]
            ):
                reason_codes.append("CALL_CHAIN_MISMATCH")
            if plan is not None and witness["runtime_binding"] != plan["chosen_runtime"]:
                reason_codes.append("RUNTIME_BINDING_MISMATCH")

    expected_status = compare_to_required_basis(witness)
    if witness["witness_status"] != expected_status:
        reason_codes.append("WITNESS_STATUS_INCONSISTENT")
    reason_codes.extend(minimal_consistency_reason_codes(witness))

    verified = not reason_codes
    witness_status = witness["witness_status"] if verified else "unverifiable"
    result = verification_result(
        verification_time=verification_time,
        witness=witness,
        verified=verified,
        witness_status=witness_status,
        reason_codes=reason_codes,
    )
    require_valid("witness_verification_result.schema.json", result, registry, "witness verification result")
    return result


def effect_details_available(summary: dict[str, Any]) -> bool:
    return not summary["effects_redacted"]


def claim_coverage_entries(witness: dict[str, Any], selectors: list[str] | None = None) -> list[dict[str, Any]]:
    if selectors is None:
        return witness["observation_coverage"]["families"]
    selector_set = set(selectors)
    return [
        entry
        for entry in witness["observation_coverage"]["families"]
        if coverage_entry_selector(entry) in selector_set
    ]


def claim_coverage_state(witness: dict[str, Any], selectors: list[str] | None = None) -> str:
    matching = claim_coverage_entries(witness, selectors)
    if selectors is None:
        if witness["observation_coverage"]["overall_status"] != "complete" or witness["unmapped_observations"]:
            return "partial"
        return "complete" if all(entry["supports_absence_claims"] for entry in matching) else "partial"

    if not matching:
        return "unsupported"
    if any(
        item["family"] in set(selectors)
        for item in witness["unmapped_observations"]
    ):
        return "unsupported"
    if any(entry["mapping_status"] == MAPPING_UNSUPPORTED for entry in matching):
        return "unsupported"
    if all(
        entry["status"] == "complete" and entry["supports_absence_claims"]
        for entry in matching
    ):
        return "complete"
    return "partial"


def coverage_complete_for_claim(witness: dict[str, Any], selectors: list[str] | None = None) -> bool:
    return claim_coverage_state(witness, selectors) == "complete"


def network_audience_covers(allow: dict[str, Any], observed: dict[str, Any]) -> bool:
    host_ok = allow["host"] == "*" or allow["host"] == observed["host"]
    schemes = allow.get("schemes") or ["*"]
    scheme_ok = "*" in schemes or any(scheme in schemes for scheme in observed.get("schemes", []))
    ports = allow.get("ports") or ["*"]
    port_ok = "*" in ports or any(port in ports for port in observed.get("ports", []))
    methods = allow.get("methods") or ["*"]
    method_ok = "*" in methods or any(method in methods for method in observed.get("methods", []))
    prefixes = allow.get("path_prefixes") or ["/"]
    observed_prefixes = observed.get("path_prefixes") or ["/"]
    path_ok = all(any(path_pattern_covers(prefix, observed_prefix) for prefix in prefixes) for observed_prefix in observed_prefixes)
    return host_ok and scheme_ok and port_ok and method_ok and path_ok


def observed_delegation_hops(witness: dict[str, Any]) -> int | None:
    if witness["token_basis"] is None:
        return 0
    if witness["call_chain"] is None or witness["call_chain"]["links"] is None:
        return None
    return sum(1 for link in witness["call_chain"]["links"] if "token" in link)


def claim_result(claim: dict[str, Any], status: str, reason_codes: list[str], details: dict[str, Any]) -> dict[str, Any]:
    return {
        "claim": deepcopy(claim),
        "status": status,
        "reason_codes": canonical_reason_codes(reason_codes),
        "details": details,
    }


def evaluate_family_scope_claim(
    witness: dict[str, Any],
    claim: dict[str, Any],
    *,
    family: str,
    scope_key: str,
) -> dict[str, Any]:
    if not effect_details_available(witness["actual_exercised_authority"]):
        return claim_result(claim, "not_provable", ["REDACTION_PREVENTS_CLAIM_VERIFICATION"], {})
    coverage_state = claim_coverage_state(witness, [family])
    if coverage_state == "unsupported":
        return claim_result(
            claim,
            "unsupported",
            ["NEGATIVE_CLAIM_UNSUPPORTED_FOR_FAMILY"],
            {"family": family},
        )
    if coverage_state != "complete":
        return claim_result(claim, "not_provable", ["CLAIM_NOT_PROVABLE_FROM_COVERAGE"], {"family": family})

    violating: list[dict[str, Any]] = []
    for effect in witness["actual_exercised_authority"]["effects"]:
        if effect_selector(effect) != family:
            continue
        allowed_effect = {
            "family": family,
            "scope": deepcopy(claim[scope_key]),
            "cardinality": deepcopy(effect.get("cardinality")),
        }
        if not effect_covers(allowed_effect, effect):
            violating.append(effect)
    if violating:
        return claim_result(claim, "violated", [], {"violating_effects": stable_unique_dicts(violating)})
    return claim_result(claim, "satisfied", [], {"family": family})


def evaluate_claim(witness: dict[str, Any], claim: dict[str, Any]) -> dict[str, Any]:
    claim_type = claim["claim_type"]

    if claim_type == "no_authority_use_outside_plan":
        if witness["envelope_comparison"]["plan"]["status"] == "outside":
            return claim_result(claim, "violated", ["OBSERVED_EFFECT_OUTSIDE_PLAN"], {"outside_effects": witness["envelope_comparison"]["plan"]["outside_effects"]})
        if not coverage_complete_for_claim(witness):
            return claim_result(claim, "not_provable", ["CLAIM_NOT_PROVABLE_FROM_COVERAGE"], {})
        return claim_result(claim, "satisfied", [], {})

    if claim_type == "no_authority_use_outside_proof":
        if witness["proof_basis"] is None or witness["envelope_comparison"]["proof"] is None:
            return claim_result(claim, "unsupported", ["CLAIM_UNSUPPORTED"], {})
        if witness["envelope_comparison"]["proof"]["status"] == "outside":
            return claim_result(claim, "violated", ["OBSERVED_EFFECT_OUTSIDE_PROOF"], {"outside_effects": witness["envelope_comparison"]["proof"]["outside_effects"]})
        if not coverage_complete_for_claim(witness):
            return claim_result(claim, "not_provable", ["CLAIM_NOT_PROVABLE_FROM_COVERAGE"], {})
        return claim_result(claim, "satisfied", [], {})

    if claim_type == "no_authority_use_outside_token":
        if witness["token_basis"] is None or witness["envelope_comparison"]["token"] is None:
            return claim_result(claim, "unsupported", ["CLAIM_UNSUPPORTED"], {})
        if witness["envelope_comparison"]["token"]["status"] == "outside":
            return claim_result(claim, "violated", ["OBSERVED_EFFECT_OUTSIDE_TOKEN"], {"outside_effects": witness["envelope_comparison"]["token"]["outside_effects"]})
        if not coverage_complete_for_claim(witness):
            return claim_result(claim, "not_provable", ["CLAIM_NOT_PROVABLE_FROM_COVERAGE"], {})
        return claim_result(claim, "satisfied", [], {})

    if claim_type == "no_http_request_outside_scope":
        return evaluate_family_scope_claim(witness, claim, family="http-request", scope_key="http_request_scope")

    if claim_type == "no_read_resource_outside_scope":
        return evaluate_family_scope_claim(witness, claim, family="read-resource", scope_key="read_resource_scope")

    if claim_type == "no_invoke_skill_outside_scope":
        return evaluate_family_scope_claim(witness, claim, family="invoke-skill", scope_key="invoke_skill_scope")

    if claim_type == "no_emit_evidence_outside_scope":
        return evaluate_family_scope_claim(witness, claim, family="emit-evidence", scope_key="emit_evidence_scope")

    if claim_type == "no_log_write_outside_scope":
        return evaluate_family_scope_claim(witness, claim, family="log-write", scope_key="log_write_scope")

    if claim_type == "no_filesystem_writes_outside_prefixes":
        if not effect_details_available(witness["actual_exercised_authority"]):
            return claim_result(claim, "not_provable", ["REDACTION_PREVENTS_CLAIM_VERIFICATION"], {})
        if not coverage_complete_for_claim(witness, ["fs.write"]):
            return claim_result(claim, "not_provable", ["CLAIM_NOT_PROVABLE_FROM_COVERAGE"], {})
        violating: list[dict[str, Any]] = []
        for effect in witness["actual_exercised_authority"]["effects"]:
            if effect_selector(effect) != "fs.write":
                continue
            for path in effect["scope"]["paths"]:
                if not any(path_pattern_covers(prefix, path) for prefix in claim["paths"]):
                    violating.append(effect)
                    break
        if violating:
            return claim_result(claim, "violated", [], {"violating_effects": stable_unique_dicts(violating)})
        return claim_result(claim, "satisfied", [], {})

    if claim_type == "no_network_egress_except_allowlist":
        if not effect_details_available(witness["actual_exercised_authority"]):
            return claim_result(claim, "not_provable", ["REDACTION_PREVENTS_CLAIM_VERIFICATION"], {})
        if not coverage_complete_for_claim(witness, ["net.connect"]):
            return claim_result(claim, "not_provable", ["CLAIM_NOT_PROVABLE_FROM_COVERAGE"], {})
        violating: list[dict[str, Any]] = []
        for effect in witness["actual_exercised_authority"]["effects"]:
            if effect_selector(effect) != "net.connect":
                continue
            if not all(
                any(network_audience_covers(allow, audience) for allow in claim["network_allowlist"])
                for audience in effect["scope"]["audiences"]
            ):
                violating.append(effect)
        if violating:
            return claim_result(claim, "violated", [], {"violating_effects": stable_unique_dicts(violating)})
        return claim_result(claim, "satisfied", [], {})

    if claim_type == "no_delegation_beyond_hops":
        hops = observed_delegation_hops(witness)
        if hops is None:
            return claim_result(claim, "not_provable", ["REDACTION_PREVENTS_CLAIM_VERIFICATION"], {})
        if hops > claim["max_hops"]:
            return claim_result(claim, "violated", [], {"observed_hops": hops})
        return claim_result(claim, "satisfied", [], {"observed_hops": hops})

    if claim_type == "no_blocked_attempts_of_classes":
        if not witness["blocked_attempted_authority"]["observable"]:
            return claim_result(claim, "not_provable", ["CLAIM_NOT_PROVABLE_FROM_COVERAGE"], {})
        if witness["blocked_attempted_authority"]["effects_redacted"]:
            return claim_result(claim, "not_provable", ["REDACTION_PREVENTS_CLAIM_VERIFICATION"], {})
        blocked_classes = set(witness["blocked_attempted_authority"]["effect_classes"])
        requested_classes = set(claim["effect_classes"])
        if blocked_classes.intersection(requested_classes):
            return claim_result(
                claim,
                "violated",
                [],
                {
                    "blocked_effects": [
                        item
                        for item in witness["blocked_attempted_authority"]["effects"]
                        if effect_selector(item["effect"]) in requested_classes
                    ]
                },
            )
        return claim_result(claim, "satisfied", [], {})

    if claim_type == "no_blocked_attempts_of_families":
        if not witness["blocked_attempted_authority"]["observable"]:
            return claim_result(claim, "not_provable", ["CLAIM_NOT_PROVABLE_FROM_COVERAGE"], {})
        if witness["blocked_attempted_authority"]["effects_redacted"]:
            return claim_result(claim, "not_provable", ["REDACTION_PREVENTS_CLAIM_VERIFICATION"], {})
        blocked_families = set(witness["blocked_attempted_authority"]["effect_classes"])
        requested_families = set(claim["families"])
        if blocked_families.intersection(requested_families):
            return claim_result(
                claim,
                "violated",
                [],
                {
                    "blocked_effects": [
                        item
                        for item in witness["blocked_attempted_authority"]["effects"]
                        if effect_selector(item["effect"]) in requested_families
                    ]
                },
            )
        return claim_result(claim, "satisfied", [], {})

    return claim_result(claim, "unsupported", ["CLAIM_UNSUPPORTED"], {})


def verify_claim(
    witness: dict[str, Any],
    claim: dict[str, Any],
    *,
    issuer_keys: dict[str, dict[str, str]],
    verification_time: str,
    plan: dict[str, Any] | None = None,
    contract: dict[str, Any] | None = None,
    proof: dict[str, Any] | None = None,
    token: dict[str, Any] | None = None,
    parent_token: dict[str, Any] | None = None,
    raw_trace: Any | None = None,
) -> dict[str, Any]:
    registry = build_registry()
    require_valid("witness_record.schema.json", witness, registry, "witness record")
    require_valid("witness_verification_result.schema.json", verification_result(
        verification_time=verification_time,
        witness=witness,
        verified=True,
        witness_status=witness["witness_status"],
        reason_codes=[],
        claim_evaluation=claim_result(claim, "satisfied", [], {}),
    ), registry, "witness verification result skeleton")

    base = verify_witness(
        witness,
        issuer_keys=issuer_keys,
        verification_time=verification_time,
        plan=plan,
        contract=contract,
        proof=proof,
        token=token,
        parent_token=parent_token,
        raw_trace=raw_trace,
    )
    if not base["verified"]:
        base["claim_evaluation"] = claim_result(
            claim,
            "not_provable",
            base["reason_codes"],
            {},
        )
        require_valid("witness_verification_result.schema.json", base, registry, "witness verification result")
        return base

    claim_evaluation = evaluate_claim(witness, claim)
    result = verification_result(
        verification_time=verification_time,
        witness=witness,
        verified=True,
        witness_status=witness["witness_status"],
        reason_codes=[],
        claim_evaluation=claim_evaluation,
    )
    require_valid("witness_verification_result.schema.json", result, registry, "witness verification result")
    return result
