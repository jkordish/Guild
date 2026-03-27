# TASK-04: How Guild Works Operator Reframe

## Problem

`docs/how-guild-works.md` explains the system accurately, but it still carries too much mechanism-layer detail before it explains Guild in operator terms.

## User/Persona

- Persona: Operators evaluating how Guild behaves
- Journey: Understanding system behavior after reading the README
- Surface: `docs/how-guild-works.md`

## Current Friction

The doc makes it easy to understand Guild as architecture, but harder to understand Guild as trusted operational automation.

## Desired Behavior

The doc should introduce playbooks, capabilities, admission, isolation, receipts, evidence, and replay as operator-visible concepts before diving into execution shape and trust boundaries.

## Concrete Command/Output Examples

```text
# current
architecture-first explanation

# desired
operator journey first, architecture second
```

## Acceptance Criteria

- [ ] The introduction explains Guild in operator-facing terms.
- [ ] Trust-chain concepts are framed as user value before mechanism-layer detail.
- [ ] The rewritten sections still preserve the current trust boundaries and fail-closed posture.

## Non-Goals

- Do not rewrite `ARCHITECTURE.md` in this task.
- Do not add new product claims beyond the approved strategy docs.

## Repo-Grounded Surfaces

- `docs/how-guild-works.md`
- `README.md`
- `docs/strategy/guild-repositioning/00-north-star.md`

## Validation Commands

```bash
git diff --check
cargo run -q -p xtask -- project-positioning check
```

## Migration Notes

- Keep architecture and contract references intact; change the order and framing first.
- Reuse the glossary terms exactly so the docs tree stays consistent.

## Risks / Fallback

- Risk: the doc loses technical precision while becoming more readable.
- Fallback: limit the change to the intro, summary, and section lead-ins, leaving deeper technical sections mostly intact.

## Suggested Owner

`docs`

## Size

`M`

## Suggested Labels

- `enhancement`
- `docs`
- `ux-copy`

## Linked Epic

- [EPIC-01: Narrative Reset](../epics/EPIC-01-narrative-reset.md)

## Dependency Links

- Blocked by: [TASK-01](./TASK-01-readme-hero-reset.md), [TASK-03](./TASK-03-project-positioning-decision-and-update.md)
- Blocks: [TASK-25](./TASK-25-add-trust-chain-explainer.md)
