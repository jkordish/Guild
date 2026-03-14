# Guild

**Guild** is a Rust-first, WASM-native registry and runtime for portable agent skills.

Guild sits one layer above raw MCP servers. MCP gives agents a way to discover and call tools. Guild packages **operational know-how** as versioned, capability-scoped, portable skills that can be searched, verified, executed, and shared.

> Status: pre-alpha. This repository currently focuses on **contracts, crate boundaries, and architectural guardrails** before implementation detail sprawl sets in.

## Why this exists

Most agent systems stop at "the model can call a function." That is useful, but not enough.

Guild is built around a stronger unit:

- a **skill** has identity, version, and immutable artifact digest
- a skill declares **input/output schemas**
- a skill asks for explicit **host capabilities**
- a skill returns **structured results**, evidence, and diagnostics
- a skill is executable in **inspect**, **plan**, or **apply** mode
- a skill can be **shared** across users, teams, and MCP-compatible clients

The result is a platform for **portable, auditable, reusable playbooks**, not just a loose pile of tool wrappers.

## Design stance

Guild is opinionated on purpose.

- **Rust core** for the platform, policy, runtime, and registry
- **WASM-first** for portable skill distribution
- **Host capabilities, not ambient authority**
- **Digest-pinned execution**, even when humans ask for "latest"
- **Inspect / Plan / Apply** as separate execution modes
- **Evidence is mandatory**, not decorative
- **Contracts before code generation**
- **Small MCP surface**, not one tool per skill

If a future change makes the system easier to demo but harder to trust, the trust model wins.

## What a skill is

A Guild skill is a package containing:

- manifest metadata
- an artifact, preferably a WASM component
- JSON schemas for input and output
- declared capabilities
- examples and test fixtures
- publisher and provenance information

Skills fall into a few initial classes:

- **Inventory**: gather and normalize facts
- **Explain**: interpret inventory into operator-readable meaning
- **Playbook**: recommend or carry out next steps
- **Transform**: reshape structured data for downstream use

## System overview

```text
+-------------------+        +------------------+
| MCP Client / LLM  | <----> |   guild-mcp      |
+-------------------+        +------------------+
                                     |
                                     v
                           +--------------------+
                           |  Registry + Policy |
                           +--------------------+
                                     |
                                     v
                           +--------------------+
                           |  Runner / Sandbox  |
                           +--------------------+
                                     |
                       +-------------+-------------+
                       |                           |
                       v                           v
              +------------------+       +------------------+
              | WASM Skill       |       | External Adapter |
              | (preferred)      |       | (later / narrow) |
              +------------------+       +------------------+
```

## Repository layout

```text
.
├── README.md
├── AGENTS.md
├── CONTRIBUTING.md
├── Cargo.toml
├── Makefile
├── docs/
│   ├── architecture.md
│   ├── contracts.md
│   ├── roadmap.md
│   └── adr/
│       └── 0001-core-principles.md
├── wit/
│   └── guild-skill-v1.wit
├── examples/
│   └── skills/
│       └── hello-inspect/
└── crates/
    ├── guild-types/
    ├── guild-manifest/
    ├── guild-runner/
    ├── guild-registry/
    ├── guild-mcp/
    └── guild-sdk-rust/
```

## Initial crate responsibilities

- **guild-types**: core shared types for execution, evidence, capabilities, and results
- **guild-manifest**: manifest model for published skills
- **guild-runner**: execution boundary and runtime adapter traits
- **guild-registry**: storage and resolution model for skill publication and lookup
- **guild-mcp**: MCP-facing surface and stable tool naming
- **guild-sdk-rust**: authoring trait for Rust-based skills

## Non-goals for the first phase

The first phase is intentionally narrow.

- no workflow DSL
- no shell-as-a-platform nonsense
- no raw filesystem/network/process access from skills
- no "floating latest" execution
- no one-tool-per-skill MCP explosion
- no broad write/apply mode until idempotency, approvals, and audit paths exist

## Skill lifecycle

1. **Author** a skill in Rust or another supported language.
2. **Package** it as a WASM component when possible.
3. **Describe** it with a signed manifest and schemas.
4. **Publish** it into the registry with immutable digests.
5. **Resolve** it by version requirement into a concrete digest.
6. **Execute** it through the runner with granted capabilities.
7. **Return** structured output, evidence, diagnostics, and provenance.

## Development

This repo is scaffolded as a Cargo workspace. The current code is intentionally thin and contract-heavy.

```bash
make check
make test
make fmt
make clippy
```

## First milestones

### Phase 0
- stabilize core types and manifest shapes
- stabilize `guild-skill-v1.wit`
- land a runner abstraction
- define the MCP façade
- ship one example inspect-only skill

### Phase 1
- WASM runner
- local registry
- evidence storage
- policy evaluation for capability grants
- signed package ingestion

### Phase 2
- org/public visibility
- trust tiers and publisher verification
- composition / child execution budgets
- inspect and plan mode across the full stack

### Phase 3
- carefully gated apply mode
- approvals and idempotency keys
- audit log and promotion flows
- richer skill packs by domain

## Read next

- [`docs/architecture.md`](docs/architecture.md)
- [`docs/contracts.md`](docs/contracts.md)
- [`docs/adr/0001-core-principles.md`](docs/adr/0001-core-principles.md)
- [`AGENTS.md`](AGENTS.md)

## Naming

**Guild** is the working project name because the system is fundamentally about shared craft, standards, and portable skills. It also avoids the usual "tool galaxy" naming disease, which is refreshing.

## License

No license has been selected yet. Pick one before publishing anything beyond private experimentation.
