# Guild

**Guild** is a Rust-first, WASM-native registry and runtime for portable AI skills.

Guild sits one layer above raw MCP servers. MCP gives agents a way to discover and call tools. Guild packages operational know-how as versioned, capability-scoped, portable skills that can be resolved, executed, inspected, and shared without giving guests ambient authority.

> Status: pre-alpha. The repository already has a real local inspect-oriented vertical slice: requested refs resolve through a file-backed registry, example skills execute through a Wasmtime-backed Wasm component runtime, signed local bundles can be exported and imported without rebuilding, execution attempts persist as host-owned records, and evidence persists as durable Guild objects.

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
3. execute it through the Wasm runtime with explicit granted capabilities
4. persist `ExecutionRecord` and `EvidenceRef` artifacts under local Guild URIs
5. optionally export the installed skill as a signed portable bundle and import it into a fresh Guild root

Useful local proof commands:

```bash
make test
cargo run -p guild-mcp --example inspect_local
cargo run -p guild-mcp --example explain_execution_local
cargo run -p guild-mcp --example export_import_local
```

Additional examples cover composite execution, durable rejected executions, composite portability, and tampered or untrusted bundle rejection.

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
- `guild-registry`: local installation, bundle flow, resolution, and Guild resource persistence
- `guild-runner`: runtime orchestration, capability checks, and execution boundary
- `guild-mcp`: stable facade surface and local proof examples
- `guild-sdk-rust`: guest authoring support for Rust-based skills

## Current Scope

What is real today:

- source-to-installed manifest lifecycle
- digest-pinned local resolution
- Wasmtime-backed Wasm component execution
- typed capability enforcement for the implemented host imports
- durable execution and evidence persistence
- composite child invocation with durable lineage
- signed local bundle export and import with trust-store verification

What is still deferred:

- general policy evaluation beyond explicit caller-provided grants
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
