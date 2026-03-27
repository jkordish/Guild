# TASK-24: Build One Hero Reference Example Plan

## Problem

The strategy identifies strong reference playbooks, but the repo still needs one concrete, implementation-ready plan for the first hero example that fits today's support frontier.

## User/Persona

- Persona: Maintainers and reviewers
- Journey: Choosing the first example that should graduate from docs concept to real Guild example work
- Surface: reference example planning

## Current Friction

Without a single hero example plan, the example work can stay diffuse and fail to prove the repositioning thesis in a believable way.

## Desired Behavior

The repo should identify one first hero example, its required surfaces, its acceptance criteria, and the reason it fits today's trust and capability boundaries.

## Concrete Command/Output Examples

```text
# desired
one approved first example plan, likely based on diagnose -> restart -> notify or another equally bounded workflow
```

## Acceptance Criteria

- [ ] One first example is chosen and justified.
- [ ] The plan names the exact repo surfaces and likely proof commands required.
- [ ] The example stays implementable on current Guild trust and capability boundaries.

## Non-Goals

- Do not build the example in this task.
- Do not widen the runtime frontier to make the example easier.

## Repo-Grounded Surfaces

- `examples/README.md`
- `examples/skills/guild-ops-starter/README.md`
- `docs/strategy/guild-repositioning/07-reference-playbooks.md`
- `README.md`

## Validation Commands

```bash
git diff --check
```

## Migration Notes

- Favor the workflow that best demonstrates trust and evidence, not the one with the flashiest integrations.
- If current runtime truth is too narrow, keep the example plan docs-first until code support exists.

## Risks / Fallback

- Risk: the hero example plan assumes capability coverage that does not exist.
- Fallback: keep the plan bounded to doc surfaces and current proof commands until runtime support is real.

## Suggested Owner

`runtime`

## Size

`L`

## Suggested Labels

- `enhancement`
- `examples`
- `runtime`

## Linked Epic

- [EPIC-06: Reference Playbooks](../epics/EPIC-06-reference-playbooks.md)

## Dependency Links

- Blocked by: [TASK-21](./TASK-21-publish-reference-playbook-set.md), [TASK-23](./TASK-23-guild-ops-starter-playbook-reframe.md)
- Blocks: [TASK-28](./TASK-28-trust-proof-walkthrough.md)
