# Roadmap

## Phase 0: contracts and skeleton
Goal: make the repo honest before making it impressive.

- define shared Rust types
- define manifest model
- draft the WIT ABI
- scaffold the Cargo workspace
- define MCP façade names
- add one example skill manifest

## Phase 1: local execution
Goal: inspect-mode vertical slice.

- local registry storage
- WASM runner
- capability grant evaluation
- execution records
- evidence storage
- `guild.inspect` path end to end

## Phase 2: inspect reuse and composition
Goal: richer read-only execution over durable artifacts.

- child skill invocation
- dependency snapshots
- explain/debug skills over persisted artifacts
- bounded local query resources over stored executions
- better diagnostics and provenance

## Phase 3: distribution and trust
Goal: portable skill sharing that is not naive.

- signed skill packages
- publisher identity
- trust tiers
- org vs public visibility
- package verification

## Phase 4: apply mode
Goal: mutation with discipline, not wishful thinking.

- idempotency keys
- approvals
- audit records
- bounded retries
- clearer effect semantics
