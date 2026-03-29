# Portable Skill Receipts And Reference Apps Execution Guide

This guide turns the open follow-on GitHub issue set for the next Guild phase
into execution-ready planning. It is not a runtime contract source. Runtime,
ABI, and security truth still live in [`SPECS.md`](../../../SPECS.md),
[`ARCHITECTURE.md`](../../../ARCHITECTURE.md), Rust types, and WIT.

Use this guide when writing issue bodies, sequencing work, or deciding whether
a proposed change belongs in the current proven frontier or in a later phase.
It is also the canonical intake point for the imported repositioning strategy
stack that has now been absorbed into the live repo docs and issue tracker.

## Program Sequence

Use this order unless a future repo change materially shifts the support
frontier:

1. `#130` ship one cohesive starter path on current read-only surfaces
2. `#136` describe the packaging/install follow-on work on current transport flows
3. `#137` define bounded starter-pack and reference-playbook progression
4. `#131` document authoring-layer guardrails so ergonomics do not fork the contract model
5. `#133` define the verification matrix from current trust signals
6. `#132` choose the first honest post-starter mutation demo
7. `#138` scope receipt-chain and replay-oriented explanation follow-on work
8. `#134` scope private-pack, policy, and governance boundaries for the later team phase

That order keeps current implementation work on already-shipped read paths
first, then sequences packaging, verification, mutation planning, replay
planning, and governance in the order they become believable.

## Issue #129: Portable Skill Receipts And Reference Playbooks Follow-On Program

### Design Guide

- Treat `#129` as the umbrella issue for sequencing and guardrails, not as a feature delivery issue.
- Keep the product thesis stable: playbooks are the user-facing layer, and the trust chain is the differentiator.
- Every child issue must say what is already shipped, what is docs-first, and what is not yet proven.
- Do not let the umbrella drift into a generic workflow-engine, marketplace, or hosted-control-plane story.

### Implementation Guide

1. Keep the roadmap epic doc and the GitHub issue tree synchronized.
2. Add or remove child issues only when the support frontier changes materially.
3. Keep one canonical sequence in this guide and use it when prioritizing child issues.
4. Update success criteria when a child issue meaningfully changes the phase boundary.

### Suggested Subtasks

- [ ] Keep the child-issue list current in the roadmap epic doc and GitHub epic body.
- [ ] Reclassify any child issue that broadens support without matching runtime proof.
- [ ] Record deliberate deferrals instead of leaving them implied.

### Validation

- `git diff --check`
- repo guide coverage test in `crates/guild-mcp/tests/guild_cli.rs`

## Issue #130: Cohesive Incident-Casefile-First Starter Path

### Design Guide

- The starter path must stay read-only and explicit-ref-first.
- The user promise is one bounded outcome: inspect a stored execution, trace evidence, and render one casefile from exact refs.
- The starter path must not imply broad action support, replay execution, or ambient discovery.
- `Guild Ops Starter` remains a repo-local starter set and release slice, not the whole product thesis.
- The hero example remains `diagnose service -> restart workload -> notify on-call`, but only the diagnose and verify side is runnable today.

### Implementation Guide

1. Keep one primary operator path: `guild why`, `guild why --lineage`, then `guild run incident-casefile ...`.
2. Make every starter example consume exact execution, query, or evidence refs as input.
3. Keep Codex/bootstrap helpers aligned with the same honest CLI-first path.
4. Add drift guards that reject unsupported starter claims.

### Reference Playbook Boundary

Keep the current progression explicit:

- real now: incident-casefile-first operational review over stored refs
- docs-first next: restart-and-notify and rollback-and-annotate playbook stories
- likely first mutation-demo candidate: `cache purge with evidence trail`
- deferred until broader action support: certificate rotation, node remediation, and secret rotation flows

### Suggested Subtasks

- [ ] Keep `incident-casefile` inputs explicit and read-only.
- [ ] Keep README, examples, and quickstart docs aligned to the same starter flow.
- [ ] Pin the starter wording in CLI/docs regressions.
- [ ] Reconcile stale checklist items when the starter path makes prior planning obsolete.

### Validation

- `cargo test -p guild-mcp --test guild_cli`
- `cargo test -p guild-mcp --test codex_workflow`
- `cargo run -q -p xtask -- project-positioning check`

## Issue #131: Authoring-Layer Guardrails

### Design Guide

- Guild may improve author ergonomics, but runtime truth remains Rust/WIT/spec first.
- Any authoring layer must compile down to current manifest/runtime truth instead of competing with it.
- Candidate metadata from `epics.md` such as `use_cases`, `risk`, `examples`, and `eval_scenarios` should be classified as either docs-only or compile-time convenience before they are treated as product surface.
- Do not make `SKILL.md` the canonical execution contract for Guild.

### Implementation Guide

1. Inventory the current contract sources and explain their roles.
2. Define a small rubric for future authoring metadata: advisory, derived, or normative.
3. Document what an authoring layer may generate versus what the runtime must verify directly.
4. Keep the result tightly linked to the existing contracts-first repo posture.

### Imported Design Inputs To Keep Visible

The removed strategy stack contributed useful design inputs that should remain
visible here without becoming implied commitments:

- a future humane authoring layer may separate reusable skill, operator-facing playbook, and installable bundle concepts
- evidence requirements and approval requirements should stay visible at authoring time rather than hidden later
- example names such as `guild.skill.yaml`, `guild.playbook.yaml`, and `guild-pack.yaml` are design inputs only, not accepted contract or CLI surface
- any authoring layer must compile down to current manifest/runtime truth rather than replace it

### Suggested Subtasks

- [ ] Write a source-of-truth matrix for Rust types, manifests, WIT, and docs.
- [ ] Classify proposed authoring metadata fields by enforcement level.
- [ ] Document compilation boundaries and failure modes for any future authoring layer.
- [ ] Add anti-goals that forbid contract duplication.

### Validation

- `git diff --check`
- docs review against `SPECS.md`, `ARCHITECTURE.md`, and the roadmap epic

## Issue #132: First Honest Post-Starter Mutation Demo

### Design Guide

- The mutation demo should be the smallest action slice that proves approvals, idempotency, audit, and evidence matter.
- Prefer a narrow, trust-heavy operation over a flashy, broad integration.
- Treat broad `k8s`, chat, incident, and secrets actions as deferred unless the runtime support becomes real.
- The current default candidate is `cache purge with evidence trail`, because it is narrower and easier to reason about than restart, rollback, or chat posting flows.

### Implementation Guide

1. Compare 2-3 candidate actions against the current runtime, capability, and audit boundaries.
2. Pick one candidate and explain why the others remain deferred.
3. Define the minimum approval, evidence, retry, and idempotency requirements before implementation begins.
4. Keep the output docs-first until the runtime path is proven.

### Candidate Ordering

Use this ordering unless the support frontier changes materially:

1. `cache purge with evidence trail`
2. rollback-and-annotate
3. restart-and-notify

The hero example stays above those mutation candidates as the long-term
operator story, but it is not the first action slice because it implies a
broader support surface.

### Suggested Subtasks

- [ ] Write the candidate comparison matrix.
- [ ] Choose one preferred mutation demo and one fallback.
- [ ] Define the approval and idempotency invariants for that demo.
- [ ] Record the reasons broader actions remain deferred.

### Validation

- `git diff --check`
- consistency review against `docs/roadmap.md` and `ARCHITECTURE.md`

## Issue #133: Verification Matrix And Curated-Pack Labels

### Design Guide

- The matrix must start from the signals the repo already has, not from future branding.
- Current trustworthy signals include transport verification state, trust tier, durable execution receipts, evidence records, explainability, and proof-backed support boundaries.
- Future signals such as eval pass rate or mutation risk scoring must be labeled as future until the data and runtime support exist.
- The status vocabulary should stay small and honest: `experimental`, `curated`, and `verified` are enough if their meanings are explicit.

### Implementation Guide

1. Inventory the proof, verification, transport, and trust signals already emitted by the repo.
2. Define which matrix columns are current, derived, or future.
3. Write status-label definitions and the minimum bar for each one.
4. Ensure the proposal does not imply automatic ranking or safety guarantees the repo cannot prove.

### Suggested Subtasks

- [ ] Build the current-signal inventory.
- [ ] Draft the first matrix table with “current” and “future” markers.
- [ ] Define label semantics and promotion criteria.
- [ ] Add examples showing what does not qualify as `verified` yet.

### Validation

- `git diff --check`
- consistency review against trust docs and transport/export/import behavior

## Issue #134: Private-Pack, Policy, And Governance Boundaries

### Design Guide

- This issue is about later-phase scoping, not immediate implementation.
- The design center is team adoption with explicit policy, audit, retention, and blast-radius review.
- Keep the future governance surface anchored to the current trust chain, durable refs, and fail-closed behavior.
- Avoid drifting into a vague SaaS or hosted-control-plane story.

### Implementation Guide

1. Define the future private-distribution and policy-control concerns in Guild terms.
2. Separate team-governance requirements from runtime-surface changes.
3. Clarify which concerns belong to policy, which belong to audit, and which belong to retention/redaction.
4. Keep each proposed future surface explicitly marked as planning-only until runtime work exists.

### Suggested Subtasks

- [ ] Write the governance problem statement in team-review terms.
- [ ] Define the policy, audit, retention, and redaction buckets.
- [ ] List the future runtime dependencies without committing to them as shipped work.
- [ ] Record anti-goals for marketplace or control-plane drift.

### Validation

- `git diff --check`
- consistency review against `ARCHITECTURE.md` and the umbrella epic

## Issue #136: Packaging And Install-Surface Follow-On

### Design Guide

- Treat current signed bundle export/import/push/pull and trust verification as the packaging base layer.
- Do not redesign the packaging system or invent a new pack type.
- The user-facing goal is clearer curated-pack installation and compatibility review on top of what already ships.
- Packaging language must stay transport- and trust-accurate rather than drifting back into generic registry rhetoric.

### Implementation Guide

1. Document the already-shipped transport flows and what they prove today.
2. Identify the remaining packaging work that still needs planning or implementation.
3. Define how future curated-pack installation should present compatibility metadata without broadening contracts.
4. Keep the output tightly scoped to current bundle/OCI/trust-review behavior.

### Suggested Subtasks

- [ ] Write the current-state packaging map.
- [ ] Identify the minimum compatibility metadata needed for curated-pack installs.
- [ ] Clarify what packaging work is docs-only versus code-follow-on.
- [ ] Add anti-goals that rule out marketplace language.

### Validation

- `git diff --check`
- consistency review against current export/import/pull/push docs and examples

## Issue #137: Starter-Pack And Reference-Playbook Progression

### Design Guide

- This issue sequences what comes after `Guild Ops Starter`; it does not authorize broad action packs today.
- Future starter packs and reference playbooks should be classified as `real now`, `docs-first`, or `deferred until apply`.
- The classification must be based on current durable refs, bounded query paths, trust review, and evidence surfaces.
- Treat broad ops/security packs from `epics.md` as candidate concepts, not implementation commitments.

### Implementation Guide

1. Inventory the starter-pack and playbook candidates implied by the current docs and `epics.md`.
2. Classify each candidate by support level and explain why.
3. Define the smallest believable next progression after `#130`.
4. Keep the chosen path aligned with current runtime truth and existing examples.

### Candidate Progression

Use this support classification when triaging future reference playbooks:

| Candidate | Support level now | Why |
| --- | --- | --- |
| diagnose -> restart -> notify | docs-first hero example with real inspect bridge | the review half is real now; restart and notify are not |
| rollback -> verify -> annotate incident | docs-first | legible follow-on, but broader mutation surface than the current starter path |
| cache purge with evidence trail | docs-first and mutation-demo candidate | narrowest believable trust-heavy mutation slice |
| cert renewal -> endpoint validation -> notify | deferred until apply | action surface and verification boundaries are not yet honest runtime truth |
| node remediation -> cordon -> drain -> verify recovery | deferred until apply | higher blast radius than the current phase supports |
| secret rotation with approval gate and receipts | deferred until apply | secrets and propagation checks stay later-phase |

### Suggested Subtasks

- [ ] Build the candidate matrix with support-level classification.
- [ ] Choose one or two docs-first starter-pack/report concepts to keep visible.
- [ ] Name the first plausible next implementation candidate, if one exists.
- [ ] Record which concepts remain deferred until apply-mode/runtime support grows.

### Validation

- `git diff --check`
- consistency review against `examples/README.md`, Guild Ops Starter docs, and the roadmap epic

## Issue #138: Receipt-Chain And Replay-Oriented Explanation Follow-On

### Design Guide

- Start from today's durable execution refs, evidence records, lineage, and `guild why` explanation flows.
- Be precise about the boundary between replay-oriented explanation and real replay execution.
- Future replay semantics must be tied to approvals, idempotency, audit, and evidence completeness.
- The trust story should remain receipt-first even before any future replay execution exists.

### Implementation Guide

1. Document the receipt and evidence chain that already exists today.
2. Define the current replay-oriented explanation model in operator terms.
3. Describe what extra state, guarantees, or controls a true replay execution feature would require.
4. Keep future replay work subordinate to mutation, policy, and audit readiness.

### Suggested Subtasks

- [ ] Write the current receipt-chain map from request to evidence.
- [ ] Explain the current operator-facing meaning of “replay-oriented explanation.”
- [ ] Define the prerequisites for any future replay execution semantics.
- [ ] Add anti-goals forbidding early replay claims in docs or examples.

### Validation

- `git diff --check`
- consistency review against trust docs, `guild why`, and stored execution/evidence resources

## Imported Strategy Intake

The imported repositioning stack is planning input, not automatic product
truth. Its durable guidance now lives here so the repo does not need a second
parallel strategy directory.

### Adoption Rules

- Treat imported strategy as planning input, not runtime truth.
- Keep already-completed M1 work closed unless the repo needs new code or docs.
- Absorb overlapping M2-M4 work into the active issue set instead of creating another roadmap.
- Reject imported assumptions that overstate the current support frontier.

### Imported Assumption Review

| Imported assumption | Current disposition | Tracking |
| --- | --- | --- |
| Keep `SKILL.md` canonical | not adopted literally | `#131` |
| Add a friendlier Guild authoring layer | accepted with guardrails | `#131` |
| Package curated starter packs now | accepted in narrower form | `#130`, `#136`, `#137` |
| Differentiate on trust primitives | accepted | `#132`, `#133`, `#138`, `#134` |
| Aim at ops / platform / security teams first | accepted | current project positioning |

### Milestone Mapping

| Imported milestone | Current tracker status | GitHub issues |
| --- | --- | --- |
| M1. Make Guild Legible | largely complete | closed `#86` through `#120` |
| M2. Make Guild Installable and Useful | active | `#130`, `#131`, `#136`, `#137` |
| M3. Make Guild Trustworthy and Differentiated | sequenced after M2 | `#132`, `#133`, `#138`, parts of `#134` |
| M4. Make Guild Adoptable by Teams | later-phase planning | `#134` |

### Imported Epic And Task Mapping

| Imported work | Current disposition | Active tracking |
| --- | --- | --- |
| EPIC-01 and M1 wording reset tasks `GR-001` through `GR-008` | completed overlap | closed `#86` through `#120` |
| EPIC-03 authoring/schema tasks `GR-009` through `GR-013` | bounded evaluation only | `#131` |
| EPIC-04 packaging/install tasks `GR-014` through `GR-018` | direct overlap on current transport flows | `#136` and starter quickstart follow-on in `#130` |
| EPIC-05 starter-pack/reference-playbook tasks `GR-019` through `GR-026` | split into current starter, progression planning, and later mutation planning | `#130`, `#137`, `#132` |
| EPIC-06 receipt/policy/replay tasks `GR-027` through `GR-032` | partly already shipped, partly later planning | `#138`, `#132`, `#134` |
| EPIC-07 verification tasks `GR-033` through `GR-036` | direct overlap | `#133` |
| EPIC-08 governance tasks `GR-037` through `GR-040` | direct overlap | `#134` |

### Cleanup Outcome

The repo-side cleanup formerly tracked in `#139` is complete once the imported
strategy directory is removed and the canonical docs/issues point here instead.
