# Evolve Guild Into A Trusted Session Substrate

## Context

Guild already has real host-owned execution identity, capability gating,
durable execution records, durable evidence records, and a thin CLI/MCP
surface over that trust chain. Today, that shipped slice is still centered on
portable skills and inspect-first execution.

The next evolution is to make that same trust chain serve a more durable
product abstraction: a long-lived session for isolated harness execution.

## Problem Statement

The current framing is honest about what ships today, but it still leaves Guild
easy to misread as a portable skill runtime or a narrow execution substrate.
That framing is too close to implementation detail. It does not yet give the
product a durable user-facing abstraction for work that may need to continue
across reconnects, resume after interruption, or recover from isolation resets.

## Thesis

Guild should evolve into a trusted session substrate for isolated harness execution.
In concrete platform terms, that means becoming the admission controller,
session broker, and receipt engine for isolated harness execution.

Calls target a durable session. The platform resumes that session when it can,
rehydrates it when it must, and cold-starts only when forced. The product abstraction is the session, not the sandbox. Harness is the new first-class execution abstraction. Sandbox lifecycle remains an internal implementation detail.

## User-Facing Abstraction

- `session`: the durable unit the caller addresses and reasons about
- `harness`: the isolated execution environment and tool/capability envelope
- `receipt`: the durable host-owned record of what was admitted, run, resumed,
  denied, rehydrated, and evidenced

## Strategic Goals

- Make the durable session the primary product abstraction in repo docs and
  planning.
- Introduce Harness as a first-class concept without pretending the harness
  packaging/runtime contract already exists.
- Define session lifecycle language that is honest about warm resume,
  rehydration, and cold-start fallback.
- Clarify the split between admission-time policy, wake-time policy, and
  receipt-time provenance.
- Preserve useful current investments: portable skill packaging,
  capability-gated execution, durable records, evidence refs, and small MCP
  surface area.

## Non-Goals

- No giant refactor from skill-first runtime to session runtime in this pass.
- No fake session manager, snapshot system, or lifecycle daemon.
- No new claim that durable session resume or rehydrate already ships.
- No widening of current runtime support by prose alone.
- No replacement of current skill manifests, WIT, or portable bundle flows in
  this pass.

## Milestone Breakdown

1. `M1 Session Vocabulary Freeze`
   Establish the new north star, glossary, ADR, roadmap, and drift guards.
2. `M2 Session Contract Scaffolding`
   Add minimal shared types and trait seams for session lifecycle and harness
   coordination.
3. `M3 Harness Packaging And Invocation Shape`
   Decide how harness identity relates to current skill packaging and runtime
   contracts.
4. `M4 Durable Session Lifecycle`
   Implement session identity, persistence tiers, and explicit warm/resume vs
   rehydrate vs cold decisions.
5. `M5 Admission And Receipt Expansion`
   Add session-aware admission decisions and richer receipt aggregation at the
   session layer.

## Risks / Failure Modes

- Replacing the current shipped story too aggressively and making the repo look
  less honest about what works today.
- Introducing `session` and `harness` terms without clarifying their relation
  to current skill/runtime concepts.
- Adding code scaffolding that implies a manifest or runtime contract the repo
  does not yet own.
- Leaving old drift guards in place so the repo fights its own new direction.
- Treating sandbox lifecycle as product surface instead of internal mechanism.

## Acceptance Criteria

- A fresh contributor can answer “What is Guild becoming?” from `README.md`,
  `AGENTS.md`, ADR 0020, and the strategy docs without reading issue history.
- The repo clearly distinguishes shipped truth from planned direction.
- Harness and session terminology is defined consistently across the docs.
- The docs explain `warm`, `resumed`, `rehydrated`, and `cold` in one place.
- The repo has a PR-sized, dependency-ordered backlog for the next phase.
- Any code scaffolding added in this pass compiles cleanly and does not change
  runtime behavior.

## Done Enough For V1

This umbrella is done enough for v1 when the direction is visible in all major
entrypoints, the main open design questions are written down, the next
implementation tasks are sequenced, and the shared type/trait seams exist
without speculative lifecycle machinery.

## Top Open Questions

- What stable identifier makes a session addressable across runtime restarts?
- Which session state is durable host truth versus rebuildable harness state?
- How should Harness relate to the current skill manifest and resolved skill
  identity model?
- When a session wakes, which policy checks rerun at wake time versus invoke
  time?
- Should receipts aggregate at the session layer, the execution-attempt layer,
  or both?
