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


EXAMPLES = [
    ("skill_contract.schema.json", "examples/local-log-analyzer.contract.json"),
    ("skill_contract.schema.json", "examples/zero-authority.contract.json"),
    ("skill_contract.schema.json", "examples/fetch-transform.contract.json"),
    ("skill_contract.schema.json", "examples/cluster-rollout.contract.json"),
    ("runtime_guarantee.schema.json", "examples/wasmtime-strict.runtime.json"),
    ("runtime_guarantee.schema.json", "examples/node-wasi-basic.runtime.json"),
    ("comparator_profile.schema.json", "examples/local-log-analyzer.canonical-json.comparator.json"),
    ("comparator_profile.schema.json", "examples/local-log-analyzer.unavailable.comparator.json"),
    ("comparator_profile.schema.json", "examples/fetch-transform.postconditions.comparator.json"),
    ("comparator_profile.schema.json", "examples/fetch-transform.bounded.comparator.json"),
    ("comparator_profile.schema.json", "examples/zero-authority.pure.comparator.json"),
    ("proof_record.schema.json", "examples/local-log-analyzer.proof.json"),
    ("proof_record.schema.json", "examples/local-log-analyzer.cache-hit.proof.json"),
    ("proof_record.schema.json", "examples/local-log-analyzer.comparator-unavailable.proof.json"),
    ("proof_record.schema.json", "examples/fetch-transform.no-reduction.proof.json"),
    ("proof_record.schema.json", "examples/fetch-transform.bounded.proof.json"),
    ("proof_record.schema.json", "examples/zero-authority.proof.json"),
    ("witness_record.schema.json", "examples/cluster-rollout.witness.json"),
    ("admission_request.schema.json", "examples/zero-authority.admit.request.json"),
    ("admission_request.schema.json", "examples/zero-authority.migrate.request.json"),
    ("admission_request.schema.json", "examples/fetch-transform.downgrade.request.json"),
    ("admission_request.schema.json", "examples/fetch-transform.no-reduction.request.json"),
    ("admission_request.schema.json", "examples/local-log-analyzer.admit.request.json"),
    ("admission_request.schema.json", "examples/cluster-rollout.refuse.request.json"),
    ("admission_request.schema.json", "examples/cluster-rollout.admit.request.json"),
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


def main() -> int:
    registry = build_registry()
    failures: list[str] = []
    failures.extend(verify_examples(registry))
    failures.extend(verify_admission_cases())
    failures.extend(verify_invalid_runtime_probes(registry))
    failures.extend(verify_minimization_cases())
    failures.extend(verify_token_cases())

    if failures:
        print("Validation failed:")
        for failure in failures:
            print(f" - {failure}")
        return 1

    print("All bundled examples, admission cases, minimization cases, and M6 token cases validate cleanly.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
