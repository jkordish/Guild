# TASK-21: Publish Reference Playbook Set

## Problem

The strategy docs name the right operator workflows, but the repo still needs one execution task that turns them into the canonical reference set for follow-on work.

## User/Persona

- Persona: Maintainers and operators evaluating the roadmap
- Journey: Deciding which example workflows Guild should lead with
- Surface: reference-playbook docs

## Current Friction

Without a published reference set, example tasks can drift toward whichever workflow looks interesting rather than the strategically chosen ones.

## Desired Behavior

The repo should have one approved set of reference playbooks with clear sequencing, capability needs, and strategic rationale.

## Concrete Command/Output Examples

```text
diagnose service -> restart pods -> notify on-call
rollback deployment -> verify health -> annotate incident
```

## Acceptance Criteria

- [ ] The reference set names at least the approved operator workflows.
- [ ] Each playbook records capability needs and sequencing.
- [ ] The set stays tied to current Guild truth instead of fantasy integrations.

## Non-Goals

- Do not build all examples in this task.
- Do not expand runtime support just to make the list look complete.

## Repo-Grounded Surfaces

- `docs/strategy/guild-repositioning/07-reference-playbooks.md`
- `docs/strategy/guild-repositioning/03-capability-taxonomy-v1.md`

## Validation Commands

```bash
git diff --check
```

## Migration Notes

- Keep the list consistent with the capability taxonomy and playbook surface docs.
- Mark any workflow that needs future runtime work as docs-first.

## Risks / Fallback

- Risk: the playbook set becomes too broad or too speculative.
- Fallback: keep the set limited to the approved anchor workflows and explicitly defer anything else.

## Suggested Owner

`docs`

## Size

`S`

## Suggested Labels

- `enhancement`
- `docs`
- `examples`

## Linked Epic

- [EPIC-06: Reference Playbooks](../epics/EPIC-06-reference-playbooks.md)

## Dependency Links

- Blocked by: [TASK-15](./TASK-15-add-playbook-concept-entrypoint.md)
- Blocks: [TASK-22](./TASK-22-examples-index-operator-reframe.md), [TASK-23](./TASK-23-guild-ops-starter-playbook-reframe.md), [TASK-24](./TASK-24-build-one-hero-reference-example-plan.md)
