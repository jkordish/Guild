# Guild

**Guild** is a Rust-first, WASM-native registry and runtime for portable AI skills.

Guild sits one layer above raw MCP servers. MCP gives agents a way to discover and call tools. Guild packages operational know-how as versioned, capability-scoped, portable skills that can be resolved, executed, inspected, and shared without giving guests ambient authority.

> Status: pre-alpha. The repository already has a real local inspect-oriented vertical slice: requested refs resolve through a file-backed registry, example skills execute through a Wasmtime-backed Wasm component runtime, Guild can run as a real MCP stdio server, signed installed bundles can be exported and imported without rebuilding, those same signed installed bundles can also be transported as local OCI image layouts, execution attempts persist as host-owned records with host-minted durable IDs, and evidence persists as durable Guild objects with distinct blob and record identity.

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
5. optionally export the installed skill as a signed portable transport unit and import it into a fresh Guild root through either the native signed bundle directory or a local OCI image layout

Useful local proof commands:

```bash
make test
cargo run -p guild-mcp --example inspect_local
cargo run -p guild-mcp --example inspect_http_json_local
cargo run -p guild-mcp --example inspect_policy_local
cargo run -p guild-mcp --example inspect_composite_local
cargo run -p guild-mcp --example explain_execution_local
cargo run -p guild-mcp --example explain_execution_tree_local
cargo run -p guild-mcp --example explain_failure_local
cargo run -p guild-mcp --example export_import_local
cargo run -p guild-mcp --example export_import_oci_local
cargo run -p guild-mcp --example export_import_composite_local
cargo run -p guild-mcp --example export_import_composite_oci_local
cargo run -p guild-mcp --example signed_import_failures_local
cargo run -p guild-mcp --example signed_import_oci_failures_local
cargo run -p guild-mcp --example mcp_stdio_local
```

Additional examples cover bounded local HTTP inspection, host-owned policy reduction and denial, composite execution, persisted execution-tree explanation, durable rejected executions, native signed-bundle portability, OCI image layout portability, and tampered or untrusted import rejection.

### HTTP Proof Flow

Guild now has one real new inspect-slice capability family: `http-request`.

The canonical local proof command is:

```bash
cargo run -p guild-mcp --example inspect_http_json_local
```

That example starts a deterministic local HTTP server, installs the primitive `inspect-http-json` skill, grants it bounded outbound HTTP authority through `guild.inspect`, prints the successful stored execution, then runs a denied host-mismatch request and prints the persisted rejected execution. The public MCP surface does not change; HTTP is exercised through `guild.inspect`, not a new MCP tool.

### Policy Proof Flow

Guild now also has a real local host-owned policy evaluator.

The canonical local proof command is:

```bash
cargo run -p guild-mcp --example inspect_policy_local
```

That example writes a local `policy.json` into its isolated Guild root, installs `hello-inspect`, shows a successful execution where policy reduces caller-requested capabilities before guest start, then runs a second execution that policy rejects because the actor is not allowed to receive the skill's required `emit-evidence` grant. The persisted execution record retains both the caller-requested capability set and the smaller host-granted set together with host-owned policy reasons.

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
- resource templates are exposed for `guild://executions/{execution_id}`, `guild://objects/records/{evidence_record_id}`, and `guild://objects/sha256/{digest}`

Unsuccessful inspect executions that reached the real runtime path are surfaced as MCP tool errors with `isError: true`, while preserving the persisted execution record URI instead of collapsing it into an opaque protocol failure.

## Integrity Model

The current inspect slice is intentionally strict about a few things:

- durable execution IDs are minted by the host; caller-supplied IDs are correlation data only
- `EvidenceRef` identifies a host-issued evidence record URI for a single emission, while payload blobs remain content-addressed by digest
- requested resolution fails closed if the same skill key and version exist under multiple digests
- local source installs stage and validate in a temporary directory before an atomic move into installed state
- host authorization denials persist as host-owned rejected executions instead of leaking into guest-owned failure semantics
- the active Wasm inspect slice only supports `http-request`, `read-resource`, `invoke-skill`, `emit-evidence`, and `log-write`; broader typed families are rejected before execution
- `read-resource` grants now match parsed canonical Guild URI scopes rather than loose raw string prefixes
- `http-request` is host-mediated, typed, bounded, and fail-closed; method, scheme, host, port, path, timeout, and response-size checks stay host-owned
- caller-requested capabilities are policy input, not final authority; a local `policy.json` plus host-owned defaults decide the granted capability set before execution
- policy reductions and rejections persist as host-owned execution metadata and stay visible to explain/debug flows
- durable execution records now carry host-stamped start and finish timestamps
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
- `guild-registry`: local installation, native signed-bundle flow, OCI image layout flow, resolution, and Guild resource persistence
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
- local file-backed policy evaluation that can allow, reduce, or reject caller-requested capabilities before execution
- host-minted durable execution IDs with create-only execution persistence
- durable execution persistence with host-stamped timestamps
- split evidence blob and evidence-record persistence
- composite child invocation with durable lineage
- one primitive `inspect-http-json` example that proves bounded HTTP fetches through `guild.inspect`
- real stdio MCP server support for `guild.inspect` and Guild URI resources
- signed local bundle export and import with trust-store verification
- local OCI image layout export and import as an additional transport for the same signed installed-state bundle semantics

What is still deferred:

- remote or distributed policy beyond the local host-owned evaluator
- a broader policy language beyond the current typed local `policy.json` profile
- full `plan` mode
- `apply` mode
- remote registries, publication flows, and transparency infrastructure

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
