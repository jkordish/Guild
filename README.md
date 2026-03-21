# Guild

**Guild** is a contracts-first Rust/Wasm runtime and control-plane for portable AI skills.

Guild is best read as a milestone program, not as a generic agent wrapper:

- `M3`: define the portable contract bundle
- `M4`: compute a fail-closed upper-bound execution plan
- `M5`: minimize authority when a narrower claim can actually be proven
- `M6`: issue invocation-bound delegated capability tokens
- `M7`: record exercised and blocked authority as durable witness facts
- `M8a`, `M8b`, `M8c`: align the draft control-plane with the live Rust runtime and add live proof only where the runtime can honestly support it

MCP is only the transport facade here. Guild is the layer that admits, runs, delegates, and witnesses authority under explicit host control instead of ambient guest access.

> Status: pre-alpha. Read the repository milestone-first.
>
> - `M3` and `M4`: implemented as the draft-v1 contract and admission bundle under `docs/schemas/draft-v1/`
> - `M5`, `M6`, and `M7`: implemented there as bounded draft-v1 minimization, token, and witness paths
> - `M8a`: complete; the live Rust runtime vocabulary is canonical
> - `M8b`: complete for the active canonical families in draft-v1
> - `M8c`: partial; `read-resource` has bounded live proof with checked plan -> proof -> token -> witness linkage, `log-write` has real live family proof, and `http-request` has bounded live proof only for two deterministic replay-fixtured `GET` loopback IP slices over `http`: one at `http://127.0.0.1:<port><exact-path>` and one at `http://127.0.0.1/<exact-path>` using the implicit default HTTP port, both with no query and no redirects
> - broader `http-request` shapes, including loopback hostname forms, `HEAD`, query or fragment components, redirects, multiple exercised requests, and `https`, plus `invoke-skill` and `emit-evidence`, remain outside the live-proof envelope
> - `M9` and `M10`: not started

## Why Guild

Guild is strict about a few things because the milestone program requires them:

- requested identity is not executable identity
- the host, not the guest, owns trust-sensitive authority
- evidence is a durable artifact, not a prompt scrap
- inspect, plan, and apply are distinct modes
- the MCP surface should stay small and boring

The goal is not a loose agent wrapper layer. It is a portable skill system where admission, delegation, and witness claims stay tied to real runtime truth.

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

- `M3 Define the schemas`: complete in `docs/schemas/draft-v1/`, with checked examples and deterministic validation.
- `M4 Build the admission engine`: complete in that same bundle, producing fail-closed `admit`, `downgrade`, `migrate`, and `refuse` execution plans.
- `M5 Build the counterfactual authority minimizer`: complete as a bounded draft-v1 minimization path with explicit `exact_minimal`, `bounded_minimal`, `reduced`, `no_reduction`, and `not_proven` outcomes. Live proof exists only where `M8c` says it does.
- `M6 Build delegation-chain-bound capability tokens`: complete as a bounded draft-v1 token path with proof-backed issuance by default, explicit `m4_upper_bound` issuance, and fail-closed verification. This is still a draft-local token layer, not runtime-general enforcement.
- `M7 Build the bounded draft-v1 witness layer`: complete for the bounded draft-v1 witness path, including exercised authority, blocked attempts, coverage semantics, redaction semantics, and fixed claim checks.
- `M8a Runtime Alignment and Canonical Effect Vocabulary`: complete. The live Rust capability-family surface is canonical, the runtime persists durable `authority_observations`, and draft-v1 maps live runtime data through explicit `exact`, `narrowing`, `partial`, and `unsupported` outcomes.
- `M8b Direct Canonical Family Support in Draft-v1`: complete for the active runtime slice. Draft-v1 now carries direct canonical `http-request`, `read-resource`, `invoke-skill`, `emit-evidence`, and `log-write` families for admission, minimization, tokens, and witnesses.
- `M8c Live Proof Basis and Honest End-to-End Linkage`: partial. `read-resource` has bounded live proof and checked plan -> proof -> token -> witness linkage over immutable `guild://executions/...` and `guild://objects/records/...` resources. `log-write` has real live family proof over observed level slices. `http-request` has bounded plan -> proof -> token -> witness linkage only for two deterministic replay-fixtured loopback IP `GET` slices over `http`: one with an explicit port and one using the implicit default HTTP port, both with no query and no redirects. Loopback hostname forms, `HEAD`, query or fragment components, redirects, multiple exercised requests, and `https` remain `not_proven`, as do `invoke-skill` and `emit-evidence`.
- `M9 Draft the patent packet`: not started.
- `M10 Filing hygiene`: not started.

## What Is Real Today

Read the current repository in milestone buckets.

Live runtime work through `M8a`, `M8b`, and the implemented slice of `M8c` already has:

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

Draft-v1 control-plane work through `M3` through `M7`, plus the draft-facing side of `M8a`, `M8b`, and `M8c`, already has:

- M3 and M4 contract/runtime/request/plan artifacts under `docs/schemas/draft-v1/`
- M5 bounded minimization proofs
- M6 bounded delegated capability tokens
- M7 bounded witness generation and verification
- M8a live-runtime alignment fixtures and validators
- M8b direct canonical family support for the five active runtime families
- M8c live-proof consumption and honest linkage inside the real live-proof envelope

The current milestone boundary is:

- the live Rust runtime vocabulary is canonical
- the draft-v1 bundle is still draft and still non-canonical
- live proof support is narrow by design: `read-resource` is bounded live-proof-backed only for immutable `guild://executions/...` and `guild://objects/records/...` reads, `log-write` has real live family proof over observed log-level slices, and `http-request` is bounded live-proof-backed only for two deterministic replay-fixtured loopback IP `GET` slices over `http`, one with an explicit port and one using the implicit default HTTP port, both with exact observed path, no query, and no redirects; loopback hostname forms, `HEAD`, query or fragment components, redirects, multiple exercised requests, `https`, and all current `invoke-skill` and `emit-evidence` flows remain `not_proven`
- proof-backed token issuance and proof-linked witnesses are honest only inside that live proof envelope; outside it, the draft-v1 path stays explicit about upper-bound issuance, unlinked witnesses, or `not_proven` status

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
