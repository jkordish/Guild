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
- docs-first next: `service-recovery review pack`
- secondary docs-first concept kept visible: `rollback verification pack`
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

### Current Docs-First Outcome

The current bounded output for this issue is
[`docs/authoring-layer-guardrails.md`](../../../docs/authoring-layer-guardrails.md).
That doc keeps the evaluation docs-first and fail-closed:

- it writes the current source-of-truth matrix instead of treating "docs" as
  one blob
- it classifies future authoring metadata as advisory, derived, or normative
- it defines compile-down rules for what a future authoring layer may generate
  versus what the runtime must still verify directly
- it keeps `SKILL.md` and proposed YAML authoring files as advisory inputs
  unless and until they compile down into today's manifest, WIT, Rust, and
  spec truth

### Current Default Classification For Candidate Authoring Metadata

Use this default unless a later runtime change makes a field concrete:

- advisory today: `use_cases`, `risk`, `examples`, `eval_scenarios`, approval
  notes, and future authoring YAML
- derived today: support matrices, compatibility reports, install reports, and
  trust labels generated from current signals
- normative today: manifest fields, WIT imports/exports, Rust capability
  evaluation, and `SPECS.md` contract language

### Compile-Down Guardrails

Keep the compile-down boundary explicit:

- a friendlier authoring layer may generate `manifest.json`, examples, docs,
  and lint output
- the runtime must still verify manifest identity, requirements, dependency
  wiring, WIT compatibility, and host-owned capability evaluation directly
- if a proposed field changes runtime behavior but cannot compile down exactly,
  it should fail closed rather than inventing hidden semantics
- generated normative files must remain reviewable and subordinate to the
  current manifest/runtime truth

### Explicit Out-Of-Scope Boundary

Keep the issue's non-goals visible in the docs-first output too:

- this does not introduce a new `v1alpha1` contract surface
- this does not let authoring metadata compete with Rust, manifests, WIT, or
  `SPECS.md`
- this does not make authoring YAML a second runtime acceptance path

### Imported Design Inputs To Keep Visible

The removed strategy stack contributed useful design inputs that should remain
visible here without becoming implied commitments:

- a future humane authoring layer may separate reusable skill, operator-facing playbook, and installable bundle concepts
- evidence requirements and approval requirements should stay visible at authoring time rather than hidden later
- example names such as `guild.skill.yaml`, `guild.playbook.yaml`, and `guild-pack.yaml` are design inputs only, not accepted contract or CLI surface
- any authoring layer must compile down to current manifest/runtime truth rather than replace it

### Suggested Subtasks

- [x] Write a source-of-truth matrix for Rust types, manifests, WIT, and docs.
- [x] Classify proposed authoring metadata fields by enforcement level.
- [x] Document compilation boundaries and failure modes for any future authoring layer.
- [x] Add anti-goals that forbid contract duplication.

### Validation

- `git diff --check`
- docs review against `SPECS.md`, `ARCHITECTURE.md`, and the roadmap epic
- `cargo run -q -p xtask -- project-positioning check`
- confirm the result keeps future authoring-layer discussion free of ambiguity
  about what the runtime actually trusts

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

### Current Packaging Map

Use this map when deciding whether a packaging or install idea is already
shipped, only needs docs cleanup, or still requires code follow-on:

| Surface | Current commands | What it proves now | What it is not |
| --- | --- | --- | --- |
| Native signed installed-state bundle directory | `guild export bundle`, `guild import bundle`, `guild import bundle --preview` | one signed installed-state artifact can move between Guild roots without re-resolving source trees | not a source-directory tarball, package registry, or retag primitive |
| OCI image layout directory | `guild export oci-layout`, `guild import oci-layout`, `guild import oci-layout --preview` | the same signed installed-state payload can travel in OCI-layout form without changing local trust or verification rules | not a second trust model and not a bypass around bundle verification |
| OCI registry transport | `guild push`, `guild pull`, `guild pull --preview` | the same OCI-mapped signed payload can move through a registry reference and still verify locally before installation | not registry-to-registry copy, not retag-only promotion, and not remote trust-store sync |
| Target-root trust review | `guild trust list`, `guild trust add`, preview on `import`/`pull`, `guild verify -v` | publisher review, trust tier, verification result, and installed-state outcome remain host-owned and local to the target Guild root | not globally shared trust, not automatic environment promotion, and not a hidden admission side effect |

Keep these current repository boundaries explicit:

- the transport unit is installed executable state, never a source tree
- the native signed bundle directory remains the canonical signed transport unit
- `export` and `push` are publication steps that require a signer and create a
  fresh signed transport artifact
- `import` and `pull` are admission steps into a target root and must verify
  trust, signature, and bundled digests before installation
- `--preview` is the shipped read-only preflight slice for `import bundle`,
  `import oci-layout`, and `pull`; it is not a general detached package report

### Minimum Compatibility Metadata For Future Curated-Pack Installs

If Guild later presents a more curated install view, the minimum compatibility
metadata should stay host-derived from the current installed-bundle and
verification truth rather than becoming a second package contract:

- resolved skill identity plus the concrete installed bundle digest or OCI-backed transport digest context
- bundled closure scope, including which dependency aliases or bundled installed records arrive together
- publisher identity, signature presence, verification result, and local trust tier in the target root
- transport shape and source reference actually reviewed by the operator: bundle path, OCI-layout path, or OCI registry reference
- runtime-entrypoint and declared capability-surface summary already carried by current manifest/runtime truth and surfaced as compatibility review, not as a new manifest layer
- resulting installed-state classification such as `verified-import`, `trusted-imported`, or `restricted`

That metadata should be presented as a host-owned install report layered on top
of current `preview`, `verify`, and manifest/runtime checks. It should not
become a new `pack` schema, a parallel metadata file, or a second source of
truth beside Rust types, manifests, WIT, and the current host-owned verification
records.

### Docs-Only Versus Code Follow-On

For this phase, keep the split below explicit:

| Bucket | Follow-on work |
| --- | --- |
| Docs-only now | tighten the operator wording around bundle, OCI layout, OCI registry, local trust review, and preview; explain that curated-pack install is a presentation layer over current transport units; keep mirroring and promotion guidance aligned with the current commands |
| Code follow-on later | improve install/preview UX so the current compatibility metadata is easier to review in one place; make closure scope and compatibility summaries more legible in existing `preview` or `verify` surfaces; add bounded curated-install presentation only if it reuses current bundle/import semantics instead of inventing a new packaging model |
| Explicitly deferred | registry-to-registry mirror, retag-only promotion, remote trust synchronization, automatic environment promotion workflows, marketplace-style discovery, and any new pack type or pack manifest |

### Anti-Goals For Packaging Language

- Do not introduce a `guild pack` command family, pack manifest, or other second
  package contract surface.
- Do not describe `export` or `push` as silent copy, mirror, or retag
  operations; they are fresh publication events over installed state.
- Do not describe curated installs as bypassing local trust review, preview, or
  verification in the target root.
- Do not collapse bundle, OCI layout, and OCI registry into vague generic
  registry rhetoric; the transport shape still matters operationally.
- Do not let packaging language drift into marketplace, hosted-control-plane, or
  broad distribution promises the current repo does not ship.

### Suggested Subtasks

- [x] Write the current-state packaging map.
- [x] Identify the minimum compatibility metadata needed for curated-pack installs.
- [x] Clarify what packaging work is docs-only versus code-follow-on.
- [x] Add anti-goals that rule out marketplace language.

### Validation

- `git diff --check`
- `cargo run -q -p xtask -- project-positioning check`
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

### Chosen Next Progression

The next believable progression after `#130` is a docs-first
`service-recovery review pack`.

Keep that choice bounded like this:

- It stays on current read-only runtime truth: one subject execution, one
  comparison execution, one bounded failures query, and optional evidence
  review rendered through the existing casefile and drill-down skills.
- It keeps the current hero story visible in honest terms: diagnose and verify
  are real now, while restart and notify remain future action steps rather than
  starter-pack claims.
- It is closer to current shipped surfaces than a rollback or cache-mutation
  story, so it is the sensible docs-first progression immediately after Guild
  Ops Starter.

Keep one secondary docs-first concept visible too:

- `rollback verification pack`: use the same read-only explain, compare, and
  evidence surfaces to frame the review half of rollback work, while rollback
  and incident annotation remain deferred actions.

Keep the first plausible next implementation candidate explicit:

- `cache purge with evidence trail` remains the leading mutation-demo candidate
  once later work (`#132`) chooses the first honest apply-oriented slice.

### Suggested Subtasks

- [x] Build the candidate matrix with support-level classification.
- [x] Choose one or two docs-first starter-pack/report concepts to keep visible.
- [x] Name the first plausible next implementation candidate, if one exists.
- [x] Record which concepts remain deferred until apply-mode/runtime support grows.

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
