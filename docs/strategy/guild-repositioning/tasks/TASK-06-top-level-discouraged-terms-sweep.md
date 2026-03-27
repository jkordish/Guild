# TASK-06: Top-Level Discouraged-Terms Sweep

## Problem

The repo still uses discouraged lead terms such as "artifact," "substrate," and "reference application" in entrypoint docs where operator-facing language should come first.

## User/Persona

- Persona: New evaluators and maintainers
- Journey: Reading top-level docs after the north-star is approved
- Surface: `README.md`, `docs/`, `examples/`

## Current Friction

Even after the narrative reset, old lead terms can keep resurfacing and dilute the new operator story.

## Desired Behavior

Top-level docs should lead with the approved operator vocabulary and reserve discouraged terms for technically necessary contexts only.

## Concrete Command/Output Examples

```text
# current
portable skill artifact / substrate / reference application

# desired
ops playbook / capability / admission / evidence / trust chain
```

## Acceptance Criteria

- [ ] Touched entrypoint docs no longer lead with discouraged terms.
- [ ] Any remaining discouraged terms are clearly scoped to mechanism-layer detail.
- [ ] The change list stays focused on top-level docs rather than the entire repo in one PR.

## Non-Goals

- Do not sweep every historical doc in the repository.
- Do not change code symbols just to match wording.

## Repo-Grounded Surfaces

- `README.md`
- `docs/how-guild-works.md`
- `docs/roadmap.md`
- `examples/README.md`

## Validation Commands

```bash
git diff --check
cargo run -q -p xtask -- project-positioning check
rg -n 'artifact|substrate|reference application' README.md docs examples
```

## Migration Notes

- Follow the glossary doc for approved replacements.
- Keep historical or technical uses only when they are needed for precision.

## Risks / Fallback

- Risk: the sweep becomes a noisy repo-wide copy-edit.
- Fallback: limit the first pass to the docs explicitly called out in the repositioning audit.

## Suggested Owner

`docs`

## Size

`M`

## Suggested Labels

- `enhancement`
- `docs`
- `ux-copy`

## Linked Epic

- [EPIC-02: Glossary And Language Simplification](../epics/EPIC-02-glossary-and-language-simplification.md)

## Dependency Links

- Blocked by: [TASK-05](./TASK-05-publish-glossary-entrypoint.md)
- Blocks: [TASK-26](./TASK-26-trust-heavy-docs-realignment.md)
