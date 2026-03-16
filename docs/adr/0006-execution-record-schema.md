# ADR 0006: Execution record schema

## Status

Accepted

## Context

ADR 0002 split skill-authored output from host-owned execution records.
ADR 0003 froze the guest ABI versus durable host-record boundary.

Guild now also has a real durable execution store with:

- host-minted execution IDs
- persisted success, failure, and rejection records
- top-level failure receipts
- child execution lineage
- host-stamped timestamps

What was still missing was an ADR that freezes the actual durable execution object shape and, just as importantly, the boundary for what does and does not count as a persisted execution attempt in the current milestone.

## Decision

Guild's durable execution model has three distinct layers:

- `CallerRequest` is caller intent and correlation data
- `ResolvedExecutionEnvelope` is the host-enriched pre-execution object
- `ExecutionRecord` is the durable host-owned record of what actually happened

The host owns durable execution identity:

- durable execution IDs are minted by the host
- callers do not supply durable execution record IDs
- `ExecutionReceipt` is the durable locator returned for a persisted attempt
- the receipt currently contains `execution_id`, `uri`, `trace_id`, and `status`
- the durable URI is always the host-issued execution URI

The persisted-attempt boundary in the current implementation is:

- pre-resolution request and lookup failures are not persisted in this milestone
- once a resolved attempt enters `Runner::execute`, Guild mints a host execution ID
- if record persistence succeeds, Guild persists success, failure, or rejection before returning
- unsuccessful top-level inspect calls still return errors, but those errors carry the persisted host-issued receipt when the attempt was durably recorded

`ExecutionRecord` is the current durable schema and contains:

- `receipt`
- `request`
- `policy_decision`
- `resolved_skill`
- `parent_execution_id`
- `status`
- optional `output`
- optional `termination`
- `granted_capabilities`
- `emitted_evidence`
- `metrics`
- `provenance`
- `child_executions`

The current status model for the active inspect substrate is:

- `Succeeded` for successful runtime completion
- `Rejected` for validation, grant, and mode rejections
- `Failed` for runtime-load, runtime-exec, child-invocation, persistence, and skill-domain failures

`ExecutionStatus::Partial` exists in shared types but is not part of the active emitted execution status model for the current inspect slice.

`TerminationDetail` is host-owned and records:

- phase
- code
- message
- retryability
- optional structured detail

`PolicyDecision` is host-owned durable metadata:

- allowed requests retain the host policy decision from the execution envelope
- reduced requests retain the host policy decision together with reduction reasons
- rejected attempts persist a host-owned rejected `PolicyDecision`
- guests do not author durable denial classification

Timestamps are host-stamped:

- `provenance.started_at_utc`
- `provenance.finished_at_utc`

Child execution persistence is explicit:

- each child attempt gets its own full `ExecutionRecord`
- the parent stores `ChildExecutionRecord` edges rather than inlining child output or child evidence
- those edges retain alias, child execution identity, status, policy decision, termination, granted capabilities, metrics, and provenance
- full child requested identity remains available from the child's own `ExecutionRecord.request`

What does not belong in the current `ExecutionRecord` contract:

- guest-authored durable execution IDs
- guest-authored denial classification
- a durable resource-read ledger or persisted read-set
- task, subscription, or queue state
- apply-mode audit trail semantics

## Consequences

Positive:

- Guild now has a durable execution object that cleanly separates caller intent, resolved execution context, and host-owned outcome
- top-level failures and rejections remain inspectable rather than disappearing into transport errors
- child lineage is durable without collapsing parent and child records together
- host-issued receipts give callers a stable path back to persisted execution truth

Costs and current limits:

- the current durable record does not yet preserve a read ledger for `read-resource`
- pre-resolution failures are still outside the persisted-attempt boundary
- incident taxonomy remains intentionally narrow to the current status and termination model

## Explicit invariants

- `RequestedSkillRef` is not executable identity
- `ResolvedSkillRef` is the only executable identity
- durable execution IDs and execution URIs are host-issued
- success, failure, and rejection are all durable outcomes once a resolved attempt enters the execution path
- `SkillOutput` remains guest-authored data embedded inside a host-owned record when present
- denial classification and termination metadata remain host-owned
- parent and child executions persist as distinct records with durable lineage

## Explicit non-goals / deferred work

- a richer policy engine
- tasks, subscriptions, or queue-oriented execution models
- a broader incident taxonomy than the current status plus termination detail model
- provisional or in-progress durable execution records
- `apply` mutation audit trail design

## Cross-references

- `README.md`
- `SPECS.md`
- `ARCHITECTURE.md`
- `MEMORY.md`
- `docs/adr/0002-skill-output-and-execution-record.md`
- `docs/adr/0003-guest-abi-vs-host-record-boundary.md`
- `crates/guild-types/src/lib.rs`
- `crates/guild-runner/src/lib.rs`
- `crates/guild-registry/src/lib.rs`
- `crates/guild-mcp/src/lib.rs`
- `crates/guild-mcp/examples/explain_failure_local.rs`
- `crates/guild-runner/tests/inspect_slice.rs`
- `crates/guild-runner/tests/composition.rs`
