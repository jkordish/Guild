# TASK-20: CLI Example Journey Rewrite

## Problem

Guild's CLI examples currently teach the real commands, but they do not yet read like an operator journey centered on admission, execution, inspection, and replay.

## User/Persona

- Persona: Operators learning Guild by example
- Journey: Copying command examples from docs into their terminal
- Surface: `docs/command-language.md`, `README.md`, related CLI examples

## Current Friction

Examples are accurate but still feel like substrate navigation rather than guided operational automation.

## Desired Behavior

CLI examples should read like a compatible operator journey while staying honest about current command names and support boundaries.

## Concrete Command/Output Examples

```text
# desired
review authority -> execute bounded action -> inspect result -> verify trust/evidence
```

## Acceptance Criteria

- [ ] CLI examples are ordered as an operator journey.
- [ ] Current command names stay visible where they are still the actual entrypoints.
- [ ] Example wording uses the approved glossary and capability language.

## Non-Goals

- Do not add new CLI features in this task.
- Do not duplicate every example in the docs tree.

## Repo-Grounded Surfaces

- `docs/command-language.md`
- `README.md`
- `crates/guild-mcp/tests/guild_cli.rs`

## Validation Commands

```bash
git diff --check
cargo test -p guild-mcp --test guild_cli
cargo run -q -p guild-mcp --bin guild -- --help
```

## Migration Notes

- Keep examples aligned with the mapping table and any alias-preview work.
- Favor a small number of high-signal examples over exhaustive command lists.

## Risks / Fallback

- Risk: the examples imply a CLI flow the binary cannot yet support.
- Fallback: use explanatory captions that separate current commands from target conceptual stages.

## Suggested Owner

`CLI`

## Size

`M`

## Suggested Labels

- `enhancement`
- `cli`
- `docs`

## Linked Epic

- [EPIC-05: CLI Tightening](../epics/EPIC-05-cli-tightening.md)

## Dependency Links

- Blocked by: [TASK-18](./TASK-18-command-mapping-table.md), [TASK-19](./TASK-19-inspect-first-cli-alias-preview.md)
- Blocks: [TASK-27](./TASK-27-repo-local-launch-copy-pack.md)
