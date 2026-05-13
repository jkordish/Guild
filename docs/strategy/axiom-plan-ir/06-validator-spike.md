# Validator Spike

## Status

Status: exploratory, validator-only, not execution.

This spike validates a proposed Axiom Plan IR document as a pre-admission
planning artifact. It is not Guild admission, not Guild runtime truth, not a
lowering path, and not evidence or receipt creation.

Guild executes. Axiom plans.

## What It Checks

- top-level shape for `kind`, `version`, `name`, and non-empty `nodes`
- duplicate node IDs and the exploratory node ID pattern
- unknown dependencies and self-dependencies
- dependency graph cycles
- basic exploratory `skill://namespace/name@version-ish` string shape
- forbidden runtime-truth claims at plan and node boundaries
- shallow reference syntax for `$input.*` and dependency-accessible `$node.*`
- requested grant shape as requested authority only

## What It Does Not Check

- skill availability
- Guild resolution
- Guild admission
- granted authority
- runtime execution
- evidence persistence
- receipt creation
- real policy reduction
- full Guild skill-ref grammar

## Commands

Validate one plan:

```bash
cargo run -q -p xtask -- axiom-plan validate docs/strategy/axiom-plan-ir/examples/valid/basic-two-node-plan.json
```

Validate all docs-local examples:

```bash
cargo run -q -p xtask -- axiom-plan validate-examples
```

The examples check expects every file in `examples/valid/` to pass and every
file in `examples/invalid/` to fail. Diagnostics use stable fields internally:
`code`, `severity`, `path`, and `message`. The prototype currently prints a
readable text form; JSON output is future work.

## Relationship To Schema

The JSON Schema checks local shape for the docs-local Axiom vocabulary. The
validator checks semantic graph constraints that JSON Schema does not express
well, such as duplicate node IDs, unknown dependencies, cycles, and shallow
reference roots.

Neither the schema nor this validator creates Guild runtime truth. A valid
Axiom plan is still only a reviewable planning artifact. Guild remains
canonical for skill resolution, admission, grant narrowing, execution identity,
receipts, evidence persistence, and inspect/explain truth.

## Promotion Criteria

This spike should not become a crate unless:

- it catches meaningful plan-review failures
- it stays above Guild runtime
- it does not duplicate Guild composite skill execution
- it can lower to a non-executing Guild run-plan preview later

## Kill Criteria

Stop if:

- it starts duplicating Guild runtime or admission logic
- it requires WIT or runtime contract changes
- it claims granted authority
- it becomes a generic workflow engine
