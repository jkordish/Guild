# Axiom Plan IR Schema And Examples

## Status

Status: exploratory docs-local schema and validator spike, not execution.

This package adds a docs-local JSON Schema and example fixtures for the
proposed Axiom Plan IR shape:

- schema: [`schema/axiom-plan-ir.schema.json`](schema/axiom-plan-ir.schema.json)
- examples: [`examples/`](examples/)

The schema and validator describe only a proposed pre-admission plan shape.
They do not add runtime behavior, do not define Rust types, crates, WIT
contracts, manifest fields, `SPECS.md` contract text, or execution semantics,
and do not claim that Axiom exists as an implemented Guild component.

Guild executes; Axiom plans. Guild remains the only source of runtime
admission, grant narrowing, execution identity, receipts, evidence persistence,
and inspect/explain truth.

## Schema Checks

The schema uses JSON Schema 2020-12 and keeps Axiom-owned objects strict with
`additionalProperties: false`.

The version-1 shape is camelCase and intentionally small:

- top-level fields: `kind`, `version`, `name`, `nodes`
- node fields: `id`, `skill`, `args`, `dependsOn`, `requestedGrants`,
  `expectedOutputs`, `expectedEvidence`, `failureBehavior`

The schema requires `kind: "axiom.plan"` and `version: "1"`. Each node
requires only `id` and `skill`; the remaining node fields are allowed but
optional. Node IDs match:

```text
^[a-zA-Z][a-zA-Z0-9_-]*$
```

Each node's string `skill` uses the exploratory skill-ref regex:

```text
^skill://[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+@[^\s]+$
```

This regex is deliberately not Guild's normative requested-skill-reference
grammar. It is only a docs-spike guardrail so malformed examples fail before
any future Guild-facing surface exists.

Flexible JSON is allowed only where the plan needs planner-owned payloads or
hints:

- `args`
- `requestedGrants[].constraints`
- `expectedOutputs[].shapeHint`
- `expectedOutputs[].schemaHint`

Those flexible fields are planner payloads and hints. They are not a second
copy of Guild's normative manifest, grant, output, or evidence schemas. The
validator also treats skill-owned `args` as payload rather than as an Axiom
runtime-truth surface; it does not recursively police arbitrary nested `args`
keys for forbidden runtime-truth names.

Runtime-truth fields are intentionally absent at the plan and node boundaries.
Fields such as `executionId`, `receipt`, `grantedAuthority`,
`effectiveAuthority`, `hostDecision`, and `runtimeStatus` fail validation
because the schema admits only the Axiom-owned fields above and the validator
also performs an explicit forbidden-field traversal over Axiom-owned planning
objects.

The only authority field is `requestedGrants`. The schema does not define
`grants`, `grantedGrants`, or any final or effective authority field.

Guild refs in the examples use the plural canonical roots only:

- `guild://executions/...`
- `guild://objects/sha256/...`
- `guild://objects/records/...`
- `guild://queries/executions/...`

## Validator Checks

Run the docs-local validator with:

```bash
cargo run -q -p xtask -- axiom-plan validate-examples
```

`validate` parses JSON, runs JSON Schema shape checks, then runs conservative
semantic graph checks where the structure is safe enough to inspect. It reports
combined diagnostics instead of stopping after the first schema issue.

Stable schema diagnostic codes are:

- `axiom.schema.missing_required_field`
- `axiom.schema.additional_property`
- `axiom.schema.invalid_shape`

The validator also checks:

- duplicate node IDs
- unknown dependency references
- self-dependencies
- graph cycles
- dependency-aware shallow references
- malformed exploratory skill refs
- malformed requested-grant shape
- forbidden runtime-truth fields on Axiom-owned planning objects

Forbidden runtime-truth traversal is intentionally bounded to the top-level
plan object, node objects, object-form `skill`, `requestedGrants[]`,
`expectedOutputs[]`, `expectedEvidence[]`, and object-like
`failureBehavior`. It does not recursively inspect arbitrary nested `args`
payloads.

## Validator Limits

Schema and semantic checks do not make the plan admitted, executable, or true.
These remain unchecked here:

- skill availability
- Guild resolution
- Guild admission
- granted authority
- runtime execution
- evidence persistence
- receipt creation
- real policy reduction
- full normative Guild skill-ref grammar

Guild reference resolution and dependency ordering beyond the conservative
local graph checks are also outside this docs-local validator.

## Validation Expectations

The four files under `examples/valid/` should pass `validate-examples`.

The seven files under `examples/invalid/` should fail with at least these
stable diagnostic codes:

- `examples/invalid/duplicate-node-id.json`:
  `axiom.duplicate_node_id`
- `examples/invalid/unknown-dependency.json`:
  `axiom.unknown_dependency`
- `examples/invalid/malformed-skill-ref.json`:
  `axiom.malformed_skill_ref`
- `examples/invalid/granted-authority-claim.json`:
  `axiom.forbidden_runtime_truth_field`
- `examples/invalid/cycle.json`:
  `axiom.dependency_cycle`
- `examples/invalid/bad-reference.json`:
  `axiom.unknown_reference`, `axiom.unsupported_reference`
- `examples/invalid/missing-required-field.json`:
  `axiom.schema.missing_required_field`

Invalid fixtures may emit additional relevant diagnostics. The examples check
fails only when an invalid fixture passes unexpectedly or lacks its expected
stable code.

These commands validate the docs-local package:

```bash
cargo run -q -p xtask -- axiom-plan validate-examples
make axiom-plan
```

They are not wired into `verify`.

## Guild-Owned Boundaries

The validator still does not check:

- installed skill resolution
- Guild admission
- policy reduction
- granted authority
- runtime execution
- receipt persistence
- evidence persistence
- whether expected outputs or expected evidence actually exist

Those remain Guild-owned boundaries.

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
- [`examples/valid/with-references.json`](examples/valid/with-references.json)
  shows shallow `$input.*` and dependency-accessible `$<nodeId>.*`
  references.

## Invalid Examples

- [`examples/invalid/duplicate-node-id.json`](examples/invalid/duplicate-node-id.json)
  fails semantic validation with `axiom.duplicate_node_id`.
- [`examples/invalid/unknown-dependency.json`](examples/invalid/unknown-dependency.json)
  fails semantic validation with `axiom.unknown_dependency`.
- [`examples/invalid/malformed-skill-ref.json`](examples/invalid/malformed-skill-ref.json)
  fails validation because the `skill` field does not match the exploratory
  skill-ref shape.
- [`examples/invalid/granted-authority-claim.json`](examples/invalid/granted-authority-claim.json)
  fails validation because a node claims `grantedAuthority`, which is runtime
  truth owned by Guild rather than Axiom.
- [`examples/invalid/cycle.json`](examples/invalid/cycle.json)
  fails semantic validation with `axiom.dependency_cycle`.
- [`examples/invalid/bad-reference.json`](examples/invalid/bad-reference.json)
  fails semantic validation for unknown and unsupported shallow references.
- [`examples/invalid/missing-required-field.json`](examples/invalid/missing-required-field.json)
  fails schema validation because a node is missing the required `skill` field.

## Future Notes

Any future promotion must stay above Guild and reject malformed graph shape
before any lowering step. It still must not admit executions, reduce policy, grant
authority, resolve installed skill identity, execute skills, mint execution
identity, persist receipts, or persist evidence. Those remain Guild-owned
boundaries.

A future lowerer may translate a validated plan into a non-executing Guild run
plan or a sequence of ordinary Guild requests for review. Any concrete
execution path must still enter Guild through normal resolution, admission,
policy, runtime, receipt, and evidence paths.
