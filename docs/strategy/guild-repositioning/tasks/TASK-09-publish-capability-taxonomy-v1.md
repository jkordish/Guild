# TASK-09: Publish Capability Taxonomy V1

## Problem

The repositioning docs define an external capability taxonomy, but it still needs to be established as an execution artifact that later docs and examples can point to.

## User/Persona

- Persona: Operators and maintainers
- Journey: Understanding what a playbook is allowed to do
- Surface: capability docs

## Current Friction

Without a canonical taxonomy entrypoint, later tasks will keep falling back to internal family names or ad hoc examples.

## Desired Behavior

Guild should have one clear operator-facing capability taxonomy using `domain:action` names such as `k8s:restart` and `logs:query`.

## Concrete Command/Output Examples

```text
# current
http-request / read-resource / invoke-skill

# desired
k8s:restart / metrics:query / incident:create
```

## Acceptance Criteria

- [ ] The taxonomy doc is positioned as the user-facing capability vocabulary.
- [ ] Naming, scoping, and design rules are explicit.
- [ ] The doc says clearly that v1 is a UX layer first.

## Non-Goals

- Do not rename Rust or WIT types.
- Do not widen runtime support in this task.

## Repo-Grounded Surfaces

- `docs/strategy/guild-repositioning/03-capability-taxonomy-v1.md`
- `docs/strategy/guild-repositioning/02-glossary-and-banned-terms.md`

## Validation Commands

```bash
git diff --check
```

## Migration Notes

- Keep the operator-readable taxonomy distinct from the current internal family names.
- Reuse the exact capability families approved in the strategy doc.

## Risks / Fallback

- Risk: the taxonomy reads like a hidden runtime rename.
- Fallback: add stronger UX-layer disclaimers and explicit mapping notes before expanding examples.

## Suggested Owner

`docs`

## Size

`S`

## Suggested Labels

- `enhancement`
- `docs`
- `capabilities`

## Linked Epic

- [EPIC-03: Capability Model V1](../epics/EPIC-03-capability-model-v1.md)

## Dependency Links

- Blocked by: [TASK-05](./TASK-05-publish-glossary-entrypoint.md)
- Blocks: [TASK-10](./TASK-10-map-external-capabilities-to-current-families.md), [TASK-11](./TASK-11-update-capability-examples-in-docs.md), [TASK-13](./TASK-13-publish-playbook-v1-surface.md)
