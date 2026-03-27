# TASK-25: Add Trust-Chain Explainer

## Problem

Guild has the substance for trust, but the repo still needs one concise, operator-facing explainer that ties admission, isolation, receipts, evidence, and replay together.

## User/Persona

- Persona: Ops, platform, SRE, and security engineers
- Journey: Understanding why Guild is safer than ad hoc automation
- Surface: top-level docs

## Current Friction

The trust story is currently distributed across architecture and runtime-oriented docs rather than being summarized as a user-facing value proposition.

## Desired Behavior

The repo should have one short trust-chain explainer that an operator can read quickly and then trace into deeper docs if needed.

## Concrete Command/Output Examples

```text
# desired
admission -> bounded execution -> receipt -> evidence -> replayable explanation
```

## Acceptance Criteria

- [ ] The explainer is concise and operator-facing.
- [ ] It connects the key trust terms without claiming unsupported features.
- [ ] It links naturally to the deeper trust docs.

## Non-Goals

- Do not rewrite all trust-heavy docs in this task.
- Do not define new trust primitives.

## Repo-Grounded Surfaces

- `README.md`
- `docs/how-guild-works.md`
- `docs/strategy/guild-repositioning/00-north-star.md`

## Validation Commands

```bash
git diff --check
cargo run -q -p xtask -- project-positioning check
```

## Migration Notes

- Use the approved glossary exactly.
- Keep replay language honest if replay is still target-state rather than first-class implementation.

## Risks / Fallback

- Risk: the explainer becomes too hand-wavy or too implementation-heavy.
- Fallback: keep one short operator summary and link out to the trust-heavy docs for detail.

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

- Blocked by: [TASK-01](./TASK-01-readme-hero-reset.md), [TASK-05](./TASK-05-publish-glossary-entrypoint.md)
- Blocks: [TASK-26](./TASK-26-trust-heavy-docs-realignment.md), [TASK-27](./TASK-27-repo-local-launch-copy-pack.md)
