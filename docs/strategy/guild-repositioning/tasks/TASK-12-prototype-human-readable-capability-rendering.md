# TASK-12: Prototype Human-Readable Capability Rendering

## Problem

The docs can introduce external capability names, but contributors still need one concrete example of how those names could surface in the CLI or docs output without touching the internal contract model yet.

## User/Persona

- Persona: CLI maintainers and reviewers
- Journey: Evaluating whether the external capability model can appear in user-facing output honestly
- Surface: CLI or docs rendering path

## Current Friction

Without a prototype rendering path, the operator-readable capability model risks staying purely abstract.

## Desired Behavior

Guild should have one narrow prototype showing how user-facing output could present external capability names first and implementation detail second.

## Concrete Command/Output Examples

```text
# desired
Capabilities:
- k8s:restart (mapped from current internal capability family constraints)
```

## Acceptance Criteria

- [ ] The prototype uses a real existing output or docs rendering surface.
- [ ] The prototype does not rename the underlying internal contract structures.
- [ ] The output makes the mapping explicit enough to review for honesty.

## Non-Goals

- Do not do a repo-wide capability-output redesign.
- Do not treat the prototype as a normative runtime contract.

## Repo-Grounded Surfaces

- `crates/guild-mcp/src/cli.rs`
- `crates/guild-mcp/src/cli_presenter.rs`
- `crates/guild-mcp/tests/guild_cli.rs`
- `docs/strategy/guild-repositioning/03-capability-taxonomy-v1.md`

## Validation Commands

```bash
git diff --check
cargo test -p guild-mcp --test guild_cli
cargo run -q -p guild-mcp --bin guild -- help grants
```

## Migration Notes

- Keep the prototype isolated and clearly labeled as a presentation-layer change.
- If the CLI is not the right first surface, use a docs-rendered example instead.

## Risks / Fallback

- Risk: the prototype starts an uncontrolled CLI redesign.
- Fallback: confine the work to one presenter/help-output path or one documented mock rendering.

## Suggested Owner

`CLI`

## Size

`M`

## Suggested Labels

- `enhancement`
- `cli`
- `capabilities`

## Linked Epic

- [EPIC-03: Capability Model V1](../epics/EPIC-03-capability-model-v1.md)

## Dependency Links

- Blocked by: [TASK-10](./TASK-10-map-external-capabilities-to-current-families.md)
- Blocks: [TASK-19](./TASK-19-inspect-first-cli-alias-preview.md)
