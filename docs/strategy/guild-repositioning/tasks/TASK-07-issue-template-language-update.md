# TASK-07: Issue-Template Language Update

## Problem

The GitHub issue templates already guide UX work, but they still use generic wording and do not reinforce the new operator-facing vocabulary.

## User/Persona

- Persona: Maintainers filing follow-on work
- Journey: Creating epics and tasks after the repositioning strategy is approved
- Surface: `.github/ISSUE_TEMPLATE/ux-epic.md`, `.github/ISSUE_TEMPLATE/ux-task.md`

## Current Friction

If the issue templates continue to use older or generic phrasing, new work items will drift away from the repositioning language even when the docs are updated.

## Desired Behavior

The issue templates should nudge authors toward operator-facing language, explicit non-goals, and repo-grounded acceptance criteria.

## Concrete Command/Output Examples

```text
# current
generic UX-hardening issue copy

# desired
task and epic templates that naturally produce operator-facing Guild work items
```

## Acceptance Criteria

- [ ] Both UX templates reflect the approved vocabulary where it fits.
- [ ] The templates still warn against contract widening and aspirational CLI claims.
- [ ] The update makes future issue filing easier rather than more verbose.

## Non-Goals

- Do not create new issue-template types.
- Do not add workflow-specific fields that only apply to this repositioning program.

## Repo-Grounded Surfaces

- `.github/ISSUE_TEMPLATE/ux-epic.md`
- `.github/ISSUE_TEMPLATE/ux-task.md`
- `docs/strategy/guild-repositioning/02-glossary-and-banned-terms.md`

## Validation Commands

```bash
git diff --check
```

## Migration Notes

- Preserve the existing template structure wherever possible.
- Favor wording changes over structural complexity.

## Risks / Fallback

- Risk: the templates become too specific to one project.
- Fallback: keep the structure stable and only update the wording and examples.

## Suggested Owner

`docs`

## Size

`S`

## Suggested Labels

- `enhancement`
- `docs`
- `planning`

## Linked Epic

- [EPIC-02: Glossary And Language Simplification](../epics/EPIC-02-glossary-and-language-simplification.md)

## Dependency Links

- Blocked by: [TASK-05](./TASK-05-publish-glossary-entrypoint.md)
- Blocks: none
