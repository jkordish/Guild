# ADR 0021: Adopt The Effect Kernel As Guild's Mutation Truth Boundary

- Status: proposed
- Date: 2026-09-02

## Context

Guild currently ships an inspect-first runtime with host-mediated capability
admission, execution-attempt receipts, durable evidence records, and a thin
CLI/MCP surface. It is evolving toward durable sessions for isolated harness
execution. Axiom Plan IR provides an exploratory pre-admission planning and
review surface above Guild.

Guild still rejects `apply` globally because it does not yet have a complete
approval, idempotency, audit, external-effect, and recovery contract.

The recovered Jidoka autonomous change kernel design defines that missing
contract in a narrower form: exact warrants and approvals, permanent effect
idempotency bindings, leases and fences, durable start before mutation,
authoritative typed observations, independent postcondition and causality,
exactly-once terminal effect receipts, custody deeds, and recovery that never
repeats a started mutation.

Keeping Jidoka and Guild as independent products would duplicate admission,
evidence, receipt, policy, CLI, and control-plane concepts. Importing Jidoka's
entire workstation control plane into Guild would preserve the duplication
inside one repository instead of solving it.

## Decision

Guild will own one pure Rust crate named `guild-effect-kernel` as its
authoritative external-mutation state machine.

The crate will implement Jidoka effect protocol v1 while retaining v1 canonical
protocol identifiers and golden bytes. The Jidoka name remains protocol
provenance, not a separate product surface.

The crate:

- accepts values and returns proposed values;
- performs no filesystem, network, process, clock, randomness, database,
  provider, Wasm, MCP, session, or AI operation;
- depends on no other Guild crate;
- owns effect warrants, approvals, reservations, leases, fences, bindings,
  typed evidence classification, effect receipts, deeds, custody, replay, and
  recovery laws;
- does not own executable resolution, capability envelopes, harness execution,
  execution receipts, sessions, planning, provider I/O, persistence, or
  authentication.

Guild's host layers may depend on the kernel. The kernel must never depend on
Guild's host layers.

Guild will preserve these distinct truth layers:

1. Axiom planning truth: advisory intent and expectations.
2. Guild execution/session truth: resolved identity, capability admission,
   attempt outcome, durable evidence records, and session lineage.
3. Effect truth: exact mutation admission, authoritative external outcome, and
   custody.

Guild execution receipts, effect receipts, and future session receipts remain
separate. No generic receipt envelope may collapse their subjects or authority.

A durable effect `Started` event is a retry barrier across execution retries,
session resume, rehydration, cold start, agent retries, and operator retries.
After start, Guild may only re-probe and terminalize the existing effect; it
must never repeat the protected mutation.

The first closed effect family remains static artifact publication and
separation using a local-file workstation adapter. This replaces cache purge as
the first formal mutation proof. Cache purge and other effect families remain
deferred until the closed v1 state machine and conformance suite are complete.

The historical Jidoka v2 CLI, planner, provider framework, policy engine, and
run-state model will not be imported as active Guild architecture. Useful
workstation intent may be selectively redesigned as a later Guild adapter.

## Consequences

Positive:

- Guild has one mutation truth boundary instead of a parallel control plane.
- The effect kernel remains reusable outside Guild through a strict leaf-crate
  boundary.
- Existing execution, evidence, Axiom, and session contracts remain honest and
  separately inspectable.
- Mutation retry and recovery behavior becomes explicit before `apply` is
  enabled.
- The macOS workstation remains a useful proving ground without defining the
  general protocol around Homebrew, Terraform, or another tool.

Costs:

- Guild gains a substantial formal state machine and conformance burden.
- Host integration still requires authenticated store, clock, principal,
  probe, adapter, and execution-link designs after the pure crate is proven.
- Operators will see separate execution and effect outcomes because collapsing
  them would be simpler but dishonest.
- The old Jidoka repository must be preserved and redirected rather than simply
  deleted.

## Guardrails

- Do not enable or advertise `apply` from documentation alone.
- Do not make `guild-effect-kernel` depend on Guild runtime types or I/O.
- Do not treat `CallerRequest.idempotency_key` as a kernel binding without an
  explicit future contract.
- Do not treat `EvidenceRef` or Axiom `expectedEvidence` as authoritative
  kernel observation.
- Do not let execution or session retry policy cross an effect start barrier.
- Do not introduce a generic effect plugin ABI in v1.
- Do not rewrite v1 canonical identities during repository or crate renaming.
- Do not import the historical workstation control plane wholesale.

## Detailed Design

See
[`../superpowers/specs/2026-09-02-guild-effect-kernel-migration-design.md`](../superpowers/specs/2026-09-02-guild-effect-kernel-migration-design.md).
