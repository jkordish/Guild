# Roadmap

Guild is sequenced as a small set of ordered epics. The labels below keep the existing phase order, but the outcome comes first.
Current framing and vocabulary freeze live in [`project-positioning.md`](project-positioning.md).
Canonical operator-facing vocabulary lives in
[`strategy/guild-repositioning/02-glossary-and-banned-terms.md`](strategy/guild-repositioning/02-glossary-and-banned-terms.md).

The first five epics built the contract, execution, trust, and evidence
surfaces. The next narrative wave turns that existing trust chain into
operator-facing playbooks, capability review, receipts, and evidence rather
than describing Guild primarily as runtime plumbing.

## Epic 1: Define The Contract Surface

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

## Next Phase

The next planning anchor after `P2-project-refocus-and-message-freeze` is
[`portable-skill-receipts-and-reference-apps.md`](roadmap/epics/portable-skill-receipts-and-reference-apps.md).

Outcome: turn the existing trust and receipt layer into operator-facing
playbooks, starter sets, and capability review instead of widening Guild back
into a generic runtime story.

- operator-first onboarding and trust review
- playbook-first docs and examples on top of the current skill surfaces
- receipt-first execution, evidence, and bounded-query views
- bounded starter-set and reference-application reports on current proven surfaces
- drift guards that keep the project thesis and anti-thesis stable
