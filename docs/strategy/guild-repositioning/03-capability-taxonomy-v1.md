# Capability Taxonomy V1

This document is the canonical operator-facing capability vocabulary for Guild's
repositioning work.

Use it when docs, examples, playbooks, or approval flows need capability names
that read like operator intent rather than runtime mechanics. For the broader
wording rules, use
[`02-glossary-and-banned-terms.md`](02-glossary-and-banned-terms.md). For the
repo framing and guardrails, use
[`../../project-positioning.md`](../../project-positioning.md).

## Summary

Guild's first external capability taxonomy is a user-facing naming layer for
operators.

It is not a runtime-contract rename. The current internal family identifiers and
typed constraints remain the implementation truth until a later implementation
phase explicitly changes them.

That means v1 does three things only:

- gives operators capability names that read like approvals
- gives docs and examples one consistent capability vocabulary
- keeps the active runtime frontier and the broader manifest contract explicit instead of hidden

It does not claim that every external capability listed here is runnable today.
Where the current runtime does not have an honest implementation frontier yet,
the name remains an operator-facing target vocabulary only.

## Design Principles

- Capability names should read like approvals an operator can understand.
- Names should describe intent, not transport or implementation detail.
- The capability surface should be smaller than the possible implementation detail behind it.
- Scope belongs in policy, admission, or parameters, not in the capability name itself.
- External names can map to current internal families, future families, or multiple underlying checks.

## Naming Rules

- Format: `domain:action`
- Use lowercase kebab-case.
- Use verbs operators already use in runbooks.
- Prefer the smallest action that still has clear operational meaning.
- Do not include environment, namespace, cluster, or target identifiers in the name.
- Do not encode transport details such as HTTP, URI prefixes, or child aliases into the external name.

## Current Runtime Frontier

Guild's current CLI and grant-authoring surfaces still expose these internal
families as the active runtime frontier in the current inspect slice:

- `http-request`
- `read-resource`
- `invoke-skill`
- `emit-evidence`
- `log-write`

Those names remain canonical in the current runtime and in `guild grants
template` output. The external capability taxonomy does not replace them in v1.
It gives operators and docs a clearer approval vocabulary that can be mapped
back to those internal families when precision is needed.

Broader manifest-level `CapabilityId` contract truth still exists beyond that
active runtime frontier. The shared type layer currently defines additional
capability IDs such as `get-secret`, `cache-read`, `cache-write`,
`filesystem`, `monotonic-clock`, and `wall-clock`.

Those IDs are real manifest- and contract-level vocabulary, but they are not
all executable in the current inspect runtime. In particular, `filesystem` is
already a typed host-side contract family with explicit guardrails in
[`../../adr/0018-filesystem-policy-contract-not-yet-implemented.md`](../../adr/0018-filesystem-policy-contract-not-yet-implemented.md),
while the active inspect slice still rejects it before guest start.

## Capability Families

| External Capability | Meaning | Current Status | Current Internal-Family Expression |
| --- | --- | --- | --- |
| `k8s:restart` | Restart a workload or pod set | docs-first target only | no direct first-class runtime family today; any future implementation should stay host-mediated and policy-scoped |
| `k8s:scale` | Change replica count | docs-first target only | no direct first-class runtime family today |
| `k8s:cordon` | Mark a node unschedulable | docs-first target only | no direct first-class runtime family today |
| `k8s:drain` | Evict work from a node | docs-first target only | no direct first-class runtime family today |
| `metrics:query` | Query health or performance metrics | expressible today in bounded cases | usually a narrow `http-request` grant in the current runtime; not a dedicated metrics family |
| `logs:query` | Read logs or structured operational events | partially expressible today | `read-resource` for Guild-owned refs or a narrow `http-request` grant for backend-specific log APIs; not a generic live logs family |
| `chat:post` | Post to an operator chat destination | expressible today in bounded cases | usually a narrow `http-request` grant; not a dedicated chat family |
| `incident:create` | Create or annotate an incident record | expressible today in bounded cases | usually a narrow `http-request` grant; not a dedicated incident family |
| `deploy:rollback` | Roll back a deployment | docs-first target only | no direct first-class runtime family today |
| `secrets:rotate` | Rotate a secret and record evidence | docs-first target only | no direct first-class runtime family today |
| `cache:purge` | Purge or invalidate cache content | docs-first target only | no direct first-class runtime family today |

Supporting internal families such as `emit-evidence` and `log-write` remain
important to the current implementation, but they are not the right top-level
operator approval vocabulary for this v1 taxonomy.

## Scoping Guidance

- Put target scope in playbook inputs or policy:
  - cluster
  - namespace
  - deployment name
  - service name
  - environment
- Put safety boundaries in admission:
  - environment allowlist
  - time window
  - actor approval
  - evidence requirements
- Put transport and protocol details in the implementation layer:
  - URI prefixes
  - hosts
  - aliases
  - timeout and byte limits

This is the key distinction for v1:

- name scope answers "what kind of operator action is this?"
- parameter and policy scope answer "where, how far, and under which limits may it run?"

## Examples

- Good:
  - `k8s:restart`
  - `metrics:query`
  - `chat:post`
- Too low level:
  - `http-request`
  - `invoke-skill`
  - `read-resource`
- Too specific:
  - `k8s:restart-prod-api-deployment`
  - `chat:post-slack-webhook`

## Migration Implications

- Current internal `CapabilityId` values remain canonical in `crates/guild-types`, `crates/guild-runner`, manifests, and CLI plumbing for now.
- External names become the preferred docs, playbook, and approval vocabulary first.
- A later CLI phase may accept external names in templates and playbook tooling, but that is not part of the first wave.
- A later runtime/manifest phase may introduce first-class mappings if the implementation work justifies it.

## Policy Presentation Guidance

- Show the external capability name first in operator-facing approval text.
- Keep environment, namespace, host, alias, timeout, and byte limits out of the name itself.
- When current implementation detail matters, present it second, for example:
  - `metrics:query` expressed today through bounded `http-request`
  - `logs:query` expressed today through bounded `read-resource` or `http-request`, depending on backend
- If the current runtime does not support a direct expression yet, say so explicitly:
  - `k8s:restart` is a docs-first target in v1, not a currently shipped first-class runtime family
- Preserve fail-closed wording. If the repo cannot explain a capability honestly with current surfaces, mark it deferred instead of pretending the implementation already exists.

## Canonical Usage

Use this document as the capability-naming source when Guild needs
operator-facing examples such as:

- playbook capability lists
- approval and admission language
- docs that explain what a workflow is allowed to do
- example workflows and reference playbooks

Do not use this document to imply:

- that manifest capability IDs have already been renamed
- that the live runtime supports every external capability listed here
- that transport- or family-level implementation detail no longer matters for maintainers

## Migration Recommendations

- In docs and examples, lead with external capability names and add current internal-family notes only where they improve honesty or reviewability.
- In current CLI help, manifests, and `guild grants template`, keep using the live internal family names until a later CLI/runtime phase changes that explicitly.
- If future CLI output starts rendering external names, show the current internal-family expression alongside them during the transition rather than hiding the mapping.
- Do not rename Rust, WIT, manifest, or policy identifiers in the first wave.

## What V1 Does Not Do

- It does not rename current manifest capability identifiers.
- It does not claim that every named external capability is runnable today.
- It does not collapse all capability checks into one generic permission blob.
- It does not remove the need for typed internal constraints or family-aware policy.

## Recommended Rollout

1. Use these names in strategy docs, example playbooks, approval flows, and roadmap language.
2. Add mapping notes to docs and examples so operators can relate external names to current Guild mechanics.
3. Introduce CLI and policy affordances only after the vocabulary is stable and the playbook surface is defined.
