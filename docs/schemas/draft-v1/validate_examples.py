import json
from pathlib import Path

from jsonschema import Draft202012Validator

try:
    from referencing import Registry, Resource
except ModuleNotFoundError as exc:
    raise SystemExit(
        "Missing validation dependency: referencing. Install local deps with `pip install -r requirements.txt`."
    ) from exc


BASE = Path(__file__).resolve().parent
SCHEMA_FILES = [
    "common.schema.json",
    "skill_contract.schema.json",
    "runtime_guarantee.schema.json",
    "proof_record.schema.json",
    "witness_record.schema.json",
]
EXAMPLES = [
    ("skill_contract.schema.json", "examples/local-log-analyzer.contract.json"),
    ("skill_contract.schema.json", "examples/zero-authority.contract.json"),
    ("skill_contract.schema.json", "examples/fetch-transform.contract.json"),
    ("skill_contract.schema.json", "examples/cluster-rollout.contract.json"),
    ("runtime_guarantee.schema.json", "examples/wasmtime-strict.runtime.json"),
    ("runtime_guarantee.schema.json", "examples/node-wasi-basic.runtime.json"),
    ("proof_record.schema.json", "examples/local-log-analyzer.proof.json"),
    ("witness_record.schema.json", "examples/cluster-rollout.witness.json"),
]


def build_registry() -> Registry:
    registry = Registry()
    for name in SCHEMA_FILES:
        path = BASE / name
        contents = json.loads(path.read_text())
        resource = Resource.from_contents(contents)
        registry = registry.with_resource(path.as_uri(), resource)
        registry = registry.with_resource(name, resource)
    return registry


def validate(schema_name: str, instance_name: str, registry: Registry) -> list[str]:
    schema = json.loads((BASE / schema_name).read_text())
    instance = json.loads((BASE / instance_name).read_text())
    validator = Draft202012Validator(schema, registry=registry)
    errors = sorted(validator.iter_errors(instance), key=lambda e: list(e.path))
    return [f"{instance_name}: {'/'.join(map(str, e.path)) or '<root>'}: {e.message}" for e in errors]


def main() -> int:
    registry = build_registry()
    failures: list[str] = []
    for schema_name, instance_name in EXAMPLES:
        failures.extend(validate(schema_name, instance_name, registry))

    if failures:
        print("Validation failed:")
        for failure in failures:
            print(f" - {failure}")
        return 1

    print("All bundled examples validate cleanly.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
