# Project Positioning

This document is the current explanatory framing for Guild. It freezes project
thesis, vocabulary, operator-facing capability language, and anti-thesis
guardrails for repo docs and planning.

It is not a runtime-contract source. For normative runtime ownership, use
[`../SPECS.md`](../SPECS.md), [`../wit/guild-skill-v1.wit`](../wit/guild-skill-v1.wit),
and the core Rust runtime/types. For the bounded draft proof/control-plane
harness, use [`schemas/draft-v1/README.md`](schemas/draft-v1/README.md).

## One-Line Definition

Guild lets ops, platform, and security teams run trusted playbooks that package
steps, permissions, approvals, and evidence.

## Project Thesis

Guild is trusted operational automation for engineering teams.

## Product Thesis

The playbook is the application. The trust chain is the product.

Guild should read as the system that lets operators review and admit an ops
playbook under explicit capability policy, run it in isolation, and keep
receipts and evidence they can inspect later and use for replay-oriented
explanation.

Today, the repo still exposes that model through skills, durable Guild refs,
local policy decisions, and bounded runtime surfaces. The playbook surface is a
product direction, not a claim that a broad playbook engine already ships.
The current explanation path is replay-oriented explanation over stored refs,
not a first-class replay engine.

The external capability taxonomy is also docs-first in this phase. It is the
operator-facing approval vocabulary, while the current internal family
identifiers and typed constraints remain the implementation truth until a later
phase changes that explicitly.

That does not imply every slice is proof-backed today. `bounded`,
`proof-backed`, `upper-bound`, `linked`, `unlinked`, and `not_proven` remain
explicit where the live runtime and checked draft-v1 surfaces require them.

## First Operator Starter Set Thesis

Guild Ops Starter is the first operator starter set in the repo. It is a
repo-local release slice built on that trust chain. It uses receipts and
evidence to summarize incidents, compare runs, explain evidence, and generate
bounded operational reports without pretending it is the whole product.

## Primary Users

- platform engineers
- SREs and incident commanders
- DevOps engineers
- security engineers
- staff-plus engineers standardizing risky operational workflows
- internal developer platform teams packaging operational knowledge

## Jobs To Be Done

1. Turn a tribal runbook into a reusable playbook.
2. Let an AI system execute useful work without getting vague or reckless.
3. Require approval before risky mutations.
4. Produce evidence that can be reviewed after the fact.
5. Package the workflow so it can move across compatible hosts.

## Product Promises

Guild should keep these promises together rather than trading one for another:

1. Legibility: operators can tell what a workflow can do and what it cannot do.
2. Portability: packaged skills and playbooks move across compatible hosts.
3. Control: risky mutations can be gated by approval and policy.
4. Evidence: serious runs produce inspectable receipts and evidence.
5. Verification: curated assets can be labeled from explicit proof and trust signals.

## Anti-Thesis

Guild is not primarily:

- a generic agent orchestration platform
- a generic workflow engine
- a broad MCP wrapper
- a marketplace story
- a product that hides its trust and runtime boundaries behind softer language

## Canonical Operator Vocabulary

Use these terms in README copy, examples, issue bodies, CLI docs, and roadmap
planning unless precision demands lower-level contract language.

| Term | Definition | Use when | Avoid when |
| --- | --- | --- | --- |
| **Capability** | A permissionable action alias such as `runs:inspect` or `cache:purge`. | Explaining what an operator-facing workflow is allowed to do. | Referring to a whole workflow or product tier. |
| **Skill** | A reusable procedural unit that Guild installs and executes today. | Explaining the current portable execution building block. | Referring to the end-user outcome or whole operator story. |
| **Playbook** | The operator-facing workflow or review surface built on one or more skills. | Explaining the target user-facing automation unit. | Claiming Guild already ships a broad playbook engine. |
| **Approval** | A policy or human gate that must pass before risky mutation. | Talking about control and governance. | Referring to generic auth or identity. |
| **Evidence** | The facts collected during a run: inputs, observations, checks, and outputs. | Talking about what was gathered. | Referring to the whole run record. |
| **Receipt** | The structured record of intent, approvals, actions, evidence, and outcome for a run. | Talking about run history, auditability, or replay-oriented explanation. | Talking about a single evidence item. |
| **Verify** | Evaluate whether a skill or future curated asset is installable, compatible, and backed by explicit trust signals. | Talking about curation and trust. | Talking about runtime health checks only. |
| **Experimental** | Visible on purpose, but still relying on docs-first or only partially proven current surfaces. | Labeling an honest early slice without overstating trust. | Treating it as already curated or verified. |
| **Replay-oriented explanation** | Re-check or explain prior work from stored refs and receipts. | Talking about what Guild can do today from durable state. | Claiming first-class replay execution already exists. |
| **Curated** | Reviewed against the current signal inventory and support frontier. | Labeling the first honest trust tier. | Claiming deep compatibility, mutation safety, or eval coverage that does not exist. |
| **Verified** | Passed the current verification matrix for every supported surface it claims. | Labeling the strongest current trust state. | Any asset lacking proof-backed or trust-backed criteria for its current claims. |

## Terms To Avoid As Primary Framing

These terms may be accurate in narrow context, but they are not the lead story
for Guild and should not be the headline framing.

| Term | Why it hurts | Prefer instead |
| --- | --- | --- |
| **Agent operating system** | Too broad and infrastructure-heavy. | trusted playbook layer |
| **Workflow engine** | Sounds generic and hides the trust chain. | playbook layer or bounded operator workflow |
| **Orchestration layer** | Leads with mechanism instead of operator outcome. | playbook layer or execution layer |
| **Artifact** | Too abstract as a headline noun. | skill, playbook, receipt, or evidence |
| **Autonomous remediation** | Over-claims trust and control. | policy-gated remediation |
| **Self-healing** | Sounds magical unless narrowly proven. | verified remediation playbook |
| **Marketplace** | Premature breadth-over-trust framing. | curated catalog or curated distribution |
| **Memory platform** | Competes in a different category. | not a Guild headline concept |
| **Multi-agent system** | Exposes internals users did not ask for. | playbooks, approvals, receipts |

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

## Operator-Facing Capability Vocabulary

Keep the external capability model coarse and human-readable. The operator
should be able to infer the blast radius from the name without seeing the
adapter implementation details.

Capability grammar:

```text
<domain>:<verb>
```

Examples:

- `runs:inspect`
- `runs:compare`
- `failures:query`
- `evidence:inspect`
- `metrics:query`
- `logs:query`
- `k8s:restart`
- `cache:purge`
- `chat:post`

Rules:

1. The domain must be user-recognizable.
2. The verb should come from a short approved verb set.
3. Do not encode the concrete tool or vendor in the capability name.
4. Do not encode the environment in the capability name.
5. Keep names stable even if adapters change underneath.

Approved verb set in this phase:

- observe: `read`, `query`, `list`, `describe`, `inspect`, `compare`
- coordination: `post`, `create`, `annotate`
- mutation: `restart`, `scale`, `rollback`, `update`, `rotate`, `purge`, `cordon`, `drain`, `dispatch`

Current operator-facing examples that map cleanly to the live repo truth:

- `runs:inspect`
- `runs:compare`
- `failures:query`
- `evidence:inspect`

Docs-first target names that are useful for planning but not yet broad runnable
truth:

- `metrics:query`
- `logs:query`
- `k8s:restart`
- `deploy:rollback`
- `cache:purge`
- `chat:post`

Suggested risk classes for policy defaults:

- `observe`: read-only review
- `assist`: annotations, notifications, or preparation without mutation
- `mutate`: production-adjacent state changes
- `critical`: identity, secrets, routing, or other high-blast-radius changes

Operator-facing names are not the same thing as the current internal family
IDs used by manifests, help text, or `guild grants template`.
The taxonomy is docs and approval vocabulary in this phase, not a command
rename or a runtime support claim.

## Messaging Hierarchy

When describing Guild, keep this order:

1. Outcome: trusted playbooks for ops and security automation.
2. Mechanism: portable skills, capability review, host-owned policy, and isolation.
3. Proof: receipts, evidence, and replay-oriented explanation from stored refs.

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
