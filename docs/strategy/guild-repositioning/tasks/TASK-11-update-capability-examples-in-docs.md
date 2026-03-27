# TASK-11: Update Capability Examples In Docs

## Problem

The docs and examples still teach capabilities through implementation-facing names, which undercuts the new operator-facing story.

## User/Persona

- Persona: Operators reading examples and maintainers updating docs
- Journey: Learning how capability review should read in Guild
- Surface: docs and examples

## Current Friction

Readers must mentally translate internal family names into operational intent because the examples do not yet show the approved external capability language.

## Desired Behavior

Docs and examples should show external capability names first, with mapping or implementation detail only where it adds necessary precision.

## Concrete Command/Output Examples

```text
# current
read-resource / http-request / invoke-skill

# desired
metrics:query / logs:query / k8s:restart
```

## Acceptance Criteria

- [ ] Touched examples lead with operator-readable capability names.
- [ ] Internal family names are still visible where needed for truth and mapping.
- [ ] No example suggests broader capability coverage than the current frontier.

## Non-Goals

- Do not rewrite every example in the repository.
- Do not change runtime behavior in this task.

## Repo-Grounded Surfaces

- `docs/strategy/guild-repositioning/03-capability-taxonomy-v1.md`
- `examples/README.md`
- `examples/skills/guild-ops-starter/README.md`
- `docs/command-language.md`

## Validation Commands

```bash
git diff --check
cargo run -q -p xtask -- project-positioning check
```

## Migration Notes

- Pair each external capability example with mapping or notes if the internal family is still important for trust review.
- Keep the examples concrete and operator-facing, not theoretical.

## Risks / Fallback

- Risk: examples become inconsistent with the actual mapping guidance.
- Fallback: update the mapping doc first, then land example edits in a smaller second step.

## Suggested Owner

`docs`

## Size

`M`

## Suggested Labels

- `enhancement`
- `docs`
- `examples`

## Linked Epic

- [EPIC-03: Capability Model V1](../epics/EPIC-03-capability-model-v1.md)

## Dependency Links

- Blocked by: [TASK-09](./TASK-09-publish-capability-taxonomy-v1.md), [TASK-10](./TASK-10-map-external-capabilities-to-current-families.md)
- Blocks: [TASK-21](./TASK-21-publish-reference-playbook-set.md)
