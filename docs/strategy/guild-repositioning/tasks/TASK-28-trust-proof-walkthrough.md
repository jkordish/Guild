# TASK-28: Trust-Proof Walkthrough

## Problem

The repositioning story says the trust chain is the product, but the repo still needs one concrete walkthrough showing what an operator sees before, during, and after a run using today's receipts and evidence surfaces.

## User/Persona

- Persona: Operators and reviewers
- Journey: Deciding whether Guild's trust story is believable in practice
- Surface: trust docs, examples, walkthrough content

## Current Friction

The repo has the pieces for trust proof, but not yet one end-to-end operator walkthrough anchored in the current live path.

## Desired Behavior

Guild should have one walkthrough that shows authority review, bounded execution, receipt lookup, evidence lookup, and replay-oriented explanation using current surfaces only.

## Concrete Command/Output Examples

```text
# desired
review capability scope -> run bounded workflow -> inspect execution -> inspect evidence -> explain what happened
```

## Acceptance Criteria

- [ ] The walkthrough uses only current Guild surfaces and proof paths.
- [ ] The walkthrough shows receipts and evidence in operator-facing terms.
- [ ] The walkthrough is specific enough to support launch copy and example follow-on work.

## Non-Goals

- Do not invent a new replay engine.
- Do not broaden the proof frontier for storytelling convenience.

## Repo-Grounded Surfaces

- `README.md`
- `docs/testing.md`
- `docs/how-guild-works.md`
- `docs/strategy/guild-repositioning/validation-and-acceptance.md`

## Validation Commands

```bash
git diff --check
cargo run -q -p guild-mcp --bin guild -- codex smoke --registry-root target/dev-local-registry/codex-local --flow explain-execution
cargo run -q -p guild-mcp --bin guild -- codex smoke --registry-root target/dev-local-registry/codex-local --flow explain-execution-tree
```

## Migration Notes

- Prefer a current proof-backed path such as explain-execution and explain-execution-tree over speculative replay wording.
- If replay remains target-state only, describe the current surface as replay-oriented explanation rather than promising first-class replay.

## Risks / Fallback

- Risk: the walkthrough overclaims current replay or playbook support.
- Fallback: keep the walkthrough firmly tied to existing receipts, evidence, and explain flows.

## Suggested Owner

`docs`

## Size

`M`

## Suggested Labels

- `enhancement`
- `docs`
- `trust`

## Linked Epic

- [EPIC-07: Trust Docs And Site Realignment](../epics/EPIC-07-trust-docs-and-site-realignment.md)

## Dependency Links

- Blocked by: [TASK-24](./TASK-24-build-one-hero-reference-example-plan.md), [TASK-26](./TASK-26-trust-heavy-docs-realignment.md)
- Blocks: none
