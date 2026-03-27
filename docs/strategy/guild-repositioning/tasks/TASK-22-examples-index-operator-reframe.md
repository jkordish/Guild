# TASK-22: Examples Index Operator Reframe

## Problem

`examples/README.md` still reads more like a technical example catalog than a set of operator workflows anchored in the new playbook story.

## User/Persona

- Persona: Operators browsing examples
- Journey: Choosing the most relevant example or starter path
- Surface: `examples/README.md`

## Current Friction

The examples index does not yet help users see which examples map to the new reference playbooks or the trusted operational automation story.

## Desired Behavior

The examples index should organize and describe examples in operator-facing terms, with clear links to the reference playbook set.

## Concrete Command/Output Examples

```text
# current
technical example inventory

# desired
operator workflow inventory with clear playbook anchors
```

## Acceptance Criteria

- [ ] The examples index explains examples as operator workflows first.
- [ ] The index links examples back to the approved reference playbook set.
- [ ] The change stays within the current example surface rather than inventing new runnable examples.

## Non-Goals

- Do not rewrite every example README in one task.
- Do not add new example code.

## Repo-Grounded Surfaces

- `examples/README.md`
- `docs/strategy/guild-repositioning/07-reference-playbooks.md`

## Validation Commands

```bash
git diff --check
cargo run -q -p xtask -- project-positioning check
```

## Migration Notes

- Use the approved playbook names and sequence.
- Keep technical detail available, but not as the lead framing.

## Risks / Fallback

- Risk: the examples index gets ahead of the actual example inventory.
- Fallback: label future or planned playbook mappings clearly rather than implying runnable coverage.

## Suggested Owner

`docs`

## Size

`M`

## Suggested Labels

- `enhancement`
- `docs`
- `examples`

## Linked Epic

- [EPIC-06: Reference Playbooks](../epics/EPIC-06-reference-playbooks.md)

## Dependency Links

- Blocked by: [TASK-21](./TASK-21-publish-reference-playbook-set.md)
- Blocks: [TASK-23](./TASK-23-guild-ops-starter-playbook-reframe.md), [TASK-27](./TASK-27-repo-local-launch-copy-pack.md)
