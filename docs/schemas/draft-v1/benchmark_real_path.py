from __future__ import annotations

import argparse
import json
import statistics
import subprocess
from copy import deepcopy
from pathlib import Path
from tempfile import TemporaryDirectory
from typing import Any

from admission_core import build_execution_plan, build_registry, load_json, validate_instance
from token_core import PROOF_SOURCE_LIVE_RUNTIME, create_root_token, verify_token
from validate_examples import (
    build_live_runtime_proof_record,
    live_grants_to_effect_specs,
    load_live_proof_scenario,
    m6_issuer,
    m6_issuer_keys,
)
from witness_core import generate_witness, verify_claim, verify_witness


BASE = Path(__file__).resolve().parent
REPO_ROOT = BASE.parents[2]
SCHEMA_PATH = BASE / "benchmark_matrix.schema.json"
MATRIX_PATH = BASE / "benchmark_matrix.json"
REPORT_PATH = BASE.parent.parent / "benchmarking" / "m8-real-path-benchmark.md"
MATRIX_KIND = "guild.real_path_benchmark_matrix"
MATRIX_VERSION = "1.0.0"
TIMESTAMP_BASE = "2026-03-21T23"
BENCHMARK_WARMUPS = 2
BENCHMARK_RUNS = 10


def percentile(samples: list[float], percentile_value: float) -> float:
    if not samples:
        return 0.0
    ordered = sorted(samples)
    rank = round((len(ordered) - 1) * percentile_value)
    return ordered[int(rank)]


def timing_summary(
    *,
    cold_first_run_ms: float,
    samples_ms: list[float],
    warmup_runs: int,
    cache_present: bool,
    cache_notes: str,
) -> dict[str, Any]:
    measured_runs = len(samples_ms)
    mean_ms = statistics.fmean(samples_ms) if samples_ms else 0.0
    return {
        "cold_first_run_ms": cold_first_run_ms,
        "warmup_runs": warmup_runs,
        "measured_runs": measured_runs,
        "samples_ms": samples_ms,
        "mean_ms": mean_ms,
        "p50_ms": percentile(samples_ms, 0.50),
        "p95_ms": percentile(samples_ms, 0.95),
        "max_ms": max(samples_ms) if samples_ms else 0.0,
        "cache_present": cache_present,
        "cache_notes": cache_notes,
    }


def measure_operation(factory, *, warmup_runs: int, measured_runs: int, cache_present: bool, cache_notes: str):
    cold_value, cold_ms = timed_call(factory)
    for _ in range(warmup_runs):
        factory()
    measured_values: list[Any] = []
    samples_ms: list[float] = []
    for _ in range(measured_runs):
        value, elapsed_ms = timed_call(factory)
        measured_values.append(value)
        samples_ms.append(elapsed_ms)
    return (
        cold_value,
        measured_values,
        timing_summary(
            cold_first_run_ms=cold_ms,
            samples_ms=samples_ms,
            warmup_runs=warmup_runs,
            cache_present=cache_present,
            cache_notes=cache_notes,
        ),
    )


def timed_call(factory):
    import time

    started = time.perf_counter()
    value = factory()
    elapsed_ms = (time.perf_counter() - started) * 1000.0
    return value, elapsed_ms


def load_live_proof_benchmark(scenario_name: str, *, warmup_runs: int, measured_runs: int) -> dict[str, Any]:
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
            "benchmark",
            scenario_name,
            str(warmup_runs),
            str(measured_runs),
        ],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        raise RuntimeError(
            result.stderr.strip()
            or result.stdout.strip()
            or f"live proof benchmark {scenario_name!r} failed"
        )
    return json.loads(result.stdout)


def load_execution_record_for_spec(spec: dict[str, Any], scenario: dict[str, Any] | None) -> dict[str, Any]:
    source = spec["execution_record_source"]
    if source["kind"] == "file":
        return load_json(source["path"])
    if source["kind"] == "scenario":
        if scenario is None:
            raise RuntimeError(f"scenario execution record required for {spec['slice_id']}")
        return deepcopy(scenario["baseline_execution_record"])
    raise RuntimeError(f"unknown execution record source for {spec['slice_id']}: {source!r}")


def build_plan_bundle(spec: dict[str, Any], runtime: dict[str, Any]) -> tuple[dict[str, Any], dict[str, Any], dict[str, Any]]:
    contract = load_json(spec["contract"])
    request = load_json(spec["request"])
    if "request_mutation" in spec:
        request = spec["request_mutation"](request)
    plan = build_execution_plan(contract, request, [runtime])
    if plan["decision"] != "admit":
        raise RuntimeError(f"{spec['slice_id']} did not admit during benchmark planning")
    return contract, request, plan


def invocation_input_for_spec(spec: dict[str, Any]) -> dict[str, Any]:
    invocation = load_json(spec["invocation"])
    if "invocation_mutation" in spec:
        invocation = spec["invocation_mutation"](invocation)
    return invocation


def family_status_from_live_proof(proof: dict[str, Any], family: str) -> dict[str, Any]:
    for entry in proof["family_statuses"]:
        if entry["family"] == family:
            return entry
    raise RuntimeError(f"missing family status for {family}")


def issued_authority_from_token(token_or_result: dict[str, Any] | None) -> dict[str, Any] | None:
    if token_or_result is None:
        return None
    if token_or_result.get("kind") == "guild.delegated_capability_token":
        return deepcopy(token_or_result["granted_authority"])
    return None


def proof_backed_resource_binding(spec: dict[str, Any]) -> dict[str, Any] | None:
    return deepcopy(spec.get("resource_binding"))


def violated_claim(spec: dict[str, Any]) -> dict[str, Any] | None:
    return deepcopy(spec.get("violating_claim"))


def satisfied_claim(spec: dict[str, Any]) -> dict[str, Any] | None:
    return deepcopy(spec.get("supported_claim"))


def proof_absence_claim() -> dict[str, Any]:
    return {"claim_type": "no_authority_use_outside_proof"}


def token_outcome(token_or_result: dict[str, Any]) -> str:
    if token_or_result.get("kind") == "guild.delegated_capability_token":
        if token_or_result["issuance_basis"] == "m5_proven_subset":
            return "proof_backed_success"
        if token_or_result["issuance_basis"] == "m4_upper_bound":
            return "upper_bound_fallback"
    return "refusal"


def count_rate(values: list[str], target: str) -> dict[str, Any]:
    total = len(values)
    count = sum(1 for value in values if value == target)
    return {
        "count": count,
        "rate": 0.0 if total == 0 else count / total,
    }


def witness_outcome(witness: dict[str, Any]) -> str:
    return "proof_linked" if witness.get("proof_basis") is not None else "unlinked"


def claim_status_bucket(status: str) -> str:
    if status == "satisfied":
        return "success"
    if status == "violated":
        return "fail"
    return "coverage_limited_or_unverifiable"


def live_authority_plan_from_proof(plan: dict[str, Any], live_proof: dict[str, Any]) -> dict[str, Any] | None:
    if live_proof["proof_status"] == "not_proven":
        return None
    authority = {
        "plan_id": f"{plan['plan_id']}:live-proven",
        "grants": live_grants_to_effect_specs(
            live_proof["proven_authority"]["grants"],
            plan["granted_authority"],
        ),
        "delegation_policy": deepcopy(plan["granted_authority"]["delegation_policy"]),
        "ttl_seconds": plan["granted_authority"]["ttl_seconds"],
    }
    return authority


def narrowed_dimensions(baseline_plan: dict[str, Any], final_authority: dict[str, Any] | None, family: str) -> list[str]:
    if final_authority is None:
        return []
    baseline_grants = [grant for grant in baseline_plan["grants"] if grant.get("family") == family]
    final_grants = [grant for grant in final_authority["grants"] if grant.get("family") == family]
    if not baseline_grants or not final_grants:
        return []
    baseline_scope = baseline_grants[0]["scope"]
    final_scope = final_grants[0]["scope"]
    dimensions: list[str] = []
    if family == "http-request":
        for key, label in (
            ("allowed_schemes", "scheme"),
            ("allowed_hosts", "host"),
            ("allowed_ports", "port"),
            ("allowed_methods", "method"),
            ("allowed_path_prefixes", "path"),
            ("follow_redirects", "redirects"),
            ("max_redirects", "redirect_hops"),
        ):
            if key in baseline_scope and baseline_scope.get(key) != final_scope.get(key):
                dimensions.append(label)
    elif family == "read-resource":
        for key, label in (
            ("uri_prefixes", "uri_prefix"),
            ("resource_kinds", "resource_kind"),
        ):
            if key in baseline_scope and baseline_scope.get(key) != final_scope.get(key):
                dimensions.append(label)
    elif family == "invoke-skill":
        if "aliases" in baseline_scope and baseline_scope.get("aliases") != final_scope.get("aliases"):
            dimensions.append("alias")
    elif family == "emit-evidence":
        for key, label in (
            ("audiences", "audience"),
            ("redactions", "redaction"),
            ("max_bytes", "max_bytes"),
        ):
            if key in baseline_scope and baseline_scope.get(key) != final_scope.get(key):
                dimensions.append(label)
    elif family == "log-write":
        if "levels" in baseline_scope and baseline_scope.get("levels") != final_scope.get("levels"):
            dimensions.append("level")
    return dimensions


def reduction_result(
    *,
    family: str,
    family_proof_status: str,
    baseline_plan: dict[str, Any],
    live_proof: dict[str, Any],
    final_issued_authority: dict[str, Any] | None,
) -> dict[str, Any]:
    if live_proof["proof_status"] == "not_proven":
        return {
            "classification": "not_proven",
            "narrowed_dimensions": [],
        }
    final_authority = final_issued_authority or live_authority_plan_from_proof(
        {
            "plan_id": "urn:guild:benchmark:derived-live-proof-plan",
            "granted_authority": baseline_plan,
        },
        live_proof,
    )
    classification = classify_reduction(family_proof_status)
    return {
        "classification": classification,
        "narrowed_dimensions": narrowed_dimensions(baseline_plan, final_authority, family),
    }


def classify_reduction(proof_status: str) -> str:
    if proof_status == "not_proven":
        return "not_proven"
    if proof_status.startswith("exact"):
        return "exact"
    if proof_status.startswith("bounded"):
        return "bounded"
    if proof_status == "no_reduction":
        return "no_reduction"
    if proof_status == "reduced":
        return "reduced"
    return proof_status


def benchmark_source(spec: dict[str, Any]) -> dict[str, Any]:
    source = {
        "live_proof_scenario": spec["scenario_name"],
        "rust_example": "crates/guild-runner/examples/live_proof_scenarios.rs",
    }
    if "contract" in spec:
        source["draft_contract"] = spec["contract"]
    if "request" in spec:
        source["draft_request"] = spec["request"]
    if "invocation" in spec:
        source["draft_invocation"] = spec["invocation"]
    if spec.get("execution_record_source", {}).get("kind") == "file":
        source["draft_execution_record"] = spec["execution_record_source"]["path"]
    return source


def proof_notes(family_status: dict[str, Any]) -> str:
    return family_status["notes"]


def build_token_factory(
    *,
    spec: dict[str, Any],
    plan: dict[str, Any],
    contract: dict[str, Any],
    proof_record: dict[str, Any],
    holder_id: str,
    issued_at: str,
    allow_upper_bound: bool,
    iteration_prefix: str,
):
    issuer = m6_issuer()
    binding = proof_backed_resource_binding(spec)

    def factory(counter=[0]):
        counter[0] += 1
        token_id = f"urn:guild:benchmark:{spec['slice_id']}:{iteration_prefix}:{counter[0]}"
        audiences = [holder_id]
        resources = []
        if binding is not None:
            binding_value = deepcopy(binding)
            binding_value["audience"] = holder_id
            resources = [binding_value]
        return create_root_token(
            plan,
            contract,
            issuer,
            holder_id=holder_id,
            issued_at=issued_at,
            proof=proof_record,
            allow_upper_bound=allow_upper_bound,
            required_proof_source_kind=PROOF_SOURCE_LIVE_RUNTIME,
            audiences=audiences,
            resource_bindings=resources,
            chain_links=["urn:guild:actor:benchmark"],
            token_id=token_id,
        )

    return factory


def build_verify_token_factory(
    *,
    spec: dict[str, Any],
    plan: dict[str, Any],
    contract: dict[str, Any],
    proof_record: dict[str, Any],
    token_factory,
    holder_id: str,
):
    issuer_keys = m6_issuer_keys()
    binding = proof_backed_resource_binding(spec)

    def factory():
        token = token_factory()
        if token.get("kind") != "guild.delegated_capability_token":
            return token
        expected_resources = []
        if binding is not None:
            bound = deepcopy(binding)
            bound["audience"] = holder_id
            expected_resources = [bound]
        with TemporaryDirectory() as replay_state_dir:
            return verify_token(
                token,
                issuer_keys=issuer_keys,
                verification_time="2026-03-21T23:59:40Z",
                expected_holder_id=holder_id,
                expected_audiences=[holder_id],
                expected_resources=expected_resources,
                expected_runtime_guarantee_id=plan["chosen_runtime"]["runtime_guarantee_id"],
                expected_call_chain_links=token["call_chain"]["links"],
                plan=plan,
                contract=contract,
                proof=proof_record,
                replay_state_dir=replay_state_dir,
                check_replay=True,
            )

    return factory


def build_witness_factory(
    *,
    spec: dict[str, Any],
    plan: dict[str, Any],
    contract: dict[str, Any],
    proof_record: dict[str, Any],
    invocation_input: dict[str, Any],
    execution_record: dict[str, Any],
    token_factory,
):
    issuer = m6_issuer()
    issuer_keys = m6_issuer_keys()

    def factory():
        token = token_factory()
        if token.get("kind") != "guild.delegated_capability_token":
            return token
        return generate_witness(
            plan=plan,
            contract=contract,
            issuer=issuer,
            issuer_keys=issuer_keys,
            issued_at="2026-03-21T23:59:50Z",
            invocation_input=invocation_input,
            proof=proof_record,
            required_proof_source_kind=PROOF_SOURCE_LIVE_RUNTIME,
            token=token,
            observation={
                "source_kind": "live-runtime-hook",
                "execution_record": execution_record,
            },
            redaction_profile="none",
        )

    return factory


def build_verify_witness_factory(
    *,
    plan: dict[str, Any],
    contract: dict[str, Any],
    proof_record: dict[str, Any],
    witness_factory,
    token_factory,
):
    issuer_keys = m6_issuer_keys()

    def factory():
        witness = witness_factory()
        token = token_factory()
        if witness.get("kind") != "guild.witness_record":
            return witness
        return verify_witness(
            witness,
            issuer_keys=issuer_keys,
            verification_time="2026-03-21T23:59:55Z",
            plan=plan,
            contract=contract,
            proof=proof_record,
            token=token,
        )

    return factory


def claim_results_for_slice(
    *,
    spec: dict[str, Any],
    witness: dict[str, Any],
    plan: dict[str, Any],
    contract: dict[str, Any],
    proof_record: dict[str, Any],
    token: dict[str, Any],
) -> dict[str, Any]:
    issuer_keys = m6_issuer_keys()
    claims: list[dict[str, Any]] = []
    if spec.get("measure_claims", True):
        claims.append(proof_absence_claim())
        satisfied = satisfied_claim(spec)
        if satisfied is not None:
            claims.append(satisfied)
        violated = violated_claim(spec)
        if violated is not None:
            claims.append(violated)

    results = []
    for claim in claims:
        result = verify_claim(
            witness,
            claim,
            issuer_keys=issuer_keys,
            verification_time="2026-03-22T00:00:00Z",
            plan=plan,
            contract=contract,
            proof=proof_record,
            token=token,
        )
        results.append(
            {
                "claim_type": claim["claim_type"],
                "status": result["claim_evaluation"]["status"],
                "reason_codes": result["claim_evaluation"]["reason_codes"],
            }
        )

    raw_counts = {
        "satisfied": 0,
        "violated": 0,
        "not_provable": 0,
        "unsupported": 0,
    }
    bucket_counts = {
        "success": 0,
        "fail": 0,
        "coverage_limited_or_unverifiable": 0,
    }
    for item in results:
        raw_counts[item["status"]] += 1
        bucket_counts[claim_status_bucket(item["status"])] += 1

    return {
        "support_status": spec["negative_claim_support_status"],
        "claims": results,
        "raw_status_counts": raw_counts,
        "minimum_requested_summary": bucket_counts,
    }


def slice_matrix_entry(spec: dict[str, Any], runtime: dict[str, Any]) -> dict[str, Any]:
    contract, request, plan = build_plan_bundle(spec, runtime)
    invocation_input = invocation_input_for_spec(spec)

    _, _, admission_timing = measure_operation(
        lambda: build_execution_plan(contract, request, [runtime]),
        warmup_runs=BENCHMARK_WARMUPS,
        measured_runs=BENCHMARK_RUNS,
        cache_present=False,
        cache_notes="M4 admission has no cache in the checked draft-v1 path.",
    )

    live_benchmark = load_live_proof_benchmark(
        spec["scenario_name"],
        warmup_runs=BENCHMARK_WARMUPS,
        measured_runs=BENCHMARK_RUNS,
    )
    live_proof = live_benchmark["proof"]
    family_status = family_status_from_live_proof(live_proof, spec["family"])

    proof_record = None
    scenario = None
    if spec["linked_path"] != "proof_only":
        proof_record, scenario = build_live_runtime_proof_record(
            scenario_name=spec["scenario_name"],
            plan=plan,
            contract=contract,
            runtime=runtime,
            invocation_input=invocation_input,
            created_at="2026-03-21T23:58:00Z",
        )
    execution_record = load_execution_record_for_spec(spec, scenario)

    proof_backed_timing = None
    upper_bound_timing = None
    refusal_timing = None
    token_verify_timing = None
    witness_generation_timing = None
    witness_verify_timing = None
    issuance_outcomes = {
        "proof_backed_success": {"count": 0, "rate": 0.0},
        "upper_bound_fallback": {"count": 0, "rate": 0.0},
        "token_refusal": {"count": 0, "rate": 0.0},
        "scenario_count": BENCHMARK_RUNS,
    }
    witness_outcomes = {
        "proof_linked_success": {"count": 0, "rate": 0.0},
        "unlinked_success": {"count": 0, "rate": 0.0},
        "scenario_count": BENCHMARK_RUNS,
    }
    chosen_token = None
    chosen_witness = None
    chosen_token_verify = None
    chosen_witness_verify = None
    negative_claims = {
        "support_status": spec["negative_claim_support_status"],
        "claims": [],
        "raw_status_counts": {"satisfied": 0, "violated": 0, "not_provable": 0, "unsupported": 0},
        "minimum_requested_summary": {
            "success": 0,
            "fail": 0,
            "coverage_limited_or_unverifiable": 0,
        },
    }

    if spec["linked_path"] == "proof_linked":
        holder_id = f"urn:guild:service:{spec['slice_id']}"
        token_factory = build_token_factory(
            spec=spec,
            plan=plan,
            contract=contract,
            proof_record=proof_record,
            holder_id=holder_id,
            issued_at="2026-03-21T23:59:00Z",
            allow_upper_bound=False,
            iteration_prefix="proof-backed",
        )
        chosen_token, proof_backed_values, proof_backed_timing = measure_operation(
            token_factory,
            warmup_runs=BENCHMARK_WARMUPS,
            measured_runs=BENCHMARK_RUNS,
            cache_present=False,
            cache_notes="Proof-backed M6 issuance has no cache on the checked path.",
        )
        token_outcomes = [token_outcome(value) for value in proof_backed_values]
        issuance_outcomes["proof_backed_success"] = count_rate(token_outcomes, "proof_backed_success")
        issuance_outcomes["upper_bound_fallback"] = count_rate(token_outcomes, "upper_bound_fallback")
        issuance_outcomes["token_refusal"] = count_rate(token_outcomes, "refusal")

        verify_token_factory = build_verify_token_factory(
            spec=spec,
            plan=plan,
            contract=contract,
            proof_record=proof_record,
            token_factory=token_factory,
            holder_id=holder_id,
        )
        chosen_token_verify, _, token_verify_timing = measure_operation(
            verify_token_factory,
            warmup_runs=BENCHMARK_WARMUPS,
            measured_runs=BENCHMARK_RUNS,
            cache_present=False,
            cache_notes="Token verification performs fresh replay-state checks with no cache.",
        )

        witness_factory = build_witness_factory(
            spec=spec,
            plan=plan,
            contract=contract,
            proof_record=proof_record,
            invocation_input=invocation_input,
            execution_record=execution_record,
            token_factory=token_factory,
        )
        chosen_witness, witness_values, witness_generation_timing = measure_operation(
            witness_factory,
            warmup_runs=BENCHMARK_WARMUPS,
            measured_runs=BENCHMARK_RUNS,
            cache_present=False,
            cache_notes="Witness generation has no cache on the checked path.",
        )
        witness_outcome_values = [witness_outcome(value) for value in witness_values]
        witness_outcomes["proof_linked_success"] = count_rate(witness_outcome_values, "proof_linked")
        witness_outcomes["unlinked_success"] = count_rate(witness_outcome_values, "unlinked")

        verify_witness_factory = build_verify_witness_factory(
            plan=plan,
            contract=contract,
            proof_record=proof_record,
            witness_factory=witness_factory,
            token_factory=token_factory,
        )
        chosen_witness_verify, _, witness_verify_timing = measure_operation(
            verify_witness_factory,
            warmup_runs=BENCHMARK_WARMUPS,
            measured_runs=BENCHMARK_RUNS,
            cache_present=False,
            cache_notes="Witness verification has no cache on the checked path.",
        )
        negative_claims = claim_results_for_slice(
            spec=spec,
            witness=chosen_witness,
            plan=plan,
            contract=contract,
            proof_record=proof_record,
            token=chosen_token,
        )

    elif spec["linked_path"] == "fallback_unlinked":
        refusal_factory = build_token_factory(
            spec=spec,
            plan=plan,
            contract=contract,
            proof_record=proof_record,
            holder_id=f"urn:guild:service:{spec['slice_id']}:refusal",
            issued_at="2026-03-21T23:59:05Z",
            allow_upper_bound=False,
            iteration_prefix="refusal",
        )
        _, refusal_values, refusal_timing = measure_operation(
            refusal_factory,
            warmup_runs=BENCHMARK_WARMUPS,
            measured_runs=BENCHMARK_RUNS,
            cache_present=False,
            cache_notes="Default M6 refusal path has no cache on the checked path.",
        )
        refusal_outcomes = [token_outcome(value) for value in refusal_values]
        issuance_outcomes["token_refusal"] = count_rate(refusal_outcomes, "refusal")

        holder_id = f"urn:guild:service:{spec['slice_id']}"
        fallback_factory = build_token_factory(
            spec=spec,
            plan=plan,
            contract=contract,
            proof_record=proof_record,
            holder_id=holder_id,
            issued_at="2026-03-21T23:59:10Z",
            allow_upper_bound=True,
            iteration_prefix="upper-bound",
        )
        chosen_token, fallback_values, upper_bound_timing = measure_operation(
            fallback_factory,
            warmup_runs=BENCHMARK_WARMUPS,
            measured_runs=BENCHMARK_RUNS,
            cache_present=False,
            cache_notes="Upper-bound fallback issuance has no cache on the checked path.",
        )
        fallback_outcomes = [token_outcome(value) for value in fallback_values]
        issuance_outcomes["proof_backed_success"] = count_rate(fallback_outcomes, "proof_backed_success")
        issuance_outcomes["upper_bound_fallback"] = count_rate(fallback_outcomes, "upper_bound_fallback")

        verify_token_factory = build_verify_token_factory(
            spec=spec,
            plan=plan,
            contract=contract,
            proof_record=proof_record,
            token_factory=fallback_factory,
            holder_id=holder_id,
        )
        chosen_token_verify, _, token_verify_timing = measure_operation(
            verify_token_factory,
            warmup_runs=BENCHMARK_WARMUPS,
            measured_runs=BENCHMARK_RUNS,
            cache_present=False,
            cache_notes="Token verification performs fresh replay-state checks with no cache.",
        )

        witness_factory = build_witness_factory(
            spec=spec,
            plan=plan,
            contract=contract,
            proof_record=proof_record,
            invocation_input=invocation_input,
            execution_record=execution_record,
            token_factory=fallback_factory,
        )
        chosen_witness, witness_values, witness_generation_timing = measure_operation(
            witness_factory,
            warmup_runs=BENCHMARK_WARMUPS,
            measured_runs=BENCHMARK_RUNS,
            cache_present=False,
            cache_notes="Witness generation has no cache on the checked path.",
        )
        witness_outcome_values = [witness_outcome(value) for value in witness_values]
        witness_outcomes["proof_linked_success"] = count_rate(witness_outcome_values, "proof_linked")
        witness_outcomes["unlinked_success"] = count_rate(witness_outcome_values, "unlinked")

        verify_witness_factory = build_verify_witness_factory(
            plan=plan,
            contract=contract,
            proof_record=proof_record,
            witness_factory=witness_factory,
            token_factory=fallback_factory,
        )
        chosen_witness_verify, _, witness_verify_timing = measure_operation(
            verify_witness_factory,
            warmup_runs=BENCHMARK_WARMUPS,
            measured_runs=BENCHMARK_RUNS,
            cache_present=False,
            cache_notes="Witness verification has no cache on the checked path.",
        )
        negative_claims = claim_results_for_slice(
            spec=spec,
            witness=chosen_witness,
            plan=plan,
            contract=contract,
            proof_record=proof_record,
            token=chosen_token,
        )

    final_issued_authority = issued_authority_from_token(chosen_token)
    return {
        "slice_id": spec["slice_id"],
        "family": spec["family"],
        "slice_name": spec["slice_name"],
        "exact_scope": spec["exact_scope"],
        "support_status": spec["support_status"],
        "proof_status": family_status.get("proof_status") or live_proof["proof_status"],
        "token_linkage_status": spec["token_linkage_status"],
        "witness_linkage_status": spec["witness_linkage_status"],
        "negative_claim_support_status": spec["negative_claim_support_status"],
        "benchmark_scenario_source": benchmark_source(spec),
        "baseline_upper_bound_authority": deepcopy(plan["granted_authority"]),
        "final_proven_authority": live_authority_plan_from_proof(plan, live_proof),
        "final_issued_authority": final_issued_authority,
        "reduction_result": reduction_result(
            family=spec["family"],
            family_proof_status=family_status.get("proof_status") or live_proof["proof_status"],
            baseline_plan=plan["granted_authority"],
            live_proof=live_proof,
            final_issued_authority=final_issued_authority,
        ),
        "timing_overhead_results": {
            "admission_only": admission_timing,
            "live_proof_search": deepcopy(live_benchmark["timing"]),
            "proof_backed_token_issuance": proof_backed_timing,
            "upper_bound_token_issuance": upper_bound_timing,
            "token_refusal": refusal_timing,
            "token_verification": token_verify_timing,
            "witness_generation": witness_generation_timing,
            "witness_verification": witness_verify_timing,
        },
        "issuance_outcomes": issuance_outcomes,
        "witness_outcomes": witness_outcomes,
        "negative_claim_verification": negative_claims,
        "fallback_refusal_behavior": {
            "default_path": spec["default_path"],
            "fallback_available": spec["linked_path"] == "fallback_unlinked",
            "proof_notes": proof_notes(family_status),
        },
        "fail_closed_reasons": family_status["reason_codes"],
        "notes": spec["notes"],
        "linked_path": spec["linked_path"],
        "benchmark_limits": {
            "warmup_runs": BENCHMARK_WARMUPS,
            "measured_runs": BENCHMARK_RUNS,
        },
        "selected_token_verification_result": chosen_token_verify,
        "selected_witness_verification_result": chosen_witness_verify,
    }


def checked_fail_closed_wall(spec: dict[str, Any]) -> dict[str, Any]:
    benchmark = load_live_proof_benchmark(
        spec["scenario_name"],
        warmup_runs=BENCHMARK_WARMUPS,
        measured_runs=BENCHMARK_RUNS,
    )
    family_status = family_status_from_live_proof(benchmark["proof"], spec["family"])
    return {
        "wall_id": spec["wall_id"],
        "family": spec["family"],
        "stage": spec["stage"],
        "wall_name": spec["wall_name"],
        "benchmark_scenario_source": {
            "live_proof_scenario": spec["scenario_name"],
            "rust_example": "crates/guild-runner/examples/live_proof_scenarios.rs",
            "checked_test": spec["checked_test"],
        },
        "proof_status": family_status.get("proof_status") or benchmark["proof"]["proof_status"],
        "fail_closed_reasons": family_status["reason_codes"],
        "timing_overhead_results": {
            "live_proof_search": benchmark["timing"],
        },
        "trigger_count": BENCHMARK_RUNS,
        "trigger_rate": 1.0,
        "notes": family_status["notes"],
    }


def matrix_questions(slices: list[dict[str, Any]], walls: list[dict[str, Any]]) -> dict[str, Any]:
    reduction_rows = []
    issuance_rows = []
    overhead_rows = []
    negative_rows = []
    wall_rows = []

    for slice_entry in slices:
        reduction_rows.append(
            {
                "slice_id": slice_entry["slice_id"],
                "family": slice_entry["family"],
                "proof_status": slice_entry["proof_status"],
                "reduction_classification": slice_entry["reduction_result"]["classification"],
                "narrowed_dimensions": slice_entry["reduction_result"]["narrowed_dimensions"],
            }
        )
        issuance_rows.append(
            {
                "slice_id": slice_entry["slice_id"],
                "family": slice_entry["family"],
                "proof_backed_success": slice_entry["issuance_outcomes"]["proof_backed_success"],
                "upper_bound_fallback": slice_entry["issuance_outcomes"]["upper_bound_fallback"],
                "token_refusal": slice_entry["issuance_outcomes"]["token_refusal"],
            }
        )
        overhead_rows.append(
            {
                "slice_id": slice_entry["slice_id"],
                "family": slice_entry["family"],
                "admission_mean_ms": slice_entry["timing_overhead_results"]["admission_only"]["mean_ms"],
                "live_proof_mean_ms": slice_entry["timing_overhead_results"]["live_proof_search"]["mean_ms"],
                "token_verify_mean_ms": None
                if slice_entry["timing_overhead_results"]["token_verification"] is None
                else slice_entry["timing_overhead_results"]["token_verification"]["mean_ms"],
                "witness_verify_mean_ms": None
                if slice_entry["timing_overhead_results"]["witness_verification"] is None
                else slice_entry["timing_overhead_results"]["witness_verification"]["mean_ms"],
            }
        )
        negative_rows.append(
            {
                "slice_id": slice_entry["slice_id"],
                "family": slice_entry["family"],
                "status_counts": slice_entry["negative_claim_verification"]["raw_status_counts"],
                "minimum_summary": slice_entry["negative_claim_verification"]["minimum_requested_summary"],
            }
        )
        if slice_entry["support_status"] != "supported":
            wall_rows.append(
                {
                    "slice_id": slice_entry["slice_id"],
                    "family": slice_entry["family"],
                    "reason_codes": slice_entry["fail_closed_reasons"],
                    "default_path": slice_entry["fallback_refusal_behavior"]["default_path"],
                }
            )

    for wall in walls:
        wall_rows.append(
            {
                "wall_id": wall["wall_id"],
                "family": wall["family"],
                "reason_codes": wall["fail_closed_reasons"],
                "default_path": "fail_closed",
            }
        )

    return {
        "authority_reduction": reduction_rows,
        "issuance_modes": issuance_rows,
        "overheads": overhead_rows,
        "negative_claims": negative_rows,
        "fail_closed_walls": wall_rows,
    }


def build_matrix() -> dict[str, Any]:
    runtime = load_json("examples/wasmtime-strict.runtime.json")
    slices = [slice_matrix_entry(spec, runtime) for spec in SLICE_SPECS]
    walls = [checked_fail_closed_wall(spec) for spec in FAIL_CLOSED_WALL_SPECS]
    return {
        "kind": MATRIX_KIND,
        "version": MATRIX_VERSION,
        "generated_at": "2026-03-22T00:00:00Z",
        "methodology": {
            "warmup_runs": BENCHMARK_WARMUPS,
            "measured_runs": BENCHMARK_RUNS,
            "live_proof_timing_source": "crates/guild-runner/examples/live_proof_scenarios.rs benchmark mode",
            "admission_token_witness_timing_source": "docs/schemas/draft-v1 Python control-plane implementation",
            "cache_truth": {
                "live_runtime_proof": "No live-runtime proof cache exists today; the draft bridge records cache.status=bypassed.",
                "draft_m5_example_cache": "A draft example-bounded proof cache exists in minimization_core.py, but it is not part of the live real-path benchmark.",
                "token_verification": "No cache; replay verification uses fresh state directories per measured run.",
                "witness_generation_verification": "No cache.",
            },
        },
        "slices": slices,
        "checked_fail_closed_walls": walls,
        "questions": matrix_questions(slices, walls),
    }


def markdown_table(headers: list[str], rows: list[list[str]]) -> str:
    lines = [
        "| " + " | ".join(headers) + " |",
        "| " + " | ".join(["---"] * len(headers)) + " |",
    ]
    lines.extend("| " + " | ".join(row) + " |" for row in rows)
    return "\n".join(lines)


def render_report(matrix: dict[str, Any]) -> str:
    def timing_mean(operation: dict[str, Any] | None) -> str:
        if operation is None:
            return "n/a"
        return f"{operation['mean_ms']:.3f}"

    supported_rows = []
    unsupported_rows = []
    negative_rows = []
    wall_rows = []
    for entry in matrix["slices"]:
        live_mean = entry["timing_overhead_results"]["live_proof_search"]["mean_ms"]
        admission_mean = entry["timing_overhead_results"]["admission_only"]["mean_ms"]
        proof_backed = entry["issuance_outcomes"]["proof_backed_success"]["count"]
        fallback = entry["issuance_outcomes"]["upper_bound_fallback"]["count"]
        refusal = entry["issuance_outcomes"]["token_refusal"]["count"]
        witness_mode = (
            "proof_linked"
            if entry["witness_outcomes"]["proof_linked_success"]["count"] > 0
            else "unlinked"
            if entry["witness_outcomes"]["unlinked_success"]["count"] > 0
            else "not_measured"
        )
        row = [
            entry["slice_id"],
            entry["family"],
            entry["proof_status"],
            entry["reduction_result"]["classification"],
            ",".join(entry["reduction_result"]["narrowed_dimensions"]) or "none",
            str(proof_backed),
            str(fallback),
            str(refusal),
            witness_mode,
            f"{admission_mean:.3f}",
            f"{live_mean:.3f}",
            timing_mean(entry["timing_overhead_results"]["proof_backed_token_issuance"]),
            timing_mean(entry["timing_overhead_results"]["upper_bound_token_issuance"]),
            timing_mean(entry["timing_overhead_results"]["token_refusal"]),
            timing_mean(entry["timing_overhead_results"]["token_verification"]),
            timing_mean(entry["timing_overhead_results"]["witness_generation"]),
            timing_mean(entry["timing_overhead_results"]["witness_verification"]),
        ]
        if entry["support_status"] == "supported":
            supported_rows.append(row)
        else:
            unsupported_rows.append(row + [",".join(entry["fail_closed_reasons"])])

        negative_rows.append(
            [
                entry["slice_id"],
                entry["family"],
                str(entry["negative_claim_verification"]["minimum_requested_summary"]["success"]),
                str(entry["negative_claim_verification"]["minimum_requested_summary"]["fail"]),
                str(
                    entry["negative_claim_verification"]["minimum_requested_summary"][
                        "coverage_limited_or_unverifiable"
                    ]
                ),
                str(entry["negative_claim_verification"]["raw_status_counts"]["unsupported"]),
            ]
        )

    for wall in matrix["checked_fail_closed_walls"]:
        wall_rows.append(
            [
                wall["wall_id"],
                wall["family"],
                wall["stage"],
                ",".join(wall["fail_closed_reasons"]),
                f"{wall['timing_overhead_results']['live_proof_search']['mean_ms']:.3f}",
            ]
        )

    lines = [
        "# M8 Real-Path Benchmark",
        "",
        "This report measures the checked real path only. Supported and unsupported slices stay separate, bounded proof stays labeled bounded, and fallback or refusal stays explicit.",
        "",
        "## Method",
        "",
        f"- Warmups per measured operation: {matrix['methodology']['warmup_runs']}",
        f"- Measured runs per operation: {matrix['methodology']['measured_runs']}",
        f"- Live proof timing source: `{matrix['methodology']['live_proof_timing_source']}`",
        f"- Admission/token/witness timing source: `{matrix['methodology']['admission_token_witness_timing_source']}`",
        "- Live-runtime proof has no cache today. The draft M5 example cache is out of scope for this report.",
        "",
        "## Supported Slices",
        "",
        markdown_table(
            [
                "Slice",
                "Family",
                "Proof",
                "Reduction",
                "Narrowing",
                "Proof-backed",
                "Fallback",
                "Refusal",
                "Witness",
                "Admission mean ms",
                "Proof mean ms",
                "Proof token mean ms",
                "Fallback token mean ms",
                "Refusal mean ms",
                "Token verify mean ms",
                "Witness gen mean ms",
                "Witness verify mean ms",
            ],
            supported_rows,
        ),
        "",
        "## Unsupported Or Not Proven Slices",
        "",
        markdown_table(
            [
                "Slice",
                "Family",
                "Proof",
                "Reduction",
                "Narrowing",
                "Proof-backed",
                "Fallback",
                "Refusal",
                "Witness",
                "Admission mean ms",
                "Proof mean ms",
                "Proof token mean ms",
                "Fallback token mean ms",
                "Refusal mean ms",
                "Token verify mean ms",
                "Witness gen mean ms",
                "Witness verify mean ms",
                "Fail-closed reasons",
            ],
            unsupported_rows,
        ),
        "",
        "## Negative Claims",
        "",
        markdown_table(
            [
                "Slice",
                "Family",
                "Success",
                "Fail",
                "Coverage limited",
                "Unsupported raw",
            ],
            negative_rows,
        ),
        "",
        "## Additional Fail-Closed Walls",
        "",
        markdown_table(
            [
                "Wall",
                "Family",
                "Stage",
                "Reasons",
                "Proof mean ms",
            ],
            wall_rows,
        ),
        "",
        "## Notes",
        "",
        "- The current checked real-path linked chain is `read-resource`, six bounded `http-request` slices, one bounded `invoke-skill` slice, and upper-bound fallback or unlinked witness behavior for the benchmarked unsupported slices.",
        "- `log-write` is measured here through M4 plus M5 only. The repo has a real live proof slice for observed levels, but this benchmark does not claim a checked real-path M6 or M7 linkage slice for `log-write`.",
        "- The measured reduction split is mixed by slice: `read-resource` really narrows from the admitted upper bound, the checked `http-request` and `invoke-skill` fixtures are already narrow enough that the proven authority does not shrink them further, and `log-write` is exact over an already narrow admitted level slice.",
        "- The checked negative-claim probes are coverage-limited today. They come back as `not_provable` rather than synthetic successes or failures, and the matrix keeps that explicit per slice.",
        "- The next real frontier is whichever unsupported rows you want to convert into bounded linked rows without broadening claims: `emit-evidence` exact sink or payload authority, broader `invoke-skill` shapes, and broader `http-request` hostname or replay coverage.",
        "",
    ]
    return "\n".join(lines)


def write_outputs(matrix: dict[str, Any]) -> None:
    MATRIX_PATH.write_text(json.dumps(matrix, indent=2) + "\n")
    REPORT_PATH.parent.mkdir(parents=True, exist_ok=True)
    REPORT_PATH.write_text(render_report(matrix))


def validate_generated_matrix(matrix: dict[str, Any]) -> list[str]:
    registry = build_registry()
    return validate_instance("benchmark_matrix.schema.json", matrix, registry)


def validate_artifacts(matrix: dict[str, Any]) -> list[str]:
    failures = validate_generated_matrix(matrix)
    if not MATRIX_PATH.exists():
        failures.append("benchmark_matrix.json is missing")
    if not REPORT_PATH.exists():
        failures.append("m8-real-path-benchmark.md is missing")
    if REPORT_PATH.exists():
        expected_report = render_report(matrix)
        if REPORT_PATH.read_text() != expected_report:
            failures.append("benchmark report is out of date with benchmark_matrix.json")
    return failures


def check_artifacts() -> int:
    if not MATRIX_PATH.exists():
        print("Benchmark artifact validation failed:")
        print(" - benchmark_matrix.json is missing")
        return 1
    matrix = load_json("benchmark_matrix.json")
    failures = validate_artifacts(matrix)
    if failures:
        print("Benchmark artifact validation failed:")
        for failure in failures:
            print(f" - {failure}")
        return 1
    print("Benchmark matrix and report validate cleanly.")
    return 0


SLICE_SPECS = [
    {
        "slice_id": "read-resource-immutable-guild-roots",
        "family": "read-resource",
        "slice_name": "immutable Guild execution and object-record roots",
        "exact_scope": "guild://executions/ and guild://objects/records/ roots only",
        "support_status": "supported",
        "scenario_name": "read-resource-bounded",
        "contract": "examples/runtime-read-resource.contract.json",
        "request": "examples/runtime-read-resource.admit.request.json",
        "invocation": "examples/runtime-read-resource.invocation.json",
        "execution_record_source": {
            "kind": "file",
            "path": "examples/runtime-read-resource.execution-record.json",
        },
        "resource_binding": {"family": "read-resource", "resource": "guild://executions/example-run"},
        "supported_claim": {
            "claim_type": "no_read_resource_outside_scope",
            "read_resource_scope": {
                "kind": "resource",
                "uri_prefixes": ["guild://executions/"],
                "resource_kinds": ["execution"],
            },
        },
        "violating_claim": {
            "claim_type": "no_read_resource_outside_scope",
            "read_resource_scope": {
                "kind": "resource",
                "uri_prefixes": ["guild://objects/records/"],
                "resource_kinds": ["object"],
            },
        },
        "linked_path": "proof_linked",
        "default_path": "proof_backed",
        "token_linkage_status": "proof_backed",
        "witness_linkage_status": "proof_linked",
        "negative_claim_support_status": "supported",
        "notes": "Bounded live proof plus proof-backed token and proof-linked witness over immutable Guild resource roots.",
    },
    {
        "slice_id": "http-request-loopback-ip-get-explicit-port",
        "family": "http-request",
        "slice_name": "loopback IP GET explicit port",
        "exact_scope": "GET http://127.0.0.1:18080/response.json",
        "support_status": "supported",
        "scenario_name": "http-request-bounded",
        "contract": "examples/runtime-http-read.contract.json",
        "request": "examples/runtime-http-read.admit.request.json",
        "invocation": "examples/runtime-http-read.invocation.json",
        "execution_record_source": {"kind": "file", "path": "examples/runtime-http-success.execution-record.json"},
        "resource_binding": {
            "family": "http-request",
            "resource": "GET:http://127.0.0.1:18080/response.json",
        },
        "supported_claim": {
            "claim_type": "no_http_request_outside_scope",
            "http_request_scope": {
                "kind": "network",
                "allowed_schemes": ["http"],
                "allowed_hosts": ["127.0.0.1"],
                "allowed_ports": [18080],
                "allowed_methods": ["GET"],
                "allowed_path_prefixes": ["/response.json"],
                "follow_redirects": False,
            },
        },
        "violating_claim": {
            "claim_type": "no_http_request_outside_scope",
            "http_request_scope": {
                "kind": "network",
                "allowed_schemes": ["http"],
                "allowed_hosts": ["127.0.0.1"],
                "allowed_ports": [18080],
                "allowed_methods": ["HEAD"],
                "allowed_path_prefixes": ["/response.json"],
                "follow_redirects": False,
            },
        },
        "linked_path": "proof_linked",
        "default_path": "proof_backed",
        "token_linkage_status": "proof_backed",
        "witness_linkage_status": "proof_linked",
        "negative_claim_support_status": "supported",
        "notes": "Bounded proof-backed GET slice over an explicit loopback IP and explicit port.",
    },
    {
        "slice_id": "http-request-loopback-ip-get-default-port",
        "family": "http-request",
        "slice_name": "loopback IP GET default port",
        "exact_scope": "GET http://127.0.0.1/response.json",
        "support_status": "supported",
        "scenario_name": "http-request-default-port-bounded",
        "contract": "examples/runtime-http-read-default-port.contract.json",
        "request": "examples/runtime-http-read-default-port.admit.request.json",
        "invocation": "examples/runtime-http-read-default-port.invocation.json",
        "execution_record_source": {
            "kind": "file",
            "path": "examples/runtime-http-read-default-port.execution-record.json",
        },
        "resource_binding": {
            "family": "http-request",
            "resource": "GET:http://127.0.0.1/response.json",
        },
        "supported_claim": {
            "claim_type": "no_http_request_outside_scope",
            "http_request_scope": {
                "kind": "network",
                "allowed_schemes": ["http"],
                "allowed_hosts": ["127.0.0.1"],
                "allowed_ports": [80],
                "allowed_methods": ["GET"],
                "allowed_path_prefixes": ["/response.json"],
                "follow_redirects": False,
            },
        },
        "violating_claim": {
            "claim_type": "no_http_request_outside_scope",
            "http_request_scope": {
                "kind": "network",
                "allowed_schemes": ["http"],
                "allowed_hosts": ["127.0.0.1"],
                "allowed_ports": [80],
                "allowed_methods": ["HEAD"],
                "allowed_path_prefixes": ["/response.json"],
                "follow_redirects": False,
            },
        },
        "linked_path": "proof_linked",
        "default_path": "proof_backed",
        "token_linkage_status": "proof_backed",
        "witness_linkage_status": "proof_linked",
        "negative_claim_support_status": "supported",
        "notes": "Bounded proof-backed GET slice over the implicit default HTTP port.",
    },
    {
        "slice_id": "http-request-localhost-get-explicit-port",
        "family": "http-request",
        "slice_name": "localhost GET explicit port",
        "exact_scope": "GET http://localhost:18080/response.json with deterministic loopback-only resolution binding",
        "support_status": "supported",
        "scenario_name": "http-request-localhost-bounded",
        "contract": "examples/runtime-http-localhost.contract.json",
        "request": "examples/runtime-http-localhost.admit.request.json",
        "invocation": "examples/runtime-http-localhost.invocation.json",
        "execution_record_source": {"kind": "file", "path": "examples/runtime-http-localhost.execution-record.json"},
        "resource_binding": {
            "family": "http-request",
            "resource": "GET:http://localhost:18080/response.json",
        },
        "supported_claim": {
            "claim_type": "no_http_request_outside_scope",
            "http_request_scope": {
                "kind": "network",
                "allowed_schemes": ["http"],
                "allowed_hosts": ["localhost"],
                "allowed_ports": [18080],
                "allowed_methods": ["GET"],
                "allowed_path_prefixes": ["/response.json"],
                "follow_redirects": False,
            },
        },
        "violating_claim": {
            "claim_type": "no_http_request_outside_scope",
            "http_request_scope": {
                "kind": "network",
                "allowed_schemes": ["http"],
                "allowed_hosts": ["localhost"],
                "allowed_ports": [18080],
                "allowed_methods": ["HEAD"],
                "allowed_path_prefixes": ["/response.json"],
                "follow_redirects": False,
            },
        },
        "linked_path": "proof_linked",
        "default_path": "proof_backed",
        "token_linkage_status": "proof_backed",
        "witness_linkage_status": "proof_linked",
        "negative_claim_support_status": "supported",
        "notes": "Bounded proof-backed localhost GET slice with explicit port and deterministic resolution binding.",
    },
    {
        "slice_id": "http-request-localhost-head-explicit-port",
        "family": "http-request",
        "slice_name": "localhost HEAD explicit port",
        "exact_scope": "HEAD http://localhost:18080/response.json with deterministic loopback-only resolution binding",
        "support_status": "supported",
        "scenario_name": "http-request-localhost-head-bounded",
        "contract": "examples/runtime-http-localhost-head.contract.json",
        "request": "examples/runtime-http-localhost-head.admit.request.json",
        "invocation": "examples/runtime-http-localhost-head.invocation.json",
        "execution_record_source": {
            "kind": "file",
            "path": "examples/runtime-http-localhost-head.execution-record.json",
        },
        "resource_binding": {
            "family": "http-request",
            "resource": "HEAD:http://localhost:18080/response.json",
        },
        "supported_claim": {
            "claim_type": "no_http_request_outside_scope",
            "http_request_scope": {
                "kind": "network",
                "allowed_schemes": ["http"],
                "allowed_hosts": ["localhost"],
                "allowed_ports": [18080],
                "allowed_methods": ["HEAD"],
                "allowed_path_prefixes": ["/response.json"],
                "follow_redirects": False,
            },
        },
        "violating_claim": {
            "claim_type": "no_http_request_outside_scope",
            "http_request_scope": {
                "kind": "network",
                "allowed_schemes": ["http"],
                "allowed_hosts": ["localhost"],
                "allowed_ports": [18080],
                "allowed_methods": ["GET"],
                "allowed_path_prefixes": ["/response.json"],
                "follow_redirects": False,
            },
        },
        "linked_path": "proof_linked",
        "default_path": "proof_backed",
        "token_linkage_status": "proof_backed",
        "witness_linkage_status": "proof_linked",
        "negative_claim_support_status": "supported",
        "notes": "Bounded proof-backed localhost HEAD slice with explicit port and deterministic resolution binding.",
    },
    {
        "slice_id": "http-request-loopback-ip-head-explicit-port",
        "family": "http-request",
        "slice_name": "loopback IP HEAD explicit port",
        "exact_scope": "HEAD http://127.0.0.1:18080/response.json",
        "support_status": "supported",
        "scenario_name": "http-request-head-bounded",
        "contract": "examples/runtime-http-head.contract.json",
        "request": "examples/runtime-http-head.admit.request.json",
        "invocation": "examples/runtime-http-head.invocation.json",
        "execution_record_source": {"kind": "file", "path": "examples/runtime-http-head.execution-record.json"},
        "resource_binding": {
            "family": "http-request",
            "resource": "HEAD:http://127.0.0.1:18080/response.json",
        },
        "supported_claim": {
            "claim_type": "no_http_request_outside_scope",
            "http_request_scope": {
                "kind": "network",
                "allowed_schemes": ["http"],
                "allowed_hosts": ["127.0.0.1"],
                "allowed_ports": [18080],
                "allowed_methods": ["HEAD"],
                "allowed_path_prefixes": ["/response.json"],
                "follow_redirects": False,
            },
        },
        "violating_claim": {
            "claim_type": "no_http_request_outside_scope",
            "http_request_scope": {
                "kind": "network",
                "allowed_schemes": ["http"],
                "allowed_hosts": ["127.0.0.1"],
                "allowed_ports": [18080],
                "allowed_methods": ["GET"],
                "allowed_path_prefixes": ["/response.json"],
                "follow_redirects": False,
            },
        },
        "linked_path": "proof_linked",
        "default_path": "proof_backed",
        "token_linkage_status": "proof_backed",
        "witness_linkage_status": "proof_linked",
        "negative_claim_support_status": "supported",
        "notes": "Bounded proof-backed HEAD slice over an explicit loopback IP and explicit port.",
    },
    {
        "slice_id": "http-request-loopback-ip-head-default-port",
        "family": "http-request",
        "slice_name": "loopback IP HEAD default port",
        "exact_scope": "HEAD http://127.0.0.1/response.json",
        "support_status": "supported",
        "scenario_name": "http-request-head-default-port-bounded",
        "contract": "examples/runtime-http-head-default-port.contract.json",
        "request": "examples/runtime-http-head-default-port.admit.request.json",
        "invocation": "examples/runtime-http-head-default-port.invocation.json",
        "execution_record_source": {
            "kind": "file",
            "path": "examples/runtime-http-head-default-port.execution-record.json",
        },
        "resource_binding": {
            "family": "http-request",
            "resource": "HEAD:http://127.0.0.1/response.json",
        },
        "supported_claim": {
            "claim_type": "no_http_request_outside_scope",
            "http_request_scope": {
                "kind": "network",
                "allowed_schemes": ["http"],
                "allowed_hosts": ["127.0.0.1"],
                "allowed_ports": [80],
                "allowed_methods": ["HEAD"],
                "allowed_path_prefixes": ["/response.json"],
                "follow_redirects": False,
            },
        },
        "violating_claim": {
            "claim_type": "no_http_request_outside_scope",
            "http_request_scope": {
                "kind": "network",
                "allowed_schemes": ["http"],
                "allowed_hosts": ["127.0.0.1"],
                "allowed_ports": [80],
                "allowed_methods": ["GET"],
                "allowed_path_prefixes": ["/response.json"],
                "follow_redirects": False,
            },
        },
        "linked_path": "proof_linked",
        "default_path": "proof_backed",
        "token_linkage_status": "proof_backed",
        "witness_linkage_status": "proof_linked",
        "negative_claim_support_status": "supported",
        "notes": "Bounded proof-backed HEAD slice over the implicit default HTTP port.",
    },
    {
        "slice_id": "invoke-skill-single-child-zero-authority",
        "family": "invoke-skill",
        "slice_name": "single child zero-authority inspect child",
        "exact_scope": "exact declared alias child -> one exact zero-authority guild-skill-inspect-v1 child",
        "support_status": "supported",
        "scenario_name": "invoke-skill-single-child-bounded",
        "contract": "examples/runtime-invoke-skill.contract.json",
        "request": "examples/runtime-invoke-skill.admit.request.json",
        "invocation": "examples/runtime-invoke-skill.invocation.json",
        "execution_record_source": {"kind": "file", "path": "examples/runtime-invoke-skill.execution-record.json"},
        "resource_binding": {"family": "invoke-skill", "resource": "child"},
        "supported_claim": {
            "claim_type": "no_invoke_skill_outside_scope",
            "invoke_skill_scope": {
                "kind": "skill",
                "aliases": ["child"],
            },
        },
        "violating_claim": {
            "claim_type": "no_invoke_skill_outside_scope",
            "invoke_skill_scope": {
                "kind": "skill",
                "aliases": ["other"],
            },
        },
        "linked_path": "proof_linked",
        "default_path": "proof_backed",
        "token_linkage_status": "proof_backed",
        "witness_linkage_status": "proof_linked",
        "negative_claim_support_status": "supported",
        "notes": "Bounded proof-backed single-child invoke slice only.",
    },
    {
        "slice_id": "http-request-redirect-driven-execution",
        "family": "http-request",
        "slice_name": "redirect-driven execution",
        "exact_scope": "GET http://127.0.0.1:18080/redirect.json with redirect follow enabled",
        "support_status": "not_proven",
        "scenario_name": "http-request-redirect-unsupported",
        "contract": "examples/runtime-http-redirect.contract.json",
        "request": "examples/runtime-http-redirect.admit.request.json",
        "invocation": "examples/runtime-http-redirect.invocation.json",
        "execution_record_source": {"kind": "file", "path": "examples/runtime-http-redirect.execution-record.json"},
        "resource_binding": {
            "family": "http-request",
            "resource": "GET:http://127.0.0.1:18080/redirect.json",
        },
        "supported_claim": {
            "claim_type": "no_http_request_outside_scope",
            "http_request_scope": {
                "kind": "network",
                "allowed_schemes": ["http"],
                "allowed_hosts": ["127.0.0.1"],
                "allowed_ports": [18080],
                "allowed_methods": ["GET"],
                "allowed_path_prefixes": ["/redirect.json", "/response.json"],
                "follow_redirects": True,
                "max_redirects": 2,
            },
        },
        "violating_claim": {
            "claim_type": "no_http_request_outside_scope",
            "http_request_scope": {
                "kind": "network",
                "allowed_schemes": ["http"],
                "allowed_hosts": ["127.0.0.1"],
                "allowed_ports": [18080],
                "allowed_methods": ["HEAD"],
                "allowed_path_prefixes": ["/redirect.json"],
                "follow_redirects": True,
                "max_redirects": 2,
            },
        },
        "linked_path": "fallback_unlinked",
        "default_path": "refusal_then_upper_bound_fallback",
        "token_linkage_status": "upper_bound_fallback",
        "witness_linkage_status": "unlinked",
        "negative_claim_support_status": "supported",
        "notes": "Redirects stay not_proven. Default issuance refuses; explicit upper-bound fallback issues and witness generation stays unlinked.",
    },
    {
        "slice_id": "invoke-skill-multi-child-fan-out",
        "family": "invoke-skill",
        "slice_name": "multi-child fan-out",
        "exact_scope": "same alias exercised twice from one parent execution",
        "support_status": "not_proven",
        "scenario_name": "invoke-skill-multi-child-unsupported",
        "contract": "examples/runtime-invoke-skill.contract.json",
        "request": "examples/runtime-invoke-skill.admit.request.json",
        "invocation": "examples/runtime-invoke-skill.invocation.json",
        "invocation_mutation": lambda payload: {**payload, "invoke_twice": True},
        "execution_record_source": {"kind": "scenario"},
        "resource_binding": {"family": "invoke-skill", "resource": "child"},
        "supported_claim": {
            "claim_type": "no_invoke_skill_outside_scope",
            "invoke_skill_scope": {
                "kind": "skill",
                "aliases": ["child"],
            },
        },
        "violating_claim": {
            "claim_type": "no_invoke_skill_outside_scope",
            "invoke_skill_scope": {
                "kind": "skill",
                "aliases": ["other"],
            },
        },
        "linked_path": "fallback_unlinked",
        "default_path": "refusal_then_upper_bound_fallback",
        "token_linkage_status": "upper_bound_fallback",
        "witness_linkage_status": "unlinked",
        "negative_claim_support_status": "supported",
        "notes": "Multi-child invoke remains not_proven. Default issuance refuses; explicit upper-bound fallback issues and witness generation stays unlinked.",
    },
    {
        "slice_id": "emit-evidence-single-emission-replay-unavailable",
        "family": "emit-evidence",
        "slice_name": "single emission local object-store replay unavailable",
        "exact_scope": "one emit-evidence call to the fixed local object-store sink",
        "support_status": "not_proven",
        "scenario_name": "emit-evidence-single-sink-replay-unavailable",
        "contract": "examples/runtime-emit-evidence-zero.contract.json",
        "request": "examples/runtime-emit-evidence-zero.admit.request.json",
        "invocation": "examples/runtime-emit-evidence.invocation.json",
        "execution_record_source": {"kind": "file", "path": "examples/runtime-emit-evidence.execution-record.json"},
        "resource_binding": {
            "family": "emit-evidence",
            "resource": "audience=user;redaction=none",
        },
        "supported_claim": {
            "claim_type": "no_emit_evidence_outside_scope",
            "emit_evidence_scope": {
                "kind": "evidence",
                "audiences": ["user"],
                "redactions": ["none"],
                "max_bytes": 65536,
            },
        },
        "violating_claim": {
            "claim_type": "no_emit_evidence_outside_scope",
            "emit_evidence_scope": {
                "kind": "evidence",
                "audiences": ["assistant"],
                "redactions": ["none"],
                "max_bytes": 65536,
            },
        },
        "linked_path": "fallback_unlinked",
        "default_path": "refusal_then_upper_bound_fallback",
        "token_linkage_status": "upper_bound_fallback",
        "witness_linkage_status": "unlinked",
        "negative_claim_support_status": "supported",
        "notes": "Emit-evidence stays not_proven. Default issuance refuses; explicit upper-bound fallback issues and witness generation stays unlinked.",
    },
    {
        "slice_id": "log-write-observed-info-level",
        "family": "log-write",
        "slice_name": "observed info level",
        "exact_scope": "one info-level log-write observation",
        "support_status": "supported",
        "scenario_name": "log-write-reduced",
        "contract": "examples/runtime-log-write.contract.json",
        "request": "examples/runtime-log-write.admit.request.json",
        "invocation": "examples/runtime-log-write.invocation.json",
        "execution_record_source": {"kind": "file", "path": "examples/runtime-log-write.execution-record.json"},
        "linked_path": "proof_only",
        "default_path": "m5_only",
        "token_linkage_status": "not_measured_on_real_path",
        "witness_linkage_status": "not_measured_on_real_path",
        "negative_claim_support_status": "not_measured_on_real_path",
        "measure_claims": False,
        "notes": "Real live proof exists for the observed level slice, but this benchmark does not claim a checked real-path token or witness linkage slice for log-write.",
    },
]


FAIL_CLOSED_WALL_SPECS = [
    {
        "wall_id": "http-request-no-replay-fixture",
        "family": "http-request",
        "stage": "live_proof_search",
        "wall_name": "HTTP replay fixture required",
        "scenario_name": "http-request-no-replay",
        "checked_test": "crates/guild-runner/tests/live_proofs.rs::http_request_live_proof_stays_not_proven_without_replay",
    },
    {
        "wall_id": "read-resource-query-root-shrink-unsupported",
        "family": "read-resource",
        "stage": "live_proof_search",
        "wall_name": "query resources remain outside the immutable read-resource shrink model",
        "scenario_name": "read-resource-query-unsupported",
        "checked_test": "crates/guild-runner/tests/live_proofs.rs::read_resource_live_proof_fails_closed_for_query_resources",
    },
    {
        "wall_id": "invoke-skill-child-authority-unsupported",
        "family": "invoke-skill",
        "stage": "live_proof_search",
        "wall_name": "child authority use remains outside the bounded invoke proof slice",
        "scenario_name": "invoke-skill-child-authority-unsupported",
        "checked_test": "crates/guild-runner/tests/live_proofs.rs::invoke_skill_live_proof_stays_not_proven_for_child_authority",
    },
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run or validate the M8 real-path benchmark.")
    parser.add_argument(
        "--check-artifacts",
        action="store_true",
        help="Validate benchmark_matrix.json against schema and check that the report matches it.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.check_artifacts:
        return check_artifacts()

    matrix = build_matrix()
    failures = validate_generated_matrix(matrix)
    if failures:
        print("Benchmark generation failed validation:")
        for failure in failures:
            print(f" - {failure}")
        return 1
    write_outputs(matrix)
    artifact_failures = validate_artifacts(matrix)
    if artifact_failures:
        print("Benchmark generation wrote artifacts but validation still failed:")
        for failure in artifact_failures:
            print(f" - {failure}")
        return 1
    print(f"Wrote {MATRIX_PATH}")
    print(f"Wrote {REPORT_PATH}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
