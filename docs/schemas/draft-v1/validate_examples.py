from copy import deepcopy

from admission_core import (
    AdmissionInputError,
    SCHEMA_FILES,
    build_execution_plan,
    build_registry,
    canonical_json,
    load_json,
    validate_instance,
)


EXAMPLES = [
    ("skill_contract.schema.json", "examples/local-log-analyzer.contract.json"),
    ("skill_contract.schema.json", "examples/zero-authority.contract.json"),
    ("skill_contract.schema.json", "examples/fetch-transform.contract.json"),
    ("skill_contract.schema.json", "examples/cluster-rollout.contract.json"),
    ("runtime_guarantee.schema.json", "examples/wasmtime-strict.runtime.json"),
    ("runtime_guarantee.schema.json", "examples/node-wasi-basic.runtime.json"),
    ("proof_record.schema.json", "examples/local-log-analyzer.proof.json"),
    ("witness_record.schema.json", "examples/cluster-rollout.witness.json"),
    ("admission_request.schema.json", "examples/zero-authority.admit.request.json"),
    ("admission_request.schema.json", "examples/zero-authority.migrate.request.json"),
    ("admission_request.schema.json", "examples/fetch-transform.downgrade.request.json"),
    ("admission_request.schema.json", "examples/cluster-rollout.refuse.request.json"),
    ("execution_plan.schema.json", "examples/zero-authority.admit.plan.json"),
    ("execution_plan.schema.json", "examples/zero-authority.migrate.plan.json"),
    ("execution_plan.schema.json", "examples/fetch-transform.downgrade.plan.json"),
    ("execution_plan.schema.json", "examples/cluster-rollout.refuse.plan.json"),
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
        "contract": "examples/cluster-rollout.contract.json",
        "request": "examples/cluster-rollout.refuse.request.json",
        "runtimes": ["examples/node-wasi-basic.runtime.json"],
        "expected_plan": "examples/cluster-rollout.refuse.plan.json",
    },
]


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


def main() -> int:
    registry = build_registry()
    failures: list[str] = []
    failures.extend(verify_examples(registry))
    failures.extend(verify_admission_cases())
    failures.extend(verify_invalid_runtime_probes(registry))

    if failures:
        print("Validation failed:")
        for failure in failures:
            print(f" - {failure}")
        return 1

    print("All bundled examples and admission cases validate cleanly.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
