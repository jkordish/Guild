# TASK-16: Draft Manifest-To-Playbook Translation Note

## Problem

Contributors who understand today's manifests still need a bridge document that shows how the new playbook story relates to current examples without forcing a runtime redesign.

## User/Persona

- Persona: Maintainers and advanced contributors
- Journey: Translating current Guild examples into the playbook model
- Surface: strategy docs and example guidance

## Current Friction

Without a translation note, the repositioning work can look disconnected from the current manifest-driven reality in the repo.

## Desired Behavior

One clear note should show how a current manifest/example maps to the proposed playbook framing so future contributors can reason about the migration honestly.

## Concrete Command/Output Examples

```text
# desired
current example manifest -> public playbook framing -> unchanged underlying trust/runtime facts
```

## Acceptance Criteria

- [ ] The note uses a real current example from the repo.
- [ ] The translation explains what changes in wording and what stays the same underneath.
- [ ] The note avoids inventing a new implementation path.

## Non-Goals

- Do not convert examples to a new file format in this task.
- Do not broaden example behavior.

## Repo-Grounded Surfaces

- `examples/skills/guild-ops-starter/README.md`
- `docs/strategy/guild-repositioning/04-playbook-surface-v1.md`
- `docs/strategy/guild-repositioning/07-reference-playbooks.md`

## Validation Commands

```bash
git diff --check
```

## Migration Notes

- Prefer Guild Ops Starter unless the team chooses a different first playbook anchor.
- Keep the note translation-oriented, not normative.

## Risks / Fallback

- Risk: the note accidentally becomes a shadow spec.
- Fallback: label it explicitly as a translation guide and keep the schema details in the playbook surface doc only.

## Suggested Owner

`docs`

## Size

`M`

## Suggested Labels

- `enhancement`
- `docs`
- `playbooks`

## Linked Epic

- [EPIC-04: Playbook Surface V1](../epics/EPIC-04-playbook-surface-v1.md)

## Dependency Links

- Blocked by: [TASK-14](./TASK-14-document-playbook-to-skill-composition.md)
- Blocks: [TASK-23](./TASK-23-guild-ops-starter-playbook-reframe.md)
