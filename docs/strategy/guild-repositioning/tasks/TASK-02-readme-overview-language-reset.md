# TASK-02: README Overview Language Reset

## Problem

Even after the hero is fixed, the `README.md` overview sections can still pull the story back toward artifact-first or substrate-first framing.

## User/Persona

- Persona: Ops, platform, SRE, and security engineers
- Journey: Understanding what Guild does after the first screen
- Surface: `README.md`

## Current Friction

The overview text risks reintroducing portable-artifact-first language before it shows the operator-facing model of playbooks, capabilities, admission, and evidence.

## Desired Behavior

The README overview should explain Guild's operator model in clear, concrete language while preserving the real trust and execution boundaries.

## Concrete Command/Output Examples

```text
# current
reference application / portable skill artifact / trust layer

# desired
ops playbook / capability review / admission / receipts / evidence
```

## Acceptance Criteria

- [ ] Overview sections use the approved glossary for lead concepts.
- [ ] Any remaining artifact or substrate language is clearly scoped to the mechanism layer.
- [ ] The README overview no longer describes Guild Ops Starter as the whole product.

## Non-Goals

- Do not edit example READMEs in this task.
- Do not change command examples outside the README.

## Repo-Grounded Surfaces

- `README.md`
- `docs/strategy/guild-repositioning/02-glossary-and-banned-terms.md`
- `docs/strategy/guild-repositioning/06-roadmap.md`

## Validation Commands

```bash
git diff --check
cargo run -q -p xtask -- project-positioning check
rg -n 'artifact|substrate|reference application' README.md
```

## Migration Notes

- Keep any technical references that are still needed for accuracy, but do not let them lead the section.
- Reuse the exact terms approved in the glossary instead of inventing synonyms.

## Risks / Fallback

- Risk: the task turns into a full README rewrite.
- Fallback: limit the change to the overview and thesis sections only, then defer deeper cleanup.

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

- Blocked by: [TASK-01](./TASK-01-readme-hero-reset.md)
- Blocks: [TASK-26](./TASK-26-trust-heavy-docs-realignment.md)
