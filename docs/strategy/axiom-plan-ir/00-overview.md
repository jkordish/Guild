# Axiom Plan IR Overview

Status: exploratory planning note; docs-local schema and validator spike exist,
but there is no Guild runtime implementation.

## Boundary

Guild executes; Axiom plans.

Axiom Plan IR is an AI-facing graph language for planning compositions of
Guild skills. It is a proposed pre-admission artifact that lets an AI or
another planning layer describe a reviewable skill composition before any Guild
runtime boundary is crossed.

Axiom Plan IR is treated as a pre-admission planning artifact. It may describe requested skill refs, dependency edges, arguments, requested grants, expected outputs, and expected evidence, but Guild remains the only source of runtime admission, grant narrowing, execution identity, receipts, evidence persistence, and inspect/explain truth. Any Axiom plan trace is explanatory and subordinate to Guild durable records.

This note does not add runtime behavior. It does not define new Rust types,
crates, WIT contracts, manifest fields, or execution semantics.

## AI Value

Axiom Plan IR is useful only if it gives an AI a smaller, clearer action space
than issuing commands directly:

- explicit `skill://` refs instead of broad natural-language tool choices
- explicit requested grants before policy decides what is actually allowed
- a dependency graph that separates parallel, ordered, and conditional work
- reviewable arguments, expected outputs, and expected evidence before
  execution
- one plan artifact that a human, validator, or lowerer can inspect without
  trusting the AI's hidden chain of thought

The plan remains advisory. It can make intent legible, but it cannot make a
Guild execution safe, admitted, verified, or true on its own.

## Flow

```text
AI/planner -> Axiom Plan IR -> validator -> lowerer -> Guild
```

The docs-local validator checks graph shape, node IDs, dependency references,
skill reference syntax, requested grant declarations, and forbidden runtime-truth
claims at Axiom-owned planning boundaries. A future lowerer would translate the
accepted plan into a non-executing Guild run plan or a sequence of ordinary Guild
requests for review. Guild would still perform resolution, admission, grant
narrowing, execution, receipt persistence, evidence persistence, and later
inspection.

## What Axiom Does Not Own

Axiom does not execute skills. It does not admit runs, grant authority, define
runtime truth, own receipts, or persist evidence. It does not replace Guild's
resolved skill identity, policy decision, execution record, receipt, evidence
record, or inspect/explain surfaces.

If an Axiom plan and Guild durable records disagree, Guild durable records win.

## Reading Order

- [`01-core-model.md`](01-core-model.md)
- [`02-guild-mapping.md`](02-guild-mapping.md)
- [`03-non-goals.md`](03-non-goals.md)
- [`04-spike-plan.md`](04-spike-plan.md)
- [`05-schema-and-examples.md`](05-schema-and-examples.md)
