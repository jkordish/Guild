# ADR Index

Guild uses ADRs to record platform-shaping decisions and to queue the next contract work that should be captured as decisions rather than implied by code drift.

## Current ADRs

- `0001-core-principles.md` - accepted foundation for Rust core, WASM-first execution, explicit capabilities, digest-pinned resolution, and a small MCP facade
- `0002-skill-output-and-execution-record.md` - accepted split between skill-authored output and host-owned execution records
- `0003-guild-thesis.md` - proposed project framing for local-first, durable, policy-bounded skill execution

## Backlog

The next ADRs should land in this order so the contract surface grows in the same sequence the system executes:

1. Bundle format
   Define the portable installed bundle payload, signature envelope, import verification semantics, and compatibility versioning.
2. Capability schema
   Define capability family versioning, typed constraint registry rules, and compatibility expectations across Rust types, manifests, and WIT.
3. Execution record schema
   Define the durable execution object shape, terminal metadata, lineage fields, and query-oriented invariants.
4. Evidence schema
   Define durable evidence object metadata, provenance expectations, content-addressing rules, and read semantics.

## Working Rule

If a change materially alters trust boundaries, installed portability, runtime semantics, or durable record shape, it should land with a new ADR or an explicit update to an existing one.
