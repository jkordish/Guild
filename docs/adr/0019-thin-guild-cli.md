# ADR 0019: Thin First-Class `guild` CLI

- Status: accepted
- Date: 2026-03-18

## Context

Guild's inspect/runtime/resource substrate is now real enough that the operator-facing command language also needs to be real.

Before this ADR, the repository had:

- a real local install, inspect, read, export/import, and OCI transport substrate
- a real stdio MCP server with one public tool, `guild.inspect`
- repo examples and helper binaries that proved the substrate honestly

But the public command language was still split between conceptual verbs, proof examples, and helper-specific commands. That made the product story sharper than the operator workflow.

## Decision

Guild now ships one thin first-class local CLI binary: `guild`.

The stable v1 command surface is:

- `guild inspect`
- `guild read`
- `guild install`
- `guild export`
- `guild import`
- `guild push`
- `guild pull`
- `guild trust ...`
- `guild mcp serve --stdio`

The CLI is intentionally substrate-backed rather than a second runtime layer:

- `guild inspect` delegates to the same inspect path used by `guild.inspect`
- `guild read` delegates to the same resource backend used by MCP `resources/read` and guest `read-resource`
- install/export/import/push/pull delegate to the current registry and installer substrate
- `guild mcp serve --stdio` launches the current stdio MCP server without widening the MCP surface

Registry root selection stays explicit and local-first:

- `--registry-root <path>` wins
- otherwise `GUILD_REGISTRY_ROOT`
- otherwise the CLI fails with usage guidance

The CLI accepts canonical human-facing skill refs in the form:

- `skill://<namespace>/<name>@<version-or-range>`

For convenience, the CLI also accepts the bare alias form:

- `<namespace>/<name>@<version-or-range>`

## Consequences

Positive:

- Guild now has one real local command language
- README, example docs, Codex workflow docs, and future site snippets can point at honest commands
- Codex config can launch the `guild` binary directly while keeping `guild-codex` as a thin helper for bootstrap and deterministic scenario/smoke flows

Intentional non-decisions:

- `guild build` remains deferred because the current substrate does not yet define an honest standalone build artifact contract separate from install
- `guild deploy` remains deferred because Guild does not yet have one precise deployment target model
- this ADR does not add new capability families, new MCP tools, subscriptions, search, indexing, or a management console

## Guardrail

The `guild` CLI must remain a thin reflection of actual Guild substrate behavior.

Guild should not add headline verbs or workflows to the CLI unless the underlying runtime, registry, trust, and durability semantics already exist and can be documented honestly.
