# Project Positioning

This document is the current explanatory framing for Guild. It freezes project
thesis, product wording, and anti-thesis language for repo docs and planning.

It is not a runtime-contract source. For normative runtime ownership, use
[`../SPECS.md`](../SPECS.md), [`../wit/guild-skill-v1.wit`](../wit/guild-skill-v1.wit),
and the core Rust runtime/types. For the bounded draft proof/control-plane
harness, use [`schemas/draft-v1/README.md`](schemas/draft-v1/README.md).

## Project Thesis

Guild creates portable, capability-bounded skill artifacts and a trust layer
for how they are admitted, executed, and evidenced.

## Product Thesis

Guild turns a skill run into a verifiable receipt chain tied to exact bundle
identity, granted authority, observed effects, and durable artifacts.

That does not imply every slice is proof-backed today. `bounded`,
`proof-backed`, `upper-bound`, `linked`, `unlinked`, and `not_proven` remain
explicit where the live runtime and checked draft-v1 surfaces require them.

## First Reference Application Thesis

Guild Ops Starter is the first reference application built on that trust
layer. It uses receipts to summarize incidents, compare runs, explain
evidence, and generate bounded operational reports.

## Anti-Thesis

Guild is not primarily:

- a generic agent orchestration platform
- a workflow engine
- a broad MCP wrapper
- a broad ops playbook runtime
- a marketplace story

## Preferred Core Terms

- portable skill artifact
- capability-bounded
- capability envelope
- admission receipt
- execution receipt
- evidence artifact
- evidence chain
- reference application
- fail-closed
- bounded proof
- proof-backed
- upper-bound
- linked
- unlinked
- `not_proven`

## Terms To Avoid As Primary Framing

These terms can be accurate in narrow context, but they are not the lead story
for Guild:

- platform
- substrate
- multi-step runtime
- ops playbook engine
- generic orchestration
- secure by design
- agentic framework

## Sane Defaults

- local-first
- fail-closed
- compact default UX
- trust explicit, not implied
- outputs cite exact refs and status where practical

## Sane Assumptions

- bounded proof is bounded, not general
- unsupported slices stay unsupported
- portable does not mean magical universal runtime compatibility
- reference applications should be built on proven surfaces

## Sane Expectations

- users should get value quickly from real refs and resources
- docs should explain the first useful workflow, not the whole ontology
- workflows should be understandable in under five minutes

## Sane Implementations

- do not invent new abstractions without pressure
- do not create multiple truth surfaces
- do not layer large workflow engines on top of narrow proven slices
- prefer explicit receipts and artifacts over chat-only state

## Boundary

This doc is explanatory and strategic. It does not change runtime semantics,
support claims, or draft-v1 semantics by prose alone.

Next planning anchor:

- [`roadmap/epics/portable-skill-receipts-and-reference-apps.md`](roadmap/epics/portable-skill-receipts-and-reference-apps.md)
