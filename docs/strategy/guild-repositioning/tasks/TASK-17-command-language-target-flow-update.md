# TASK-17: Command-Language Target-Flow Update

## Problem

`docs/command-language.md` currently teaches the real CLI, but it does not yet present the repositioned operator journey centered on admit, exec, inspect, and replay.

## User/Persona

- Persona: Operators learning Guild through the CLI docs
- Journey: Understanding the intended command flow before running commands
- Surface: `docs/command-language.md`

## Current Friction

The command-language doc is accurate, but it reads like the current substrate rather than the target operator flow.

## Desired Behavior

The doc should teach the target operator flow while staying honest about which verbs are current and which are target-state or compatibility aliases.

## Concrete Command/Output Examples

```text
# desired
admit -> exec -> inspect -> replay

# current compatibility
run / show / why / verify remain the actual commands until code changes land
```

## Acceptance Criteria

- [ ] The doc introduces the target operator flow explicitly.
- [ ] The doc preserves honesty about which commands exist today.
- [ ] The update does not break the current command reference usefulness.

## Non-Goals

- Do not rename the CLI in this task.
- Do not change binary behavior solely to satisfy the docs.

## Repo-Grounded Surfaces

- `docs/command-language.md`
- `docs/strategy/guild-repositioning/05-cli-simplification.md`
- `crates/guild-mcp/src/cli.rs`

## Validation Commands

```bash
git diff --check
cargo run -q -p xtask -- project-positioning check
cargo run -q -p guild-mcp --bin guild -- --help
```

## Migration Notes

- Present target verbs as target-state or aliases-first follow-ons unless implemented.
- Keep current commands discoverable in the same doc.

## Risks / Fallback

- Risk: the command doc becomes aspirational and mismatched with the CLI.
- Fallback: keep the target flow in a clearly marked future or migration section if needed.

## Suggested Owner

`docs`

## Size

`M`

## Suggested Labels

- `enhancement`
- `docs`
- `cli`

## Linked Epic

- [EPIC-05: CLI Tightening](../epics/EPIC-05-cli-tightening.md)

## Dependency Links

- Blocked by: [TASK-05](./TASK-05-publish-glossary-entrypoint.md), [TASK-15](./TASK-15-add-playbook-concept-entrypoint.md)
- Blocks: [TASK-18](./TASK-18-command-mapping-table.md), [TASK-19](./TASK-19-inspect-first-cli-alias-preview.md)
