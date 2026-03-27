# Playbook Surface V1

This is the bounded v1 public playbook surface for Guild. It defines the
operator-facing concept, the minimum schema shape, and the current repo
boundary. Unless implementation catches up, treat it as a planning and UX
target rather than a shipped runtime contract.

## Definition

A playbook is the operator-facing automation unit in Guild.

A playbook expresses an operational procedure in a reviewable form:

- what the automation is trying to achieve
- what inputs it needs
- what capabilities it may use
- which skills execute each step
- what evidence and receipts must exist afterward

## Relationship Between Playbooks And Skills

- A playbook is the application.
- A portable skill is the reusable execution unit.
- A playbook may call one or more skills.
- Skills remain the runtime-resolved executable identity.
- Playbooks add operator structure, reviewability, and capability intent on top of existing skill execution.

## Playbook-To-Skill Composition

Today, Guild still runs skills directly rather than a first-class playbook
runtime. The current composition story is therefore:

- a playbook step points at a reusable skill through `uses`
- the referenced skill still resolves to immutable executable identity before it
  runs
- the host still evaluates caller-requested authority and computes the final
  granted capability slice for the execution
- receipts and evidence still come from the underlying skill executions that
  actually ran
- a future playbook layer should add operator structure on top of that trust
  chain rather than replacing it

## Minimum Viable Schema Shape

This is a planning target for v1, not a shipped manifest contract.

| Field | Purpose |
| --- | --- |
| `apiVersion` | Version the playbook document shape |
| `kind` | Set to `Playbook` |
| `metadata.name` | Human-readable playbook name |
| `intent` | Short operator summary of what the playbook does |
| `inputs` | Required and optional operator inputs |
| `capabilities` | Operator-readable requested capabilities |
| `steps` | Ordered execution steps |
| `admission` | Pre-execution policy and approval expectations |
| `evidence` | Required receipts, evidence, and annotations |
| `success` | Conditions that define completion |

## Example YAML

```yaml
apiVersion: guild.dev/v1alpha1
kind: Playbook
metadata:
  name: diagnose-restart-notify
intent: Diagnose a service issue, restart pods, and notify on-call
inputs:
  service:
    type: string
  namespace:
    type: string
  oncall_channel:
    type: string
capabilities:
  - id: metrics:query
  - id: logs:query
  - id: k8s:restart
  - id: chat:post
steps:
  - id: diagnose
    uses: skill://ops/diagnose-service@^0.1
    with:
      service: ${inputs.service}
      namespace: ${inputs.namespace}
  - id: restart
    if: ${steps.diagnose.output.recommend_restart == true}
    uses: skill://ops/restart-workload@^0.1
    with:
      service: ${inputs.service}
      namespace: ${inputs.namespace}
  - id: notify
    uses: skill://ops/post-oncall-update@^0.1
    with:
      channel: ${inputs.oncall_channel}
      service: ${inputs.service}
      action: restarted
admission:
  mode: explicit
  require:
    - capability_review
    - isolated_execution
evidence:
  require_receipt: true
  require_step_evidence:
    - diagnose
    - restart
    - notify
success:
  requires:
    - ${steps.notify.status == "succeeded"}
```

## Current Repo Boundary

This document is a product and UX surface, not a shipped runtime contract.

Today the repository still:

- installs and executes skills directly
- narrows requested authority through host-owned policy before guest start
- persists durable execution and evidence records at the skill execution layer
- keeps `inspect`, `plan`, and `apply` as distinct execution-mode boundaries

The example YAML above is illustrative. Its playbook shape and operator-facing
capability names are meant to guide docs and later UX work, not to claim that
Guild already ships a broad playbook engine or that every example capability is
executable today.

## Execution And Evidence Model

If Guild grows a first-class playbook execution path later, it should reuse the
current trust and evidence backbone rather than inventing a parallel one.

- Playbook admission should show the requested operator-readable capabilities before execution starts.
- Execution should still resolve each underlying skill to immutable executable identity.
- The receipt chain should show:
  - playbook request
  - admitted capabilities
  - executed steps
  - produced evidence
  - final outcome
- Replay should use stored receipt context to re-run or re-check a playbook in a bounded way.

## What V1 Should Reuse From The Current Repo

- Existing `inspect / plan / apply` execution mode distinctions
- Existing durable execution and evidence records
- Existing skill resolution and receipt model
- Existing capability enforcement model
- Existing `SkillCategory::Playbook` seam for future classification

## Open Questions

- Should a playbook live as a standalone YAML file, a manifest-adjacent document, or a higher-level wrapper that still resolves to current manifests?
- Does v1 support multi-step branching, or only linear step lists plus simple guards?
- Is replay a full rerun, a step-by-step re-check, or a receipt-driven simulation in the first implementation wave?
- How much playbook state should become first-class in stored receipts versus remaining derived from step receipts?

## Related Docs

- [`../../how-guild-works.md`](../../how-guild-works.md) is the thin main-docs entrypoint for readers coming from the README and CLI docs.
- [`07-reference-playbooks.md`](07-reference-playbooks.md) captures the candidate operator stories this surface is meant to support.
- [`08-manifest-to-playbook-translation-note.md`](08-manifest-to-playbook-translation-note.md) shows how a real current repo example maps into this framing without changing runtime truth.
