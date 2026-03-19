# Command Language

Guild now has one real local command surface: `guild`.

If you are using a built binary, run `guild ...`.
If you are working from the repository, use the repo-local wrapper:

```bash
cargo run -q -p guild-mcp --bin guild -- ...
```

This document is the source of truth for Guild's public command and URI grammar. The repository does not currently include landing-page source, so the terminal snippets here are also the canonical in-repo hero examples for future site work.

## Canonical Verbs

Guild's first-class local verbs are:

- `guild inspect`
- `guild read`
- `guild list`
- `guild install`
- `guild export`
- `guild import`
- `guild push`
- `guild pull`
- `guild trust ...`
- `guild codex ...`
- `guild mcp serve --stdio`

Intentionally deferred in this milestone:

- `guild build`
  There is not yet a separate standalone build artifact contract outside source install, so `build` stays deferred instead of becoming a second name for `install`.
- `guild deploy`
  Guild does not yet have one honest deployment target model, so `deploy` stays out of the public CLI.

## URI Families

Guild uses three distinct public identifier families:

- `skill://<namespace>/<name>@<version-or-range>`
  Human-facing executable skill references. The runtime still resolves these to exact installed `ResolvedSkillRef` values before execution.
- `guild://executions/<id>`
  Durable execution records.
- `guild://objects/records/<id>`
  Evidence-record payload dereference.
- `guild://objects/records/<id>/metadata`
  Host-owned evidence-record metadata.
- `guild://objects/sha256/<digest>`
  Raw content-addressed blobs.
- `guild://queries/...`
  Bounded query resources over durable Guild state.
- `<registry>/<repo>:<tag>` or `<registry>/<repo>@<digest>`
  Standard OCI registry references for transport and publication.

Guild intentionally does not use `guild://` for transport publication. Installed transport units move through signed bundle directories, OCI image layouts, and OCI registry references.

## Skill Ref Syntax

Canonical public skill syntax uses:

- `skill://<namespace>/<name>@<version-or-range>`

The CLI also accepts bare `<namespace>/<name>@<version-or-range>` as operator convenience syntax, but docs, examples, and future site snippets should prefer the canonical `skill://...` form.

## Registry Roots

Registry root selection is explicit:

- there is no implicit `.guild/` root
- there is no implicit `target/dev-local-registry/...` root
- `--registry-root <path>` wins
- otherwise `GUILD_REGISTRY_ROOT`
- otherwise the CLI fails with usage guidance

## Trust Scope

`guild trust ...` manages the current local trust store only:

- generate local publisher identities
- add, list, and remove local trusted publisher records
- no remote trust distribution
- no transparency-log semantics
- no remote publisher policy management

## List

`guild list` is the thin local summary view for state already owned by the registry:

- `guild list`
  Shows installed skills plus recent persisted executions.
- `guild list skills`
  Shows installed skills only.
- `guild list executions --limit 20`
  Shows recent persisted execution activity only.

Guild does not currently expose a live loaded-runtime module registry. The honest current answer to "what is loaded here?" is recent persisted execution activity.

## Hero Flows

Happy path:

```bash
export GUILD_REGISTRY_ROOT=target/dev-local-registry/hero
cargo run -q -p guild-mcp --bin guild -- install examples/skills/hello-inspect
cargo run -q -p guild-mcp --bin guild -- inspect \
  skill://example/hello-inspect@^0.1 \
  --input-json '{"name":"Ada"}' \
  --grants-json '{"grants":[{"id":"emit-evidence","access":"write","constraints":{"max_bytes":65536,"audiences":["user"],"redactions":["none"]}}]}' \
  --json
cargo run -q -p guild-mcp --bin guild -- read guild://executions/<execution-id>
```

What this teaches:

- install is source-to-installed, not source-to-runtime bypass
- inspect executes a `skill://...` ref, not an ambient tool name
- success produces a durable `guild://executions/...` receipt
- read goes back through the same Guild resource backend used by MCP and guest `read-resource`

Host-owned denial:

```bash
export GUILD_REGISTRY_ROOT=target/dev-local-registry/hero
cargo run -q -p guild-mcp --bin guild -- inspect \
  skill://example/hello-inspect@^0.1 \
  --input-json '{"name":"Ada"}'
```

Expected shape:

- the command fails closed because `hello-inspect` requires `emit-evidence`
- stderr includes the persisted `guild://executions/<id>` receipt for the rejected execution
- the denial stays host-owned and explainable after the fact

Transport:

```bash
cargo run -q -p guild-mcp --bin guild -- trust generate \
  --publisher-id local.example \
  --display-name "Local Example" \
  --output target/dev-local-registry/local.example.json

cargo run -q -p guild-mcp --bin guild -- --registry-root target/dev-local-registry/a export bundle \
  skill://example/hello-inspect@^0.1 \
  --signer target/dev-local-registry/local.example.json \
  --output target/dev-local-registry/bundle

cargo run -q -p guild-mcp --bin guild -- --registry-root target/dev-local-registry/b trust add \
  --identity-file target/dev-local-registry/local.example.json

cargo run -q -p guild-mcp --bin guild -- --registry-root target/dev-local-registry/b import bundle \
  target/dev-local-registry/bundle
```

What this teaches:

- export/import operate on installed signed bundle semantics, not source directories
- `guild trust ...` is explicit local trust-store management, not remote trust distribution
- OCI transport is the same installed signed bundle contract carried through another shape, not a second artifact model

## Mapping To Today's Substrate

The CLI is intentionally thin:

- `guild inspect` uses the same `GuildMcpFacade::inspect` path used by `guild.inspect`
- `guild read` uses the same local resource backend used by MCP `resources/read` and guest `read-resource`
- `guild list` uses the local registry's installed-skill view plus recent persisted execution records
- `guild install` uses `LocalSourceInstaller`
- `guild export` and `guild push` export signed installed state from `LocalRegistry`
- `guild import` and `guild pull` re-run the existing trust, signature, and install checks
- `guild mcp serve --stdio` launches the existing stdio MCP server without widening the public MCP tool surface

## Codex

Codex setup uses the same command language.

- `guild codex` is the primary helper surface for bootstrap, scenario prep, and smoke flows.
- `guild-codex` remains available as a compatibility wrapper for existing scripts.
- The real local server launch it prints is now `guild mcp serve --stdio`.
- The printed Codex config runs the `guild` binary, not a second public server dialect.
