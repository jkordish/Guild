# TASK-01: README Hero Reset

## Problem

The `README.md` opening still leads with platform and substrate language before it explains the operator problem Guild solves.

## User/Persona

- Persona: Ops, platform, SRE, and security engineers
- Journey: First-pass repo evaluation
- Surface: `README.md`

## Current Friction

A new reader has to translate portable-artifact and trust-layer language into an operational use case before they can decide whether Guild is relevant.

## Desired Behavior

The first screen of `README.md` should describe Guild as trusted operational automation expressed through playbooks, capabilities, admission, evidence, and replay.

## Concrete Command/Output Examples

```text
# current
Guild builds portable, capability-bounded skill artifacts ...

# desired
Guild is trusted operational automation for ops teams ...
```

## Acceptance Criteria

- [ ] The `README.md` hero and opening bullets lead with trusted operational automation.
- [ ] The opening paragraph names the target audience directly.
- [ ] The rewritten hero does not imply broader runtime support than the repo currently proves.

## Non-Goals

- Do not rewrite the whole README in this task.
- Do not rename internal runtime or contract types.

## Repo-Grounded Surfaces

- `README.md`
- `docs/strategy/guild-repositioning/00-north-star.md`
- `docs/strategy/guild-repositioning/01-messaging-audit.md`

## Validation Commands

```bash
git diff --check
cargo run -q -p xtask -- project-positioning check
```

## Migration Notes

- Keep mechanism-layer detail available lower in the README.
- If the new hero wording conflicts with the current guardrail, sequence the guardrail update before merging.

## Risks / Fallback

- Risk: the hero becomes vague marketing copy.
- Fallback: keep one short operator-first paragraph followed by one precise trust-boundary paragraph.

## Suggested Owner

`docs`

## Size

`M`

## Suggested Labels

- `enhancement`
- `docs`
- `positioning`

## Linked Epic

- [EPIC-01: Narrative Reset](../epics/EPIC-01-narrative-reset.md)

## Dependency Links

- Blocked by: north-star approval
- Blocks: [TASK-02](./TASK-02-readme-overview-language-reset.md), [TASK-04](./TASK-04-how-guild-works-operator-reframe.md)
