# Axiom Plan IR Spike Plan

Status: docs-first, code-later.

This spike should stay above Guild. It should prove whether Axiom Plan IR gives
AI and human reviewers a clearer pre-admission surface before any crate, runtime
path, WIT contract, manifest field, or execution semantic changes are proposed.

## Sequence

1. Inventory Guild concepts relevant to Axiom Plan IR.
2. Define a JSON schema for `axiom.plan`.
3. Add a validator-only prototype for graph shape, node IDs, dependencies, references, and grant declarations.
4. Add a lowerer prototype that emits a non-executing Guild run plan.
5. Add tests using example skills only.
6. Add a dry-run explain surface.
7. Decide whether this deserves a crate.

The docs-local schema and examples spike for step 2 is
[`05-schema-and-examples.md`](05-schema-and-examples.md).

## Kill Criteria

- If Axiom duplicates Guild composite skill semantics, stop.
- If Axiom cannot produce a clearer review surface than existing Guild commands, stop.
- If Axiom requires widening Guild runtime contracts prematurely, stop.
- If Axiom cannot remain docs/validator/lowerer above Guild, stop.

## Code-Later Constraints

A future validator should reject malformed graph structure and unknown planning
shapes before lowering, but it should not make policy promises. A future lowerer
should produce a non-executing Guild run plan for review, not a side-channel
runtime. Any executing path must still be ordinary Guild execution.

The first prototype, if any, should use example skills only and should treat
every observed output, receipt, and evidence ref as Guild-owned durable truth.
