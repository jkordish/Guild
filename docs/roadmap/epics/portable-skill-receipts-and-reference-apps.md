# Portable Skill Receipts And Reference Apps

This epic is a planning document. It is not a runtime-contract source. Current
repo framing lives in [`../../project-positioning.md`](../../project-positioning.md).
Normative runtime ownership remains in [`../../../SPECS.md`](../../../SPECS.md),
[`../../../wit/guild-skill-v1.wit`](../../../wit/guild-skill-v1.wit), and the
core Rust runtime/types.

Execution-ready guidance for the open follow-on GitHub issue set lives in
[`portable-skill-receipts-and-reference-apps-execution-guide.md`](portable-skill-receipts-and-reference-apps-execution-guide.md).
That execution guide now also absorbs the imported repositioning milestone,
epic, and task mapping so the repo does not need a separate strategy stack for
tracker intake.

## Summary

The next phase should turn Guild's existing trust and receipt layer into an
operator-facing playbook and starter-set story without widening the repo back
into a generic runtime or workflow-engine story.

## Problem Statement

Guild already has real runtime, trust, storage, and observation machinery, but
the repo is still easy to misread as a multi-step runtime story or an ops
playbook engine. That misframing pulls build priorities toward runtime plumbing
instead of toward operator workflows, capability review, receipts, evidence,
and the first starter sets that consume them.

## Project Thesis

Guild is trusted operational automation for engineering teams.

## Product Thesis

The playbook is the application. The trust chain is the product.

## First Operator Starter Set Thesis

Guild Ops Starter is the first operator starter set in the repo. It is a
repo-local release slice built on that trust chain. It uses receipts to
summarize incidents, compare runs, explain evidence, and generate bounded
operational reports.

## In Scope

- make operator-facing playbooks and starter sets the default story while keeping the current skill and receipt model explicit
- build starter-set and repo-local release-slice views on the current installed-state, execution, evidence, and bounded-query surfaces
- keep trust review, execution explanation, and evidence explanation tied to exact refs and explicit status labels
- improve starter-set ergonomics only on already proven or already shipped surfaces
- keep wording and planning aligned with the project-positioning doc

## Out Of Scope

- new capability families
- runtime rewrite
- CLI redesign
- trust or OCI full freeze
- proof, token, or witness expansion as the milestone thesis
- packaging-system redesign
- marketplace or hosted control-plane positioning

## Core Objects

- skill artifact: installed executable state with digest-pinned identity and signed transport forms
- admission receipt: the bounded draft-v1 admission result for one requested invocation when that draft control-plane slice is used; not a new live runtime object in this epic
- execution receipt: the host-owned `guild://executions/...` reference for one persisted run
- evidence artifact / chain: evidence record, metadata resource, blob linkage, and producing-execution linkage
- playbook / casefile / report / repo-local release-slice view: a bounded read-only summary built from those durable refs and bounded query resources

## Current Proven Frontier

### Real Now

- local install, resolve, execute, persist, and explain flows in Rust
- signed installed-bundle export, import, push, pull, and trust verification
- durable execution and evidence resources under `guild://...`
- bounded execution-query resources and the thin MCP surface
- Guild Ops Starter and related example skills over stored refs

### Bounded Now

- live proof is bounded to the current `read-resource`, eight `http-request`, two `invoke-skill`, and one exact `emit-evidence` checked slices, plus proof-only `log-write`
- draft-v1 admission, token, and witness semantics remain bounded and separate from the live runtime contract
- starter-set composition remains inside exact single-child zero-authority formatter paths where already proven

### Not Proven

- broader `http-request` shapes
- broader `invoke-skill` shapes
- broader `emit-evidence` flows beyond the exact single-emission fixed local-object-store slice
- runtime-general proof-backed minimization
- any claim that repo-local release slices are a generic workflow engine

## Expected Next Deliverables

- operator-first onboarding and verification paths that still start from installed skill refs and trust review
- one honest verification matrix that keeps `experimental`, `curated`, and `verified` tied to current trust signals instead of future scoring ideas
- receipt-first navigation and explanation across execution, evidence, and bounded query resources
- bounded playbook, casefile, and report views built only on current durable refs and proven read paths
- tighter starter-set docs and examples that stay inside the current support frontier
- drift guards that keep project framing, anti-thesis wording, and starter-set positioning stable

## Current Follow-On Progression

The current bounded progression after Guild Ops Starter is:

- docs-first next progression: `service-recovery review pack`
- secondary docs-first concept kept visible: `rollback verification pack`
- first plausible later implementation candidate: `cache purge with evidence trail`
- deferred until broader apply/runtime support: certificate renewal, node remediation, and secret rotation packs

That progression stays honest because the review half already fits today's
durable execution refs, bounded failure queries, evidence inspection, and
casefile/report surfaces, while the action-heavy steps stay explicitly future.

## Success Criteria

- top-level docs and examples lead with operator workflows, capability review, receipts, evidence, and starter sets instead of runtime or substrate-first language
- Guild Ops Starter clearly reads as the first operator starter set and a repo-local release slice, not the whole product thesis
- new follow-on work is framed in terms of playbooks on top of portable skills, receipt chains, and durable refs
- no runtime or draft-v1 support claim is broadened by wording alone
- repo-native validation catches framing drift in the selected top-level docs

## Initial GitHub Issue Map

The first follow-on GitHub tracking set created from this planning note is:

- `#129` EPIC: Portable skill receipts and reference playbooks follow-on program
- `#130` Make Guild Ops Starter a cohesive incident-casefile-first starter path
- `#131` Document authoring-layer guardrails without creating a second contract surface
- `#132` Choose the first honest post-starter mutation demo
- `#133` Define a verification matrix and curated-pack labeling story on current trust signals
- `#134` Scope private-pack, policy, and governance phase boundaries
- `#136` Document packaging and install-surface follow-on work on current shipped transport flows
- `#137` Define bounded starter-pack and reference-playbook progression beyond Guild Ops Starter
- `#138` Scope receipt-chain and replay-oriented explanation follow-on work

Issue `#130` is the first concrete delivery slice because it stays completely
inside the current proven frontier: installed portable skills, durable
execution and evidence refs, bounded execution-query resources, trust review,
and docs/tests that keep the starter story honest.

Issue `#136` is the next follow-on slice because it still stays on already
shipped transport and trust behavior: native signed bundle export/import, OCI
layout export/import, OCI registry push/pull, local preview before risky
admission, and target-root trust review with `guild verify -v`. The point of
that slice is to make curated-pack installation and compatibility review more
legible on top of those existing flows, not to invent a new pack type,
marketplace surface, or broader distribution contract.

## Anti-Goals

- do not broaden support
- do not hide fail-closed or `not_proven` boundaries
- do not add a workflow engine
- do not invent a new pack type
- do not create a second normative contract surface
- do not turn this epic into branding copy

## Risks / Drift Vectors

- README or architecture language drifting back to runtime-first or substrate-first framing
- the ops starter being described as the whole product
- new helper docs inventing competing glossary or thesis text
- draft-v1 admission artifacts being mistaken for live runtime guarantees
- future examples widening the story faster than the measured support frontier
