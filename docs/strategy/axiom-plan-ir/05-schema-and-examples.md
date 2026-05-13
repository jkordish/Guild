# Axiom Plan IR Schema And Examples

## Status

Status: exploratory docs spike, not implemented.

This package adds a docs-local JSON Schema and example fixtures for the
proposed Axiom Plan IR shape:

- schema: [`schema/axiom-plan-ir.schema.json`](schema/axiom-plan-ir.schema.json)
- examples: [`examples/`](examples/)

The schema describes only a proposed pre-admission plan shape. It does not add
runtime behavior, does not define Rust types, crates, WIT contracts, manifest
fields, `SPECS.md` contract text, or execution semantics, and does not claim
that Axiom exists as an implemented Guild component.

Guild executes; Axiom plans. Guild remains the only source of runtime
admission, grant narrowing, execution identity, receipts, evidence persistence,
and inspect/explain truth.

## Schema Checks

The schema uses JSON Schema 2020-12 and keeps the Axiom-owned vocabulary
strict with `additionalProperties: false`.

The version-1 shape is camelCase and intentionally small:

- top-level fields: `kind`, `version`, `name`, `nodes`
- node fields: `id`, `skill`, `args`, `dependsOn`, `requestedGrants`,
  `expectedOutputs`, `expectedEvidence`, `failureBehavior`

The schema requires `kind: "axiom.plan"` and `version: "1"`. Each node must use
the exploratory skill-ref regex:

```text
^skill://[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+@[^\s]+$
```

This regex is deliberately not Guild's normative requested-skill-reference
grammar. It is only a docs-spike guardrail so malformed examples fail before a
future validator exists.

Flexible JSON is allowed only where the plan needs planner-owned payloads or
hints:

- `args`
- `requestedGrants[].constraints`
- `expectedOutputs[].shapeHint`
- `expectedOutputs[].schemaHint`

Those flexible fields are planner payloads and hints. They are not a second
copy of Guild's normative manifest, grant, output, or evidence schemas.

Runtime-truth fields are intentionally absent at the plan and node boundaries.
Fields such as `executionId`, `receipt`, `grantedAuthority`,
`effectiveAuthority`, `hostDecision`, and `runtimeStatus` fail schema
validation because the schema admits only the Axiom-owned fields above.

The only authority field is `requestedGrants`. The schema does not define
`grants`, `grantedGrants`, or any final or effective authority field.

Guild refs in the examples use the plural canonical roots only:

- `guild://executions/...`
- `guild://objects/sha256/...`
- `guild://objects/records/...`
- `guild://queries/executions/...`

## Schema Limits

JSON Schema checks structure. It does not make the plan semantically valid,
admitted, executable, or true.

The following checks are outside JSON Schema's authority in this spike and
belong to a future semantic validator or to Guild itself:

- duplicate node IDs
- unknown dependency references
- graph cycles
- dependency ordering beyond local shape
- Guild reference resolution
- skill availability
- installed skill resolution
- Guild admission
- policy reduction
- granted authority
- runtime execution
- receipt persistence
- evidence persistence
- whether expected outputs or expected evidence actually exist

The invalid fixtures therefore split into two groups: schema failures and
future semantic-validator failures.

## Validation Expectations

When a JSON Schema validator is available, the three files under
`examples/valid/` should pass the docs-local schema.

Only these invalid fixtures should fail JSON Schema today:

- `examples/invalid/malformed-skill-ref.json`
- `examples/invalid/granted-authority-claim.json`

These invalid fixtures are expected to pass JSON Schema today and fail only once
a future semantic validator exists:

- `examples/invalid/duplicate-node-id.json`
- `examples/invalid/unknown-dependency.json`

A local check can make that split explicit:

```bash
python3 - <<'PY'
import json
from pathlib import Path

from jsonschema import Draft202012Validator

root = Path("docs/strategy/axiom-plan-ir")
schema = json.loads((root / "schema/axiom-plan-ir.schema.json").read_text())
validator = Draft202012Validator(schema)

valid = sorted((root / "examples/valid").glob("*.json"))
schema_invalid = [
    root / "examples/invalid/malformed-skill-ref.json",
    root / "examples/invalid/granted-authority-claim.json",
]
semantic_only = [
    root / "examples/invalid/duplicate-node-id.json",
    root / "examples/invalid/unknown-dependency.json",
]

for path in valid:
    validator.validate(json.loads(path.read_text()))
for path in schema_invalid:
    errors = list(validator.iter_errors(json.loads(path.read_text())))
    assert errors, f"{path} unexpectedly passed JSON Schema"
for path in semantic_only:
    validator.validate(json.loads(path.read_text()))
PY
```

## Valid Examples

- [`examples/valid/basic-two-node-plan.json`](examples/valid/basic-two-node-plan.json)
  shows two nodes where the second depends on the first and neither node
  requests grants.
- [`examples/valid/with-requested-grants.json`](examples/valid/with-requested-grants.json)
  shows a `read-resource` request over `guild://executions/` and
  `guild://queries/executions/`. Those entries are requested grants, not
  granted authority.
- [`examples/valid/with-expected-evidence.json`](examples/valid/with-expected-evidence.json)
  shows expected evidence declarations. They are expectations only, not claims
  that Guild evidence records exist.

## Invalid Examples

- [`examples/invalid/duplicate-node-id.json`](examples/invalid/duplicate-node-id.json)
  is a future semantic-validator failure. It may pass JSON Schema because JSON
  Schema does not compare node IDs across array items.
- [`examples/invalid/unknown-dependency.json`](examples/invalid/unknown-dependency.json)
  is a future semantic-validator failure. It may pass JSON Schema because JSON
  Schema does not resolve `dependsOn` values to node IDs.
- [`examples/invalid/malformed-skill-ref.json`](examples/invalid/malformed-skill-ref.json)
  is a JSON Schema failure because the `skill` field does not match the
  exploratory skill-ref regex.
- [`examples/invalid/granted-authority-claim.json`](examples/invalid/granted-authority-claim.json)
  is a JSON Schema failure because a node claims `grantedAuthority`, which is
  runtime truth owned by Guild rather than Axiom.

## Future Validator Notes

A future validator should stay above Guild and should reject malformed graph
shape before any lowering step. It should check node ID uniqueness, dependency
references, cycles, and any allowed planning vocabulary that cannot be encoded
cleanly in JSON Schema.

That validator still must not admit executions, reduce policy, grant
authority, resolve installed skill identity, execute skills, mint execution
identity, persist receipts, or persist evidence. Those remain Guild-owned
boundaries.

A future lowerer may translate a validated plan into a non-executing Guild run
plan or a sequence of ordinary Guild requests for review. Any concrete
execution path must still enter Guild through normal resolution, admission,
policy, runtime, receipt, and evidence paths.
