# TASK-18: Command Mapping Table

## Problem

Readers need an explicit bridge from the current CLI verbs to the target operator flow, or they will misread the repositioning docs as incompatible with today's binary.

## User/Persona

- Persona: Operators and maintainers
- Journey: Translating current commands into the new conceptual model
- Surface: command docs and migration notes

## Current Friction

The relationship between `run`, `show`, `get`, `why`, `verify` and the target verbs is currently spread across narrative docs instead of being made explicit in one table.

## Desired Behavior

Guild should have one clear command mapping table that shows current verb, target verb, status, and migration notes.

## Concrete Command/Output Examples

```text
run    -> exec
show   -> inspect
why    -> inspect
verify -> trust review / verify
```

## Acceptance Criteria

- [ ] The mapping table is explicit and easy to scan.
- [ ] Each mapping records whether the target verb is current, alias-preview, or future only.
- [ ] The table stays consistent with the CLI simplification doc.

## Non-Goals

- Do not implement aliases in this task.
- Do not remove current command documentation.

## Repo-Grounded Surfaces

- `docs/command-language.md`
- `docs/strategy/guild-repositioning/05-cli-simplification.md`

## Validation Commands

```bash
git diff --check
```

## Migration Notes

- Keep the table close to the command-language doc so readers do not need multiple tabs to understand the migration.
- Use the aliases-first posture consistently.

## Risks / Fallback

- Risk: the mapping table diverges from the CLI simplification doc.
- Fallback: make the command-language doc link directly back to the simplification doc for source-of-truth wording.

## Suggested Owner

`docs`

## Size

`S`

## Suggested Labels

- `enhancement`
- `docs`
- `cli`

## Linked Epic

- [EPIC-05: CLI Tightening](../epics/EPIC-05-cli-tightening.md)

## Dependency Links

- Blocked by: [TASK-17](./TASK-17-command-language-target-flow-update.md)
- Blocks: [TASK-20](./TASK-20-cli-example-journey-rewrite.md)
