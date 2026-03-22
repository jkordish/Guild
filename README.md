# Guild

**Guild** is a contracts-first Rust/Wasm runtime and control-plane for portable AI skills.

Guild is best read as a milestone program, not as a generic agent wrapper:

- `M3`: define the portable contract bundle
- `M4`: compute a fail-closed upper-bound execution plan
- `M5`: minimize authority when a narrower claim can actually be proven
- `M6`: issue invocation-bound delegated capability tokens
- `M7`: record exercised and blocked authority as durable witness facts
- `M8a`, `M8b`, `M8c`: align the draft control-plane with the live Rust runtime and add live proof only where the runtime can honestly support it
- `M8-proper`: benchmark the actual checked real path slice by slice, including refusal and fallback walls

MCP is only the transport facade here. Guild is the layer that admits, runs, delegates, and witnesses authority under explicit host control instead of ambient guest access.

> Status: pre-alpha. Read the repository milestone-first.
>
> - `M3` and `M4`: implemented as the draft-v1 contract and admission bundle under `docs/schemas/draft-v1/`
> - `M5`, `M6`, and `M7`: implemented there as bounded draft-v1 minimization, token, and witness paths
> - `M8a`: complete; the live Rust runtime vocabulary is canonical
> - `M8b`: complete for the active canonical families in draft-v1
> - `M8c`: partial; `read-resource` has bounded live proof with checked plan -> proof -> token -> witness linkage, `log-write` has real live family proof, `http-request` has bounded live proof only for six deterministic replay-fixtured slices over `http`, and `invoke-skill` now has one bounded live-proof-backed exact single-child slice only: one declared alias resolved through the installed dependency snapshot to one exact zero-authority child on `guild-skill-inspect-v1`, with deterministic child input, a child-aware comparator, and zero nested child executions
> - `M8-proper`: complete as a slice-aware real-path benchmark under `docs/schemas/draft-v1/benchmark_matrix.json` and `docs/benchmarking/m8-real-path-benchmark.md`
> - the standard draft-v1 truth path is now Rust-native and repo-native through `cargo run -q -p xtask -- draft-v1 ...`; the checked JSON and Markdown artifacts remain outputs, and `docs/schemas/draft-v1/` remains draft rather than a public stable CLI surface
> - broader `http-request` shapes, including `localhost` default-port `GET`, `localhost` default-port `HEAD`, other hostname forms, query or fragment components, redirects, multiple exercised requests, and `https`, plus broader `invoke-skill` shapes such as dynamic or broader resolution, multi-child fan-out, recursion, child-side authority use, non-inspect child targets, and all current `emit-evidence` flows, remain outside the live-proof envelope; `emit-evidence` now binds a fixed local-object-store sink descriptor and uses a dedicated comparator profile in the runtime, but the tested exact single-emission shrink still fails closed on replay, so there is still no honest proof-backed linkage
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
- `M8c Live Proof Basis and Honest End-to-End Linkage`: partial. `read-resource` has bounded live proof and checked plan -> proof -> token -> witness linkage over immutable `guild://executions/...` and `guild://objects/records/...` resources. `log-write` has real live family proof over observed level slices. `http-request` has bounded plan -> proof -> token -> witness linkage only for six deterministic replay-fixtured slices over `http`: loopback IP `GET` and `HEAD`, each with an explicit-port form and an implicit-default-port form, plus explicit-port `localhost` `GET` and `HEAD` with deterministic loopback-only resolution bindings, all with exact observed path, no query, and no redirects. `invoke-skill` has one bounded plan -> proof -> token -> witness slice only for exactly one declared alias resolved through the installed dependency snapshot to one exact zero-authority child on `guild-skill-inspect-v1`, with deterministic child input, the child-aware normalized inspect comparator, and zero nested child executions. `emit-evidence` still remains `not_proven`: the runtime now binds a host-owned sink descriptor and uses a dedicated single-sink comparator profile, but the tested exact single-emission shrink still does not re-execute equivalently under that comparator, so proof-backed token issuance and proof-linked witnesses stay unavailable. The current canonical `emit-evidence` authority shape and draft-v1 control-plane also remain too coarse to justify smuggling exact sink or payload specifics through coarser fields. `localhost` default-port `GET`, `localhost` default-port `HEAD`, other hostname forms, query or fragment components, redirects, multiple exercised requests, and `https` remain `not_proven` for `http-request`; dynamic or broader resolution, multi-child fan-out, recursion, child-side authority use, and non-inspect child targets remain `not_proven` for `invoke-skill`.
- `M8-proper Real-Path Benchmarking`: complete. The checked slice-aware benchmark now lives in `docs/schemas/draft-v1/benchmark_matrix.json` with the paired report in `docs/benchmarking/m8-real-path-benchmark.md`, and it measures supported proof-linked slices, proof-only slices, explicit upper-bound fallback paths, explicit refusal paths, fail-closed walls, and timing distributions without averaging unsupported states away.
- `M9 Draft the patent packet`: not started.
- `M10 Filing hygiene`: not started.

## M8 Proper Benchmark

The slice-aware benchmark artifacts now live at [`docs/schemas/draft-v1/benchmark_matrix.json`](docs/schemas/draft-v1/benchmark_matrix.json) and [`docs/benchmarking/m8-real-path-benchmark.md`](docs/benchmarking/m8-real-path-benchmark.md).

The standard repo truth commands for that draft bundle are now:

```bash
cargo run -q -p xtask -- draft-v1 truth check
cargo run -q -p xtask -- draft-v1 truth write
```

Those Rust-native commands replace the old Python-via-venv truth pipeline for validation, compatibility checking, support-matrix generation, and benchmark artifact generation. The checked JSON and Markdown files remain outputs in the repository, and the narrower `support-matrix`, `compatibility`, and `benchmark` subcommands remain available under `xtask` when you only want one slice of the truth path.

The measured supported slices are exactly:

- `read-resource`: immutable `guild://executions/` and `guild://objects/records/` roots, with measured narrowing from the admitted upper bound
- `http-request`: six replay-fixtured `http` slices, loopback IP `GET` and `HEAD` with explicit or implicit default port, plus explicit-port `localhost` `GET` and `HEAD`; these measured fixtures are already narrow, so the proven authority stays bounded but does not shrink further
- `invoke-skill`: one single-child zero-authority slice; this measured fixture is already narrow, so the proof result is `no_reduction`
- `log-write`: one observed `info`-level slice through M4 plus M5 only

The measured unsupported or fail-closed slices are exactly:

- `http-request` redirect-driven execution
- `invoke-skill` multi-child fan-out
- `emit-evidence` single-emission replay-unavailable
- extra fail-closed walls for unsupported `http-request` no-replay, `read-resource` execution-query shrink, and `invoke-skill` child-authority use

The measured timing story is narrow and specific, not global. The checked timing distributions now live in the benchmark artifacts themselves, and the Rust-native benchmark generator owns those values directly.

The measured behavioral split is also explicit:

- the supported proof-linked slices issued proof-backed tokens `10/10` and produced proof-linked witnesses `10/10`
- the benchmarked unsupported slices refused by default `10/10`, issued upper-bound fallback tokens `10/10` when explicitly allowed, and produced only unlinked witnesses `10/10`
- the extra fail-closed walls triggered `10/10` in the checked scenarios
- the checked negative-claim probes were coverage-limited in every measured non-`log-write` slice: `0` success, `0` fail, `3` coverage-limited outcomes per slice
- the benchmark artifact itself is now part of the checked draft-v1 validation path, so stale matrix or report output fails `cargo run -q -p xtask -- draft-v1 truth check`

## What Is Real Today

Read the current repository in milestone buckets.

Live runtime work through `M8a`, `M8b`, and the implemented slice of `M8c` already has:

- source-to-installed lifecycle with atomic local installs
- `RequestedSkillRef -> ResolvedSkillRef` execution boundaries
- real Wasmtime-backed Wasm component execution
- inspect-only primitive and composite skills
- dedicated zero-authority invoke fixtures, `invoke-parent-single-child` and `invoke-child-zero`, for the bounded M8c `invoke-skill` slice
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
- live proof support is narrow by design: `read-resource` is bounded live-proof-backed only for immutable `guild://executions/...` and `guild://objects/records/...` reads, `log-write` has real live family proof over observed log-level slices, `http-request` is bounded live-proof-backed only for six deterministic replay-fixtured slices over `http`, and `invoke-skill` is bounded live-proof-backed only for one exact single-child slice where one declared alias resolves through the installed dependency snapshot to one exact zero-authority child on `guild-skill-inspect-v1`, with deterministic child input, the child-aware comparator, and zero nested child executions; `emit-evidence` stays `not_proven` even after adding explicit sink binding and a dedicated single-sink comparator profile because the tested exact single-emission shrink still fails closed on replay, and the current authority model still cannot carry exact sink and payload identity honestly enough for proof-backed linkage; `localhost` default-port `GET`, `localhost` default-port `HEAD`, other hostname forms, query or fragment components, redirects, multiple exercised requests, `https`, and broader `invoke-skill` shapes also remain `not_proven`
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
