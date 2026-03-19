# ADR 0019: Thin First-Class `guild` CLI

- Status: accepted
- Date: 2026-03-18

## Context

Guild's inspect/runtime/resource substrate is now real enough that the operator-facing command language also needs to be real.

Before this ADR, the repository had:

- a real local install, inspect, read, export/import, and OCI transport substrate
- a real stdio MCP server with one public tool, `guild.inspect`
- repo examples and helper binaries that proved the substrate honestly

But the public command language had drifted across conceptual verbs, proof examples, and helper-specific commands. The product story was sharper than the actual operator workflow.

This ADR exists to make the command language real, substrate-backed, and explicit.

## Decision

Guild now ships one thin first-class local CLI binary: `guild`.

The stable v1 command surface is:

- `guild inspect`
- `guild read`
- `guild list`
- `guild install`
- `guild export`
- `guild import`
- `guild push`
- `guild pull`
- `guild trust ...`
- `guild mcp serve --stdio`

The canonical public URI families are:

- `skill://<namespace>/<name>@<version-or-range>` for executable skill refs
- `guild://...` for Guild-owned durable resources
- standard OCI references such as `<registry>/<repo>:<tag>` or `<registry>/<repo>@<digest>` for transport and publication artifacts

The CLI is intentionally substrate-backed rather than a second runtime layer:

- `guild inspect` delegates to the same inspect path used by `guild.inspect`
- `guild read` delegates to the same resource backend used by MCP `resources/read` and guest `read-resource`
- install/export/import/push/pull delegate to the current registry and installer substrate
- `guild mcp serve --stdio` launches the current stdio MCP server without widening the MCP surface

Registry root selection stays explicit and local-first:

- there is no implicit `.guild/` root
- there is no implicit `target/dev-local-registry/...` root
- `--registry-root <path>` wins
- otherwise `GUILD_REGISTRY_ROOT`
- otherwise the CLI fails with usage guidance

Canonical public-facing skill syntax uses:

- `skill://<namespace>/<name>@<version-or-range>`

For operator convenience, the CLI also accepts the bare alias form:

- `<namespace>/<name>@<version-or-range>`

Docs, examples, and site snippets should prefer the canonical `skill://...` form rather than teaching the bare alias form as public syntax.

`guild trust ...` refers only to current local trust-store operations:

- generate local publisher identities
- add, list, and remove trusted local publisher records
- it does not imply remote trust distribution
- it does not imply transparency-log semantics
- it does not imply remote publisher policy management

`guild list` is the thin local operator view for local state:

- `guild list` shows installed skills plus recent persisted executions
- `guild list skills` shows installed skills only
- `guild list executions` shows recent persisted execution activity
- it does not imply a live loaded-runtime module registry or a broader search/indexing surface

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

The CLI must not become an aspirational product shell. Headline verbs such as `guild deploy` stay out until Guild has a precise substrate-backed deployment model that can be named honestly.
