from copy import deepcopy
from tempfile import TemporaryDirectory

from admission_core import (
    AdmissionInputError,
    build_execution_plan,
    build_registry,
    canonical_json,
    load_json,
    validate_instance,
)
from minimization_core import build_minimization_proof
from token_core import create_child_token, create_root_token, verify_token
from witness_core import generate_witness, verify_claim, verify_witness
from witness_examples import build_witness_fixtures


EXAMPLES = [
    ("skill_contract.schema.json", "examples/local-log-analyzer.contract.json"),
    ("skill_contract.schema.json", "examples/zero-authority.contract.json"),
    ("skill_contract.schema.json", "examples/fetch-transform.contract.json"),
    ("skill_contract.schema.json", "examples/cluster-rollout.contract.json"),
    ("skill_contract.schema.json", "examples/runtime-http-read.contract.json"),
    ("skill_contract.schema.json", "examples/runtime-emit-evidence-zero.contract.json"),
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
    ("admission_request.schema.json", "examples/runtime-emit-evidence-zero.admit.request.json"),
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
    issuer = m6_issuer()
    issuer_keys = m6_issuer_keys()
    runtime = load_json("examples/wasmtime-strict.runtime.json")

    http_contract = load_json("examples/runtime-http-read.contract.json")
    http_request = load_json("examples/runtime-http-read.admit.request.json")
    http_invocation = load_json("examples/runtime-http-read.invocation.json")
    http_comparator = load_json("examples/runtime-http-read.unavailable.comparator.json")
    http_record = load_json("examples/runtime-http-success.execution-record.json")
    http_blocked_record = load_json("examples/runtime-http-blocked.execution-record.json")

    http_plan = build_execution_plan(http_contract, http_request, [runtime])
    if http_plan["decision"] != "admit":
        failures.append("runtime-http-read admission did not produce an admit plan for the live HTTP case")
        return failures

    http_proof = build_minimization_proof(
        http_plan,
        http_contract,
        http_request,
        runtime,
        http_invocation,
        http_comparator,
        created_at="2026-03-20T20:10:00Z",
    )
    if http_proof["proof_status"] != "not_proven":
        failures.append("runtime-http-read proof did not stay honest about the missing live minimization harness")
    http_witness_proof = None

    http_token = create_root_token(
        http_plan,
        http_contract,
        issuer,
        holder_id="urn:guild:service:runtime-http-read",
        issued_at="2026-03-20T20:10:30Z",
        proof=http_proof,
        allow_upper_bound=True,
        audiences=["runtime-http-read"],
        resource_bindings=[
            {
                "effect_class": "net.connect",
                "audience": "runtime-http-read",
                "resource": "http://127.0.0.1:18080/response.json",
            }
        ],
        chain_links=["urn:guild:actor:runtime-alignment-test"],
    )
    if http_token.get("kind") != "guild.delegated_capability_token":
        failures.append("runtime-http-read token issuance did not produce a delegated capability token")
        return failures

    http_witness = generate_witness(
        plan=http_plan,
        contract=http_contract,
        issuer=issuer,
        issuer_keys=issuer_keys,
        issued_at="2026-03-20T20:11:00Z",
        invocation_input=http_invocation,
        proof=http_witness_proof,
        token=http_token,
        observation={
            "source_kind": "live-runtime-hook",
            "execution_record": http_record,
        },
        redaction_profile="none",
    )
    http_witness_repeat = generate_witness(
        plan=http_plan,
        contract=http_contract,
        issuer=issuer,
        issuer_keys=issuer_keys,
        issued_at="2026-03-20T20:11:00Z",
        invocation_input=http_invocation,
        proof=http_witness_proof,
        token=http_token,
        observation={
            "source_kind": "live-runtime-hook",
            "execution_record": http_record,
        },
        redaction_profile="none",
    )
    if canonical_json(http_witness) != canonical_json(http_witness_repeat):
        failures.append("runtime-http-read live witness generation was not deterministic for identical runtime-native input")

    http_verification = verify_witness(
        http_witness,
        issuer_keys=issuer_keys,
        verification_time="2026-03-20T20:12:00Z",
        plan=http_plan,
        contract=http_contract,
        proof=http_witness_proof,
        token=http_token,
    )
    if not http_verification["verified"] or http_verification["witness_status"] != "within_envelope":
        failures.append("runtime-http-read live witness did not verify as within_envelope")

    http_claim = verify_claim(
        http_witness,
        {
            "claim_type": "no_network_egress_except_allowlist",
            "network_allowlist": [
                {
                    "host": "127.0.0.1",
                    "ports": [18080],
                    "schemes": ["http"],
                    "path_prefixes": ["/response.json"],
                    "methods": ["GET"],
                }
            ],
        },
        issuer_keys=issuer_keys,
        verification_time="2026-03-20T20:12:00Z",
        plan=http_plan,
        contract=http_contract,
        proof=http_witness_proof,
        token=http_token,
    )
    if http_claim["claim_evaluation"]["status"] != "satisfied":
        failures.append("runtime-http-read live witness did not satisfy the covered network allowlist claim")

    coverage_entry = http_witness["observation_coverage"]["families"][0]
    if coverage_entry["family"] != "http-request" or coverage_entry["mapping_status"] != "narrowing":
        failures.append("runtime-http-read live witness did not classify the net.connect -> http-request bridge as a narrowing mapping")

    blocked_witness = generate_witness(
        plan=http_plan,
        contract=http_contract,
        issuer=issuer,
        issuer_keys=issuer_keys,
        issued_at="2026-03-20T20:11:30Z",
        invocation_input=http_invocation,
        proof=http_witness_proof,
        token=http_token,
        observation={
            "source_kind": "live-runtime-hook",
            "execution_record": http_blocked_record,
        },
        redaction_profile="none",
    )
    if "net.connect" in blocked_witness["actual_exercised_authority"]["effect_classes"]:
        failures.append("runtime-http-read blocked live witness conflated blocked authority with exercised authority")
    if "net.connect" not in blocked_witness["blocked_attempted_authority"]["effect_classes"]:
        failures.append("runtime-http-read blocked live witness did not preserve the blocked HTTP attempt distinctly")
    blocked_claim = verify_claim(
        blocked_witness,
        {
            "claim_type": "no_blocked_attempts_of_classes",
            "effect_classes": ["net.connect"],
        },
        issuer_keys=issuer_keys,
        verification_time="2026-03-20T20:12:00Z",
        plan=http_plan,
        contract=http_contract,
        proof=http_witness_proof,
        token=http_token,
    )
    if blocked_claim["claim_evaluation"]["status"] != "violated":
        failures.append("runtime-http-read blocked live witness did not fail the blocked-attempt claim")

    emit_contract = load_json("examples/runtime-emit-evidence-zero.contract.json")
    emit_request = load_json("examples/runtime-emit-evidence-zero.admit.request.json")
    emit_invocation = load_json("examples/runtime-emit-evidence.invocation.json")
    emit_record = load_json("examples/runtime-emit-evidence.execution-record.json")

    emit_plan = build_execution_plan(emit_contract, emit_request, [runtime])
    if emit_plan["decision"] != "admit":
        failures.append("runtime-emit-evidence-zero admission did not produce an admit plan")
        return failures

    emit_proof = build_minimization_proof(
        emit_plan,
        emit_contract,
        emit_request,
        runtime,
        emit_invocation,
        http_comparator,
        created_at="2026-03-20T20:13:00Z",
    )
    emit_witness_proof = None
    emit_token = create_root_token(
        emit_plan,
        emit_contract,
        issuer,
        holder_id="urn:guild:service:runtime-emit-evidence-zero",
        issued_at="2026-03-20T20:13:30Z",
        proof=emit_proof,
        allow_upper_bound=True,
        chain_links=["urn:guild:actor:runtime-alignment-test"],
    )
    emit_witness = generate_witness(
        plan=emit_plan,
        contract=emit_contract,
        issuer=issuer,
        issuer_keys=issuer_keys,
        issued_at="2026-03-20T20:14:00Z",
        invocation_input=emit_invocation,
        proof=emit_witness_proof,
        token=emit_token if emit_token.get("kind") == "guild.delegated_capability_token" else None,
        observation={
            "source_kind": "live-runtime-hook",
            "execution_record": emit_record,
        },
        redaction_profile="none",
    )
    emit_verification = verify_witness(
        emit_witness,
        issuer_keys=issuer_keys,
        verification_time="2026-03-20T20:14:30Z",
        plan=emit_plan,
        contract=emit_contract,
        proof=emit_witness_proof,
        token=emit_token if emit_token.get("kind") == "guild.delegated_capability_token" else None,
    )
    if not emit_verification["verified"] or emit_verification["witness_status"] != "coverage_limited":
        failures.append("runtime-emit-evidence live witness did not verify as coverage_limited")
    emit_claim = verify_claim(
        emit_witness,
        {"claim_type": "no_authority_use_outside_plan"},
        issuer_keys=issuer_keys,
        verification_time="2026-03-20T20:14:30Z",
        plan=emit_plan,
        contract=emit_contract,
        proof=emit_witness_proof,
        token=emit_token if emit_token.get("kind") == "guild.delegated_capability_token" else None,
    )
    if emit_claim["claim_evaluation"]["status"] != "not_provable":
        failures.append("runtime-emit-evidence live witness did not fail closed for an unsupported-family absence claim")

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

    if failures:
        print("Validation failed:")
        for failure in failures:
            print(f" - {failure}")
        return 1

    print("All bundled examples, admission cases, minimization cases, token cases, witness cases, and live runtime alignment cases validate cleanly.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
