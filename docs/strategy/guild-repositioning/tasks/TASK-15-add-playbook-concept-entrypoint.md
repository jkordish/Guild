# TASK-15: Add Playbook Concept Entrypoint

## Problem

Even with a playbook surface doc, the main docs tree still needs a visible entrypoint so readers can discover playbooks as a first-class concept.

## User/Persona

- Persona: New doc readers
- Journey: Navigating the docs after reading the README
- Surface: docs entrypoints and concept pages

## Current Friction

Playbooks are easy to miss because they exist only inside the strategy pack unless the docs tree points at them intentionally.

## Desired Behavior

The docs tree should expose playbooks as a first-class Guild concept alongside the current trust and CLI surfaces.

## Concrete Command/Output Examples

```text
# desired
Docs navigation includes a clear playbook concept entrypoint
```

## Acceptance Criteria

- [ ] Readers can find the playbook concept from the main Guild docs path.
- [ ] The entrypoint explains the relationship to capabilities, admission, evidence, and skills.
- [ ] The docs still avoid implying a shipped workflow engine.

## Non-Goals

- Do not add a full docs navigation redesign.
- Do not duplicate the whole playbook surface doc into multiple places.

## Repo-Grounded Surfaces

- `README.md`
- `docs/how-guild-works.md`
- `docs/strategy/guild-repositioning/04-playbook-surface-v1.md`

## Validation Commands

```bash
git diff --check
cargo run -q -p xtask -- project-positioning check
```

## Migration Notes

- Prefer a thin entrypoint that links to the deeper playbook docs.
- Keep the same wording and examples as the playbook surface doc.

## Risks / Fallback

- Risk: the playbook concept gets duplicated and drifts.
- Fallback: add a short summary and a single deep link rather than duplicating content.

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

- Blocked by: [TASK-13](./TASK-13-publish-playbook-v1-surface.md), [TASK-14](./TASK-14-document-playbook-to-skill-composition.md)
- Blocks: [TASK-17](./TASK-17-command-language-target-flow-update.md), [TASK-21](./TASK-21-publish-reference-playbook-set.md)
