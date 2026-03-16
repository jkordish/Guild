# ADR Index

Guild uses ADRs to record platform-shaping decisions and to queue the next contract work that should be captured as decisions rather than implied by code drift.

## Current ADRs

- `0001-guild-thesis.md` - accepted foundation for local-first, Rust-core, Wasm-first, contract-first Guild execution
- `0002-skill-output-and-execution-record.md` - accepted split between skill-authored output and host-owned execution records
- `0003-guest-abi-vs-host-record-boundary.md` - accepted rule that WIT is the guest ABI, Rust types are the durable host model, and translation is explicit
- `0004-installed-bundle-format.md` - accepted current local installed-state bundle format, signature envelope, import verification, and installed portability rules
- `0005-capability-schema-and-active-inspect-profile.md` - accepted typed capability model and honest active inspect capability surface
- `0006-execution-record-schema.md` - accepted durable execution record shape, receipt model, persisted-attempt boundary, and child lineage rules
- `0007-evidence-record-schema.md` - accepted split between blob identity and per-emission evidence-record identity

## Backlog

The next ADRs should focus on still-deferred platform shape:

1. Policy model
   Define what Guild means by host policy beyond the current caller-grant plus validation model.
2. Retention and query surfaces
   Define retention, indexing, and query expectations for execution and evidence stores.
3. Remote publication and trust
   Define remote bundle publication, trust distribution, and any transparency or signing integration.

## Working Rule

If a change materially alters trust boundaries, installed portability, runtime semantics, or durable record shape, it should land with a new ADR or an explicit update to an existing one.
