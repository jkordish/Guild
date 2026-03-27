# TASK-27: Repo-Local Launch Copy Pack

## Problem

The repositioning work needs a launch-ready copy layer for repo-owned surfaces, but there is no separate in-repo website source to update today.

## User/Persona

- Persona: Maintainers preparing the relaunch
- Journey: Packaging the new story for repo-local surfaces
- Surface: README, release notes, issue templates, strategy docs

## Current Friction

Without a launch copy pack, the repo can have good strategy docs but still lack a concise, reusable set of messages for rollout moments.

## Desired Behavior

The repo should contain a reusable launch copy pack for the surfaces it actually owns, with explicit notes about any website follow-up that must happen elsewhere.

## Concrete Command/Output Examples

```text
# desired
README summary
release-note blurb
issue-filing blurb
```

## Acceptance Criteria

- [ ] The copy pack covers repo-local launch surfaces only.
- [ ] It uses the approved narrative and glossary consistently.
- [ ] It explicitly records any external website follow-up as out-of-repo work.

## Non-Goals

- Do not create or edit an external website in this task.
- Do not add new product claims for launch effect.

## Repo-Grounded Surfaces

- `README.md`
- `.github/ISSUE_TEMPLATE/ux-epic.md`
- `.github/ISSUE_TEMPLATE/ux-task.md`
- `docs/strategy/guild-repositioning/`

## Validation Commands

```bash
git diff --check
```

## Migration Notes

- Treat this as a repo-local copy pack, not a full marketing site plan.
- Reuse the trust-chain explainer and CLI story where possible rather than drafting parallel language.

## Risks / Fallback

- Risk: the task becomes blocked on a website repo that is not present here.
- Fallback: complete the repo-local copy pack and log website follow-up separately.

## Suggested Owner

`website`

## Size

`M`

## Suggested Labels

- `enhancement`
- `docs`
- `launch`

## Linked Epic

- [EPIC-07: Trust Docs And Site Realignment](../epics/EPIC-07-trust-docs-and-site-realignment.md)

## Dependency Links

- Blocked by: [TASK-22](./TASK-22-examples-index-operator-reframe.md), [TASK-25](./TASK-25-add-trust-chain-explainer.md), [TASK-20](./TASK-20-cli-example-journey-rewrite.md)
- Blocks: none
