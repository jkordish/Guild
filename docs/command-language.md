# Command Language

This document is the source of truth for Guild's public CLI verbs, grouped workflows, and ref grammar.

It is not the runtime-contract source of truth. For that, use `SPECS.md` section "Source Of Truth". For the frozen runtime URI roots and support vocabulary in the current release line, use `SPECS.md` section "Contract Surface v1 (core)".

## Install And Run

Install the operator CLI with:

```bash
cargo install --path crates/guild-mcp --bin guild
```

After install, the command language uses `guild ...` directly.
Repo-local proof commands and lower-level developer helpers live in
`docs/testing.md`.
If you want the short daily-user mental model first, read
[`docs/how-guild-works.md`](how-guild-works.md).

The default help is task-oriented:

- `guild --help`
- `guild help refs`
- `guild help trust`
- `guild help roots`
- `guild <command> --help`

## Command Groups

Guild's first-class local verbs are:

### Daily Use

- `guild show`
- `guild run`
- `guild ls`
- `guild get`
- `guild why`
- `guild verify`

### Install And Publish

- `guild install`
- `guild export`
- `guild import`
- `guild push`
- `guild pull`
- `guild trust ...`

### Setup And Integration

- `guild init`
- `guild codex ...`
- `guild mcp serve --stdio`

Legacy aliases remain supported for compatibility:

- `guild inspect` -> `guild run`
- `guild read` -> `guild get`
- `guild list` -> `guild ls`

## Ref Forms

### Skill Refs

Canonical public skill syntax is:

- `skill://<namespace>/<name>@<version-or-range>`

The CLI also accepts:

- `<namespace>/<name>@<version-or-range>`
- `<name>@<version-or-range>` when unambiguous across installed skills

Docs and scripts should prefer the canonical `skill://...` form.

### Guild Resource Refs

Guild uses these public resource families:

- `guild://executions/<id>`
- `guild://objects/records/<id>`
- `guild://objects/records/<id>/metadata`
- `guild://objects/sha256/<digest>`
- `guild://queries/executions/...`

The CLI also accepts short resource refs:

- `exec:<execution-id-prefix>`
- `evidence:<evidence-record-id-prefix>`
- `obj:<sha256-prefix>`

### OCI Refs

Transport and publication use standard OCI references:

- `<registry>/<repo>:<tag>`
- `<registry>/<repo>@<digest>`

Guild intentionally does not use `guild://` for transport publication. Installed transport units move through signed bundle directories, OCI image layouts, and OCI registry references.

## Identity Layers

Guild uses three identity layers in day-to-day CLI flows:

- source skill: the local source directory passed to `guild install`
- installed executable state: the installed record stored under the selected Guild root
- resolved executable identity: the exact installed executable selected for use, identified by resolved ref plus artifact digest

The fastest way to trace those layers for one skill is:

```bash
guild show -v skill://example/hello-inspect@^0.1
```

That verbose view shows the requested ref, resolved ref, digest, and installed path together. Use it when you need to answer "what did I ask for?" versus "what exact executable did Guild select?"

## Authority Lifecycle

Guild uses one authority lifecycle in day-to-day CLI flows:

- declared authority: capabilities declared by the installed manifest and visible in `guild show`
- requested authority: caller-requested grants passed to `guild run`
- granted authority: the final capability slice the host policy allows for that run
- effective at runtime: the authority the guest can actually exercise during execution

In other words:

- manifests declare the capability envelope
- callers request a narrower or matching slice for one run
- host policy may grant, reduce, or deny that request before guest start
- the guest only sees the final granted set at runtime

## Root Resolution

Guild chooses a root in this order:

1. `--registry-root <path>`
2. `GUILD_REGISTRY_ROOT`
3. `~/.guild`

There is no cwd-local `.guild/` fallback.

`guild init` is the explicit root-creation workflow. Read-only commands do not initialize a missing root. Write-oriented commands may create the selected root when they are already doing real work.

## Primary Workflows

### Quickstart

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

What this flow teaches:

- `install` is source-to-installed, not source-to-runtime bypass
- `show` is the primary non-executing summary path
- `show -v` traces requested ref -> resolved ref -> resolved digest -> installed path
- `run` executes a `skill://...` ref through the real Guild runtime path after host policy computes the final granted authority for that run
- success produces a durable `guild://executions/...` receipt
- `why` explains one stored execution record
- `get` reads the same backend used by MCP and guest `read-resource`
- `verify` reports installed trust and verification state for skill refs only

### Ops Starter Pack

The current user-facing example pack lives at
[`examples/skills/guild-ops-starter/README.md`](../examples/skills/guild-ops-starter/README.md).
The surrounding examples index lives at
[`examples/README.md`](../examples/README.md).

It is still just example skills under `examples/skills/`, not a new pack system.
Use it when you want a compact real-path walkthrough over persisted Guild refs:

- `incident-brief` for one stored execution ref
- `run-diff` for two stored execution refs
- `recent-failures` for one bounded execution-query ref
- `evidence-summary` for one stored evidence ref
- `render-report` as the zero-authority child formatter used by the report skills

The honest story is narrow on purpose:

- durable Guild refs and resources
- compact markdown output on `guild run`
- explicit capability grants
- exact single-child zero-authority composition only where the runtime already supports it
- no broad HTTP showcase
- no `emit-evidence` proof claims

### Trust And Transport

```bash
guild trust generate \
  --publisher-id local.example \
  --display-name "Local Example" \
  --output target/dev-local-registry/local.example.json

guild --registry-root target/dev-local-registry/a export bundle \
  skill://example/hello-inspect@^0.1 \
  --signer target/dev-local-registry/local.example.json \
  --output target/dev-local-registry/bundle

guild --registry-root target/dev-local-registry/b trust add \
  --identity-file target/dev-local-registry/local.example.json

guild --registry-root target/dev-local-registry/b import bundle \
  target/dev-local-registry/bundle
```

This flow stays within the current trust model:

- export and import operate on installed signed bundle semantics
- `guild trust ...` manages local trust-store state only
- OCI transport uses the same installed signed bundle contract through another transport shape

### Execution Plan Signing

```bash
guild trust sign-plan \
  --plan docs/schemas/draft-v1/examples/zero-authority.admit.plan.json \
  --identity-file target/dev-local-registry/local.example.json \
  --output target/dev-local-registry/zero-authority.admit.signed.plan.json

guild --registry-root target/dev-local-registry/b trust verify-plan \
  --plan target/dev-local-registry/zero-authority.admit.signed.plan.json
```

What this flow teaches:

- M4 execution plans are still unsigned by default
- plan signing reuses the same local publisher identity and trust-store model as bundle signing
- verification is fail-closed against the local Guild trust store

## Output Contract

Default human output is compact and status-forward:

- one screen by default for common `show`, `run`, `why`, and `ls` cases
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
- `-v` for important ids, digests, and installed-state details
- `-vv` for deeper technical detail
- `--debug` for full internal detail
- `--color auto|always|never`
- `NO_COLOR` disables ANSI color even when the terminal would otherwise allow it

`guild get` stays a raw resource-read path rather than a styled summary view. It supports `--json`, `--porcelain`, and `--output <path>`.

`guild run` keeps payload and human status separate:

- stdout carries the payload or structured result
- stderr carries the human execution summary

## Trust Scope

`guild trust ...` uses the current local trust model only:

- generate local publisher identities
- add, list, and remove local trusted publisher records
- sign and verify execution plans against that same local publisher and trust-store model
- no remote trust distribution
- no transparency-log semantics
- no remote publisher policy management

## Codex And MCP

Guild ships a real stdio MCP server through the same CLI:

```bash
guild mcp serve --stdio
```

The public MCP surface is intentionally small:

- one public tool: `guild.inspect`
- Guild execution, evidence, object, and bounded query resources through `resources/read`
- cursor-based pagination on `tools/list`, `resources/list`, and `resources/templates/list`

For persistent Codex integration, use:

```bash
guild init
guild init --global
```

`guild init` creates the selected Guild root, prints the exact `guild mcp serve --stdio` launch command, and can explicitly update global or project Codex config.

For deterministic repo-local scenario prep and smoke coverage from this repository, Guild also ships:

- `guild codex bootstrap`
- `guild codex print-config`
- `guild codex scenario`
- `guild codex smoke`

Those helpers are for repo-local deterministic flows. The normal setup path remains `guild init`.
