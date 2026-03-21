import json
import subprocess
from copy import deepcopy
from pathlib import Path
from tempfile import TemporaryDirectory

from admission_core import (
    AdmissionInputError,
    build_execution_plan,
    build_registry,
    canonical_json,
    digest_struct,
    load_json,
    validate_instance,
)
from minimization_core import build_minimization_proof
from runtime_alignment import LIVE_RUNTIME_SOURCE_KIND, observation_bundle_from_execution_record, runtime_mapping_for_effect
from token_core import PROOF_SOURCE_LIVE_RUNTIME, create_child_token, create_root_token, verify_token
from witness_core import generate_witness, verify_claim, verify_witness
from witness_examples import build_witness_fixtures


EXAMPLES = [
    ("skill_contract.schema.json", "examples/local-log-analyzer.contract.json"),
    ("skill_contract.schema.json", "examples/zero-authority.contract.json"),
    ("skill_contract.schema.json", "examples/fetch-transform.contract.json"),
    ("skill_contract.schema.json", "examples/cluster-rollout.contract.json"),
    ("skill_contract.schema.json", "examples/runtime-http-read.contract.json"),
    ("skill_contract.schema.json", "examples/runtime-read-resource.contract.json"),
    ("skill_contract.schema.json", "examples/runtime-invoke-skill.contract.json"),
    ("skill_contract.schema.json", "examples/runtime-emit-evidence-zero.contract.json"),
    ("skill_contract.schema.json", "examples/runtime-log-write.contract.json"),
    ("runtime_guarantee.schema.json", "examples/wasmtime-strict.runtime.json"),
    ("runtime_guarantee.schema.json", "examples/node-wasi-basic.runtime.json"),
    ("comparator_profile.schema.json", "examples/local-log-analyzer.canonical-json.comparator.json"),
    ("comparator_profile.schema.json", "examples/local-log-analyzer.unavailable.comparator.json"),
    ("comparator_profile.schema.json", "examples/fetch-transform.postconditions.comparator.json"),
    ("comparator_profile.schema.json", "examples/fetch-transform.bounded.comparator.json"),
    ("comparator_profile.schema.json", "examples/zero-authority.pure.comparator.json"),
    ("comparator_profile.schema.json", "examples/runtime-http-read.unavailable.comparator.json"),
    ("proof_record.schema.json", "examples/local-log-analyzer.proof.json"),
    ("proof_record.schema.json", "examples/local-log-analyzer.cache-hit.proof.json"),
    ("proof_record.schema.json", "examples/local-log-analyzer.comparator-unavailable.proof.json"),
    ("proof_record.schema.json", "examples/fetch-transform.no-reduction.proof.json"),
    ("proof_record.schema.json", "examples/fetch-transform.bounded.proof.json"),
    ("proof_record.schema.json", "examples/zero-authority.proof.json"),
    ("witness_record.schema.json", "examples/cluster-rollout.witness.json"),
    ("witness_record.schema.json", "examples/local-log-analyzer.within-envelope.witness.json"),
    ("witness_record.schema.json", "examples/local-log-analyzer.out-of-envelope.witness.json"),
    ("witness_record.schema.json", "examples/fetch-transform.coverage-limited.witness.json"),
    ("witness_record.schema.json", "examples/fetch-transform.redacted-claim-blocked.witness.json"),
    ("witness_record.schema.json", "examples/fetch-transform.blocked-attempt.witness.json"),
    ("witness_record.schema.json", "examples/zero-authority.witness.json"),
    ("witness_record.schema.json", "examples/runtime-mapping-limited.witness.json"),
    ("witness_record.schema.json", "examples/local-log-analyzer.runtime-mismatch.witness.json"),
    ("admission_request.schema.json", "examples/zero-authority.admit.request.json"),
    ("admission_request.schema.json", "examples/zero-authority.migrate.request.json"),
    ("admission_request.schema.json", "examples/fetch-transform.downgrade.request.json"),
    ("admission_request.schema.json", "examples/fetch-transform.no-reduction.request.json"),
    ("admission_request.schema.json", "examples/local-log-analyzer.admit.request.json"),
    ("admission_request.schema.json", "examples/cluster-rollout.refuse.request.json"),
    ("admission_request.schema.json", "examples/cluster-rollout.admit.request.json"),
    ("admission_request.schema.json", "examples/runtime-http-read.admit.request.json"),
    ("admission_request.schema.json", "examples/runtime-read-resource.admit.request.json"),
    ("admission_request.schema.json", "examples/runtime-invoke-skill.admit.request.json"),
    ("admission_request.schema.json", "examples/runtime-emit-evidence-zero.admit.request.json"),
    ("admission_request.schema.json", "examples/runtime-log-write.admit.request.json"),
    ("execution_plan.schema.json", "examples/zero-authority.admit.plan.json"),
    ("execution_plan.schema.json", "examples/zero-authority.migrate.plan.json"),
    ("execution_plan.schema.json", "examples/fetch-transform.downgrade.plan.json"),
    ("execution_plan.schema.json", "examples/fetch-transform.no-reduction.plan.json"),
    ("execution_plan.schema.json", "examples/local-log-analyzer.admit.plan.json"),
    ("execution_plan.schema.json", "examples/cluster-rollout.refuse.plan.json"),
    ("execution_plan.schema.json", "examples/cluster-rollout.admit.plan.json"),
    ("delegated_capability_token.schema.json", "examples/local-log-analyzer.proof-backed.root-token.json"),
    ("delegated_capability_token.schema.json", "examples/cluster-rollout.root-token.json"),
    ("delegated_capability_token.schema.json", "examples/cluster-rollout.child-token.json"),
    ("delegated_capability_token.schema.json", "examples/zero-authority.empty-token.json"),
]

ADMISSION_CASES = [
    {
        "contract": "examples/zero-authority.contract.json",
        "request": "examples/zero-authority.admit.request.json",
        "runtimes": ["examples/wasmtime-strict.runtime.json"],
        "expected_plan": "examples/zero-authority.admit.plan.json",
    },
    {
        "contract": "examples/zero-authority.contract.json",
        "request": "examples/zero-authority.migrate.request.json",
        "runtimes": [
            "examples/node-wasi-basic.runtime.json",
            "examples/wasmtime-strict.runtime.json",
        ],
        "expected_plan": "examples/zero-authority.migrate.plan.json",
    },
    {
        "contract": "examples/fetch-transform.contract.json",
        "request": "examples/fetch-transform.downgrade.request.json",
        "runtimes": ["examples/wasmtime-strict.runtime.json"],
        "expected_plan": "examples/fetch-transform.downgrade.plan.json",
    },
    {
        "contract": "examples/fetch-transform.contract.json",
        "request": "examples/fetch-transform.no-reduction.request.json",
        "runtimes": ["examples/wasmtime-strict.runtime.json"],
        "expected_plan": "examples/fetch-transform.no-reduction.plan.json",
    },
    {
        "contract": "examples/local-log-analyzer.contract.json",
        "request": "examples/local-log-analyzer.admit.request.json",
        "runtimes": ["examples/wasmtime-strict.runtime.json"],
        "expected_plan": "examples/local-log-analyzer.admit.plan.json",
    },
    {
        "contract": "examples/cluster-rollout.contract.json",
        "request": "examples/cluster-rollout.refuse.request.json",
        "runtimes": ["examples/node-wasi-basic.runtime.json"],
        "expected_plan": "examples/cluster-rollout.refuse.plan.json",
    },
    {
        "contract": "examples/cluster-rollout.contract.json",
        "request": "examples/cluster-rollout.admit.request.json",
        "runtimes": ["examples/wasmtime-strict.runtime.json"],
        "expected_plan": "examples/cluster-rollout.admit.plan.json",
    },
]

REPO_ROOT = Path(__file__).resolve().parents[3]
LIVE_SCOPE_KIND_BY_FAMILY = {
    "http-request": "network",
    "read-resource": "resource",
    "invoke-skill": "skill",
    "emit-evidence": "evidence",
    "log-write": "log",
}


def parse_prefixed_digest(value: str | None) -> dict | None:
    if value is None:
        return None
    algorithm, _, digest_value = value.partition(":")
    if not algorithm or not digest_value:
        raise ValueError(f"invalid digest string {value!r}")
    return {
        "algorithm": algorithm,
        "value": digest_value,
    }


def authority_plan_with_grants(plan: dict, grants: list[dict], suffix: str) -> dict:
    authority_plan = deepcopy(plan["granted_authority"])
    authority_plan["plan_id"] = f"{plan['plan_id']}:{suffix}"
    authority_plan["grants"] = deepcopy(grants)
    return authority_plan


def family_grants(authority_plan: dict, family: str) -> list[dict]:
    return [deepcopy(grant) for grant in authority_plan.get("grants", []) if grant.get("family") == family]


def baseline_family_template(authority_plan: dict, family: str) -> dict | None:
    for grant in authority_plan.get("grants", []):
        if grant.get("family") == family:
            return deepcopy(grant)
    return None


def live_grant_to_effect_spec(grant: dict, baseline_authority_plan: dict) -> dict:
    family = grant["id"]
    template = baseline_family_template(baseline_authority_plan, family) or {"family": family}
    effect = {
        "family": family,
        "scope": deepcopy(template.get("scope", {})),
    }
    constraints = deepcopy(grant.get("constraints", {}))
    if family == "http-request":
        methods = constraints.get("allowed_methods")
        if methods is not None:
            constraints["allowed_methods"] = [method.upper() for method in methods]
        effect["scope"] = {
            "kind": "network",
            **constraints,
        }
    elif family == "read-resource":
        effect["scope"] = {
            "kind": "resource",
            "uri_prefixes": constraints.get("uri_prefixes", []),
            "resource_kinds": constraints.get("resource_kinds", []),
        }
    elif family == "invoke-skill":
        effect["scope"] = {
            "kind": "skill",
            "aliases": constraints.get("aliases", []),
        }
    elif family == "emit-evidence":
        effect["scope"] = {
            "kind": "evidence",
            "max_bytes": constraints.get("max_bytes"),
            "audiences": constraints.get("audiences", []),
            "redactions": constraints.get("redactions", []),
        }
    elif family == "log-write":
        effect["scope"] = {
            "kind": "log",
            "levels": constraints.get("levels", []),
        }
    if template.get("cardinality") is not None:
        effect["cardinality"] = deepcopy(template["cardinality"])
    if template.get("justification") is not None:
        effect["justification"] = template["justification"]
    return effect


def live_grants_to_effect_specs(live_grants: list[dict], baseline_authority_plan: dict) -> list[dict]:
    return [live_grant_to_effect_spec(grant, baseline_authority_plan) for grant in live_grants]


def live_trial_result(trial: dict) -> str:
    if trial["accepted"]:
        return "equivalent"
    if trial["comparator_status"] == "mismatch":
        return "non_equivalent"
    if trial["execution_status"] == "validation_error":
        return "error"
    if trial["execution_status"] in {"failed", "rejected"}:
        return "error"
    if trial["comparator_status"] in {"error", "unavailable"}:
        return "error"
    return "not_proven"


def live_shadow_status(trial: dict) -> str:
    if trial["execution_status"] == "succeeded":
        return "success"
    if trial.get("error_code") == "timeout":
        return "timeout"
    return "error"


def live_comparator_summary_status(trial: dict) -> str:
    status = trial["comparator_status"]
    if status in {"match", "mismatch", "error", "unavailable"}:
        return status
    return "error"


def live_comparator_spec(descriptor: dict) -> dict:
    return {
        "comparator_id": descriptor["comparator_id"],
        "version": descriptor["version"],
        "comparator_kind": "canonical_structured_output",
        "reference": descriptor["notes"],
        "canonicalization": "rfc8785-jcs",
        "checker_ref": descriptor["comparator_id"],
        "output_pointer": "/output/structured",
        "inputs": {
            "profile": descriptor["comparator_id"],
        },
        "assumptions": {
            "ambient_clock_controlled": True,
            "ambient_network_controlled": True,
            "ambient_random_controlled": True,
        },
    }


def live_candidate_attempt(trial: dict, baseline_authority_plan: dict) -> dict:
    family = trial["family"]
    candidate_authority_plan = authority_plan_with_grants(
        {"granted_authority": baseline_authority_plan, "plan_id": baseline_authority_plan["plan_id"]},
        live_grants_to_effect_specs(trial["candidate_envelope"]["granted_capabilities"]["grants"], baseline_authority_plan),
        trial["trial_id"],
    )
    baseline_family = family_grants(baseline_authority_plan, family)
    candidate_family = family_grants(candidate_authority_plan, family)
    removed_grants = [grant for grant in baseline_family if grant not in candidate_family]
    shadow_execution = {
        "status": live_shadow_status(trial),
        "error_code": trial.get("error_code"),
        "observed_effects": [],
        "observed_families": trial.get("observed_families", []),
        "output_digest": parse_prefixed_digest(trial.get("output_digest")),
    }
    comparator_summary = {
        "status": live_comparator_summary_status(trial),
        "compared_output_digest": parse_prefixed_digest(trial.get("output_digest")),
    }
    if trial.get("error_code") is not None:
        comparator_summary["details"] = trial["error_code"]

    return {
        "trial_id": trial["trial_id"],
        "candidate_authority_plan": candidate_authority_plan,
        "change_kind": trial["change_kind"],
        "target_effect_class": family,
        "target_scope_kind": LIVE_SCOPE_KIND_BY_FAMILY[family],
        "removed_grants": removed_grants,
        "shrink_from": baseline_family[0] if baseline_family and candidate_family else None,
        "shrink_to": candidate_family[0] if candidate_family else None,
        "trial_result": live_trial_result(trial),
        "reason_codes": trial["reason_codes"],
        "shadow_execution": shadow_execution,
        "comparator_summary": comparator_summary,
    }


def live_retained_authorities(residual_authority_plan: dict, family_statuses: list[dict]) -> list[dict]:
    by_family = {entry["family"]: entry for entry in family_statuses}
    retained: list[dict] = []
    for grant in residual_authority_plan["grants"]:
        family_status = by_family.get(grant.get("family"))
        retained.append(
            {
                "grant": deepcopy(grant),
                "reason_codes": [] if family_status is None else family_status["reason_codes"],
                "details": "Residual authority stayed outside the live proof envelope."
                if family_status is None
                else family_status["notes"],
            }
        )
    return retained


def load_live_proof_scenario(name: str) -> dict:
    result = subprocess.run(
        [
            "cargo",
            "run",
            "-q",
            "-p",
            "guild-runner",
            "--example",
            "live_proof_scenarios",
            "--",
            name,
        ],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip() or result.stdout.strip() or f"live proof scenario {name!r} failed")
    return json.loads(result.stdout)


def build_live_runtime_proof_record(
    *,
    scenario_name: str,
    plan: dict,
    contract: dict,
    runtime: dict,
    invocation_input: dict,
    created_at: str,
) -> tuple[dict, dict]:
    scenario = load_live_proof_scenario(scenario_name)
    live_proof = scenario["proof"]
    baseline_authority_plan = deepcopy(plan["granted_authority"])
    proven_authority_plan = authority_plan_with_grants(plan, live_proof["proven_authority"]["grants"], "live-proven")
    proven_authority_plan["grants"] = live_grants_to_effect_specs(live_proof["proven_authority"]["grants"], baseline_authority_plan)
    residual_authority_plan = authority_plan_with_grants(plan, live_proof["residual_authority"]["grants"], "live-residual")
    residual_authority_plan["grants"] = live_grants_to_effect_specs(
        live_proof["residual_authority"]["grants"], baseline_authority_plan
    )
    observation_summary, _ = observation_bundle_from_execution_record(
        scenario["baseline_execution_record"],
        baseline_authority_plan,
    )
    comparator = live_comparator_spec(live_proof["comparator"])
    proof_record = {
        "kind": "guild.proof_record",
        "version": "1.0.0",
        "proof_id": f"urn:guild:proof:{scenario_name}:live:v1",
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
        "proof_source_kind": PROOF_SOURCE_LIVE_RUNTIME,
        "proof_method": "counterfactual_shadow_execution",
        "comparator": comparator,
        "comparator_digest": digest_struct(comparator),
        "minimization_algorithm": {
            "id": "urn:guild:algorithm:live-proof:family-conservative:v1",
            "version": "1.0.0",
            "shrink_model_version": "1.0.0",
            "harness_id": "urn:guild:harness:live-runtime-replay:v1",
            "harness_version": "1.0.0",
        },
        "search_model": {
            "search_domain": "canonical-live-runtime-families",
            "discrete_grant_search": {
                "strategy": "family_conservative_search",
                "exact": live_proof["proof_status"] in {"exact_minimal", "no_reduction"},
                "candidate_plans_evaluated": len(live_proof["candidate_trials"]),
            },
            "scope_shrinkers": [
                {
                    "scope_kind": LIVE_SCOPE_KIND_BY_FAMILY[entry["family"]],
                    "strategy": "family_conservative_search",
                    "bounded": entry["support"] == "bounded-live-proof",
                    "attempts": len(
                        [
                            trial
                            for trial in live_proof["candidate_trials"]
                            if trial["family"] == entry["family"] and trial["change_kind"] == "shrink_scope"
                        ]
                    ),
                    "accepted": len(
                        [
                            trial
                            for trial in live_proof["candidate_trials"]
                            if trial["family"] == entry["family"]
                            and trial["change_kind"] == "shrink_scope"
                            and trial["accepted"]
                        ]
                    ),
                }
                for entry in live_proof["family_statuses"]
            ],
            "search_limits": {
                "max_candidate_plans": max(1, len(live_proof["candidate_trials"])),
                "truncated": False,
            },
            "assumptions": live_proof["comparator"]["assumptions"],
        },
        "baseline_authority_plan": baseline_authority_plan,
        "proven_authority_plan": proven_authority_plan,
        "residual_authority_plan": residual_authority_plan,
        "family_proof_statuses": deepcopy(live_proof["family_statuses"]),
        "candidate_attempts": [
            live_candidate_attempt(trial, baseline_authority_plan) for trial in live_proof["candidate_trials"]
        ],
        "retained_authorities": live_retained_authorities(residual_authority_plan, live_proof["family_statuses"]),
        "proof_status": live_proof["proof_status"],
        "minimization_reason_codes": live_proof["minimization_reason_codes"],
        "observed_effect_summary": {
            "effects": observation_summary["observed_effects"],
            "output_digest": parse_prefixed_digest(live_proof.get("baseline_output_digest")),
            "trace_digest": digest_struct(observation_summary["raw_trace"]),
        },
        "cache": {
            "status": "bypassed",
            "cache_key_digest": digest_struct(
                {
                    "scenario": scenario_name,
                    "execution_plan_digest": digest_struct(plan),
                    "runtime_guarantee_digest": plan["chosen_runtime"]["runtime_guarantee_digest"],
                }
            ),
            "reused_proof_id": None,
            "bypass_reason_codes": ["PROOF_CACHE_BYPASSED"],
            "key_material": {
                "execution_plan_digest": digest_struct(plan),
                "contract_digest": digest_struct(contract),
                "component_digest": deepcopy(contract["component"]["digest"]),
                "export_name": plan["export_name"],
                "input_class_fingerprint": deepcopy(plan["input_class_fingerprint"]),
                "invocation_input_digest": digest_struct(invocation_input),
                "runtime_guarantee_digest": plan["chosen_runtime"]["runtime_guarantee_digest"],
                "comparator_digest": digest_struct(comparator),
                "minimization_algorithm_version": "1.0.0",
                "shrink_model_version": "1.0.0",
                "harness_id": "urn:guild:harness:live-runtime-replay:v1",
                "harness_version": "1.0.0",
            },
        },
        "created_at": created_at,
        "expires_at": None,
        "notes": "Live-runtime proof record derived from the Rust runner's real counterfactual execution path.",
    }
    return proof_record, scenario


def m6_issuer() -> dict:
    return {
        "issuer_id": "urn:guild:issuer:draft-control-plane:v1",
        "key_id": "draft-hmac-2026-03",
        "shared_secret": "guild-draft-shared-secret-2026-03",
        "issuer_epoch": 3,
    }


def m6_issuer_keys() -> dict[str, dict[str, str]]:
    issuer = m6_issuer()
    return {issuer["issuer_id"]: {issuer["key_id"]: issuer["shared_secret"]}}


def cluster_child_authority_plan() -> dict:
    return {
        "plan_id": "urn:guild:token:cluster-rollout:child-authority:v1",
        "grants": [
            {
                "effect_class": "net.connect",
                "scope": {
                    "kind": "network",
                    "audiences": [
                        {
                            "host": "kube-api.prod.example.internal",
                            "ports": [443],
                            "schemes": ["https"],
                            "path_prefixes": ["/apis/apps/"],
                            "methods": ["PATCH"],
                        }
                    ],
                },
                "cardinality": {
                    "max_calls": 5,
                    "max_bytes": 524288,
                },
            }
        ],
        "delegation_policy": {
            "mode": "forbidden",
            "max_hops": 0,
            "audience_binding_required": True,
            "call_chain_binding_required": True,
            "anti_replay_required": True,
            "ttl_seconds_max": 30,
        },
        "ttl_seconds": 30,
    }


def cluster_child_resource_binding() -> dict:
    return {
        "effect_class": "net.connect",
        "audience": "cluster-prod",
        "resource": "https://kube-api.prod.example.internal/apis/apps/",
    }


def verify_examples(registry) -> list[str]:
    failures: list[str] = []
    for schema_name, instance_name in EXAMPLES:
        instance = load_json(instance_name)
        errors = validate_instance(schema_name, instance, registry)
        failures.extend(f"{instance_name}: {error}" for error in errors)
    return failures


def verify_admission_cases() -> list[str]:
    failures: list[str] = []
    for case in ADMISSION_CASES:
        contract = load_json(case["contract"])
        request = load_json(case["request"])
        runtimes = [load_json(path) for path in case["runtimes"]]
        expected_plan = load_json(case["expected_plan"])

        produced_once = build_execution_plan(contract, request, runtimes)
        produced_twice = build_execution_plan(contract, request, runtimes)

        if canonical_json(produced_once) != canonical_json(produced_twice):
            failures.append(f"{case['request']}: repeated admission produced non-deterministic output")

        if canonical_json(produced_once) != canonical_json(expected_plan):
            failures.append(f"{case['request']}: engine output did not match {case['expected_plan']}")

    return failures


def verify_invalid_runtime_probes(registry) -> list[str]:
    failures: list[str] = []
    base_runtime = load_json("examples/wasmtime-strict.runtime.json")

    missing_granularity = deepcopy(base_runtime)
    del missing_granularity["network_policy_granularity"]
    missing_errors = validate_instance("runtime_guarantee.schema.json", missing_granularity, registry)
    if not missing_errors:
        failures.append("negative probe: omitted runtime network_policy_granularity unexpectedly passed schema validation")

    unknown_granularity = deepcopy(base_runtime)
    unknown_granularity["network_policy_granularity"] = "super-url"
    unknown_errors = validate_instance("runtime_guarantee.schema.json", unknown_granularity, registry)
    if not unknown_errors:
        failures.append("negative probe: unknown runtime network_policy_granularity unexpectedly passed schema validation")

    contract = load_json("examples/zero-authority.contract.json")
    request = load_json("examples/zero-authority.admit.request.json")
    for label, runtime_doc in (
        ("missing_granularity", missing_granularity),
        ("unknown_granularity", unknown_granularity),
    ):
        try:
            plan = build_execution_plan(contract, request, [runtime_doc])
        except AdmissionInputError as error:
            failures.append(f"negative probe {label}: runtime invalidity escaped as input error instead of structured refusal: {error}")
            continue

        if plan["decision"] != "refuse":
            failures.append(f"negative probe {label}: expected refuse, got {plan['decision']}")
        if "RUNTIME_GUARANTEE_INVALID" not in plan["decision_reason_codes"]:
            failures.append(f"negative probe {label}: missing RUNTIME_GUARANTEE_INVALID in decision_reason_codes")
        if "NO_ADMISSIBLE_RUNTIME" not in plan["decision_reason_codes"]:
            failures.append(f"negative probe {label}: missing NO_ADMISSIBLE_RUNTIME in decision_reason_codes")

    return failures


def verify_minimization_cases() -> list[str]:
    failures: list[str] = []

    local_contract = load_json("examples/local-log-analyzer.contract.json")
    local_request = load_json("examples/local-log-analyzer.admit.request.json")
    local_plan = load_json("examples/local-log-analyzer.admit.plan.json")
    strict_runtime = load_json("examples/wasmtime-strict.runtime.json")
    local_invocation = load_json("examples/local-log-analyzer.invocation.json")
    local_comparator = load_json("examples/local-log-analyzer.canonical-json.comparator.json")
    local_missing_comparator = load_json("examples/local-log-analyzer.unavailable.comparator.json")

    fetch_contract = load_json("examples/fetch-transform.contract.json")
    fetch_request = load_json("examples/fetch-transform.no-reduction.request.json")
    fetch_plan = load_json("examples/fetch-transform.no-reduction.plan.json")
    fetch_bounded_request = load_json("examples/fetch-transform.downgrade.request.json")
    fetch_bounded_plan = load_json("examples/fetch-transform.downgrade.plan.json")
    fetch_invocation = load_json("examples/fetch-transform.invocation.json")
    fetch_comparator = load_json("examples/fetch-transform.postconditions.comparator.json")
    fetch_bounded_comparator = load_json("examples/fetch-transform.bounded.comparator.json")

    zero_contract = load_json("examples/zero-authority.contract.json")
    zero_request = load_json("examples/zero-authority.admit.request.json")
    zero_plan = load_json("examples/zero-authority.admit.plan.json")
    zero_invocation = load_json("examples/zero-authority.invocation.json")
    zero_comparator = load_json("examples/zero-authority.pure.comparator.json")

    expected_local = load_json("examples/local-log-analyzer.proof.json")
    expected_local_cache = load_json("examples/local-log-analyzer.cache-hit.proof.json")
    expected_local_missing = load_json("examples/local-log-analyzer.comparator-unavailable.proof.json")
    expected_fetch = load_json("examples/fetch-transform.no-reduction.proof.json")
    expected_fetch_bounded = load_json("examples/fetch-transform.bounded.proof.json")
    expected_zero = load_json("examples/zero-authority.proof.json")

    with TemporaryDirectory() as cache_dir:
        produced_local = build_minimization_proof(
            local_plan,
            local_contract,
            local_request,
            strict_runtime,
            local_invocation,
            local_comparator,
            created_at="2026-03-20T12:10:00Z",
            cache_dir=cache_dir,
        )
        if canonical_json(produced_local) != canonical_json(expected_local):
            failures.append("local-log-analyzer exact minimization output did not match checked proof example")

        produced_local_repeat = build_minimization_proof(
            local_plan,
            local_contract,
            local_request,
            strict_runtime,
            local_invocation,
            local_comparator,
            created_at="2026-03-20T12:10:00Z",
            cache_dir=None,
        )
        produced_local_repeat_again = build_minimization_proof(
            local_plan,
            local_contract,
            local_request,
            strict_runtime,
            local_invocation,
            local_comparator,
            created_at="2026-03-20T12:10:00Z",
            cache_dir=None,
        )
        if canonical_json(produced_local_repeat) != canonical_json(produced_local_repeat_again):
            failures.append("local-log-analyzer minimization was non-deterministic for identical inputs")

        produced_local_cache = build_minimization_proof(
            local_plan,
            local_contract,
            local_request,
            strict_runtime,
            local_invocation,
            local_comparator,
            created_at="2026-03-20T12:11:00Z",
            cache_dir=cache_dir,
        )
        if canonical_json(produced_local_cache) != canonical_json(expected_local_cache):
            failures.append("local-log-analyzer cache-hit output did not match checked proof example")

        produced_local_missing = build_minimization_proof(
            local_plan,
            local_contract,
            local_request,
            strict_runtime,
            local_invocation,
            local_missing_comparator,
            created_at="2026-03-20T12:12:00Z",
            cache_dir=cache_dir,
        )
        if canonical_json(produced_local_missing) != canonical_json(expected_local_missing):
            failures.append("local-log-analyzer comparator-unavailable output did not match checked proof example")
        if produced_local_missing["cache"]["status"] == "hit":
            failures.append("comparator-changed minimization unexpectedly reused a cached proof")

        runtime_changed = deepcopy(strict_runtime)
        runtime_changed["runtime"]["version"] = "strict-profile-2026-04"
        runtime_changed["notes"] = "cache invalidation probe"
        runtime_probe = build_minimization_proof(
            local_plan,
            local_contract,
            local_request,
            runtime_changed,
            local_invocation,
            local_comparator,
            created_at="2026-03-20T12:13:00Z",
            cache_dir=cache_dir,
        )
        if runtime_probe["cache"]["status"] == "hit":
            failures.append("runtime-changed minimization unexpectedly reused a cached proof")
        if "PLAN_RUNTIME_MISMATCH" not in runtime_probe["minimization_reason_codes"]:
            failures.append("runtime-changed minimization did not fail closed with PLAN_RUNTIME_MISMATCH")

        plan_changed = deepcopy(local_plan)
        plan_changed["plan_validity"]["ttl_seconds"] = 299
        plan_probe = build_minimization_proof(
            plan_changed,
            local_contract,
            local_request,
            strict_runtime,
            local_invocation,
            local_comparator,
            created_at="2026-03-20T12:14:00Z",
            cache_dir=cache_dir,
        )
        if plan_probe["cache"]["status"] == "hit":
            failures.append("plan-changed minimization unexpectedly reused a cached proof")

    produced_fetch = build_minimization_proof(
        fetch_plan,
        fetch_contract,
        fetch_request,
        strict_runtime,
        fetch_invocation,
        fetch_comparator,
        created_at="2026-03-20T12:20:00Z",
        cache_dir=None,
    )
    if canonical_json(produced_fetch) != canonical_json(expected_fetch):
        failures.append("fetch-transform no-reduction output did not match checked proof example")
    if produced_fetch["proof_status"] != "no_reduction":
        failures.append("fetch-transform no-reduction case did not remain an honest no_reduction proof")

    produced_fetch_bounded = build_minimization_proof(
        fetch_bounded_plan,
        fetch_contract,
        fetch_bounded_request,
        strict_runtime,
        fetch_invocation,
        fetch_bounded_comparator,
        created_at="2026-03-20T12:25:00Z",
        cache_dir=None,
    )
    if canonical_json(produced_fetch_bounded) != canonical_json(expected_fetch_bounded):
        failures.append("fetch-transform bounded shrink output did not match checked proof example")
    if produced_fetch_bounded["proof_status"] != "bounded_minimal":
        failures.append("fetch-transform bounded case did not stay bounded_minimal")

    produced_zero = build_minimization_proof(
        zero_plan,
        zero_contract,
        zero_request,
        strict_runtime,
        zero_invocation,
        zero_comparator,
        created_at="2026-03-20T12:30:00Z",
        cache_dir=None,
    )
    if canonical_json(produced_zero) != canonical_json(expected_zero):
        failures.append("zero-authority output did not match checked proof example")
    if produced_zero["proof_status"] != "exact_minimal":
        failures.append("zero-authority case did not prove exact minimality at the empty authority set")

    return failures


def verify_token_cases() -> list[str]:
    failures: list[str] = []
    issuer = m6_issuer()
    issuer_keys = m6_issuer_keys()

    local_contract = load_json("examples/local-log-analyzer.contract.json")
    local_plan = load_json("examples/local-log-analyzer.admit.plan.json")
    local_proof = load_json("examples/local-log-analyzer.proof.json")
    expected_local_root = load_json("examples/local-log-analyzer.proof-backed.root-token.json")

    cluster_contract = load_json("examples/cluster-rollout.contract.json")
    cluster_plan = load_json("examples/cluster-rollout.admit.plan.json")
    expected_cluster_root = load_json("examples/cluster-rollout.root-token.json")
    expected_cluster_child = load_json("examples/cluster-rollout.child-token.json")

    fetch_contract = load_json("examples/fetch-transform.contract.json")
    fetch_plan = load_json("examples/fetch-transform.no-reduction.plan.json")
    expected_fetch_refusal = load_json("examples/fetch-transform.upper-bound-refusal.json")

    zero_contract = load_json("examples/zero-authority.contract.json")
    zero_plan = load_json("examples/zero-authority.admit.plan.json")
    zero_proof = load_json("examples/zero-authority.proof.json")
    expected_zero_root = load_json("examples/zero-authority.empty-token.json")

    local_root = create_root_token(
        local_plan,
        local_contract,
        issuer,
        holder_id="urn:guild:service:local-log-analyzer",
        issued_at="2026-03-20T13:00:00Z",
        proof=local_proof,
        token_id="urn:guild:token:local-log-analyzer:root:v1",
    )
    if canonical_json(local_root) != canonical_json(expected_local_root):
        failures.append("local-log-analyzer proof-backed root token did not match the checked example")

    local_root_repeat = create_root_token(
        local_plan,
        local_contract,
        issuer,
        holder_id="urn:guild:service:local-log-analyzer",
        issued_at="2026-03-20T13:00:00Z",
        proof=local_proof,
        token_id="urn:guild:token:local-log-analyzer:root:v1",
    )
    if canonical_json(local_root_repeat) != canonical_json(local_root):
        failures.append("local-log-analyzer root issuance was non-deterministic for identical claims and key material")

    live_required_refusal = create_root_token(
        local_plan,
        local_contract,
        issuer,
        holder_id="urn:guild:service:local-log-analyzer",
        issued_at="2026-03-20T13:00:00Z",
        proof=local_proof,
        required_proof_source_kind=PROOF_SOURCE_LIVE_RUNTIME,
        token_id="urn:guild:token:local-log-analyzer:live-required:v1",
    )
    if live_required_refusal.get("issued") is not False or "PROOF_NOT_ACCEPTABLE" not in live_required_refusal.get(
        "reason_codes", []
    ):
        failures.append("draft-example proof was incorrectly accepted when live-runtime proof linkage was required")

    cluster_root = create_root_token(
        cluster_plan,
        cluster_contract,
        issuer,
        holder_id="urn:guild:service:cluster-rollout",
        issued_at="2026-03-20T13:05:00Z",
        allow_upper_bound=True,
        token_id="urn:guild:token:cluster-rollout:root:v1",
    )
    if canonical_json(cluster_root) != canonical_json(expected_cluster_root):
        failures.append("cluster-rollout upper-bound root token did not match the checked example")

    cluster_child = create_child_token(
        cluster_root,
        cluster_plan,
        cluster_contract,
        cluster_child_authority_plan(),
        issuer,
        holder_id="urn:guild:service:kube-api-client",
        issued_at="2026-03-20T13:05:10Z",
        audiences=["cluster-prod"],
        resource_bindings=[cluster_child_resource_binding()],
        token_id="urn:guild:token:cluster-rollout:child:v1",
    )
    if canonical_json(cluster_child) != canonical_json(expected_cluster_child):
        failures.append("cluster-rollout child token did not match the checked example")

    fetch_refusal = create_root_token(
        fetch_plan,
        fetch_contract,
        issuer,
        holder_id="urn:guild:service:fetch-transform",
        issued_at="2026-03-20T13:10:00Z",
        token_id="urn:guild:token:fetch-transform:root:v1",
    )
    if canonical_json(fetch_refusal) != canonical_json(expected_fetch_refusal):
        failures.append("fetch-transform upper-bound refusal did not match the checked example")

    zero_root = create_root_token(
        zero_plan,
        zero_contract,
        issuer,
        holder_id="urn:guild:service:zero-authority",
        issued_at="2026-03-20T13:15:00Z",
        proof=zero_proof,
        token_id="urn:guild:token:zero-authority:root:v1",
    )
    if canonical_json(zero_root) != canonical_json(expected_zero_root):
        failures.append("zero-authority empty token did not match the checked example")

    with TemporaryDirectory() as replay_dir:
        success = verify_token(
            cluster_child,
            issuer_keys=issuer_keys,
            verification_time="2026-03-20T13:05:20Z",
            expected_holder_id="urn:guild:service:kube-api-client",
            expected_audiences=["cluster-prod"],
            expected_resources=[cluster_child_resource_binding()],
            expected_runtime_guarantee_id="urn:guild:runtime:wasmtime-strict:v1",
            expected_call_chain_links=cluster_child["call_chain"]["links"],
            plan=cluster_plan,
            contract=cluster_contract,
            parent_token=cluster_root,
            replay_state_dir=replay_dir,
        )
        if not success["verified"] or success["reason_codes"]:
            failures.append("cluster-rollout child-token verification did not succeed for the bounded happy path")

        replayed = verify_token(
            cluster_child,
            issuer_keys=issuer_keys,
            verification_time="2026-03-20T13:05:21Z",
            expected_holder_id="urn:guild:service:kube-api-client",
            expected_audiences=["cluster-prod"],
            expected_resources=[cluster_child_resource_binding()],
            expected_runtime_guarantee_id="urn:guild:runtime:wasmtime-strict:v1",
            expected_call_chain_links=cluster_child["call_chain"]["links"],
            plan=cluster_plan,
            contract=cluster_contract,
            parent_token=cluster_root,
            replay_state_dir=replay_dir,
        )
        if replayed["verified"] or "TOKEN_REPLAYED" not in replayed["reason_codes"]:
            failures.append("cluster-rollout child-token replay did not fail closed with TOKEN_REPLAYED")

    wrong_audience = verify_token(
        cluster_child,
        issuer_keys=issuer_keys,
        verification_time="2026-03-20T13:05:22Z",
        expected_holder_id="urn:guild:service:kube-api-client",
        expected_audiences=["cluster-staging"],
        expected_resources=[cluster_child_resource_binding()],
        expected_runtime_guarantee_id="urn:guild:runtime:wasmtime-strict:v1",
        expected_call_chain_links=cluster_child["call_chain"]["links"],
        plan=cluster_plan,
        contract=cluster_contract,
        parent_token=cluster_root,
        check_replay=False,
    )
    if wrong_audience["verified"] or "AUDIENCE_MISMATCH" not in wrong_audience["reason_codes"]:
        failures.append("wrong-audience verification did not fail closed with AUDIENCE_MISMATCH")

    wrong_holder = verify_token(
        cluster_child,
        issuer_keys=issuer_keys,
        verification_time="2026-03-20T13:05:23Z",
        expected_holder_id="urn:guild:service:wrong-child",
        expected_audiences=["cluster-prod"],
        expected_resources=[cluster_child_resource_binding()],
        expected_runtime_guarantee_id="urn:guild:runtime:wasmtime-strict:v1",
        expected_call_chain_links=cluster_child["call_chain"]["links"],
        plan=cluster_plan,
        contract=cluster_contract,
        parent_token=cluster_root,
        check_replay=False,
    )
    if wrong_holder["verified"] or "HOLDER_BINDING_MISMATCH" not in wrong_holder["reason_codes"]:
        failures.append("wrong-holder verification did not fail closed with HOLDER_BINDING_MISMATCH")

    passthrough = verify_token(
        cluster_root,
        issuer_keys=issuer_keys,
        verification_time="2026-03-20T13:05:24Z",
        expected_holder_id="urn:guild:service:kube-api-client",
        expected_audiences=["cluster-prod"],
        expected_resources=[
            {
                "effect_class": "net.connect",
                "audience": "cluster-prod",
                "resource": "https://kube-api.prod.example.internal/apis/",
            }
        ],
        expected_runtime_guarantee_id="urn:guild:runtime:wasmtime-strict:v1",
        expected_call_chain_links=cluster_root["call_chain"]["links"],
        plan=cluster_plan,
        contract=cluster_contract,
        check_replay=False,
    )
    if passthrough["verified"] or "TOKEN_PASSTHROUGH_FORBIDDEN" not in passthrough["reason_codes"]:
        failures.append("passthrough attempt did not fail closed with TOKEN_PASSTHROUGH_FORBIDDEN")

    broadened_authority = cluster_child_authority_plan()
    broadened_authority["grants"][0]["cardinality"]["max_calls"] = 25
    broadened_authority["grants"][0]["scope"]["audiences"][0]["path_prefixes"] = ["/apis/"]
    broadened_authority["delegation_policy"]["ttl_seconds_max"] = 60
    broadened_child = create_child_token(
        cluster_root,
        cluster_plan,
        cluster_contract,
        broadened_authority,
        issuer,
        holder_id="urn:guild:service:kube-api-client",
        issued_at="2026-03-20T13:05:25Z",
        audiences=["cluster-prod", "cluster-staging"],
        resource_bindings=[cluster_child_resource_binding()],
        token_id="urn:guild:token:cluster-rollout:child-broadened:v1",
    )
    if broadened_child.get("issued") is not False or "PARENT_CHILD_AUDIENCE_BROADENING" not in broadened_child.get("reason_codes", []):
        failures.append("broadening child issuance did not fail closed with PARENT_CHILD_AUDIENCE_BROADENING")

    chain_mismatch = verify_token(
        cluster_child,
        issuer_keys=issuer_keys,
        verification_time="2026-03-20T13:05:26Z",
        expected_holder_id="urn:guild:service:kube-api-client",
        expected_audiences=["cluster-prod"],
        expected_resources=[cluster_child_resource_binding()],
        expected_runtime_guarantee_id="urn:guild:runtime:wasmtime-strict:v1",
        expected_call_chain_links=cluster_root["call_chain"]["links"],
        plan=cluster_plan,
        contract=cluster_contract,
        parent_token=cluster_root,
        check_replay=False,
    )
    if chain_mismatch["verified"] or "CALL_CHAIN_MISMATCH" not in chain_mismatch["reason_codes"]:
        failures.append("chain-mismatch verification did not fail closed with CALL_CHAIN_MISMATCH")

    runtime_mismatch = verify_token(
        local_root,
        issuer_keys=issuer_keys,
        verification_time="2026-03-20T13:00:30Z",
        expected_holder_id="urn:guild:service:local-log-analyzer",
        expected_audiences=[],
        expected_resources=[],
        expected_runtime_guarantee_id="urn:guild:runtime:node-wasi-basic:v1",
        expected_call_chain_links=local_root["call_chain"]["links"],
        plan=local_plan,
        contract=local_contract,
        proof=local_proof,
        check_replay=False,
    )
    if runtime_mismatch["verified"] or "RUNTIME_BINDING_MISMATCH" not in runtime_mismatch["reason_codes"]:
        failures.append("runtime-mismatch verification did not fail closed with RUNTIME_BINDING_MISMATCH")

    expired = verify_token(
        zero_root,
        issuer_keys=issuer_keys,
        verification_time="2026-03-20T13:17:01Z",
        expected_holder_id="urn:guild:service:zero-authority",
        expected_audiences=[],
        expected_resources=[],
        expected_runtime_guarantee_id="urn:guild:runtime:wasmtime-strict:v1",
        expected_call_chain_links=zero_root["call_chain"]["links"],
        plan=zero_plan,
        contract=zero_contract,
        proof=zero_proof,
    )
    if expired["verified"] or "TOKEN_EXPIRED" not in expired["reason_codes"]:
        failures.append("expired-token verification did not fail closed with TOKEN_EXPIRED")

    return failures


def verify_witness_cases() -> list[str]:
    failures: list[str] = []
    fixtures = build_witness_fixtures()
    fixtures_repeat = build_witness_fixtures()
    issuer_keys = {
        "urn:guild:issuer:draft-control-plane:v1": {
            "draft-hmac-2026-03": "guild-draft-shared-secret-2026-03"
        }
    }

    for filename, data in fixtures.items():
        checked = load_json(f"examples/{filename}")
        if canonical_json(data["witness"]) != canonical_json(checked):
            failures.append(f"{filename}: generated witness did not match the checked example")
        if canonical_json(data["witness"]) != canonical_json(fixtures_repeat[filename]["witness"]):
            failures.append(f"{filename}: repeated witness generation was non-deterministic")

    within = fixtures["local-log-analyzer.within-envelope.witness.json"]
    within_verification = verify_witness(
        within["witness"],
        issuer_keys=issuer_keys,
        verification_time="2026-03-20T16:00:00Z",
        plan=within["plan"],
        contract=within["contract"],
        proof=within["proof"],
        token=within["token"],
    )
    if not within_verification["verified"] or within_verification["witness_status"] != "within_envelope":
        failures.append("within-envelope witness did not verify cleanly as within_envelope")
    within_claim = verify_claim(
        within["witness"],
        {"claim_type": "no_authority_use_outside_token"},
        issuer_keys=issuer_keys,
        verification_time="2026-03-20T16:00:00Z",
        plan=within["plan"],
        contract=within["contract"],
        proof=within["proof"],
        token=within["token"],
    )
    if within_claim["claim_evaluation"]["status"] != "satisfied":
        failures.append("within-envelope witness did not satisfy the proof-backed token absence claim")

    out_of_envelope = fixtures["local-log-analyzer.out-of-envelope.witness.json"]
    out_verification = verify_witness(
        out_of_envelope["witness"],
        issuer_keys=issuer_keys,
        verification_time="2026-03-20T16:00:00Z",
        plan=out_of_envelope["plan"],
        contract=out_of_envelope["contract"],
        proof=out_of_envelope["proof"],
        token=out_of_envelope["token"],
    )
    if not out_verification["verified"] or out_verification["witness_status"] != "out_of_envelope":
        failures.append("out-of-envelope witness did not verify as an authentic out_of_envelope record")
    out_claim = verify_claim(
        out_of_envelope["witness"],
        {"claim_type": "no_authority_use_outside_token"},
        issuer_keys=issuer_keys,
        verification_time="2026-03-20T16:00:00Z",
        plan=out_of_envelope["plan"],
        contract=out_of_envelope["contract"],
        proof=out_of_envelope["proof"],
        token=out_of_envelope["token"],
    )
    if out_claim["claim_evaluation"]["status"] != "violated" or "OBSERVED_EFFECT_OUTSIDE_TOKEN" not in out_claim["claim_evaluation"]["reason_codes"]:
        failures.append("out-of-envelope witness did not report token-envelope violation correctly")

    coverage_limited = fixtures["fetch-transform.coverage-limited.witness.json"]
    coverage_verification = verify_witness(
        coverage_limited["witness"],
        issuer_keys=issuer_keys,
        verification_time="2026-03-20T16:00:00Z",
        plan=coverage_limited["plan"],
        contract=coverage_limited["contract"],
    )
    if not coverage_verification["verified"] or coverage_verification["witness_status"] != "coverage_limited":
        failures.append("coverage-limited witness did not verify as coverage_limited")
    coverage_claim = verify_claim(
        coverage_limited["witness"],
        {
            "claim_type": "no_network_egress_except_allowlist",
            "network_allowlist": [
                {
                    "host": "api.vendor.example.com",
                    "ports": [443],
                    "schemes": ["https"],
                    "path_prefixes": ["/v1/source/daily.json"],
                    "methods": ["GET"],
                }
            ],
        },
        issuer_keys=issuer_keys,
        verification_time="2026-03-20T16:00:00Z",
        plan=coverage_limited["plan"],
        contract=coverage_limited["contract"],
    )
    if coverage_claim["claim_evaluation"]["status"] != "not_provable" or "CLAIM_NOT_PROVABLE_FROM_COVERAGE" not in coverage_claim["claim_evaluation"]["reason_codes"]:
        failures.append("coverage-limited witness did not fail closed for the network absence claim")

    redacted = fixtures["fetch-transform.redacted-claim-blocked.witness.json"]
    redacted_verification = verify_witness(
        redacted["witness"],
        issuer_keys=issuer_keys,
        verification_time="2026-03-20T16:00:00Z",
        plan=redacted["plan"],
        contract=redacted["contract"],
    )
    if not redacted_verification["verified"] or redacted_verification["witness_status"] != "within_envelope":
        failures.append("counts-only redacted witness did not remain a valid within_envelope record")
    redacted_claim = verify_claim(
        redacted["witness"],
        {
            "claim_type": "no_filesystem_writes_outside_prefixes",
            "paths": ["/workspace/output/**"],
        },
        issuer_keys=issuer_keys,
        verification_time="2026-03-20T16:00:00Z",
        plan=redacted["plan"],
        contract=redacted["contract"],
    )
    if redacted_claim["claim_evaluation"]["status"] != "not_provable" or "REDACTION_PREVENTS_CLAIM_VERIFICATION" not in redacted_claim["claim_evaluation"]["reason_codes"]:
        failures.append("redacted witness did not fail closed for the filesystem absence claim")

    blocked = fixtures["fetch-transform.blocked-attempt.witness.json"]
    blocked_verification = verify_witness(
        blocked["witness"],
        issuer_keys=issuer_keys,
        verification_time="2026-03-20T16:00:00Z",
        plan=blocked["plan"],
        contract=blocked["contract"],
    )
    if not blocked_verification["verified"] or blocked_verification["witness_status"] != "within_envelope":
        failures.append("blocked-attempt witness did not verify as a valid within_envelope record")
    if "net.connect" in blocked["witness"]["actual_exercised_authority"]["effect_classes"]:
        failures.append("blocked-attempt witness incorrectly conflated blocked network activity with exercised authority")
    if "net.connect" not in blocked["witness"]["blocked_attempted_authority"]["effect_classes"]:
        failures.append("blocked-attempt witness did not preserve blocked network activity separately")
    blocked_claim = verify_claim(
        blocked["witness"],
        {
            "claim_type": "no_blocked_attempts_of_classes",
            "effect_classes": ["net.connect"],
        },
        issuer_keys=issuer_keys,
        verification_time="2026-03-20T16:00:00Z",
        plan=blocked["plan"],
        contract=blocked["contract"],
    )
    if blocked_claim["claim_evaluation"]["status"] != "violated":
        failures.append("blocked-attempt witness did not report the blocked net.connect attempt")

    cluster = fixtures["cluster-rollout.witness.json"]
    cluster_verification = verify_witness(
        cluster["witness"],
        issuer_keys=issuer_keys,
        verification_time="2026-03-20T16:00:00Z",
        plan=cluster["plan"],
        contract=cluster["contract"],
        token=cluster["token"],
        parent_token=cluster["parent_token"],
    )
    if not cluster_verification["verified"] or cluster_verification["witness_status"] != "within_envelope":
        failures.append("delegation-chain witness did not verify as within_envelope")
    cluster_claim_ok = verify_claim(
        cluster["witness"],
        {
            "claim_type": "no_delegation_beyond_hops",
            "max_hops": 1,
        },
        issuer_keys=issuer_keys,
        verification_time="2026-03-20T16:00:00Z",
        plan=cluster["plan"],
        contract=cluster["contract"],
        token=cluster["token"],
        parent_token=cluster["parent_token"],
    )
    if cluster_claim_ok["claim_evaluation"]["status"] != "satisfied":
        failures.append("delegation-chain witness did not satisfy the one-hop delegation claim")
    cluster_claim_bad = verify_claim(
        cluster["witness"],
        {
            "claim_type": "no_delegation_beyond_hops",
            "max_hops": 0,
        },
        issuer_keys=issuer_keys,
        verification_time="2026-03-20T16:00:00Z",
        plan=cluster["plan"],
        contract=cluster["contract"],
        token=cluster["token"],
        parent_token=cluster["parent_token"],
    )
    if cluster_claim_bad["claim_evaluation"]["status"] != "violated":
        failures.append("delegation-chain witness did not fail the zero-hop delegation claim")

    zero = fixtures["zero-authority.witness.json"]
    zero_verification = verify_witness(
        zero["witness"],
        issuer_keys=issuer_keys,
        verification_time="2026-03-20T16:00:00Z",
        plan=zero["plan"],
        contract=zero["contract"],
        proof=zero["proof"],
        token=zero["token"],
    )
    if not zero_verification["verified"] or zero_verification["witness_status"] != "within_envelope":
        failures.append("zero-authority witness did not verify cleanly")
    if zero["witness"]["actual_exercised_authority"]["total_effects"] != 0:
        failures.append("zero-authority witness unexpectedly recorded exercised authority")
    if zero["witness"]["blocked_attempted_authority"]["total_effects"] != 0:
        failures.append("zero-authority witness unexpectedly recorded blocked attempts")
    zero_claim = verify_claim(
        zero["witness"],
        {"claim_type": "no_authority_use_outside_token"},
        issuer_keys=issuer_keys,
        verification_time="2026-03-20T16:00:00Z",
        plan=zero["plan"],
        contract=zero["contract"],
        proof=zero["proof"],
        token=zero["token"],
    )
    if zero_claim["claim_evaluation"]["status"] != "satisfied":
        failures.append("zero-authority witness did not satisfy the no-authority-use claim")

    mapping = fixtures["runtime-mapping-limited.witness.json"]
    mapping_verification = verify_witness(
        mapping["witness"],
        issuer_keys=issuer_keys,
        verification_time="2026-03-20T16:00:00Z",
        plan=mapping["plan"],
        contract=mapping["contract"],
    )
    if not mapping_verification["verified"] or mapping_verification["witness_status"] != "coverage_limited":
        failures.append("runtime-mapping-limited witness did not verify as coverage_limited")
    mapping_claim = verify_claim(
        mapping["witness"],
        {"claim_type": "no_authority_use_outside_plan"},
        issuer_keys=issuer_keys,
        verification_time="2026-03-20T16:00:00Z",
        plan=mapping["plan"],
        contract=mapping["contract"],
    )
    if mapping_claim["claim_evaluation"]["status"] != "not_provable" or "CLAIM_NOT_PROVABLE_FROM_COVERAGE" not in mapping_claim["claim_evaluation"]["reason_codes"]:
        failures.append("runtime-mapping-limited witness did not fail closed for an absence claim")

    runtime_mismatch = fixtures["local-log-analyzer.runtime-mismatch.witness.json"]
    mismatch_verification = verify_witness(
        runtime_mismatch["witness"],
        issuer_keys=issuer_keys,
        verification_time="2026-03-20T16:00:00Z",
        plan=runtime_mismatch["plan"],
        contract=runtime_mismatch["contract"],
        proof=runtime_mismatch["proof"],
        token=runtime_mismatch["token"],
    )
    if mismatch_verification["verified"] or "RUNTIME_BINDING_MISMATCH" not in mismatch_verification["reason_codes"]:
        failures.append("runtime-binding-mismatch witness did not fail closed with RUNTIME_BINDING_MISMATCH")

    return failures


def verify_live_runtime_alignment_cases() -> list[str]:
    failures: list[str] = []
    registry = build_registry()
    issuer = m6_issuer()
    issuer_keys = m6_issuer_keys()
    runtime = load_json("examples/wasmtime-strict.runtime.json")

    http_contract = load_json("examples/runtime-http-read.contract.json")
    http_request = load_json("examples/runtime-http-read.admit.request.json")
    http_invocation = load_json("examples/runtime-http-read.invocation.json")
    http_record = load_json("examples/runtime-http-success.execution-record.json")
    read_contract = load_json("examples/runtime-read-resource.contract.json")
    read_request = load_json("examples/runtime-read-resource.admit.request.json")
    read_invocation = load_json("examples/runtime-read-resource.invocation.json")
    read_record = load_json("examples/runtime-read-resource.execution-record.json")
    log_contract = load_json("examples/runtime-log-write.contract.json")
    log_request = load_json("examples/runtime-log-write.admit.request.json")

    http_plan = build_execution_plan(http_contract, http_request, [runtime])
    read_plan = build_execution_plan(read_contract, read_request, [runtime])
    log_plan = build_execution_plan(log_contract, log_request, [runtime])
    for label, plan in (
        ("runtime-http-read", http_plan),
        ("runtime-read-resource", read_plan),
        ("runtime-log-write", log_plan),
    ):
        if plan["decision"] != "admit":
            failures.append(f"{label} admission did not produce an admit plan")
            return failures

    read_proof, _read_scenario = build_live_runtime_proof_record(
        scenario_name="read-resource-bounded",
        plan=read_plan,
        contract=read_contract,
        runtime=runtime,
        invocation_input=read_invocation,
        created_at="2026-03-20T20:15:30Z",
    )
    read_proof_errors = validate_instance("proof_record.schema.json", read_proof, registry)
    failures.extend(f"runtime-read-resource live proof schema validation failed: {error}" for error in read_proof_errors)
    read_family_status = next(
        (entry for entry in read_proof["family_proof_statuses"] if entry["family"] == "read-resource"),
        None,
    )
    if read_proof["proof_source_kind"] != PROOF_SOURCE_LIVE_RUNTIME:
        failures.append("runtime-read-resource live proof record did not mark proof_source_kind as live-runtime")
    if read_proof["proof_status"] != "bounded_minimal":
        failures.append("runtime-read-resource live proof did not stay bounded_minimal")
    if (
        read_family_status is None
        or read_family_status["support"] != "bounded-live-proof"
        or "LIVE_PROOF_BOUNDED" not in read_family_status["reason_codes"]
    ):
        failures.append("runtime-read-resource live proof did not carry honest bounded family support metadata")
    if read_proof["residual_authority_plan"]["grants"]:
        failures.append("runtime-read-resource live proof unexpectedly left residual authority outside the proven envelope")

    read_token = create_root_token(
        read_plan,
        read_contract,
        issuer,
        holder_id="urn:guild:service:runtime-read-resource",
        issued_at="2026-03-20T20:15:45Z",
        proof=read_proof,
        required_proof_source_kind=PROOF_SOURCE_LIVE_RUNTIME,
        audiences=["runtime-read-resource"],
        resource_bindings=[
            {
                "family": "read-resource",
                "audience": "runtime-read-resource",
                "resource": "guild://executions/example-run",
            }
        ],
        chain_links=["urn:guild:actor:runtime-alignment-test"],
    )
    if read_token.get("kind") != "guild.delegated_capability_token":
        failures.append("runtime-read-resource live proof-backed token issuance did not produce a delegated capability token")
        return failures
    if read_token["issuance_basis"] != "m5_proven_subset" or read_token.get("proof_source_kind") != PROOF_SOURCE_LIVE_RUNTIME:
        failures.append("runtime-read-resource token did not stay explicitly live proof-backed")

    read_token_verification = verify_token(
        read_token,
        issuer_keys=issuer_keys,
        verification_time="2026-03-20T20:15:50Z",
        expected_holder_id="urn:guild:service:runtime-read-resource",
        expected_audiences=["runtime-read-resource"],
        expected_resources=[
            {
                "family": "read-resource",
                "audience": "runtime-read-resource",
                "resource": "guild://executions/example-run",
            }
        ],
        expected_runtime_guarantee_id="urn:guild:runtime:wasmtime-strict:v1",
        expected_call_chain_links=read_token["call_chain"]["links"],
        plan=read_plan,
        contract=read_contract,
        proof=read_proof,
        check_replay=False,
    )
    if not read_token_verification["verified"] or read_token_verification["decision"] != "allow":
        failures.append("runtime-read-resource live proof-backed token did not verify cleanly")

    read_witness = generate_witness(
        plan=read_plan,
        contract=read_contract,
        issuer=issuer,
        issuer_keys=issuer_keys,
        issued_at="2026-03-20T20:16:00Z",
        invocation_input=read_invocation,
        proof=read_proof,
        required_proof_source_kind=PROOF_SOURCE_LIVE_RUNTIME,
        token=read_token,
        observation={
            "source_kind": LIVE_RUNTIME_SOURCE_KIND,
            "execution_record": read_record,
        },
        redaction_profile="none",
    )
    if read_witness["proof_basis"] is None or read_witness["proof_basis"]["proof_source_kind"] != PROOF_SOURCE_LIVE_RUNTIME:
        failures.append("runtime-read-resource witness did not keep an honest live proof linkage")
    read_verification = verify_witness(
        read_witness,
        issuer_keys=issuer_keys,
        verification_time="2026-03-20T20:16:30Z",
        plan=read_plan,
        contract=read_contract,
        proof=read_proof,
        token=read_token,
    )
    if not read_verification["verified"] or read_verification["witness_status"] != "within_envelope":
        failures.append("runtime-read-resource live witness did not verify as within_envelope")
    read_claim = verify_claim(
        read_witness,
        {
            "claim_type": "no_authority_use_outside_proof",
        },
        issuer_keys=issuer_keys,
        verification_time="2026-03-20T20:16:30Z",
        plan=read_plan,
        contract=read_contract,
        proof=read_proof,
        token=read_token,
    )
    if read_claim["claim_evaluation"]["status"] != "satisfied":
        failures.append("runtime-read-resource live witness did not satisfy the proof-envelope absence claim")
    read_scope_claim = verify_claim(
        read_witness,
        {
            "claim_type": "no_read_resource_outside_scope",
            "read_resource_scope": {
                "kind": "resource",
                "uri_prefixes": [
                    "guild://executions/",
                ],
                "resource_kinds": ["execution"],
            },
        },
        issuer_keys=issuer_keys,
        verification_time="2026-03-20T20:16:30Z",
        plan=read_plan,
        contract=read_contract,
        proof=read_proof,
        token=read_token,
    )
    if read_scope_claim["claim_evaluation"]["status"] != "satisfied":
        failures.append("runtime-read-resource live witness did not satisfy the bounded canonical read-resource claim")

    http_proof, _http_scenario = build_live_runtime_proof_record(
        scenario_name="http-request-not-proven",
        plan=http_plan,
        contract=http_contract,
        runtime=runtime,
        invocation_input=http_invocation,
        created_at="2026-03-20T20:10:00Z",
    )
    http_proof_errors = validate_instance("proof_record.schema.json", http_proof, registry)
    failures.extend(f"runtime-http-read live proof schema validation failed: {error}" for error in http_proof_errors)
    http_family_status = next(
        (entry for entry in http_proof["family_proof_statuses"] if entry["family"] == "http-request"),
        None,
    )
    if http_proof["proof_status"] != "not_proven":
        failures.append("runtime-http-read live proof did not stay honest as not_proven")
    if (
        http_family_status is None
        or http_family_status["support"] != "not-proven"
        or "LIVE_REPLAY_UNAVAILABLE" not in http_family_status["reason_codes"]
    ):
        failures.append("runtime-http-read live proof did not preserve the replay-unavailable not_proven status")

    http_token = create_root_token(
        http_plan,
        http_contract,
        issuer,
        holder_id="urn:guild:service:runtime-http-read",
        issued_at="2026-03-20T20:10:30Z",
        proof=http_proof,
        required_proof_source_kind=PROOF_SOURCE_LIVE_RUNTIME,
        allow_upper_bound=True,
        audiences=["runtime-http-read"],
        resource_bindings=[
            {
                "family": "http-request",
                "audience": "runtime-http-read",
                "resource": "GET:http://127.0.0.1:18080/response.json",
            }
        ],
        chain_links=["urn:guild:actor:runtime-alignment-test"],
    )
    if http_token.get("kind") != "guild.delegated_capability_token":
        failures.append("runtime-http-read upper-bound token issuance did not produce a delegated capability token")
        return failures
    if http_token["issuance_basis"] != "m4_upper_bound" or http_token.get("proof_id") is not None:
        failures.append("runtime-http-read token issuance did not fall back honestly to the upper-bound basis")

    http_witness = generate_witness(
        plan=http_plan,
        contract=http_contract,
        issuer=issuer,
        issuer_keys=issuer_keys,
        issued_at="2026-03-20T20:11:00Z",
        invocation_input=http_invocation,
        proof=http_proof,
        required_proof_source_kind=PROOF_SOURCE_LIVE_RUNTIME,
        token=http_token,
        observation={
            "source_kind": LIVE_RUNTIME_SOURCE_KIND,
            "execution_record": http_record,
        },
        redaction_profile="none",
    )
    if http_witness["proof_basis"] is not None:
        failures.append("runtime-http-read witness incorrectly linked an unavailable live proof")
    if "WITNESS_PROOF_LINKAGE_UNAVAILABLE" not in http_witness["reason_codes"]:
        failures.append("runtime-http-read witness did not preserve the unavailable live proof linkage reason")
    http_verification = verify_witness(
        http_witness,
        issuer_keys=issuer_keys,
        verification_time="2026-03-20T20:12:00Z",
        plan=http_plan,
        contract=http_contract,
        proof=http_proof,
        token=http_token,
    )
    if not http_verification["verified"] or http_verification["witness_status"] != "within_envelope":
        failures.append("runtime-http-read live witness did not verify as within_envelope after proof linkage was withheld")
    http_claim = verify_claim(
        http_witness,
        {
            "claim_type": "no_http_request_outside_scope",
            "http_request_scope": {
                "kind": "network",
                "allowed_schemes": ["http"],
                "allowed_hosts": ["127.0.0.1"],
                "allowed_ports": [18080],
                "allowed_methods": ["GET"],
                "allowed_path_prefixes": ["/response.json"],
            },
        },
        issuer_keys=issuer_keys,
        verification_time="2026-03-20T20:12:00Z",
        plan=http_plan,
        contract=http_contract,
        proof=http_proof,
        token=http_token,
    )
    if http_claim["claim_evaluation"]["status"] != "satisfied":
        failures.append("runtime-http-read live witness did not satisfy the covered canonical http-request claim")

    log_live = load_live_proof_scenario("log-write-reduced")
    log_family_status = next(
        (entry for entry in log_live["proof"]["family_statuses"] if entry["family"] == "log-write"),
        None,
    )
    emit_family_status = next(
        (entry for entry in log_live["proof"]["family_statuses"] if entry["family"] == "emit-evidence"),
        None,
    )
    if (
        log_family_status is None
        or log_family_status["support"] != "live-proof-supported"
        or log_family_status["proof_status"] != "exact_minimal"
    ):
        failures.append("runtime-log-write live proof did not prove exact family support over the observed log slice")
    if emit_family_status is None or emit_family_status["support"] != "not-proven":
        failures.append("runtime-log-write live proof did not keep emit-evidence explicitly outside the proven slice")

    legacy_http_mapping = runtime_mapping_for_effect(load_json("examples/fetch-transform.contract.json")["required_effects"][2])
    if legacy_http_mapping["mapping_status"] != "narrowing" or legacy_http_mapping["family"] != "http-request":
        failures.append("legacy net.connect compatibility mapping did not stay an explicit narrowing to http-request")
    legacy_invoke_mapping = runtime_mapping_for_effect(load_json("examples/cluster-rollout.contract.json")["required_effects"][3])
    if legacy_invoke_mapping["mapping_status"] != "narrowing" or legacy_invoke_mapping["family"] != "invoke-skill":
        failures.append("legacy component.invoke compatibility mapping did not stay an explicit narrowing to invoke-skill")
    rejected_legacy_http_mapping = runtime_mapping_for_effect(load_json("examples/cluster-rollout.contract.json")["required_effects"][1])
    if rejected_legacy_http_mapping["mapping_status"] != "unsupported":
        failures.append("broad legacy net.connect scopes were not rejected when direct canonical http-request support was narrower")

    return failures


def verify_family_support_matrix() -> list[str]:
    failures: list[str] = []
    matrix = load_json("family_support_matrix.json")

    expected_layers = {
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
        "proof_witness_linkage",
    }
    expected_families = {
        "http-request",
        "read-resource",
        "invoke-skill",
        "emit-evidence",
        "log-write",
    }

    if matrix.get("kind") != "guild.family_support_matrix":
        failures.append("family_support_matrix.json kind did not stay guild.family_support_matrix")
    if matrix.get("canonical_runtime_vocabulary") is not True:
        failures.append("family_support_matrix.json did not keep the live runtime vocabulary canonical")
    if set(matrix.get("layers", [])) != expected_layers:
        failures.append("family_support_matrix.json layers did not match the expected M8c support matrix shape")

    families = matrix.get("families", {})
    if set(families.keys()) != expected_families:
        failures.append("family_support_matrix.json families did not match the active canonical runtime set")

    for family in sorted(expected_families):
        layer_map = families.get(family, {}).get("layers", {})
        if set(layer_map.keys()) != expected_layers:
            failures.append(f"family_support_matrix.json layer set for {family} did not match the expected shape")
            continue
        if layer_map["admission_runtime_guarantee_matching"]["status"] != "supported":
            failures.append(f"family_support_matrix.json did not mark {family} admission matching as supported")
        if layer_map["execution_plan_representation"]["status"] != "supported":
            failures.append(f"family_support_matrix.json did not mark {family} execution-plan representation as supported")
        if layer_map["token_issuance_basis"]["status"] != "supported":
            failures.append(f"family_support_matrix.json did not mark {family} token issuance basis as supported")
        if layer_map["token_verification"]["status"] != "supported":
            failures.append(f"family_support_matrix.json did not mark {family} token verification as supported")
        if layer_map["witness_generation"]["status"] != "supported":
            failures.append(f"family_support_matrix.json did not mark {family} witness generation as supported")
        if layer_map["witness_verification"]["status"] != "supported":
            failures.append(f"family_support_matrix.json did not mark {family} witness verification as supported")
        if layer_map["positive_claim_verification"]["status"] != "unsupported":
            failures.append(f"family_support_matrix.json did not keep {family} positive-claim verification unsupported")
        if layer_map["negative_claim_verification"]["status"] != "supported":
            failures.append(f"family_support_matrix.json did not mark {family} negative-claim verification as supported")

    if families["read-resource"]["layers"]["live_minimization_proof"]["status"] != "bounded":
        failures.append("family_support_matrix.json did not mark read-resource live minimization proof as bounded")
    if families["read-resource"]["layers"]["plan_proof_token_linkage"]["status"] != "bounded":
        failures.append("family_support_matrix.json did not mark read-resource plan->proof->token linkage as bounded")
    if families["read-resource"]["layers"]["proof_witness_linkage"]["status"] != "bounded":
        failures.append("family_support_matrix.json did not mark read-resource proof->witness linkage as bounded")

    if families["log-write"]["layers"]["live_minimization_proof"]["status"] != "supported":
        failures.append("family_support_matrix.json did not mark log-write live minimization proof as supported")
    if families["http-request"]["layers"]["live_minimization_proof"]["status"] != "not_proven":
        failures.append("family_support_matrix.json did not keep http-request live minimization proof not_proven")
    if families["invoke-skill"]["layers"]["live_minimization_proof"]["status"] != "not_proven":
        failures.append("family_support_matrix.json did not keep invoke-skill live minimization proof not_proven")
    if families["emit-evidence"]["layers"]["live_minimization_proof"]["status"] != "not_proven":
        failures.append("family_support_matrix.json did not keep emit-evidence live minimization proof not_proven")
    if families["http-request"]["layers"]["proof_witness_linkage"]["status"] != "not_proven":
        failures.append("family_support_matrix.json did not keep http-request proof->witness linkage unavailable")

    aliases = matrix.get("compatibility_aliases", {})
    if aliases.get("net.connect", {}).get("status") != "partial":
        failures.append("family_support_matrix.json did not keep net.connect as an explicit partial compatibility alias")
    if aliases.get("component.invoke", {}).get("status") != "partial":
        failures.append("family_support_matrix.json did not keep component.invoke as an explicit partial compatibility alias")
    if aliases.get("net.resolve", {}).get("status") != "unsupported":
        failures.append("family_support_matrix.json did not keep net.resolve rejected for M8c")

    return failures


def main() -> int:
    registry = build_registry()
    failures: list[str] = []
    failures.extend(verify_examples(registry))
    failures.extend(verify_admission_cases())
    failures.extend(verify_invalid_runtime_probes(registry))
    failures.extend(verify_minimization_cases())
    failures.extend(verify_token_cases())
    failures.extend(verify_witness_cases())
    failures.extend(verify_live_runtime_alignment_cases())
    failures.extend(verify_family_support_matrix())

    if failures:
        print("Validation failed:")
        for failure in failures:
            print(f" - {failure}")
        return 1

    print("All bundled examples, admission cases, minimization cases, token cases, witness cases, and live runtime alignment cases validate cleanly.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
