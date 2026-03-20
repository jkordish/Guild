from __future__ import annotations

from copy import deepcopy
from itertools import combinations
from pathlib import Path
from typing import Any
from urllib.parse import urlparse

from admission_core import (
    build_registry,
    canonical_json,
    digest_struct,
    effect_covers,
    intersect_effect,
    normalize_effect,
    require_valid,
    runtime_can_enforce_effect,
    stable_unique_dicts,
    stable_unique_strings,
)


ALGORITHM_ID = "urn:guild:minimizer:draft-v1:example-bounded"
ALGORITHM_VERSION = "1.0.0"
SHRINK_MODEL_VERSION = "1.0.0"
HARNESS_ID = "urn:guild:harness:draft-v1:example-bounded"
HARNESS_VERSION = "1.0.0"
PROOF_VERSION = "1.0.0"


def build_minimization_proof(
    plan: dict[str, Any],
    contract: dict[str, Any],
    request: dict[str, Any],
    runtime: dict[str, Any],
    invocation_input: dict[str, Any],
    comparator_profile: dict[str, Any],
    *,
    created_at: str,
    expires_at: str | None = None,
    cache_dir: str | Path | None = None,
    max_candidate_plans: int = 128,
) -> dict[str, Any]:
    registry = build_registry()
    require_valid("execution_plan.schema.json", plan, registry, "execution plan")
    require_valid("skill_contract.schema.json", contract, registry, "skill contract")
    require_valid("admission_request.schema.json", request, registry, "admission request")
    require_valid("runtime_guarantee.schema.json", runtime, registry, "runtime guarantee")
    require_valid("comparator_profile.schema.json", comparator_profile, registry, "comparator profile")

    plan_validation_codes = validate_plan_alignment(plan, contract, request, runtime)
    if plan_validation_codes:
        return build_failure_proof(
            plan,
            contract,
            runtime,
            invocation_input,
            comparator_profile,
            created_at=created_at,
            expires_at=expires_at,
            reason_codes=plan_validation_codes,
            cache_status="bypassed" if cache_dir else "miss",
            cache_bypass_reason_codes=plan_validation_codes if cache_dir else [],
            search_limits={"max_candidate_plans": max_candidate_plans, "truncated": False},
        )

    cache_key_material = build_cache_key_material(
        plan,
        contract,
        invocation_input,
        runtime,
        comparator_profile,
    )
    cache_key_digest = digest_struct(cache_key_material)

    if cache_dir is not None:
        cache_dir_path = Path(cache_dir)
        cache_dir_path.mkdir(parents=True, exist_ok=True)
        cached = load_cached_proof(cache_dir_path, cache_key_digest)
        if cached is not None:
            cached_copy = deepcopy(cached)
            cached_copy["cache"]["status"] = "hit"
            cached_copy["cache"]["reused_proof_id"] = cached["proof_id"]
            cached_copy["minimization_reason_codes"] = stable_unique_strings(
                sorted(
                    [
                        code
                        for code in cached_copy["minimization_reason_codes"]
                        if code != "PROOF_CACHE_MISS"
                    ]
                    + ["PROOF_CACHE_HIT"]
                )
            )
            return cached_copy

    cache_status = "miss" if cache_dir else "bypassed"
    cache_reason_codes = [] if cache_dir else ["PROOF_CACHE_BYPASSED"]

    baseline_plan = deepcopy(plan["granted_authority"])
    baseline_run = run_example_harness(contract, invocation_input, baseline_plan)
    if baseline_run["status"] != "success":
        return build_failure_proof(
            plan,
            contract,
            runtime,
            invocation_input,
            comparator_profile,
            created_at=created_at,
            expires_at=expires_at,
            reason_codes=[
                "SHADOW_HARNESS_UNAVAILABLE"
                if baseline_run["error_code"] == "unsupported-example-harness"
                else "SHADOW_EXECUTION_FAILED"
            ],
            cache_status=cache_status,
            cache_bypass_reason_codes=cache_reason_codes,
            search_limits={"max_candidate_plans": max_candidate_plans, "truncated": False},
        )

    attempts: list[dict[str, Any]] = []
    top_level_reason_codes = list(cache_reason_codes)
    if cache_status == "miss":
        top_level_reason_codes.append("PROOF_CACHE_MISS")

    discrete_result = exhaustive_discrete_search(
        baseline_plan,
        baseline_run,
        contract,
        invocation_input,
        comparator_profile,
        max_candidate_plans=max_candidate_plans,
    )
    attempts.extend(discrete_result["attempts"])
    top_level_reason_codes.extend(discrete_result["reason_codes"])

    current_plan = discrete_result["chosen_plan"]
    current_run = discrete_result["chosen_run"]

    shrink_result = bounded_scope_shrink(
        current_plan,
        current_run,
        contract,
        runtime,
        invocation_input,
        comparator_profile,
        next_trial_index=len(attempts) + 1,
    )
    attempts.extend(shrink_result["attempts"])
    top_level_reason_codes.extend(shrink_result["reason_codes"])
    current_plan = shrink_result["chosen_plan"]
    current_run = shrink_result["chosen_run"]

    retention_result = explain_retained_authorities(
        current_plan,
        current_run,
        contract,
        invocation_input,
        comparator_profile,
        next_trial_index=len(attempts) + 1,
    )
    attempts.extend(retention_result["attempts"])
    top_level_reason_codes.extend(
        code
        for item in retention_result["retained_authorities"]
        for code in item["reason_codes"]
    )

    proof_status = determine_proof_status(
        baseline_plan=baseline_plan,
        final_plan=current_plan,
        discrete_result=discrete_result,
        shrink_result=shrink_result,
        top_level_reason_codes=top_level_reason_codes,
    )

    proof = {
        "kind": "guild.proof_record",
        "version": PROOF_VERSION,
        "proof_id": f"{plan['plan_id']}:m5-proof",
        "request_id": plan["request_id"],
        "execution_plan_id": plan["plan_id"],
        "execution_plan_digest": digest_struct(plan),
        "skill_contract_id": contract["contract_id"],
        "contract_digest": digest_struct(contract),
        "component_digest": deepcopy(contract["component"]["digest"]),
        "export_name": plan["export_name"],
        "input_class_fingerprint": deepcopy(plan["input_class_fingerprint"]),
        "invocation_input_digest": digest_struct(invocation_input),
        "chosen_runtime": deepcopy(plan["chosen_runtime"]),
        "proof_method": "counterfactual_shadow_execution",
        "comparator": extract_comparator_spec(comparator_profile),
        "comparator_digest": digest_struct(comparator_profile),
        "minimization_algorithm": {
            "id": ALGORITHM_ID,
            "version": ALGORITHM_VERSION,
            "shrink_model_version": SHRINK_MODEL_VERSION,
            "harness_id": HARNESS_ID,
            "harness_version": HARNESS_VERSION,
        },
        "search_model": {
            "search_domain": "draft-v1 example-bounded minimization harness",
            "discrete_grant_search": {
                "strategy": "exhaustive_powerset",
                "exact": True,
                "candidate_plans_evaluated": discrete_result["candidate_plans_evaluated"],
            },
            "scope_shrinkers": shrink_result["scope_shrinker_stats"],
            "search_limits": {
                "max_candidate_plans": max_candidate_plans,
                "truncated": discrete_result["truncated"],
            },
            "assumptions": [
                "Only bundled draft-v1 examples have a real M5 harness in this milestone.",
                "Comparator and invocation fixtures must remain deterministic and controlled.",
                "Scope shrinkers are bounded by observed-effect projections, not a runtime-general search space.",
            ],
        },
        "baseline_authority_plan": deepcopy(baseline_plan),
        "proven_authority_plan": deepcopy(current_plan),
        "candidate_attempts": attempts,
        "retained_authorities": retention_result["retained_authorities"],
        "proof_status": proof_status,
        "minimization_reason_codes": stable_unique_strings(sorted(top_level_reason_codes)),
        "observed_effect_summary": {
            "effects": stable_unique_dicts([normalize_effect(effect) for effect in current_run["observed_effects"]]),
            "output_digest": current_run["output_digest"],
            "trace_digest": digest_struct(
                {
                    "output": current_run["output"],
                    "observed_effects": current_run["observed_effects"],
                }
            ),
        },
        "cache": {
            "status": cache_status,
            "cache_key_digest": cache_key_digest,
            "reused_proof_id": None,
            "bypass_reason_codes": cache_reason_codes,
            "key_material": cache_key_material,
        },
        "created_at": created_at,
        "expires_at": expires_at,
        "notes": "M5 draft proof record. This harness is intentionally example-bounded and does not claim runtime-general minimization.",
    }

    require_valid("proof_record.schema.json", proof, registry, "proof record")

    if cache_dir is not None and proof["proof_status"] != "not_proven":
        store_cached_proof(Path(cache_dir), cache_key_digest, proof)

    return proof


def build_failure_proof(
    plan: dict[str, Any],
    contract: dict[str, Any],
    runtime: dict[str, Any],
    invocation_input: dict[str, Any],
    comparator_profile: dict[str, Any],
    *,
    created_at: str,
    expires_at: str | None,
    reason_codes: list[str],
    cache_status: str,
    cache_bypass_reason_codes: list[str],
    search_limits: dict[str, Any],
) -> dict[str, Any]:
    baseline_plan = deepcopy(plan["granted_authority"])
    cache_key_material = build_cache_key_material(
        plan,
        contract,
        invocation_input,
        runtime,
        comparator_profile,
    )
    return {
        "kind": "guild.proof_record",
        "version": PROOF_VERSION,
        "proof_id": f"{plan['plan_id']}:m5-proof",
        "request_id": plan["request_id"],
        "execution_plan_id": plan["plan_id"],
        "execution_plan_digest": digest_struct(plan),
        "skill_contract_id": contract["contract_id"],
        "contract_digest": digest_struct(contract),
        "component_digest": deepcopy(contract["component"]["digest"]),
        "export_name": plan["export_name"],
        "input_class_fingerprint": deepcopy(plan["input_class_fingerprint"]),
        "invocation_input_digest": digest_struct(invocation_input),
        "chosen_runtime": deepcopy(plan["chosen_runtime"]) if plan.get("chosen_runtime") is not None else {
            "runtime_guarantee_id": runtime["runtime_guarantee_id"],
            "runtime_guarantee_digest": digest_struct(runtime),
            "runtime": deepcopy(runtime["runtime"]),
        },
        "proof_method": "counterfactual_shadow_execution",
        "comparator": extract_comparator_spec(comparator_profile),
        "comparator_digest": digest_struct(comparator_profile),
        "minimization_algorithm": {
            "id": ALGORITHM_ID,
            "version": ALGORITHM_VERSION,
            "shrink_model_version": SHRINK_MODEL_VERSION,
            "harness_id": HARNESS_ID,
            "harness_version": HARNESS_VERSION,
        },
        "search_model": {
            "search_domain": "draft-v1 example-bounded minimization harness",
            "discrete_grant_search": {
                "strategy": "exhaustive_powerset",
                "exact": True,
                "candidate_plans_evaluated": 0,
            },
            "scope_shrinkers": [
                {
                    "scope_kind": "filesystem",
                    "strategy": "observed_effect_projection",
                    "bounded": True,
                    "attempts": 0,
                    "accepted": 0,
                },
                {
                    "scope_kind": "network",
                    "strategy": "observed_effect_projection",
                    "bounded": True,
                    "attempts": 0,
                    "accepted": 0,
                },
                {
                    "scope_kind": "delegation",
                    "strategy": "observed_effect_projection",
                    "bounded": True,
                    "attempts": 0,
                    "accepted": 0,
                },
            ],
            "search_limits": search_limits,
            "assumptions": [
                "Proof generation failed before any candidate minimization trial could complete."
            ],
        },
        "baseline_authority_plan": deepcopy(baseline_plan),
        "proven_authority_plan": deepcopy(baseline_plan),
        "candidate_attempts": [],
        "retained_authorities": [
            {
                "grant": deepcopy(grant),
                "reason_codes": ["PLAN_NOT_ADMISSIBLE"] if "PLAN_NOT_ADMISSIBLE" in reason_codes else [],
                "details": "No retained-authority explanation was produced because M5 failed before a valid comparison run.",
            }
            for grant in baseline_plan.get("grants", [])
        ],
        "proof_status": "not_proven",
        "minimization_reason_codes": stable_unique_strings(sorted(reason_codes + cache_bypass_reason_codes)),
        "observed_effect_summary": {
            "effects": [],
            "output_digest": None,
            "trace_digest": None,
        },
        "cache": {
            "status": cache_status,
            "cache_key_digest": digest_struct(cache_key_material),
            "reused_proof_id": None,
            "bypass_reason_codes": cache_bypass_reason_codes,
            "key_material": cache_key_material,
        },
        "created_at": created_at,
        "expires_at": expires_at,
        "notes": "M5 failed closed before producing a valid minimization proof.",
    }


def validate_plan_alignment(
    plan: dict[str, Any],
    contract: dict[str, Any],
    request: dict[str, Any],
    runtime: dict[str, Any],
) -> list[str]:
    if plan["decision"] == "refuse" or plan.get("chosen_runtime") is None:
        return ["PLAN_NOT_ADMISSIBLE"]

    codes: list[str] = []
    if plan["contract_id"] != contract["contract_id"]:
        codes.append("PLAN_NOT_ADMISSIBLE")
    if plan["request_id"] != request["request_id"]:
        codes.append("PLAN_NOT_ADMISSIBLE")
    if plan["component_digest"] != contract["component"]["digest"]:
        codes.append("PLAN_NOT_ADMISSIBLE")
    if plan["chosen_runtime"]["runtime_guarantee_id"] != runtime["runtime_guarantee_id"]:
        codes.append("PLAN_RUNTIME_MISMATCH")
    if plan["chosen_runtime"]["runtime_guarantee_digest"] != digest_struct(runtime):
        codes.append("PLAN_RUNTIME_MISMATCH")
    return stable_unique_strings(sorted(codes))


def build_cache_key_material(
    plan: dict[str, Any],
    contract: dict[str, Any],
    invocation_input: dict[str, Any],
    runtime: dict[str, Any],
    comparator_profile: dict[str, Any],
) -> dict[str, Any]:
    return {
        "execution_plan_digest": digest_struct(plan),
        "contract_digest": digest_struct(contract),
        "component_digest": deepcopy(contract["component"]["digest"]),
        "export_name": plan["export_name"],
        "input_class_fingerprint": deepcopy(plan["input_class_fingerprint"]),
        "invocation_input_digest": digest_struct(invocation_input),
        "runtime_guarantee_digest": digest_struct(runtime),
        "comparator_digest": digest_struct(comparator_profile),
        "minimization_algorithm_version": ALGORITHM_VERSION,
        "shrink_model_version": SHRINK_MODEL_VERSION,
        "harness_id": HARNESS_ID,
        "harness_version": HARNESS_VERSION,
    }


def load_cached_proof(cache_dir: Path, cache_key_digest: dict[str, str]) -> dict[str, Any] | None:
    path = cache_dir / f"{cache_key_digest['value']}.json"
    if not path.exists():
        return None
    return __import__("json").loads(path.read_text())


def store_cached_proof(cache_dir: Path, cache_key_digest: dict[str, str], proof: dict[str, Any]) -> None:
    path = cache_dir / f"{cache_key_digest['value']}.json"
    path.write_text(__import__("json").dumps(proof, indent=2, sort_keys=True) + "\n")


def extract_comparator_spec(comparator_profile: dict[str, Any]) -> dict[str, Any]:
    return {
        key: deepcopy(comparator_profile[key])
        for key in (
            "comparator_id",
            "version",
            "comparator_kind",
            "reference",
            "canonicalization",
            "checker_ref",
            "output_pointer",
            "inputs",
            "assumptions",
        )
        if key in comparator_profile
    }


def exhaustive_discrete_search(
    baseline_plan: dict[str, Any],
    baseline_run: dict[str, Any],
    contract: dict[str, Any],
    invocation_input: dict[str, Any],
    comparator_profile: dict[str, Any],
    *,
    max_candidate_plans: int,
) -> dict[str, Any]:
    baseline_grants = [deepcopy(grant) for grant in baseline_plan.get("grants", [])]
    attempts: list[dict[str, Any]] = []
    acceptable_candidates: list[tuple[dict[str, Any], dict[str, Any]]] = [(deepcopy(baseline_plan), baseline_run)]
    candidate_plans_evaluated = 0
    truncated = False
    reason_codes: list[str] = []

    if not baseline_grants:
        return {
            "chosen_plan": deepcopy(baseline_plan),
            "chosen_run": baseline_run,
            "attempts": [],
            "candidate_plans_evaluated": 0,
            "truncated": False,
            "reason_codes": [],
        }

    for remove_count in range(1, len(baseline_grants) + 1):
        for removed_indexes in combinations(range(len(baseline_grants)), remove_count):
            if candidate_plans_evaluated >= max_candidate_plans:
                truncated = True
                reason_codes.append("SEARCH_BUDGET_EXCEEDED")
                break

            kept_grants = [
                deepcopy(grant)
                for index, grant in enumerate(baseline_grants)
                if index not in removed_indexes
            ]
            removed_grants = [
                deepcopy(baseline_grants[index])
                for index in removed_indexes
            ]
            candidate_plan = authority_plan_with_grants(
                baseline_plan,
                kept_grants,
                f"candidate:remove:{remove_count}:{candidate_plans_evaluated + 1}",
            )
            candidate_run = run_example_harness(contract, invocation_input, candidate_plan)
            candidate_plans_evaluated += 1
            comparison = compare_runs(
                baseline_run,
                candidate_run,
                candidate_plan,
                comparator_profile,
            )

            accepted = comparison["status"] == "match"
            if accepted:
                acceptable_candidates.append((candidate_plan, candidate_run))

            attempts.append(
                build_attempt(
                    trial_id=f"{baseline_plan['plan_id']}:trial:remove:{candidate_plans_evaluated}",
                    candidate_plan=candidate_plan,
                    change_kind="remove_grant" if len(removed_grants) == 1 else "remove_grant_set",
                    target_effect_class=removed_grants[0]["effect_class"] if len(removed_grants) == 1 else None,
                    target_scope_kind=removed_grants[0]["scope"]["kind"] if len(removed_grants) == 1 else None,
                    removed_grants=removed_grants,
                    shrink_from=None,
                    shrink_to=None,
                    trial_result=trial_result_for(candidate_run, comparison),
                    reason_codes=reason_codes_for_attempt(candidate_run, comparison),
                    shadow_run=candidate_run,
                    comparison=comparison,
                )
            )
        if truncated:
            break

    chosen_plan, chosen_run = min(
        acceptable_candidates,
        key=lambda item: (len(item[0].get("grants", [])), canonical_json(item[0])),
    )
    return {
        "chosen_plan": deepcopy(chosen_plan),
        "chosen_run": chosen_run,
        "attempts": attempts,
        "candidate_plans_evaluated": candidate_plans_evaluated,
        "truncated": truncated,
        "reason_codes": stable_unique_strings(sorted(reason_codes)),
    }


def bounded_scope_shrink(
    starting_plan: dict[str, Any],
    starting_run: dict[str, Any],
    contract: dict[str, Any],
    runtime: dict[str, Any],
    invocation_input: dict[str, Any],
    comparator_profile: dict[str, Any],
    *,
    next_trial_index: int,
) -> dict[str, Any]:
    if not comparator_profile.get("inputs", {}).get("allow_scope_shrinkers", False):
        return {
            "chosen_plan": deepcopy(starting_plan),
            "chosen_run": starting_run,
            "attempts": [],
            "reason_codes": [],
            "scope_shrinker_stats": [
                {
                    "scope_kind": "filesystem",
                    "strategy": "observed_effect_projection",
                    "bounded": True,
                    "attempts": 0,
                    "accepted": 0,
                },
                {
                    "scope_kind": "network",
                    "strategy": "observed_effect_projection",
                    "bounded": True,
                    "attempts": 0,
                    "accepted": 0,
                },
                {
                    "scope_kind": "delegation",
                    "strategy": "observed_effect_projection",
                    "bounded": True,
                    "attempts": 0,
                    "accepted": 0,
                },
            ],
        }

    attempts: list[dict[str, Any]] = []
    reason_codes: list[str] = []
    current_plan = deepcopy(starting_plan)
    current_run = starting_run
    trial_index = next_trial_index

    scope_stats = {
        "filesystem": {"attempts": 0, "accepted": 0},
        "network": {"attempts": 0, "accepted": 0},
        "delegation": {"attempts": 0, "accepted": 0},
    }

    changed = True
    while changed:
        changed = False
        for index, grant in enumerate(list(current_plan.get("grants", []))):
            shrunken = shrink_grant_from_observed(grant, current_run["observed_effects"])
            scope_kind = grant["scope"]["kind"]
            if shrunken is None:
                if scope_kind in scope_stats:
                    reason_codes.append("SCOPE_SHRINK_UNSUPPORTED")
                continue
            if canonical_json(normalize_effect(shrunken)) == canonical_json(normalize_effect(grant)):
                continue
            if not runtime_can_enforce_effect(shrunken, runtime):
                reason_codes.append("RUNTIME_CANNOT_ENFORCE_SHRUNK_SCOPE")
                scope_stats[scope_kind]["attempts"] += 1
                attempts.append(
                    build_attempt(
                        trial_id=f"{current_plan['plan_id']}:trial:shrink:{trial_index}",
                        candidate_plan=deepcopy(current_plan),
                        change_kind="shrink_scope",
                        target_effect_class=grant["effect_class"],
                        target_scope_kind=scope_kind,
                        removed_grants=[],
                        shrink_from=grant,
                        shrink_to=shrunken,
                        trial_result="not_proven",
                        reason_codes=["RUNTIME_CANNOT_ENFORCE_SHRUNK_SCOPE"],
                        shadow_run={
                            "status": "error",
                            "error_code": "runtime-cannot-enforce-shrunk-scope",
                            "observed_effects": [],
                            "output": None,
                            "output_digest": None,
                        },
                        comparison={
                            "status": "error",
                            "details": "runtime guarantee cannot enforce the shrunken scope safely",
                            "compared_output_digest": None,
                        },
                    )
                )
                trial_index += 1
                continue

            candidate_grants = deepcopy(current_plan["grants"])
            candidate_grants[index] = normalize_effect(shrunken)
            candidate_plan = authority_plan_with_grants(
                current_plan,
                candidate_grants,
                f"candidate:shrink:{trial_index}",
            )
            candidate_run = run_example_harness(contract, invocation_input, candidate_plan)
            comparison = compare_runs(
                current_run,
                candidate_run,
                candidate_plan,
                comparator_profile,
            )
            scope_stats[scope_kind]["attempts"] += 1
            accepted = comparison["status"] == "match"
            if accepted:
                scope_stats[scope_kind]["accepted"] += 1
                current_plan = candidate_plan
                current_run = candidate_run
                changed = True
            attempts.append(
                build_attempt(
                    trial_id=f"{current_plan['plan_id']}:trial:shrink:{trial_index}",
                    candidate_plan=candidate_plan,
                    change_kind="shrink_scope",
                    target_effect_class=grant["effect_class"],
                    target_scope_kind=scope_kind,
                    removed_grants=[],
                    shrink_from=grant,
                    shrink_to=shrunken,
                    trial_result=trial_result_for(candidate_run, comparison),
                    reason_codes=reason_codes_for_attempt(candidate_run, comparison),
                    shadow_run=candidate_run,
                    comparison=comparison,
                )
            )
            trial_index += 1

    return {
        "chosen_plan": current_plan,
        "chosen_run": current_run,
        "attempts": attempts,
        "reason_codes": stable_unique_strings(sorted(reason_codes)),
        "scope_shrinker_stats": [
            {
                "scope_kind": scope_kind,
                "strategy": "observed_effect_projection",
                "bounded": True,
                "attempts": values["attempts"],
                "accepted": values["accepted"],
            }
            for scope_kind, values in scope_stats.items()
        ],
    }


def explain_retained_authorities(
    final_plan: dict[str, Any],
    baseline_run: dict[str, Any],
    contract: dict[str, Any],
    invocation_input: dict[str, Any],
    comparator_profile: dict[str, Any],
    *,
    next_trial_index: int,
) -> dict[str, Any]:
    attempts: list[dict[str, Any]] = []
    retained_authorities: list[dict[str, Any]] = []
    trial_index = next_trial_index

    for index, grant in enumerate(final_plan.get("grants", [])):
        candidate_grants = [
            deepcopy(existing)
            for grant_index, existing in enumerate(final_plan.get("grants", []))
            if grant_index != index
        ]
        candidate_plan = authority_plan_with_grants(
            final_plan,
            candidate_grants,
            f"candidate:retained:{trial_index}",
        )
        candidate_run = run_example_harness(contract, invocation_input, candidate_plan)
        comparison = compare_runs(
            baseline_run,
            candidate_run,
            candidate_plan,
            comparator_profile,
        )
        reason_codes = reason_codes_for_attempt(candidate_run, comparison)
        attempts.append(
            build_attempt(
                trial_id=f"{final_plan['plan_id']}:trial:retained:{trial_index}",
                candidate_plan=candidate_plan,
                change_kind="remove_grant",
                target_effect_class=grant["effect_class"],
                target_scope_kind=grant["scope"]["kind"],
                removed_grants=[grant],
                shrink_from=None,
                shrink_to=None,
                trial_result=trial_result_for(candidate_run, comparison),
                reason_codes=reason_codes,
                shadow_run=candidate_run,
                comparison=comparison,
            )
        )
        retained_authorities.append(
            {
                "grant": deepcopy(grant),
                "reason_codes": reason_codes or ["AUTHORITY_REQUIRED_BY_TRACE"],
                "details": comparison["details"] or "Removing this authority changed the invocation result or shadow execution envelope.",
            }
        )
        trial_index += 1

    return {
        "attempts": attempts,
        "retained_authorities": retained_authorities,
    }


def determine_proof_status(
    *,
    baseline_plan: dict[str, Any],
    final_plan: dict[str, Any],
    discrete_result: dict[str, Any],
    shrink_result: dict[str, Any],
    top_level_reason_codes: list[str],
) -> str:
    removed_or_shrunk = canonical_json(baseline_plan) != canonical_json(final_plan)
    shrink_accepted = any(item["accepted"] > 0 for item in shrink_result["scope_shrinker_stats"])
    if not baseline_plan.get("grants") and not final_plan.get("grants"):
        return "exact_minimal"
    blocking_codes = {
        "COMPARATOR_UNAVAILABLE",
        "COMPARATOR_FAILED",
        "COMPARATOR_NONDETERMINISTIC",
        "SHADOW_HARNESS_UNAVAILABLE",
    }
    if any(code in blocking_codes for code in top_level_reason_codes):
        return "reduced" if removed_or_shrunk else "not_proven"
    if discrete_result["truncated"]:
        return "reduced" if removed_or_shrunk else "not_proven"
    if shrink_accepted:
        return "bounded_minimal"
    if removed_or_shrunk:
        return "exact_minimal"
    return "no_reduction"


def authority_plan_with_grants(
    base_plan: dict[str, Any],
    grants: list[dict[str, Any]],
    suffix: str,
) -> dict[str, Any]:
    plan = deepcopy(base_plan)
    plan["plan_id"] = f"{base_plan['plan_id']}:{suffix}"
    plan["grants"] = stable_unique_dicts([normalize_effect(grant) for grant in grants])
    return plan


def build_attempt(
    *,
    trial_id: str,
    candidate_plan: dict[str, Any],
    change_kind: str,
    target_effect_class: str | None,
    target_scope_kind: str | None,
    removed_grants: list[dict[str, Any]],
    shrink_from: dict[str, Any] | None,
    shrink_to: dict[str, Any] | None,
    trial_result: str,
    reason_codes: list[str],
    shadow_run: dict[str, Any],
    comparison: dict[str, Any],
) -> dict[str, Any]:
    return {
        "trial_id": trial_id,
        "candidate_authority_plan": deepcopy(candidate_plan),
        "change_kind": change_kind,
        "target_effect_class": target_effect_class,
        "target_scope_kind": target_scope_kind,
        "removed_grants": stable_unique_dicts([normalize_effect(item) for item in removed_grants]),
        "shrink_from": deepcopy(normalize_effect(shrink_from)) if shrink_from is not None else None,
        "shrink_to": deepcopy(normalize_effect(shrink_to)) if shrink_to is not None else None,
        "trial_result": trial_result,
        "reason_codes": stable_unique_strings(sorted(reason_codes)),
        "shadow_execution": {
            "status": shadow_run["status"],
            "error_code": shadow_run["error_code"],
            "observed_effects": stable_unique_dicts(
                [normalize_effect(effect) for effect in shadow_run["observed_effects"]]
            ),
            "output_digest": shadow_run["output_digest"],
        },
        "comparator_summary": comparison,
    }


def trial_result_for(shadow_run: dict[str, Any], comparison: dict[str, Any]) -> str:
    if shadow_run["status"] == "timeout":
        return "timeout"
    if shadow_run["status"] == "error" and comparison["status"] == "error":
        return "not_proven"
    if comparison["status"] == "match":
        return "equivalent"
    if comparison["status"] in {"mismatch", "unavailable"}:
        return "non_equivalent" if comparison["status"] == "mismatch" else "not_proven"
    return "error"


def reason_codes_for_attempt(shadow_run: dict[str, Any], comparison: dict[str, Any]) -> list[str]:
    codes: list[str] = []
    if shadow_run["status"] == "error":
        if shadow_run["error_code"] == "unsupported-example-harness":
            codes.append("SHADOW_HARNESS_UNAVAILABLE")
        elif shadow_run["error_code"] == "runtime-cannot-enforce-shrunk-scope":
            codes.append("RUNTIME_CANNOT_ENFORCE_SHRUNK_SCOPE")
        else:
            codes.append("AUTHORITY_REQUIRED_BY_TRACE")
    if comparison["status"] == "unavailable":
        codes.append("COMPARATOR_UNAVAILABLE")
    elif comparison["status"] == "error":
        codes.append("COMPARATOR_FAILED")
    elif comparison["status"] == "mismatch":
        codes.append("AUTHORITY_REQUIRED_BY_COMPARATOR")
    return stable_unique_strings(sorted(codes))


def compare_runs(
    baseline_run: dict[str, Any],
    candidate_run: dict[str, Any],
    candidate_plan: dict[str, Any],
    comparator_profile: dict[str, Any],
) -> dict[str, Any]:
    if candidate_run["status"] != "success":
        return {
            "status": "mismatch",
            "details": f"shadow execution failed with {candidate_run['error_code']}",
            "compared_output_digest": candidate_run["output_digest"],
        }

    comparator_kind = comparator_profile["comparator_kind"]
    try:
        if comparator_kind == "canonical_structured_output":
            left = select_output(baseline_run["output"], comparator_profile.get("output_pointer"))
            right = select_output(candidate_run["output"], comparator_profile.get("output_pointer"))
            if canonical_json(left) == canonical_json(right):
                return {
                    "status": "match",
                    "details": "canonical structured output matched exactly",
                    "compared_output_digest": digest_struct(right),
                }
            return {
                "status": "mismatch",
                "details": "canonical structured output changed",
                "compared_output_digest": digest_struct(right),
            }

        if comparator_kind == "declared_postconditions":
            checker = DECLARED_POSTCONDITION_CHECKERS.get(comparator_profile.get("checker_ref"))
            if checker is None:
                return {
                    "status": "unavailable",
                    "details": "declared postcondition checker was unavailable",
                    "compared_output_digest": candidate_run["output_digest"],
                }
            matched, details = checker(baseline_run, candidate_run, comparator_profile)
            return {
                "status": "match" if matched else "mismatch",
                "details": details,
                "compared_output_digest": candidate_run["output_digest"],
            }

        if comparator_kind == "side_effect_trace_equivalence":
            left = stable_unique_dicts(
                [normalize_effect(effect) for effect in baseline_run["observed_effects"]]
            )
            right = stable_unique_dicts(
                [normalize_effect(effect) for effect in candidate_run["observed_effects"]]
            )
            if canonical_json(left) == canonical_json(right):
                return {
                    "status": "match",
                    "details": "side-effect trace matched exactly",
                    "compared_output_digest": candidate_run["output_digest"],
                }
            return {
                "status": "mismatch",
                "details": "side-effect trace diverged",
                "compared_output_digest": candidate_run["output_digest"],
            }

        if comparator_kind == "pure_zero_authority":
            if candidate_plan.get("grants") or candidate_run["observed_effects"]:
                return {
                    "status": "mismatch",
                    "details": "pure comparator requires zero granted and observed authority",
                    "compared_output_digest": candidate_run["output_digest"],
                }
            if canonical_json(baseline_run["output"]) == canonical_json(candidate_run["output"]):
                return {
                    "status": "match",
                    "details": "pure zero-authority output matched with no observed effects",
                    "compared_output_digest": candidate_run["output_digest"],
                }
            return {
                "status": "mismatch",
                "details": "pure zero-authority output changed",
                "compared_output_digest": candidate_run["output_digest"],
            }
    except Exception as exc:  # pragma: no cover - fail closed path
        return {
            "status": "error",
            "details": f"comparator raised {type(exc).__name__}: {exc}",
            "compared_output_digest": candidate_run["output_digest"],
        }

    return {
        "status": "unavailable",
        "details": "comparator kind was unsupported",
        "compared_output_digest": candidate_run["output_digest"],
    }


def select_output(output: Any, pointer: str | None) -> Any:
    if pointer in (None, "", "/"):
        return output
    current = output
    for segment in pointer.strip("/").split("/"):
        if isinstance(current, dict):
            current = current[segment]
        elif isinstance(current, list):
            current = current[int(segment)]
        else:  # pragma: no cover - comparator failure
            raise KeyError(pointer)
    return current


def shrink_grant_from_observed(
    grant: dict[str, Any],
    observed_effects: list[dict[str, Any]],
) -> dict[str, Any] | None:
    relevant = [
        effect
        for effect in observed_effects
        if effect["effect_class"] == grant["effect_class"]
    ]
    if not relevant:
        return None

    candidates = [
        normalize_effect(effect)
        for effect in relevant
        if intersect_effect(grant, effect) is not None and effect_covers(grant, effect)
    ]
    if not candidates:
        return None

    if grant["scope"]["kind"] not in {"filesystem", "network", "delegation"}:
        return None

    chosen = min(candidates, key=canonical_json)
    return {
        "effect_class": chosen["effect_class"],
        "scope": deepcopy(chosen["scope"]),
        "cardinality": deepcopy(chosen.get("cardinality", grant.get("cardinality"))),
    }


def run_example_harness(
    contract: dict[str, Any],
    invocation_input: dict[str, Any],
    authority_plan: dict[str, Any],
) -> dict[str, Any]:
    contract_id = contract["contract_id"]
    if contract_id == "urn:guild:contract:local-log-analyzer:v1":
        return run_local_log_analyzer(invocation_input, authority_plan)
    if contract_id == "urn:guild:contract:fetch-transform:v1":
        return run_fetch_transform(invocation_input, authority_plan)
    if contract_id == "urn:guild:contract:zero-authority:v1":
        return run_zero_authority(invocation_input, authority_plan)
    return {
        "status": "error",
        "error_code": "unsupported-example-harness",
        "observed_effects": [],
        "output": None,
        "output_digest": None,
    }


def run_local_log_analyzer(invocation_input: dict[str, Any], authority_plan: dict[str, Any]) -> dict[str, Any]:
    files = invocation_input["input_files"]
    clock_value = invocation_input["clock_value"]
    read_effect = filesystem_effect(
        "fs.read",
        [item["path"] for item in files],
        max_calls=len(files),
        max_bytes=sum(len(item["content"].encode("utf-8")) for item in files),
    )
    clock_effect = system_effect("clock.read", max_calls=1)

    missing = missing_effects(authority_plan, [read_effect, clock_effect])
    if missing:
        return failure_run(missing)

    severity_totals = {"ERROR": 0, "WARN": 0, "INFO": 0}
    for item in files:
        for line in item["content"].splitlines():
            for severity in severity_totals:
                if line.startswith(severity):
                    severity_totals[severity] += 1

    output = {
        "report": {
            "file_count": len(files),
            "severity_totals": severity_totals,
            "generated_at": clock_value,
            "window": deepcopy(invocation_input["window"]),
        }
    }
    return success_run(output, [read_effect, clock_effect])


def run_fetch_transform(invocation_input: dict[str, Any], authority_plan: dict[str, Any]) -> dict[str, Any]:
    config_path = invocation_input["config_path"]
    output_path = invocation_input["output_path"]
    secret_id = invocation_input["secret_id"]
    source_request = invocation_input["source_request"]
    read_effect = filesystem_effect("fs.read", [config_path], max_calls=1, max_bytes=len(canonical_json(invocation_input["config"])))
    write_effect = filesystem_effect(
        "fs.write",
        [output_path],
        max_calls=1,
        max_bytes=len(canonical_json(invocation_input["source_response"])),
    )
    secret_effect = secret_effect_spec(secret_id)
    network_effect = network_effect_spec(
        source_request["url"],
        source_request["method"],
    )
    clock_effect = system_effect("clock.read", max_calls=1)

    required_effects = [read_effect, write_effect, secret_effect, network_effect, clock_effect]
    missing = missing_effects(authority_plan, required_effects)
    if missing:
        return failure_run(missing)

    transformed = {
        "mode": invocation_input["config"]["mode"],
        "items": [
            {
                "id": item["id"],
                "state": "enabled" if item["enabled"] else "disabled",
            }
            for item in invocation_input["source_response"]["items"]
        ],
        "generated_at": invocation_input["clock_value"],
        "output_path": output_path,
    }
    output = {
        "result": transformed,
        "write_summary": {
            "path": output_path,
            "digest": digest_struct(transformed),
        },
    }
    return success_run(output, [read_effect, write_effect, secret_effect, network_effect, clock_effect])


def run_zero_authority(invocation_input: dict[str, Any], authority_plan: dict[str, Any]) -> dict[str, Any]:
    if authority_plan.get("grants"):
        return {
            "status": "error",
            "error_code": "unexpected-authority-for-zero-authority-example",
            "observed_effects": [],
            "output": None,
            "output_digest": None,
        }
    text = invocation_input["document_text"]
    words = [word for word in text.split() if word]
    output = {
        "summary": {
            "document_title": invocation_input["document_title"],
            "word_count": len(words),
            "preview": " ".join(words[:6]),
        }
    }
    return success_run(output, [])


def missing_effects(authority_plan: dict[str, Any], required_effects: list[dict[str, Any]]) -> str | None:
    grants = authority_plan.get("grants", [])
    for effect in required_effects:
        if any(effect_covers(grant, effect) for grant in grants):
            continue
        return f"missing-required-authority:{effect['effect_class']}"
    return None


def success_run(output: dict[str, Any], observed_effects: list[dict[str, Any]]) -> dict[str, Any]:
    return {
        "status": "success",
        "error_code": None,
        "observed_effects": stable_unique_dicts([normalize_effect(effect) for effect in observed_effects]),
        "output": output,
        "output_digest": digest_struct(output),
    }


def failure_run(error_code: str) -> dict[str, Any]:
    return {
        "status": "error",
        "error_code": error_code,
        "observed_effects": [],
        "output": None,
        "output_digest": None,
    }


def filesystem_effect(
    effect_class: str,
    paths: list[str],
    *,
    max_calls: int,
    max_bytes: int | None = None,
) -> dict[str, Any]:
    effect = {
        "effect_class": effect_class,
        "scope": {
            "kind": "filesystem",
            "paths": sorted(paths),
            "symlink_policy": "deny",
            "follow_mounts": False,
        },
        "cardinality": {
            "max_calls": max_calls,
        },
    }
    if max_bytes is not None:
        effect["cardinality"]["max_bytes"] = max_bytes
    return effect


def system_effect(effect_class: str, *, max_calls: int) -> dict[str, Any]:
    return {
        "effect_class": effect_class,
        "scope": {
            "kind": "system"
        },
        "cardinality": {
            "max_calls": max_calls
        },
    }


def secret_effect_spec(secret_id: str) -> dict[str, Any]:
    return {
        "effect_class": "secret.read",
        "scope": {
            "kind": "secret",
            "secret_ids": [secret_id],
        },
        "cardinality": {
            "max_calls": 1
        },
    }


def network_effect_spec(url: str, method: str) -> dict[str, Any]:
    parsed = urlparse(url)
    port = parsed.port or (443 if parsed.scheme == "https" else 80)
    return {
        "effect_class": "net.connect",
        "scope": {
            "kind": "network",
            "audiences": [
                {
                    "host": parsed.hostname,
                    "ports": [port],
                    "schemes": [parsed.scheme],
                    "path_prefixes": [parsed.path or "/"],
                    "methods": [method.upper()],
                }
            ],
        },
        "cardinality": {
            "max_calls": 1,
            "max_bytes": 1024 * 1024,
        },
    }


def fetch_transform_postconditions(
    baseline_run: dict[str, Any],
    candidate_run: dict[str, Any],
    comparator_profile: dict[str, Any],
) -> tuple[bool, str]:
    expected_path = comparator_profile["inputs"]["expected_output_path"]
    left = baseline_run["output"]["write_summary"]
    right = candidate_run["output"]["write_summary"]
    if left["path"] != expected_path or right["path"] != expected_path:
        return False, "declared postcondition expected output path changed"
    if left["digest"] != right["digest"]:
        return False, "declared postcondition output digest changed"
    if canonical_json(baseline_run["output"]["result"]) != canonical_json(candidate_run["output"]["result"]):
        return False, "declared postcondition transformed payload changed"
    return True, "declared postconditions matched exactly"


DECLARED_POSTCONDITION_CHECKERS = {
    "urn:guild:checker:fetch-transform-postconditions:v1": fetch_transform_postconditions,
}
