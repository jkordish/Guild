# TASK-05: Publish Glossary Entrypoint

## Problem

The repositioning pack has a glossary, but the main docs tree does not yet treat it as a canonical entrypoint for future wording work.

## User/Persona

- Persona: Maintainers, doc writers, and reviewers
- Journey: Choosing the right Guild vocabulary when changing docs or help text
- Surface: docs entrypoints

## Current Friction

Without a clearly linked glossary entrypoint, later tasks are likely to drift into inconsistent synonyms or old terms.

## Desired Behavior

The canonical glossary should be easy to discover from the repositioning docs and any relevant docs entrypoints touched by this program.

## Concrete Command/Output Examples

```text
# current
glossary exists, but only if you already know where to look

# desired
docs link directly to the approved vocabulary source
```

## Acceptance Criteria

- [ ] The glossary is linked from the main repositioning surfaces.
- [ ] Later task files can reference one canonical vocabulary source.
- [ ] The glossary is described as the user-facing language source, not a runtime-contract source.

## Non-Goals

- Do not perform the full term sweep in this task.
- Do not rename internal types or schemas.

## Repo-Grounded Surfaces

- `docs/strategy/guild-repositioning/02-glossary-and-banned-terms.md`
- `docs/strategy/guild-repositioning/00-north-star.md`
- `docs/strategy/guild-repositioning/tasks.md`

## Validation Commands

```bash
git diff --check
```

## Migration Notes

- Keep the glossary lightweight and link-focused in this task.
- Use the exact approved terms from the current glossary doc.

## Risks / Fallback

- Risk: the glossary stays isolated from the docs that need it.
- Fallback: add links from the repositioning index surfaces first, then broaden later.

## Suggested Owner

`docs`

## Size

`S`

## Suggested Labels

- `enhancement`
- `docs`
- `glossary`

## Linked Epic

- [EPIC-02: Glossary And Language Simplification](../epics/EPIC-02-glossary-and-language-simplification.md)

## Dependency Links

- Blocked by: north-star approval
- Blocks: [TASK-06](./TASK-06-top-level-discouraged-terms-sweep.md), [TASK-07](./TASK-07-issue-template-language-update.md), [TASK-08](./TASK-08-cli-help-terminology-review.md)
