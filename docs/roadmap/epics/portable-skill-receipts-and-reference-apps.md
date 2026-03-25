# Portable Skill Receipts And Reference Apps

This epic is a planning document. It is not a runtime-contract source. Current
repo framing lives in [`../../project-positioning.md`](../../project-positioning.md).
Normative runtime ownership remains in [`../../../SPECS.md`](../../../SPECS.md),
[`../../../wit/guild-skill-v1.wit`](../../../wit/guild-skill-v1.wit), and the
core Rust runtime/types.

## Summary

The next phase should build on Guild's existing trust and receipt layer rather
than widening the repo back into a generic runtime or playbook story.

## Problem Statement

Guild already has real runtime, trust, storage, and observation machinery, but
the repo is still easy to misread as a multi-step runtime story or an ops
playbook engine. That misframing pulls build priorities toward substrate work
instead of toward portable artifacts, receipts, evidence, and the first
reference applications that consume them.

## Project Thesis

Guild creates portable, capability-bounded skill artifacts and a trust layer
for how they are admitted, executed, and evidenced.

## Product Thesis

Guild turns a skill run into a verifiable receipt chain tied to exact bundle
identity, granted authority, observed effects, and durable artifacts.

## First Reference Application Thesis

Guild Ops Starter is the first reference application built on that trust
layer. It uses receipts to summarize incidents, compare runs, explain
evidence, and generate bounded operational reports.

## In Scope

- make portable skill artifacts and receipts the default build and onboarding story
- build reference-application views on the current installed-state, execution, evidence, and bounded-query surfaces
- keep trust review, execution explanation, and evidence explanation tied to exact refs and explicit status labels
- improve reference-application ergonomics only on already proven or already shipped surfaces
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
- casefile / report / reference-application view: a bounded read-only summary built from those durable refs and bounded query resources

## Current Proven Frontier

### Real Now

- local install, resolve, execute, persist, and explain flows in Rust
- signed installed-bundle export, import, push, pull, and trust verification
- durable execution and evidence resources under `guild://...`
- bounded execution-query resources and the thin MCP surface
- Guild Ops Starter and related example skills over stored refs

### Bounded Now

- live proof is bounded to the current `read-resource`, six `http-request`, and one `invoke-skill` checked slices, plus proof-only `log-write`
- draft-v1 admission, token, and witness semantics remain bounded and separate from the live runtime contract
- reference-application composition remains inside exact single-child zero-authority formatter paths where already proven

### Not Proven

- broader `http-request` shapes
- broader `invoke-skill` shapes
- proof-backed `emit-evidence` linkage
- runtime-general proof-backed minimization
- any claim that reference applications are a generic workflow engine

## Expected Next Deliverables

- artifact-first onboarding and verification paths that start from installed skill refs and trust review
- receipt-first navigation and explanation across execution, evidence, and bounded query resources
- bounded casefile and report views built only on current durable refs and proven read paths
- tighter reference-application docs and examples that stay inside the current support frontier
- drift guards that keep project framing, anti-thesis wording, and first-reference-application positioning stable

## Success Criteria

- top-level docs and examples lead with artifacts, receipts, evidence, and reference applications instead of runtime or substrate-first language
- Guild Ops Starter clearly reads as the first reference application, not the whole product thesis
- new follow-on work is framed in terms of portable artifacts, receipt chains, and durable refs
- no runtime or draft-v1 support claim is broadened by wording alone
- repo-native validation catches framing drift in the selected top-level docs

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
