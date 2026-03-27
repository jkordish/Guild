# TASK-03: Project Positioning Decision And Update

## Problem

`docs/project-positioning.md` and `crates/guild-draft-truth/src/project_positioning.rs` still enforce the old portable-artifact-first framing, which can block or contradict the repositioning work.

## User/Persona

- Persona: Maintainers and doc owners
- Journey: Landing narrative changes without breaking repo-native guardrails
- Surface: `docs/project-positioning.md`, `crates/guild-draft-truth/src/project_positioning.rs`

## Current Friction

Top-level docs can move toward the new operator story while the positioning doc and guardrail still insist on the old thesis, creating merge friction and mixed messaging.

## Desired Behavior

The project-positioning source and its guardrail should either adopt the new thesis or be explicitly marked as historical/superseded in a way that keeps validation honest.

## Concrete Command/Output Examples

```text
# current
project positioning validates cleanly only for the old portable-artifact-first wording

# desired
project positioning validates the approved narrative or explicitly allows the staged migration
```

## Acceptance Criteria

- [ ] The repo has one clear answer for whether `docs/project-positioning.md` is current or historical.
- [ ] `cargo run -q -p xtask -- project-positioning check` passes with the intended status.
- [ ] The change does not turn the guardrail into a broad style linter.

## Non-Goals

- Do not broaden runtime or capability claims.
- Do not edit unrelated draft-truth checks.

## Repo-Grounded Surfaces

- `docs/project-positioning.md`
- `crates/guild-draft-truth/src/project_positioning.rs`
- `README.md`

## Validation Commands

```bash
git diff --check
cargo run -q -p xtask -- project-positioning check
```

## Migration Notes

- This task is the main dependency risk for the narrative rewrite.
- If the team wants a staged migration, document that status explicitly in `docs/project-positioning.md`.

## Risks / Fallback

- Risk: the guardrail and docs move out of sync.
- Fallback: land a guardrail-only or positioning-only transition note first, then move the README and docs after it passes.

## Suggested Owner

`docs`

## Size

`M`

## Suggested Labels

- `enhancement`
- `docs`
- `guardrails`

## Linked Epic

- [EPIC-01: Narrative Reset](../epics/EPIC-01-narrative-reset.md)

## Dependency Links

- Blocked by: north-star approval
- Blocks: [TASK-04](./TASK-04-how-guild-works-operator-reframe.md), [TASK-26](./TASK-26-trust-heavy-docs-realignment.md)
