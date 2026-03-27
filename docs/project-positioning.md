# Project Positioning

This document is the current explanatory framing for Guild. It freezes project
thesis, product wording, and anti-thesis language for repo docs and planning.

For the canonical operator-facing vocabulary and discouraged-term list used by
this repositioning work, see
[`strategy/guild-repositioning/02-glossary-and-banned-terms.md`](strategy/guild-repositioning/02-glossary-and-banned-terms.md).
That glossary is the user-facing language source for wording work; this
document remains the framing and guardrail source.

It is not a runtime-contract source. For normative runtime ownership, use
[`../SPECS.md`](../SPECS.md), [`../wit/guild-skill-v1.wit`](../wit/guild-skill-v1.wit),
and the core Rust runtime/types. For the bounded draft proof/control-plane
harness, use [`schemas/draft-v1/README.md`](schemas/draft-v1/README.md).

## Project Thesis

Guild is trusted operational automation for engineering teams.

## Product Thesis

The playbook is the application. The trust chain is the product.

Guild should read as the system that lets operators review and admit an ops
playbook under explicit capability policy, run it in isolation, and keep
receipts and evidence they can inspect and replay later.

Today, the repo still exposes that model through skills, durable Guild refs,
local policy decisions, and bounded runtime surfaces. The playbook surface is a
product direction, not a claim that a broad playbook engine already ships.

That does not imply every slice is proof-backed today. `bounded`,
`proof-backed`, `upper-bound`, `linked`, `unlinked`, and `not_proven` remain
explicit where the live runtime and checked draft-v1 surfaces require them.

## First Operator Starter Set Thesis

Guild Ops Starter is the first operator starter set in the repo. It is a
repo-local release slice built on that trust chain. It uses receipts and
evidence to summarize incidents, compare runs, explain evidence, and generate
bounded operational reports without pretending it is the whole product.

## Anti-Thesis

Guild is not primarily:

- a generic agent orchestration platform
- a generic workflow engine
- a broad MCP wrapper
- a marketplace story
- a product that hides its trust and runtime boundaries behind softer language

## Preferred Core Terms

- trusted operational automation
- ops playbook
- capability
- capability policy
- admission
- isolation
- execution receipt
- evidence
- replay
- inspectability
- reference application, only as a secondary repo-organization or release-slice term after operator-facing framing
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

- artifact
- trust layer
- platform
- substrate
- multi-step runtime
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

- playbooks are the target operator surface, but the repo still runs skills directly today
- bounded proof is bounded, not general
- unsupported slices stay unsupported
- mechanism-layer terms remain where the contract needs them
- repo-local release slices should be built on proven surfaces

## Sane Expectations

- users should get value quickly from real refs and resources
- docs should explain the first useful workflow, not the whole ontology
- workflows should be understandable in under five minutes

## Sane Implementations

- do not invent new abstractions without pressure
- do not create multiple truth surfaces
- do not layer large workflow engines on top of narrow proven slices
- prefer explicit receipts and evidence over chat-only state

## Boundary

This doc is explanatory and strategic. It does not change runtime semantics,
support claims, or draft-v1 semantics by prose alone.

Next planning anchor:

- [`roadmap/epics/portable-skill-receipts-and-reference-apps.md`](roadmap/epics/portable-skill-receipts-and-reference-apps.md)
