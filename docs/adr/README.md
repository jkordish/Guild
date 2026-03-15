# ADR Index

Guild uses ADRs to record platform-shaping decisions and to queue the next contract work that should be captured as decisions rather than implied by code drift.

## Current ADRs

- `0001-guild-thesis.md` - accepted foundation for local-first, Rust-core, Wasm-first, contract-first Guild execution
- `0002-skill-output-and-execution-record.md` - accepted split between skill-authored output and host-owned execution records

## Backlog

The next ADRs should land in this order so the contract surface grows in the same sequence the system executes:

1. `0003` Bundle format
   Define the portable installed bundle payload, signature envelope, import verification semantics, and compatibility versioning.
2. `0004` Capability schema
   Define capability family versioning, typed constraint registry rules, and compatibility expectations across Rust types, manifests, and WIT.
3. `0005` Execution record schema
   Define the durable execution object shape, terminal metadata, lineage fields, and query-oriented invariants.
4. `0006` Evidence schema
   Define durable evidence object metadata, provenance expectations, content-addressing rules, and read semantics.

## Working Rule

If a change materially alters trust boundaries, installed portability, runtime semantics, or durable record shape, it should land with a new ADR or an explicit update to an existing one.
