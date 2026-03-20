from copy import deepcopy
from pathlib import Path

from admission_core import load_json, match_hard_requirements

BASE = Path(__file__).resolve().parent

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


def build_matrix_lines(skills: list[tuple[str, dict]], runtimes: list[tuple[str, dict]]) -> list[str]:
    lines = [
        "# Compatibility Matrix",
        "",
        "Deterministic hard-requirement precheck for the bundled examples.",
        "",
        "This matrix is intentionally narrower than full M4 admission. It covers the shared fail-closed hard-requirement path used by `admission_engine.py`, not request-time narrowing, runtime migration, or final execution-plan derivation.",
        "",
        "The precheck enforces component-model compatibility, explicit WIT-world publication, required effect-class support, required-effect enforceability, and the published runtime guarantee thresholds.",
        "",
        "Published `witness_support` values in this table are M4 hard-requirement inputs only. They do not by themselves imply runtime-general M7 observation completeness.",
        "",
        "Negative fail-closed probes for omitted and unsupported `wit_worlds` declarations are asserted by `compatibility_check.py` but omitted from this table because they mutate the base runtime examples.",
        "",
        "| Skill contract | Runtime | Result | Notes |",
        "|---|---|---|---|",
    ]

    for skill_path, skill in skills:
        for runtime_path, runtime in runtimes:
            result = match_hard_requirements(skill, runtime)
            notes = (
                "; ".join(item["reason"]["message"] for item in result["unsatisfied_requirements"])
                if result["unsatisfied_requirements"]
                else "all hard requirements satisfied"
            )
            lines.append(
                f"| `{Path(skill_path).name}` | `{Path(runtime_path).name}` | "
                f"{'PASS' if result['ok'] else 'FAIL'} | {notes} |"
            )

    return lines


def assert_reason(result: dict, expected: str) -> None:
    reason_codes = {item["reason"]["reason_code"] for item in result["unsatisfied_requirements"]}
    if expected not in reason_codes:
        raise SystemExit(f"Expected reason code {expected!r}, got: {sorted(reason_codes)}")


def verify_fail_closed_wit_world_probes() -> None:
    skill = load_json("examples/local-log-analyzer.contract.json")
    runtime = load_json("examples/wasmtime-strict.runtime.json")

    omitted_worlds = deepcopy(runtime)
    del omitted_worlds["component_model_support"]["wit_worlds"]
    result = match_hard_requirements(skill, omitted_worlds)
    if result["ok"]:
        raise SystemExit("Fail-closed probe failed: omitted component_model_support.wit_worlds unexpectedly passed")
    assert_reason(result, "RUNTIME_WIT_WORLD_UNDECLARED")

    empty_worlds = deepcopy(runtime)
    empty_worlds["component_model_support"]["wit_worlds"] = []
    result = match_hard_requirements(skill, empty_worlds)
    if result["ok"]:
        raise SystemExit("Fail-closed probe failed: empty component_model_support.wit_worlds unexpectedly passed")
    assert_reason(result, "RUNTIME_WIT_WORLD_UNSUPPORTED")

    unsupported_world = deepcopy(runtime)
    unsupported_world["component_model_support"]["wit_worlds"] = ["different-world"]
    result = match_hard_requirements(skill, unsupported_world)
    if result["ok"]:
        raise SystemExit("Fail-closed probe failed: unsupported WIT world unexpectedly passed")
    assert_reason(result, "RUNTIME_WIT_WORLD_UNSUPPORTED")


def main() -> None:
    skills = [(path, load_json(path)) for path in SKILLS]
    runtimes = [(path, load_json(path)) for path in RUNTIMES]
    lines = build_matrix_lines(skills, runtimes)

    output = "\n".join(lines) + "\n"
    (BASE / "compatibility_matrix.md").write_text(output)
    print(output)
    verify_fail_closed_wit_world_probes()
    print("Verified fail-closed WIT-world compatibility probes.")


if __name__ == "__main__":
    main()
