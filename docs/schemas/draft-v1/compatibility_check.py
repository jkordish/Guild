from copy import deepcopy
import json
from pathlib import Path

BASE = Path(__file__).resolve().parent

ORDER = {
    "execution_isolation_assurance": ["none", "best_effort", "strong"],
    "filesystem_isolation_class": ["none", "path_filter", "preopen_only", "virtual_fs", "os_sandbox"],
    "network_policy_granularity": ["none", "binary", "domain", "host_port", "url"],
    "witness_level": ["summary", "decision", "hostcall", "full"],
}

SKILLS = [
    "examples/local-log-analyzer.contract.json",
    "examples/zero-authority.contract.json",
    "examples/fetch-transform.contract.json",
    "examples/cluster-rollout.contract.json",
]
RUNTIMES = [
    "examples/wasmtime-strict.runtime.json",
    "examples/node-wasi-basic.runtime.json",
]


def rank(kind: str, value: str) -> int:
    return ORDER[kind].index(value)


def load(path: str) -> dict:
    return json.loads((BASE / path).read_text())


def required_effect_classes(skill: dict) -> list[str]:
    classes = {
        effect["effect_class"]
        for collection in (skill.get("required_effects", []), skill.get("authority_ceiling", []))
        for effect in collection
    }
    return sorted(classes)


def match(skill: dict, runtime: dict) -> tuple[bool, list[str]]:
    req = skill["required_runtime_guarantees"]
    reasons: list[str] = []

    component = skill["component"]
    component_support = runtime.get("component_model_support")
    if not component_support:
        reasons.append("component model support undeclared")
    else:
        if component_support.get("component_model") != component["component_model"]:
            reasons.append("component model unsupported")
        if component["component_model_version"] not in component_support.get("supported_versions", []):
            reasons.append("component model version unsupported")
        declared_worlds = component_support.get("wit_worlds")
        if declared_worlds is None:
            reasons.append(
                "WIT world support undeclared; runtime must enumerate component_model_support.wit_worlds explicitly"
            )
        elif component["wit_world"] not in declared_worlds:
            reasons.append(
                f"WIT world unsupported: {component['wit_world']} not listed in runtime component_model_support.wit_worlds"
            )

    unsupported_effects = [
        effect_class
        for effect_class in required_effect_classes(skill)
        if effect_class not in runtime.get("supported_effect_classes", [])
    ]
    if unsupported_effects:
        reasons.append(f"unsupported effect classes: {', '.join(unsupported_effects)}")

    if rank("execution_isolation_assurance", runtime["execution_isolation_assurance"]) < rank(
        "execution_isolation_assurance", req["execution_isolation_assurance"]["minimum"]
    ):
        reasons.append("execution isolation too weak")

    if rank("filesystem_isolation_class", runtime["filesystem_isolation_class"]) < rank(
        "filesystem_isolation_class", req["filesystem_isolation_class"]["minimum"]
    ):
        reasons.append("filesystem isolation too weak")

    if rank("network_policy_granularity", runtime["network_policy_granularity"]) < rank(
        "network_policy_granularity", req["network_policy_granularity"]["minimum"]
    ):
        reasons.append("network policy granularity too weak")

    if req["child_process_policy"]["required_mode"] not in runtime["child_process_policy"]["supported_modes"]:
        reasons.append("required child-process mode unsupported")

    if req["token_passthrough_policy"]["required_mode"] not in runtime["token_passthrough_policy"]["supported_modes"]:
        reasons.append("required token passthrough mode unsupported")

    if req["revocation_behavior"]["required_mode"] not in runtime["revocation_behavior"]["supported_modes"]:
        reasons.append("required revocation mode unsupported")

    re = req["delegation_enforcement"]
    rg = runtime["delegation_enforcement"]
    if re["audience_binding_required"] and not rg["audience_binding"]:
        reasons.append("audience binding unsupported")
    if re["call_chain_binding_required"] and not rg["call_chain_binding"]:
        reasons.append("call-chain binding unsupported")
    if re["anti_replay_required"] and not rg["anti_replay"]:
        reasons.append("anti-replay unsupported")
    if re["max_hops_enforced_required"] and not rg["max_hops_enforced"]:
        reasons.append("max-hops enforcement unsupported")

    ws_req = req["witness_support"]
    ws_run = runtime["witness_support"]
    supported_level_ok = any(
        rank("witness_level", lvl) >= rank("witness_level", ws_req["minimum_level"])
        for lvl in ws_run["supported_levels"]
    )
    if not supported_level_ok:
        reasons.append("required witness level unsupported")
    if not set(ws_req["acceptable_tamper_evidence_modes"]).intersection(ws_run["tamper_evidence_modes"]):
        reasons.append("acceptable tamper-evidence mode unsupported")
    if not set(ws_req["acceptable_signature_modes"]).intersection(ws_run["signature_modes"]):
        reasons.append("acceptable signature mode unsupported")
    if ws_req["trusted_time_source_required"] and not ws_run["trusted_time_source"]:
        reasons.append("trusted time source unsupported")
    if ws_req["redacted_io_hashes_required"] and not ws_run["redacted_io_hashes"]:
        reasons.append("redacted I/O hashes unsupported")
    if ws_req["authority_plan_digest_required"] and not ws_run["authority_plan_digest"]:
        reasons.append("authority-plan digest unsupported")

    return (not reasons, reasons)


def build_matrix_lines(skills: list[tuple[str, dict]], runtimes: list[tuple[str, dict]]) -> list[str]:
    lines = [
        "# Compatibility Matrix",
        "",
        "Deterministic admission check for the bundled examples.",
        "",
        "This matrix enforces component-model compatibility, explicit WIT-world publication, and required effect-class support in addition to the runtime guarantee thresholds.",
        "",
        "Negative fail-closed probes for omitted and unsupported `wit_worlds` declarations are asserted by `compatibility_check.py` but omitted from this table because they mutate the base runtime examples.",
        "",
        "| Skill contract | Runtime | Result | Notes |",
        "|---|---|---|---|",
    ]

    for skill_path, skill in skills:
        for runtime_path, runtime in runtimes:
            ok, reasons = match(skill, runtime)
            lines.append(
                f"| `{Path(skill_path).name}` | `{Path(runtime_path).name}` | "
                f"{'PASS' if ok else 'FAIL'} | "
                f"{'; '.join(reasons) if reasons else 'all required guarantees satisfied'} |"
            )

    return lines


def assert_reason(reasons: list[str], expected: str) -> None:
    if not any(expected in reason for reason in reasons):
        raise SystemExit(f"Expected reason containing {expected!r}, got: {reasons}")


def verify_fail_closed_wit_world_probes() -> None:
    skill = load("examples/local-log-analyzer.contract.json")
    runtime = load("examples/wasmtime-strict.runtime.json")

    omitted_worlds = deepcopy(runtime)
    del omitted_worlds["component_model_support"]["wit_worlds"]
    ok, reasons = match(skill, omitted_worlds)
    if ok:
        raise SystemExit("Fail-closed probe failed: omitted component_model_support.wit_worlds unexpectedly passed")
    assert_reason(reasons, "WIT world support undeclared")

    empty_worlds = deepcopy(runtime)
    empty_worlds["component_model_support"]["wit_worlds"] = []
    ok, reasons = match(skill, empty_worlds)
    if ok:
        raise SystemExit("Fail-closed probe failed: empty component_model_support.wit_worlds unexpectedly passed")
    assert_reason(reasons, "WIT world unsupported")

    unsupported_world = deepcopy(runtime)
    unsupported_world["component_model_support"]["wit_worlds"] = ["different-world"]
    ok, reasons = match(skill, unsupported_world)
    if ok:
        raise SystemExit("Fail-closed probe failed: unsupported WIT world unexpectedly passed")
    assert_reason(reasons, "WIT world unsupported")


def main() -> None:
    skills = [(path, load(path)) for path in SKILLS]
    runtimes = [(path, load(path)) for path in RUNTIMES]
    lines = build_matrix_lines(skills, runtimes)

    output = "\n".join(lines) + "\n"
    (BASE / "compatibility_matrix.md").write_text(output)
    print(output)
    verify_fail_closed_wit_world_probes()
    print("Verified fail-closed WIT-world admission probes.")


if __name__ == "__main__":
    main()
