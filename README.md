# Guild

**Guild** is a Rust-first, WASM-native registry and runtime for portable AI skills.

Guild sits one layer above raw MCP servers. MCP gives agents a way to discover and call tools. Guild packages operational know-how as versioned, capability-scoped, portable skills that can be resolved, executed, inspected, and shared without giving guests ambient authority.

> Status: pre-alpha. Current milestone status is explicit: M3 and M4 are implemented as the draft-v1 schema and admission bundle under `docs/schemas/draft-v1/`; M5 and M6 are implemented there as bounded draft-v1 proof and token paths; M7 is complete as the bounded draft-v1 witness layer; M8a is complete as the live-runtime vocabulary and observation-alignment bridge; M9 and M10 have not started. The live Rust runtime surface is canonical. The draft bundle remains draft.

## Why Guild

Guild is opinionated about a few things:

- requested identity is not executable identity
- the host, not the guest, owns trust-sensitive authority
- evidence is a durable artifact, not a prompt scrap
- inspect, plan, and apply are distinct modes
- the MCP surface should stay small and boring

The goal is a platform for portable, auditable, reusable skills, not a pile of tool wrappers glued together by vibes.

## Real CLI

Guild now has one real first-class local CLI: `guild`.

Install it as the normal operator entrypoint with:

```bash
cargo install --path crates/guild-mcp --bin guild
```

If you are working from the repository instead, use the repo-local wrapper:

```bash
cargo run -q -p guild-mcp --bin guild -- ...
```

The current local command surface is:

- `guild init`
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

Intentionally deferred:

- `guild build`
- `guild deploy`

The canonical command and URI grammar lives at [`docs/command-language.md`](docs/command-language.md).
Public docs prefer canonical `skill://...` refs. The CLI also accepts bare `<namespace>/<name>@<version-or-range>` as convenience syntax for operators.

## Quickstart

Guild now has one sane local root rule for the operator-facing CLI:

- `--registry-root <path>` wins
- otherwise `GUILD_REGISTRY_ROOT`
- otherwise Guild uses `~/.guild`

Guild does not create a cwd-local `.guild/` directory. Read-only commands fail clearly if the selected root does not exist yet. `guild init` is the explicit way to create the selected root up front, and write-oriented commands such as `install`, `inspect`, `import`, `pull`, and the setup/bootstrap helpers can also create the selected root honestly when they are already doing real work.

### Install, List, Inspect, Read

```bash
guild init

guild install examples/skills/hello-inspect

guild list

guild inspect \
  skill://example/hello-inspect@^0.1 \
  --input-json '{"name":"Ada"}' \
  --grants-json '{"grants":[{"id":"emit-evidence","access":"write","constraints":{"max_bytes":65536,"audiences":["user"],"redactions":["none"]}}]}' \
  --json

guild list executions --limit 5

guild read guild://executions/<execution-id>
```

What that flow shows:

- `install` builds source into installed executable state
- `list` shows what is installed here and what has run recently, without pretending Guild already has a live loaded-module registry
- `inspect` executes a human-facing `skill://...` ref through the real Guild path
- success returns a durable `guild://executions/...` receipt
- `read` goes back through the same resource backend used by MCP and guest `read-resource`

If you want an explicit non-default root for local proofs or CI, keep passing it:

```bash
export GUILD_REGISTRY_ROOT=target/dev-local-registry/hello
cargo run -q -p guild-mcp --bin guild -- install examples/skills/hello-inspect
```

### Trust And Transport

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

That flow stays honest to the substrate:

- export/import operate on installed signed bundle semantics, not source directories
- `guild trust ...` is explicit local trust-store management only
- OCI transport uses the same installed signed bundle contract carried through another transport shape

## MCP And Codex

Guild ships a real stdio MCP server through the same CLI:

```bash
guild mcp serve --stdio
```

The public MCP surface is intentionally small:

- one public tool: `guild.inspect`
- Guild execution, evidence, object, and bounded query resources through `resources/read`
- cursor-based pagination on `tools/list`, `resources/list`, and `resources/templates/list`

MCP protocol hygiene in the current milestone stays honest to the real runtime:

- `guild.inspect` is annotated as not read-only and not idempotent because inspect execution persists durable execution records and may persist evidence records
- `guild.inspect` is annotated as open-world because the active inspect slice includes bounded outbound `http-request`
- `resources/list` remains a bounded recent-execution view over durable records rather than a general search/index surface
- stdio is the only MCP transport in this milestone; subscriptions, list-changed notifications, HTTP transport, and more public tools remain intentionally deferred

For persistent Codex integration, use the explicit setup workflow:

```bash
guild init
guild init --global
```

`guild init` is now the one current local setup path. It creates the resolved Guild root, prints the exact `guild mcp serve --stdio` wiring for Codex, and `guild init --global` or `guild init --project` explicitly write the matching Codex config. The operator path no longer depends on a separate `guild codex setup` command.

For deterministic local dogfood from the repository, Guild still keeps the existing helper path:

```bash
cargo run -p guild-mcp --bin guild -- codex bootstrap --registry-root target/dev-local-registry/codex-local --reset
cargo run -p guild-mcp --bin guild -- codex print-config --registry-root target/dev-local-registry/codex-local
```

`guild codex` is now the deterministic repo-local dogfood and smoke surface: bootstrap, Cargo-based `print-config`, scenario prep, and smoke flows.

## Milestone Status

- `M3 Define the schemas`: implemented as the draft-v1 schema bundle under `docs/schemas/draft-v1/`, with checked examples and deterministic validation.
- `M4 Build the admission engine`: implemented in that same draft-v1 bundle, producing fail-closed `admit`, `downgrade`, `migrate`, and `refuse` execution plans.
- `M5 Build the counterfactual authority minimizer`: implemented as a bounded draft-v1 minimization path with explicit statuses such as `exact_minimal`, `bounded_minimal`, `reduced`, `no_reduction`, and `not_proven`.
- `M6 Build delegation-chain-bound capability tokens`: implemented as a bounded draft-v1 token path with proof-backed issuance by default, explicit `m4_upper_bound` issuance, and fail-closed verification.
- `M7 Build the bounded draft-v1 witness layer`: complete for the bounded draft-v1 witness path, including exercised authority, blocked attempts, coverage semantics, redaction semantics, and fixed claim checks.
- `M8a Runtime Alignment and Canonical Effect Vocabulary`: complete as the bridge from draft harness truth to live runtime truth. The live Rust capability-family surface is canonical, the runtime persists durable `authority_observations`, and draft-v1 now maps live runtime data through explicit `exact`, `narrowing`, `partial`, and `unsupported` outcomes.
- `M9 Draft the patent packet`: not started.
- `M10 Filing hygiene`: not started.

## What Is Real Today

The current runtime and transport slice already has:

- source-to-installed lifecycle with atomic local installs
- `RequestedSkillRef -> ResolvedSkillRef` execution boundaries
- real Wasmtime-backed Wasm component execution
- inspect-only primitive and composite skills
- alias-scoped child dependency invocation
- durable host-owned execution and evidence records under `guild://...`
- durable live-runtime `authority_observations` for `http-request`, `read-resource`, `invoke-skill`, `emit-evidence`, and `log-write`
- guest-side `read-resource` over the same backend MCP uses
- explain/debug skills over persisted artifacts
- signed local bundle export/import with local trust verification
- OCI image layout and OCI registry transport for that same installed signed bundle contract
- a real stdio MCP server with one stable public tool, `guild.inspect`

The current draft control-plane slice already has:

- M3 and M4 contract/runtime/request/plan artifacts under `docs/schemas/draft-v1/`
- M5 bounded minimization proofs
- M6 bounded delegated capability tokens
- M7 bounded witness generation and verification
- M8a live-runtime alignment fixtures and validators

The current boundary is also explicit:

- the live Rust runtime vocabulary is canonical
- the draft-v1 bundle is still draft and still non-canonical
- runtime-backed draft-v1 claim support is narrow today: `http-request` is wired through the conservative `net.connect` compatibility alias, while live `read-resource`, `invoke-skill`, `emit-evidence`, and `log-write` remain coverage-limited or unsupported in draft-v1 claim semantics

For the exhaustive proof commands, regression sweeps, and example-by-example smoke flows, see [`docs/testing.md`](docs/testing.md).
For the draft admission bundle itself, see [`docs/schemas/draft-v1/README.md`](docs/schemas/draft-v1/README.md); the runnable validation path for that bundle also lives in [`docs/testing.md`](docs/testing.md).

## Canonical Docs

- [`docs/command-language.md`](docs/command-language.md) - canonical public CLI verbs, URI grammar, and terminal snippets
- [`docs/testing.md`](docs/testing.md) - local proof commands, verification commands, and smoke workflows
- [`SPECS.md`](SPECS.md) - normative contract and conformance requirements
- [`ARCHITECTURE.md`](ARCHITECTURE.md) - practical system view and data flow
- [`docs/adr/README.md`](docs/adr/README.md) - ADR index and follow-on ADR backlog
- [`AGENTS.md`](AGENTS.md) - contributor guardrails for contract-first changes
- [`docs/roadmap.md`](docs/roadmap.md) - phased build priorities

Compatibility wrappers remain at [`docs/contracts.md`](docs/contracts.md) and [`docs/architecture.md`](docs/architecture.md) so existing links keep working.
