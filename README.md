# Guild

**Guild** is a Rust-first, WASM-native registry and runtime for portable AI skills.

Guild sits one layer above raw MCP servers. MCP gives agents a way to discover and call tools. Guild packages operational know-how as versioned, capability-scoped, portable skills that can be resolved, executed, inspected, and shared without giving guests ambient authority.

> Status: pre-alpha. The repository already has a real local inspect-oriented vertical slice: requested refs resolve through a file-backed registry, example skills execute through a Wasmtime-backed Wasm component runtime, Guild can run as a real MCP stdio server, a thin `guild-codex` helper can bootstrap a local Codex dogfood root and print the real stdio MCP config, signed installed bundles can be exported and imported without rebuilding, those same signed installed bundles can also be transported as local OCI image layouts and through OCI registries, execution attempts persist as host-owned records with host-minted durable IDs, and evidence persists as durable Guild objects with distinct blob and record identity.

## Why Guild Exists

Guild is opinionated about a few things:

- requested identity is not executable identity
- the host, not the guest, owns trust-sensitive authority
- evidence is a durable artifact, not a prompt scrap
- inspect, plan, and apply are distinct modes
- the MCP surface should stay small and boring

The goal is a platform for portable, auditable, reusable skills, not a pile of tool wrappers glued together by vibes.

## Current Proof Path

The current repository proves a narrow but real path:

1. build and install a source skill into local installed state
2. resolve a `RequestedSkillRef` to a digest-pinned executable artifact
3. execute it through the Wasm runtime with host-decided granted capabilities
4. persist `ExecutionRecord` and `EvidenceRef` artifacts under local Guild URIs
5. optionally export the installed skill as a signed portable transport unit and import it into a fresh Guild root through the native signed bundle directory, a local OCI image layout, or an OCI registry

Useful local proof commands:

```bash
make test
cargo run -p guild-mcp --bin guild-codex -- bootstrap --registry-root target/dev-local-registry/codex-local --reset
cargo run -p guild-mcp --example inspect_local
cargo run -p guild-mcp --example inspect_http_json_local
cargo run -p guild-mcp --example inspect_policy_local
cargo run -p guild-mcp --example filesystem_rejection_local
cargo run -p guild-mcp --example inspect_composite_local
cargo run -p guild-mcp --example explain_execution_local
cargo run -p guild-mcp --example explain_execution_tree_local
cargo run -p guild-mcp --example explain_failure_local
cargo run -p guild-mcp --example explain_recent_failures_local
cargo run -p guild-mcp --example codex_explain_execution_local
cargo run -p guild-mcp --example codex_explain_execution_tree_local
cargo run -p guild-mcp --example export_import_local
cargo run -p guild-mcp --example export_import_oci_local
cargo run -p guild-mcp --example export_import_composite_local
cargo run -p guild-mcp --example export_import_composite_oci_local
cargo run -p guild-mcp --example signed_import_failures_local
cargo run -p guild-mcp --example signed_import_oci_failures_local
cargo run -p guild-mcp --example push_pull_oci_registry_local
cargo run -p guild-mcp --example push_pull_composite_oci_registry_local
cargo run -p guild-mcp --example signed_pull_oci_registry_failures_local
cargo run -p guild-mcp --example mcp_stdio_local
```

Additional examples cover bounded local HTTP inspection, trust-tier-aware local policy profiles, explicit deferred filesystem-contract rejection, bounded execution-query resources, composite execution, persisted execution-tree explanation, durable rejected executions, native signed-bundle portability, local OCI image layout portability, OCI registry portability, and tampered or untrusted import rejection.

### HTTP Proof Flow

Guild now has one real new inspect-slice capability family: `http-request`.

The canonical local proof command is:

```bash
cargo run -p guild-mcp --example inspect_http_json_local
```

That example starts a deterministic local HTTP server, installs the primitive `inspect-http-json` skill, grants it bounded outbound HTTP authority through `guild.inspect`, prints the successful stored execution, then runs a denied host-mismatch request and prints the persisted rejected execution. The bounded grant now makes loopback and raw IP-literal access explicit for the local proof flow, while the runtime keeps redirects disabled unless they are explicitly granted. The public MCP surface does not change; HTTP is exercised through `guild.inspect`, not a new MCP tool.

### Policy Proof Flow

Guild now also has a real local host-owned policy evaluator with named profiles and host-owned local trust tiers.

The canonical local proof command is:

```bash
cargo run -p guild-mcp --example inspect_policy_local
```

That example installs `inspect-http-json`, exports it as a signed bundle, imports it into two fresh Guild roots with different publisher trust tiers, writes a local `policy.json` with named profiles plus a tenant binding, then proves two outcomes through the same `guild.inspect` surface: a trusted imported execution that keeps bounded redirect authority and a restricted imported execution whose HTTP grant is reduced before guest start and then rejected by the host when a redirect arrives. It then runs `explain-execution` against the persisted denied execution URI to prove the denial remains host-owned, durable, and explainable through the same resource path. The persisted execution record retains the caller-requested capability set, the smaller host-granted set, the selected policy profile, the verification state, and the host-owned trust tier that influenced the decision.

### Filesystem Rejection Proof Flow

Guild now also exposes an explicit host-side filesystem capability contract while keeping runtime filesystem access deferred.

The canonical local proof command is:

```bash
cargo run -p guild-mcp --example filesystem_rejection_local
```

That example builds a temporary `hello-inspect` variant whose manifest declares the typed filesystem contract, requests a matching filesystem grant through `guild.inspect`, shows the host-owned rejected execution record, and then runs `explain-execution` against that persisted receipt. The contract is real in the host-side manifest and policy surface, but the active Wasm inspect slice still rejects filesystem before guest start. No guest filesystem import, preopened directory, or host file IO is added in this milestone.

### Artifact Query Proof Flow

Guild now also exposes one bounded local execution-query layer through Guild resources and resource templates, not through a sprawl of new MCP tools.

The canonical local proof command is:

```bash
cargo run -p guild-mcp --example explain_recent_failures_local
```

That example produces a small deterministic set of stored executions with different outcomes, reads `guild://queries/executions/failures/recent/10` directly through the host resource backend, then runs the new inspect-only `summarize-execution-query` skill against that same query URI through `guild.inspect`. The result is a structured report over canonical execution URIs, statuses, policy reasons, and evidence presence. Query reads remain host-mediated, bounded, and capability-scoped; the public MCP tool surface still does not grow.

## MCP Server

Guild now ships a real stdio MCP server entrypoint:

```bash
cargo run -p guild-mcp --bin guild-mcp-server -- --registry-root target/dev-local-registry/mcp-stdio-local
```

The current MCP surface is intentionally small:

- one public tool: `guild.inspect`
- a bounded `resources/list` view of recent execution records
- durable Guild execution records are exposed through `resources/read`
- durable evidence-record and blob URIs are exposed through `resources/read`
- bounded execution-query results are exposed through `resources/read`
- resource templates are exposed for `guild://executions/{execution_id}`, `guild://objects/records/{evidence_record_id}`, `guild://objects/sha256/{digest}`, `guild://queries/executions/recent/{limit}`, `guild://queries/executions/failures/recent/{limit}`, `guild://queries/executions/by-status/{status}/{limit}`, and `guild://queries/executions/by-skill/{namespace}/{name}/{limit}`

Unsuccessful inspect executions that reached the real runtime path are surfaced as MCP tool errors with `isError: true`, while preserving the persisted execution record URI instead of collapsing it into an opaque protocol failure.

## Codex Workflow

Guild now ships one repo-native workflow for using the real stdio server from Codex without inventing a second path:

```bash
cargo run -p guild-mcp --bin guild-codex -- bootstrap --registry-root target/dev-local-registry/codex-local --reset
```

That command:

1. creates or resets a fresh local Guild root
2. installs the example skills used by the recommended Codex dogfood flows
3. prints the exact `guild-mcp-server` launch command
4. prints a ready-to-use `codex mcp add ... -- <command>` registration command
5. prints a matching `~/.codex/config.toml` or project `.codex/config.toml` snippet

Codex's current config model supports both `~/.codex/config.toml` and project-scoped `.codex/config.toml`, with project config loaded only when the repo is trusted. Guild leans on that existing Codex behavior rather than adding a special integration hook.

The printed Codex registration uses the existing stdio server directly:

```bash
codex mcp add guild-local --env GUILD_REGISTRY_ROOT=/absolute/path/to/target/dev-local-registry/codex-local -- cargo run -q -p guild-mcp --bin guild-mcp-server --
```

The matching config snippet uses the same repo `cwd`, the same `cargo run` launcher, and the same `GUILD_REGISTRY_ROOT` environment variable. If you only need the config again later, print it without reinstalling skills:

```bash
cargo run -p guild-mcp --bin guild-codex -- print-config --registry-root target/dev-local-registry/codex-local
```

### Recommended Codex Dogfood Flows

The two recommended local-first Codex flows stay entirely on the existing stdio MCP path and prove durable artifact reuse instead of protocol handshakes alone.

1. Explain one execution.
   Ask Codex to run `example/hello-inspect` through `guild.inspect`, then run `example/explain-execution` against the returned execution URI with a `read-resource` grant scoped to `guild://executions/` and `guild://objects/records/`.
2. Explain one execution tree.
   Ask Codex to run `example/hello-composite` through `guild.inspect`, then run `example/explain-execution-tree` against the returned root execution URI with `read-resource` scoped to `guild://executions/` and `guild://objects/records/`.

The deterministic local smoke commands for those same flows are:

```bash
cargo run -p guild-mcp --example codex_explain_execution_local
cargo run -p guild-mcp --example codex_explain_execution_tree_local
```

Those examples use the real stdio MCP server and a small local MCP client harness, so they stay deterministic even when a full authenticated Codex-in-the-loop run is impractical in CI.

This milestone intentionally does not add MCP HTTP transport, subscriptions, more top-level MCP tools, new capability families, `plan`, or `apply`. Guild remains local-first, stdio-first, inspect-only, and resource-oriented.

## Integrity Model

The current inspect slice is intentionally strict about a few things:

- durable execution IDs are minted by the host; caller-supplied IDs are correlation data only
- `EvidenceRef` identifies a host-issued evidence record URI for a single emission, while payload blobs remain content-addressed by digest
- requested resolution fails closed if the same skill key and version exist under multiple digests
- local source installs stage and validate in a temporary directory before an atomic move into installed state
- host authorization denials persist as host-owned rejected executions instead of leaking into guest-owned failure semantics
- the active Wasm inspect ABI now instantiates inspect skills only against `guild-skill-inspect-v1`, which exposes only `http-request`, `read-resource`, `invoke-skill`, `emit-evidence`, and `log-write`
- broader typed families remain in shared host vocabulary, but unsupported capability imports are absent from the active inspect guest ABI and broader Guild component imports now fail closed as host-owned `unsupported-runtime-surface` rejections rather than degrading into generic runtime failures
- `filesystem` is now an explicit typed host-side capability contract with named roots, guest-path prefixes, host-path concepts, and read/write/create/append operations, but the active inspect slice still rejects it before guest start
- `read-resource` grants now match parsed canonical Guild URI scopes rather than loose raw string prefixes
- `http-request` is host-mediated, typed, bounded, and fail-closed; method, scheme, host, domain suffix, port, path, redirect, timeout, response-size, loopback, private-network, link-local, and IP-literal checks stay host-owned
- the host-to-guest inspect projection is explicit: the richer durable execution model stays host-owned, while the guest sees the inspect-only execution context and the full active HTTP grant shape
- caller-requested capabilities are policy input, not final authority; a local `policy.json` plus host-owned defaults decide the granted capability set before execution
- policy now selects a named local profile by actor and/or tenant, then evaluates grants against host-owned verification state and local trust tier metadata
- policy reductions and rejections persist as host-owned execution metadata and stay visible to explain/debug flows
- durable execution records now carry host-stamped start and finish timestamps
- bounded local execution-query resources now derive deterministically from the same persisted execution store used by explain/debug flows
- the stdio MCP server exposes only `guild.inspect` plus honest Guild resource reads; HTTP transports and subscriptions remain deferred

## Canonical Docs

- [`SPECS.md`](SPECS.md) - normative contract and conformance requirements
- [`ARCHITECTURE.md`](ARCHITECTURE.md) - practical system view and data flow
- [`docs/adr/README.md`](docs/adr/README.md) - ADR index and follow-on ADR backlog
- [`AGENTS.md`](AGENTS.md) - contributor guardrails for contract-first changes
- [`docs/roadmap.md`](docs/roadmap.md) - phased build priorities

Compatibility wrappers remain at [`docs/contracts.md`](docs/contracts.md) and [`docs/architecture.md`](docs/architecture.md) so existing links keep working.

## Repository Map

```text
.
├── README.md
├── SPECS.md
├── ARCHITECTURE.md
├── AGENTS.md
├── CONTRIBUTING.md
├── Cargo.toml
├── docs/
│   ├── adr/
│   ├── architecture.md
│   ├── contracts.md
│   └── roadmap.md
├── wit/
│   └── guild-skill-v1.wit
├── examples/
│   └── skills/
└── crates/
    ├── guild-types/
    ├── guild-manifest/
    ├── guild-registry/
    ├── guild-runner/
    ├── guild-mcp/
    └── guild-sdk-rust/
```

Current crate responsibilities:

- `guild-types`: shared types for identities, capabilities, execution, and evidence
- `guild-manifest`: source and installed manifest model
- `guild-registry`: local installation, native signed-bundle flow, OCI image layout flow, OCI registry transport flow, resolution, and Guild resource persistence
- `guild-runner`: runtime orchestration, capability checks, and execution boundary
- `guild-mcp`: stable facade surface, stdio MCP server, and local proof examples
- `guild-sdk-rust`: guest authoring support for Rust-based skills

## Current Scope

What is real today:

- source-to-installed manifest lifecycle
- digest-pinned local resolution
- Wasmtime-backed Wasm component execution
- real host-mediated outbound HTTP execution through the Wasmtime runtime path
- typed capability enforcement for the implemented host imports
- explicit host-side filesystem contract modeling plus preflight rejection guardrails for the deferred family
- local file-backed policy evaluation with named profiles, actor/tenant bindings, and host-owned trust tiers that can allow, reduce, or reject caller-requested capabilities before execution
- host-minted durable execution IDs with create-only execution persistence
- durable execution persistence with host-stamped timestamps
- split evidence blob and evidence-record persistence
- composite child invocation with durable lineage
- one primitive `inspect-http-json` example that proves bounded HTTP fetches through `guild.inspect`
- one inspect-only `summarize-execution-query` example that reads bounded execution-query resources and returns a deterministic structured report
- real stdio MCP server support for `guild.inspect` and Guild URI resources
- a thin `guild-codex` helper that bootstraps a local dogfood root and prints the real Codex stdio MCP config
- deterministic MCP-path dogfood flows for `explain-execution` and `explain-execution-tree`
- signed local bundle export and import with trust-store verification
- local OCI image layout export and import as an additional transport for the same signed installed-state bundle semantics
- OCI registry push and pull for that same signed installed-state transport without changing the local trust/signature gate on import
- bounded local execution-query resources and templates over the same backend used by guest `read-resource` and MCP `resources/read`

What is still deferred:

- remote or distributed policy beyond the local host-owned evaluator
- a broader policy language beyond the current typed local `policy.json` profile model
- filesystem runtime support, preopened directories, and host file IO for guests
- subscriptions, list-changed notifications, full-text search, and broader evidence-specific query surfaces
- full `plan` mode
- `apply` mode
- Sigstore, transparency logs, remote trust distribution, and broader publication/discovery infrastructure

## Development

Workspace commands:

```bash
make check
make test
make fmt
make clippy
```

The canonical example flows install into cleaned subdirectories under `target/dev-local-registry/` so proof runs stay isolated from one another.

## Naming

The name **Guild** implies shared operational knowledge made durable and reusable.
