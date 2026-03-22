# ADR 0019: Thin First-Class `guild` CLI

- Status: accepted
- Date: 2026-03-19

The public CLI remains an operator surface, not a separate normative runtime-contract source.

## Context

Guild's inspect/runtime/resource substrate is now real enough that the operator-facing command language also needs to be real.

Before this ADR, the repository had:

- a real local install, inspect, read, export/import, and OCI transport substrate
- a real stdio MCP server with one public tool, `guild.inspect`
- repo examples and helper binaries that proved the substrate honestly

But the public command language had drifted across conceptual verbs, proof examples, and helper-specific commands. The product story was sharper than the actual operator workflow.

This ADR originally made the command language real, substrate-backed, and explicit.

It is now also the place where Guild intentionally records the move from an explicit-only root posture to sane local operator defaults, because the CLI has become a real persistent local tool rather than a thin proof wrapper.

## Decision

Guild now ships one thin first-class local CLI binary: `guild`.

The normal operator install path is:

- `cargo install --path crates/guild-mcp --bin guild`

The stable v1 command surface is:

- `guild init`
- `guild show`
- `guild run`
- `guild ls`
- `guild get`
- `guild why`
- `guild verify`
- `guild install`
- `guild export`
- `guild import`
- `guild push`
- `guild pull`
- `guild trust ...`
- `guild codex ...`
- `guild mcp serve --stdio`

The canonical public URI families are:

- `skill://<namespace>/<name>@<version-or-range>` for executable skill refs
- `guild://...` for Guild-owned durable resources
- standard OCI references such as `<registry>/<repo>:<tag>` or `<registry>/<repo>@<digest>` for transport and publication artifacts

The CLI is intentionally substrate-backed rather than a second runtime layer:

- `guild show` is a non-executing summary surface over installed skills and stored Guild refs
- `guild run` delegates to the same inspect path used by `guild.inspect`
- `guild get` delegates to the same resource backend used by MCP `resources/read` and guest `read-resource`
- `guild why` reads one persisted execution record directly from host-owned durable state
- `guild verify` summarizes installed trust and verification state for skill refs only
- install/export/import/push/pull delegate to the current registry and installer substrate
- `guild init` creates the selected local root and may explicitly fold in local setup tasks such as Codex config writes without inventing a second state model
- `guild codex` delegates to the existing Codex bootstrap/config/scenario/smoke helpers without creating a second server model
- `guild mcp serve --stdio` launches the current stdio MCP server without widening the MCP surface

Legacy aliases remain supported for backward compatibility:

- `guild inspect` -> `guild run`
- `guild read` -> `guild get`
- `guild list` -> `guild ls`

Those aliases remain supported in the current CLI milestone and until a later explicit deprecation decision replaces them. They should be documented as legacy aliases rather than taught as the primary happy path.

Registry root selection is now local-first with one intentional default:

- `--registry-root <path>` wins
- otherwise `GUILD_REGISTRY_ROOT`
- otherwise Guild uses `~/.guild`
- there is no cwd-local `.guild/` default
- there is no `target/dev-local-registry/...` operator default
- read-only commands do not silently initialize a missing root
- write-oriented commands may create the selected root honestly when they are already performing real local mutation

This is an intentional change from the earlier explicit-only posture. Guild now has enough real local substrate that a stable home under `~/.guild` reduces friction without introducing cwd-based ambient state.

Canonical public-facing skill syntax uses:

- `skill://<namespace>/<name>@<version-or-range>`

For operator convenience, the CLI also accepts the bare alias form:

- `<namespace>/<name>@<version-or-range>`

When unambiguous across installed skills, the CLI also accepts:

- `<name>@<version-or-range>`

Docs, examples, and site snippets should prefer the canonical `skill://...` form rather than teaching the bare alias form as public syntax.

`guild trust ...` refers only to current local trust-store operations:

- generate local publisher identities
- add, list, and remove trusted local publisher records
- it does not imply remote trust distribution
- it does not imply transparency-log semantics
- it does not imply remote publisher policy management

`guild ls` is the thin local operator view for local state:

- `guild ls` shows installed skills plus recent persisted executions
- `guild ls skills` shows installed skills only
- `guild ls runs` shows recent persisted execution activity
- `guild ls evidence` shows stored evidence records
- `guild ls objects` shows stored content-addressed object records
- it does not imply a live loaded-runtime module registry or a broader search/indexing surface

`guild init` is the explicit local bootstrap workflow:

- it creates the selected Guild root, including the default `~/.guild` path when no override is present
- it prints the exact `guild mcp serve --stdio` launcher, `codex mcp add ...` command, and MCP config snippet for the running `guild` binary
- `--global` and `--project` may explicitly and idempotently update `~/.codex/config.toml` and/or `.codex/config.toml`
- it does not introduce cwd-local hidden state or silent Codex config edits

`guild codex` remains the deterministic dogfood helper surface:

- `bootstrap`, `print-config`, `scenario`, and `smoke` stay available for repo-local proofs and smoke flows
- they are not the normal persistent operator setup path

## Consequences

Positive:

- Guild now has one real local command language
- README, example docs, Codex workflow docs, and future site snippets can point at honest commands
- Guild has one predictable local home under `~/.guild` without introducing cwd-local hidden state
- Codex config can launch the real `guild` binary directly while `guild init` is the single current setup workflow and `guild codex` stays a deterministic proof/dogfood helper surface

Intentional non-decisions:

- `guild build` remains deferred because the current substrate does not yet define an honest standalone build artifact contract separate from install
- `guild deploy` remains deferred because Guild does not yet have one precise deployment target model
- this ADR does not add new capability families, new MCP tools, subscriptions, search, indexing, or a management console

## Guardrail

The `guild` CLI must remain a thin reflection of actual Guild substrate behavior.

Guild should not add headline verbs or workflows to the CLI unless the underlying runtime, registry, trust, and durability semantics already exist and can be documented honestly.

The CLI must not become an aspirational product shell. Headline verbs such as `guild deploy` stay out until Guild has a precise substrate-backed deployment model that can be named honestly.
