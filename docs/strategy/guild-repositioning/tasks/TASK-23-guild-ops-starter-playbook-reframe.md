# TASK-23: Guild Ops Starter Playbook Reframe

## Problem

`examples/skills/guild-ops-starter/README.md` is one of the most important example surfaces, but it still inherits the older reference-application framing.

## User/Persona

- Persona: Operators evaluating the first serious Guild example
- Journey: Deciding whether Guild Ops Starter is a believable playbook-oriented workflow anchor
- Surface: `examples/skills/guild-ops-starter/README.md`

## Current Friction

The current framing makes Guild Ops Starter sound like a generic reference application instead of a bounded ops playbook starter built on Guild's trust model.

## Desired Behavior

Guild Ops Starter should read as a playbook-oriented starter for trusted operational automation while still staying inside the current support frontier.

## Concrete Command/Output Examples

```text
# current
reference application

# desired
ops playbook starter / trust-first operational workflow starter
```

## Acceptance Criteria

- [ ] The README no longer leads with generic reference-application language.
- [ ] The new framing ties Guild Ops Starter to the approved playbook story.
- [ ] The README does not imply a full workflow engine or unsupported capabilities.

## Non-Goals

- Do not rename the example directory in this task.
- Do not add new runtime behavior or capabilities.

## Repo-Grounded Surfaces

- `examples/skills/guild-ops-starter/README.md`
- `examples/README.md`
- `docs/strategy/guild-repositioning/07-reference-playbooks.md`

## Validation Commands

```bash
git diff --check
cargo run -q -p xtask -- project-positioning check
```

## Migration Notes

- Keep the example anchored to the current trust and evidence model.
- If the team does not want the example to become the first playbook starter, mark it as a near-term transition state explicitly.

## Risks / Fallback

- Risk: the example gets reframed too broadly and restarts the workflow-engine concern.
- Fallback: use narrower "starter" wording and tie the example back to one or two approved reference playbooks only.

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

- Blocked by: [TASK-21](./TASK-21-publish-reference-playbook-set.md), [TASK-22](./TASK-22-examples-index-operator-reframe.md), [TASK-16](./TASK-16-draft-manifest-to-playbook-translation-note.md)
- Blocks: [TASK-24](./TASK-24-build-one-hero-reference-example-plan.md)
