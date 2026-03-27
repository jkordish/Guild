# TASK-08: CLI Help Terminology Review

## Problem

Guild's CLI help is generally solid, but key help surfaces still need a terminology pass so they align with the new operator-facing glossary.

## User/Persona

- Persona: Operators discovering Guild through the CLI
- Journey: Reading `guild --help` and subcommand help before running anything
- Surface: `guild` help output and CLI presenter/source files

## Current Friction

Help text can still sound substrate-shaped or generic even when the narrative docs move toward trusted operational automation.

## Desired Behavior

The help output should use the approved vocabulary where possible without claiming commands or behaviors that do not exist.

## Concrete Command/Output Examples

```text
# current
help text reflects current command structure but not the full operator-facing vocabulary

# desired
help text stays honest while using capability, admission, inspectability, evidence, and replay language carefully
```

## Acceptance Criteria

- [ ] The review identifies which help strings can change now and which must wait for CLI behavior changes.
- [ ] At least the top-level help and the most used subcommands are covered.
- [ ] The task produces concrete follow-on changes or a no-change rationale for commands that are still too implementation-bound to rename.

## Non-Goals

- Do not introduce new commands in this task.
- Do not rewrite the CLI flow itself here.

## Repo-Grounded Surfaces

- `crates/guild-mcp/src/cli.rs`
- `crates/guild-mcp/src/cli_presenter.rs`
- `crates/guild-mcp/tests/guild_cli.rs`
- `docs/command-language.md`

## Validation Commands

```bash
git diff --check
cargo test -p guild-mcp --test guild_cli
cargo run -q -p guild-mcp --bin guild -- --help
```

## Migration Notes

- Keep aspirational terms like `admit` and `replay` out of help text unless they are clearly marked as future or alias-preview surfaces.
- Use the glossary as the source of truth for replacements.

## Risks / Fallback

- Risk: help text becomes aspirational before the CLI does.
- Fallback: restrict changes to descriptors and explanatory copy, not command names.

## Suggested Owner

`CLI`

## Size

`M`

## Suggested Labels

- `enhancement`
- `cli`
- `ux-copy`

## Linked Epic

- [EPIC-02: Glossary And Language Simplification](../epics/EPIC-02-glossary-and-language-simplification.md)

## Dependency Links

- Blocked by: [TASK-05](./TASK-05-publish-glossary-entrypoint.md)
- Blocks: [TASK-19](./TASK-19-inspect-first-cli-alias-preview.md)
