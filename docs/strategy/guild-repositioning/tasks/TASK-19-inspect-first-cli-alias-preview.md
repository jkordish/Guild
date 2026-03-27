# TASK-19: Inspect-First CLI Alias Preview

## Problem

The repositioning strategy needs at least one concrete CLI step toward the target operator flow, but the repo explicitly warns against aspirational command names that the binary does not support honestly.

## User/Persona

- Persona: CLI users and maintainers
- Journey: Seeing how the future operator flow can surface without breaking today's CLI
- Surface: CLI help or alias behavior

## Current Friction

Without one concrete CLI-facing step, the new operator flow can remain purely narrative. Without care, it can also become misleading.

## Desired Behavior

Guild should stage one inspect-first alias or help-preview path that demonstrates the direction while preserving the current commands and trust posture.

## Concrete Command/Output Examples

```text
# desired
guild inspect ...

# compatibility
guild show ...
guild why ...
```

## Acceptance Criteria

- [ ] The change is compatibility-preserving.
- [ ] The CLI or help output makes the relationship to existing commands explicit.
- [ ] Tests cover the new alias or preview wording if code changes are involved.

## Non-Goals

- Do not promote unsupported `admit` or `replay` commands here.
- Do not remove `show` or `why`.

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

- The preferred first step is `inspect`, not a broad command-set rewrite.
- If code changes are too risky for this phase, a help-preview can satisfy the task instead.

## Risks / Fallback

- Risk: alias work causes more churn than the docs wave can support.
- Fallback: implement the inspect-first concept in help text and docs first, then delay the actual alias to a later task.

## Suggested Owner

`CLI`

## Size

`L`

## Suggested Labels

- `enhancement`
- `cli`
- `ux`

## Linked Epic

- [EPIC-05: CLI Tightening](../epics/EPIC-05-cli-tightening.md)

## Dependency Links

- Blocked by: [TASK-08](./TASK-08-cli-help-terminology-review.md), [TASK-17](./TASK-17-command-language-target-flow-update.md), [TASK-12](./TASK-12-prototype-human-readable-capability-rendering.md)
- Blocks: [TASK-20](./TASK-20-cli-example-journey-rewrite.md)
