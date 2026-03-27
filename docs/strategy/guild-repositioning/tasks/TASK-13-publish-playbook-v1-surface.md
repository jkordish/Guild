# TASK-13: Publish Playbook V1 Surface

## Problem

The repositioning story depends on playbooks, but the repo still needs a stable, issue-ready task to turn the current planning doc into the public playbook entrypoint.

## User/Persona

- Persona: Operators and doc readers
- Journey: Understanding what a Guild playbook is
- Surface: playbook docs

## Current Friction

Readers can see that playbooks matter, but they do not yet have one clear place where the first public playbook surface is established as a bounded Guild concept.

## Desired Behavior

The repo should have one concrete playbook v1 surface doc that defines the concept, the minimum schema shape, and the current boundaries.

## Concrete Command/Output Examples

```text
# desired
kind: Playbook
capabilities:
  - k8s:restart
  - chat:post
```

## Acceptance Criteria

- [ ] The playbook v1 surface is discoverable and clearly scoped.
- [ ] The doc defines playbooks as the operator-facing automation unit.
- [ ] The doc says explicitly that the playbook surface is a planning and UX target unless implementation catches up.

## Non-Goals

- Do not ship a playbook runtime.
- Do not add a generic workflow DSL.

## Repo-Grounded Surfaces

- `docs/strategy/guild-repositioning/04-playbook-surface-v1.md`
- `docs/strategy/guild-repositioning/07-reference-playbooks.md`

## Validation Commands

```bash
git diff --check
```

## Migration Notes

- Keep the wording tied to current Guild skills and trust boundaries.
- Use the approved capability vocabulary from the capability taxonomy.

## Risks / Fallback

- Risk: the doc reads as if playbooks already have a first-class runtime.
- Fallback: add stronger planning-target and current-boundary callouts at the top of the doc.

## Suggested Owner

`docs`

## Size

`S`

## Suggested Labels

- `enhancement`
- `docs`
- `playbooks`

## Linked Epic

- [EPIC-04: Playbook Surface V1](../epics/EPIC-04-playbook-surface-v1.md)

## Dependency Links

- Blocked by: [TASK-09](./TASK-09-publish-capability-taxonomy-v1.md)
- Blocks: [TASK-14](./TASK-14-document-playbook-to-skill-composition.md), [TASK-15](./TASK-15-add-playbook-concept-entrypoint.md)
