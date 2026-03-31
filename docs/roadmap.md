# Roadmap

Guild is sequenced as a small set of ordered epics. The labels below keep the
existing phase order, but the outcome comes first.

The first five epics built the contract, execution, trust, and evidence
surfaces. The next narrative wave turns that existing trust chain into
operator-facing playbooks, capability review, receipts, and evidence rather
than describing Guild primarily as runtime plumbing.

## Epic 1: Define The Contract Surface

For the current long-term direction, see
[`strategy/session-substrate/00-umbrella-epic.md`](strategy/session-substrate/00-umbrella-epic.md)
and [`strategy/session-substrate/07-roadmap.md`](strategy/session-substrate/07-roadmap.md).
[`project-positioning.md`](project-positioning.md) remains the compatibility
bridge from the prior framing to the new one.

Current mapping: Phase 0

Outcome: make the repo honest before making it impressive.

- define shared Rust types
- define the manifest model
- draft the WIT ABI
- scaffold the Cargo workspace
- define MCP facade names
- add one example skill manifest

## Epic 2: Run Skills Locally

Current mapping: Phase 1

Outcome: ship one real inspect-mode vertical slice.

- local registry storage
- Wasm runner
- capability grant evaluation
- execution records
- evidence storage
- `guild.inspect` end to end

## Epic 3: Explain And Compose Stored Work

Current mapping: Phase 2

Outcome: make durable execution state useful after the run completes.

- child skill invocation
- dependency snapshots
- explain and debug skills over persisted execution state
- bounded local query resources over stored executions
- better diagnostics and provenance

## Epic 4: Package, Trust, And Distribute

Current mapping: Phase 3

Outcome: move installed skills safely between local roots and registries.

- signed skill packages
- publisher identity
- trust tiers
- org vs public visibility
- package verification

## Epic 5: Apply With Discipline

Current mapping: Phase 4

Outcome: support mutation without pretending retries, approval, or audit are optional.

- idempotency keys
- approvals
- audit records
- bounded retries
- clearer effect semantics

## Current Milestone

The current planning anchor is the session-substrate evolution:

- umbrella epic:
  [`strategy/session-substrate/00-umbrella-epic.md`](strategy/session-substrate/00-umbrella-epic.md)
- north star:
  [`strategy/session-substrate/01-north-star.md`](strategy/session-substrate/01-north-star.md)
- milestone roadmap:
  [`strategy/session-substrate/07-roadmap.md`](strategy/session-substrate/07-roadmap.md)
- backlog:
  [`strategy/session-substrate/tasks.md`](strategy/session-substrate/tasks.md)

The prior playbook/starter-set wave remains useful historical context and still
describes parts of the current shipped slice, but it is no longer the primary
long-term planning center.
