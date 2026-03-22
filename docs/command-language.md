# Command Language

Guild now has one real local command surface: `guild`.

Install it as the operator entrypoint with:

```bash
cargo install --path crates/guild-mcp --bin guild
```

If you are using a built binary, run `guild ...`.
If you are working from the repository, use the repo-local wrapper:

```bash
cargo run -q -p guild-mcp --bin guild -- ...
```

This document is the source of truth for Guild's public command and URI grammar. The repository does not currently include landing-page source, so the terminal snippets here are also the canonical in-repo hero examples for future site work.

## Canonical Verbs

Guild's first-class local verbs are:

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

Legacy aliases remain supported in this milestone for compatibility:

- `guild inspect` -> `guild run`
- `guild read` -> `guild get`
- `guild list` -> `guild ls`

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

When unambiguous across installed skills, the CLI also accepts short `<name>@<version-or-range>` refs such as `hello-inspect@^0.1`.

Version note: the current workspace Cargo packages are `0.1.1`, but the checked-in example Guild skill manifests still use `0.1.0`, so example `skill://...@^0.1` refs and `...:0.1.0` OCI tags remain the honest values. Guild resolves requested and transported skill identity from the Guild manifest contract, not from the Cargo package version of the CLI or guest implementation crate.

## Registry Roots

Registry root selection is local-first and overrideable:

- `--registry-root <path>` wins
- otherwise `GUILD_REGISTRY_ROOT`
- otherwise Guild uses `~/.guild`
- there is no cwd-local `.guild/` fallback
- there is no `target/dev-local-registry/...` operator default
- `guild init` is the explicit root-creation workflow
- read-only commands do not initialize a missing root
- write-oriented commands may create the selected root honestly

## Output Contract

Default human output is compact and status-forward:

- one-screen by default for common `show`, `run`, `why`, and `ls` cases
- short refs and short ids by default rather than full digest and URI dumps
- stable vocabulary across commands:
  - `proof-backed`
  - `upper-bound`
  - `linked`
  - `unlinked`
  - `bounded`
  - `not_proven`
  - `refused`

Shared output controls for the human-summary commands (`show`, `run`, `ls`, `why`, and `verify`):

- `--json` for structured machine-readable output
- `--porcelain` for stable one-line machine-readable output
- `-v` for important ids, digests, and source details
- `-vv` for deeper technical detail
- `--debug` for full internal detail
- `--color auto|always|never`
- `NO_COLOR` disables ANSI color even when the terminal would otherwise allow it

`guild get` stays a raw resource-read path rather than a styled summary view. It supports `--json`, `--porcelain`, and `--output <path>`.

Color is semantic only and never the only signal:

- green: success, verified, proven
- yellow: bounded, fallback, partial
- red: refused, invalid, unsupported
- cyan: refs and ids
- magenta: runtime and type
- dim: metadata

`guild run` keeps payload and human status separate:

- stdout carries the payload or structured result
- stderr carries the human execution summary

## Trust Scope

`guild trust ...` uses the current local trust model only:

- generate local publisher identities
- add, list, and remove local trusted publisher records
- sign and verify execution plans against that same local publisher / trust-store model
- no remote trust distribution
- no transparency-log semantics
- no remote publisher policy management

## Primary Views

`guild show` is the primary non-executing inspection view:

- `guild show skill://example/hello-inspect@^0.1`
- `guild show hello-inspect@^0.1`
- `guild show exec:<id-prefix>`
- `guild show evidence:<id-prefix>`
- `guild show obj:<sha-prefix>`

`guild ls` is the thin local summary view for state already owned by the registry:

- `guild ls`
  Shows a compact summary of installed skills plus recent persisted executions.
- `guild ls skills`
  Shows installed skills only.
- `guild ls runs --limit 20`
  Shows recent persisted execution activity only.
- `guild ls evidence --limit 20`
  Shows stored evidence records only.
- `guild ls objects --limit 20`
  Shows stored content-addressed objects only.

Guild does not currently expose a live loaded-runtime module registry. The honest current answer to "what is loaded here?" is installed executable state plus durable records that already exist.

`guild get` is the machine-facing resource read path:

- `guild get guild://executions/<id>`
- `guild get exec:<id-prefix>`
- `guild get evidence:<id-prefix>`
- `guild get obj:<sha-prefix>`

`guild why` is the persisted execution explanation path:

- `guild why exec:<id-prefix>`
- `guild why guild://executions/<id>`

`guild verify` stays intentionally narrow:

- `guild verify skill://example/hello-inspect@^0.1`

Signed-plan verification remains under `guild trust verify-plan`.

## Hero Flows

Happy path:

```bash
guild init
guild install examples/skills/hello-inspect
guild show skill://example/hello-inspect@^0.1
guild run \
  skill://example/hello-inspect@^0.1 \
  --input-json '{"name":"Ada"}' \
  --grants-json '{"grants":[{"id":"emit-evidence","access":"write","constraints":{"max_bytes":65536,"audiences":["user"],"redactions":["none"]}}]}' \
  --json
guild ls runs --limit 5
guild why exec:<execution-id-prefix>
guild get guild://executions/<execution-id>
guild verify skill://example/hello-inspect@^0.1
```

What this teaches:

- install is source-to-installed, not source-to-runtime bypass
- show is the primary non-executing summary path for installed skills and stored Guild refs
- run executes a `skill://...` ref, not an ambient tool name
- success produces a durable `guild://executions/...` receipt
- why explains one stored execution record through host-owned durable state
- get goes back through the same Guild resource backend used by MCP and guest `read-resource`
- verify reports installed trust and verification state for skill refs only

For deterministic local proofs and CI, continue to pass an explicit temp or target root:

```bash
export GUILD_REGISTRY_ROOT=target/dev-local-registry/hero
cargo run -q -p guild-mcp --bin guild -- install examples/skills/hello-inspect
```

Host-owned denial:

```bash
export GUILD_REGISTRY_ROOT=target/dev-local-registry/hero
cargo run -q -p guild-mcp --bin guild -- run \
  skill://example/hello-inspect@^0.1 \
  --input-json '{"name":"Ada"}'
```

Legacy alias form:

```bash
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

Execution-plan signing:

```bash
cargo run -q -p guild-mcp --bin guild -- trust sign-plan \
  --plan docs/schemas/draft-v1/examples/zero-authority.admit.plan.json \
  --identity-file target/dev-local-registry/local.example.json \
  --output target/dev-local-registry/zero-authority.admit.signed.plan.json

cargo run -q -p guild-mcp --bin guild -- --registry-root target/dev-local-registry/b trust verify-plan \
  --plan target/dev-local-registry/zero-authority.admit.signed.plan.json
```

What this teaches:

- M4 execution plans are still generated unsigned by default
- plan signing reuses the same publisher identity and trusted-publisher model as bundle signing
- verification is fail-closed against the local Guild trust store

## Mapping To Today's Substrate

The CLI is intentionally thin:

- `guild init` creates the selected local registry layout and may explicitly fold in Codex config writes against the running `guild` binary
- `guild show` summarizes installed skills and stored Guild refs without creating a second inspection substrate
- `guild run` uses the same `GuildMcpFacade::inspect` path used by `guild.inspect`
- `guild inspect` remains a legacy alias for that same execution path
- `guild get` uses the same local resource backend used by MCP `resources/read` and guest `read-resource`
- `guild read` remains a legacy alias for that same resource path
- `guild ls` uses the local registry's installed-skill view plus persisted execution/evidence/object records
- `guild list` remains a legacy alias for that same listing path
- `guild why` reads one persisted execution record directly from durable host-owned data
- `guild verify` summarizes installed verification and trust state for skill refs only
- `guild install` uses `LocalSourceInstaller`
- `guild export` and `guild push` export signed installed state from `LocalRegistry`
- `guild import` and `guild pull` re-run the existing trust, signature, and install checks
- `guild mcp serve --stdio` launches the existing stdio MCP server without widening the public MCP tool surface

## Codex

Codex setup and dogfood helpers use the same command language.

- `guild init` is the one current operator setup path.
- `guild init` creates the resolved Guild root and prints the `guild mcp serve --stdio` launcher, the `codex mcp add ...` command, and the matching config snippet for the running `guild` binary.
- `guild init --global` updates `~/.codex/config.toml` explicitly and idempotently.
- `guild init --project` updates `.codex/config.toml` explicitly and idempotently.
- `guild codex` is the deterministic repo-local dogfood and smoke surface for `bootstrap`, Cargo-based `print-config`, scenario prep, and smoke flows.
- The real local server launch stays `guild mcp serve --stdio`.
- Persistent Codex config now points at the running `guild` binary path rather than at a repo-local `cargo run` launcher.
- The repo-local `guild codex print-config` helper remains intentionally available for deterministic in-repo dogfood flows and continues to print the Cargo-based launcher with an explicit registry root.
