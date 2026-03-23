# Guild

Guild is a contracts-first Rust/Wasm runtime and control plane for portable AI skills.

It gives you a local CLI, a small MCP surface, and explicit host-owned execution, trust, and evidence records. The main path today is straightforward: install a skill, run it locally, inspect what happened, and move installed state through signed bundles or OCI transport.

> Status: pre-alpha.
>
> Use `guild` for local workflows, `guild mcp serve --stdio` for MCP integration, and the deeper docs for proof, benchmark, and contract details.

## Why Guild

Guild is strict about a few things on purpose:

- requested identity is not executable identity
- the host, not the guest, owns trust-sensitive authority
- evidence is a durable artifact, not a prompt scrap
- inspect, plan, and apply are distinct modes
- the MCP surface stays small and boring

The goal is not a loose agent wrapper. It is a portable skill system where execution, delegation, and witness claims stay tied to real runtime behavior.

## What Works Today

Guild already has:

- a real local `guild` CLI for install, run, show, list, read, explain, verify, trust, transport, and MCP setup
- a local registry root with durable execution and evidence records under `guild://...`
- signed bundle export and import with local trust verification
- OCI image layout and OCI registry transport for installed signed bundles
- a real stdio MCP server with one public tool, `guild.inspect`, plus Guild resources
- bounded live-proof coverage for specific `read-resource`, `http-request`, `invoke-skill`, and `log-write` slices
- a user-facing starter pack of example skills for compact ops analysis over stored executions, bounded query refs, and evidence refs

The live-proof envelope is intentionally narrow. The exact current status lives in `SPECS.md`, `docs/testing.md`, and `docs/schemas/draft-v1/family_support_matrix.json`.

## CLI

Install the operator CLI with:

```bash
cargo install --path crates/guild-mcp --bin guild
```

If you are working from the repository instead, use:

```bash
cargo run -q -p guild-mcp --bin guild -- ...
```

Top-level commands are grouped around daily use, distribution, and setup:

- daily use: `guild show`, `guild run`, `guild ls`, `guild get`, `guild why`, `guild verify`
- install and publish: `guild install`, `guild export`, `guild import`, `guild push`, `guild pull`, `guild trust ...`
- setup and integration: `guild init`, `guild mcp serve --stdio`, `guild codex ...`

Legacy aliases remain available for existing scripts:

- `guild inspect` -> `guild run`
- `guild read` -> `guild get`
- `guild list` -> `guild ls`

The CLI now also ships focused help topics:

- `guild help refs`
- `guild help trust`
- `guild help roots`

Version note: the current workspace Cargo packages, including the `guild` CLI crate, are `0.1.1`. The checked-in example Guild skill manifests still resolve as `0.1.0` / `@^0.1`, and the OCI transport examples intentionally keep those manifest-driven tags. Cargo package version and Guild skill identity are separate axes.

## Quickstart

Guild chooses a local root in this order:

- `--registry-root <path>`
- `GUILD_REGISTRY_ROOT`
- `~/.guild`

There is no cwd-local `.guild/` fallback. `guild init` is the explicit root-creation workflow, and read-only commands do not silently create a missing root.

### Install, Run, Inspect, Explain

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

What that flow shows:

- `install` builds source into installed executable state
- `show` is the primary non-executing summary path
- `run` executes a human-facing `skill://...` ref through the real Guild path
- `ls` shows installed skills and recent persisted activity
- successful runs return a durable `guild://executions/...` receipt
- `why` explains a persisted execution record
- `get` reads the same resource backend used by MCP and guest `read-resource`
- `verify` reports installed trust and verification state for skill refs only

`guild run` keeps the payload on stdout and writes the human execution summary to stderr. `guild get` stays the raw resource-read path and supports `--json`, `--porcelain`, and `--output <path>` when you want machine-stable reads instead of styled summaries.

If you want an explicit non-default root for local proofs or CI, keep passing it:

```bash
export GUILD_REGISTRY_ROOT=target/dev-local-registry/hello
cargo run -q -p guild-mcp --bin guild -- install examples/skills/hello-inspect
```

## Ops Starter Pack

The current user-facing skill pack lives at [`examples/skills/guild-ops-starter/README.md`](examples/skills/guild-ops-starter/README.md).

It is intentionally ordinary example-skill layout, not a new packaging system. The pack installs as five example skills and stays inside current honest repo truth:

- `incident-brief` for one stored execution ref
- `run-diff` for two stored execution refs
- `recent-failures` for one bounded execution-query ref
- `evidence-summary` for one stored evidence ref
- `render-report` as the zero-authority child formatter used by the parent report skills

The pack is meant to show the current Guild story without broadening runtime or proof semantics: durable refs, compact terminal output, explicit capability requirements, and bounded single-child composition only where it is already real.

## Trust And Transport

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

That flow demonstrates the current trust model:

- export and import operate on installed signed bundle semantics, not source directories
- `guild trust ...` manages local trust-store state only
- OCI transport carries the same installed signed bundle contract through another transport shape

## MCP And Codex

Guild ships a real stdio MCP server through the same CLI:

```bash
guild mcp serve --stdio
```

The public MCP surface is intentionally small:

- one public tool: `guild.inspect`
- Guild execution, evidence, object, and bounded query resources through `resources/read`
- cursor-based pagination on `tools/list`, `resources/list`, and `resources/templates/list`

For persistent Codex integration, use the explicit setup workflow:

```bash
guild init
guild init --global
```

`guild init` creates the selected Guild root, prints the exact `guild mcp serve --stdio` wiring for the running `guild` binary, and can explicitly update global or project Codex config files with `--global` or `--project`.

For deterministic repo-local scenarios and smoke flows from this repository, Guild also keeps the `guild codex` helper surface:

```bash
cargo run -p guild-mcp --bin guild -- codex bootstrap --registry-root target/dev-local-registry/codex-local --reset
cargo run -p guild-mcp --bin guild -- codex print-config --registry-root target/dev-local-registry/codex-local
```

`guild codex` is not the normal setup path. It is the deterministic repo-local helper surface for bootstrap, scenario preparation, and smoke coverage.

## Status

Guild still tracks work in milestone labels, but the practical summary is:

- M3 and M4 are complete as the draft-v1 contract and admission bundle under `docs/schemas/draft-v1/`
- M5, M6, and M7 are complete as bounded draft-v1 minimization, token, and witness flows
- M8a and M8b are complete for the active live runtime vocabulary and canonical family mapping
- M8c is partial and intentionally narrow; the exact supported live-proof slices are documented in `docs/testing.md`
- M8-proper is complete as the checked real-path benchmark under `docs/schemas/draft-v1/benchmark_matrix.json` and `docs/benchmarking/m8-real-path-benchmark.md`
- M9 is complete as the measured patent packet under `docs/patent/`
- M10 is not started

If you need the full milestone-by-milestone detail, start with `docs/roadmap.md`, `docs/testing.md`, and `docs/schemas/draft-v1/README.md`.

## Canonical Docs

- `docs/command-language.md` - public CLI verbs, grouped workflows, and ref grammar
- `docs/testing.md` - verification commands, proof workflows, and smoke paths
- `SPECS.md` - normative contract and conformance language
- `ARCHITECTURE.md` - practical system view and trust boundaries
- `docs/adr/README.md` - decision log and ADR backlog
- `AGENTS.md` - contributor guardrails for contract-first changes
- `docs/roadmap.md` - ordered epics and build priorities

Compatibility wrappers remain at `docs/contracts.md` and `docs/architecture.md` so existing links keep working.
