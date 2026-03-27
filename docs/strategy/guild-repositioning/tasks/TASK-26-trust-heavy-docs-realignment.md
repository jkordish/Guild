# TASK-26: Trust-Heavy Docs Realignment

## Problem

`README.md`, `SPECS.md`, and `ARCHITECTURE.md` contain the real trust model, but they do not yet consistently present that model as operator-facing product value.

## User/Persona

- Persona: Operators and advanced evaluators
- Journey: Validating Guild's trust story after reading the top-level docs
- Surface: `README.md`, `SPECS.md`, `ARCHITECTURE.md`

## Current Friction

The reader can find the real trust boundaries, but they are often expressed as implementation detail first and user value second.

## Desired Behavior

These docs should explain admission, isolation, receipts, evidence, and replay as operator-facing value while preserving their exact technical boundaries.

## Concrete Command/Output Examples

```text
# desired
operator value first, contract and trust-boundary precision immediately after
```

## Acceptance Criteria

- [ ] Trust-heavy docs use operator-facing framing in their intros and section lead-ins.
- [ ] The docs preserve the exact current support frontier and fail-closed boundaries.
- [ ] The three docs do not present competing explanations of trust.

## Non-Goals

- Do not weaken the normative role of `SPECS.md`.
- Do not broaden the trust model by wording alone.

## Repo-Grounded Surfaces

- `README.md`
- `SPECS.md`
- `ARCHITECTURE.md`
- `docs/project-positioning.md`

## Validation Commands

```bash
git diff --check
cargo run -q -p xtask -- project-positioning check
```

## Migration Notes

- Update top-level summaries and section lead-ins first; do not attempt a full structural rewrite in one PR.
- Keep the contract and architecture docs aligned with each other and with the README trust story.

## Risks / Fallback

- Risk: trust-heavy docs diverge in tone or overstate replay and playbook support.
- Fallback: land one smaller intro/summary alignment pass before deeper section edits.

## Suggested Owner

`docs`

## Size

`L`

## Suggested Labels

- `enhancement`
- `docs`
- `trust`

## Linked Epic

- [EPIC-07: Trust Docs And Site Realignment](../epics/EPIC-07-trust-docs-and-site-realignment.md)

## Dependency Links

- Blocked by: [TASK-25](./TASK-25-add-trust-chain-explainer.md), [TASK-03](./TASK-03-project-positioning-decision-and-update.md)
- Blocks: [TASK-28](./TASK-28-trust-proof-walkthrough.md)
