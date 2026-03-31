# Receipt Engine

## Aggregation Boundary

Guild should preserve two explicit receipt layers instead of collapsing them
into one blob:

- execution-attempt receipt: the canonical host-owned receipt and durable record
  for one concrete execution attempt outcome, whether the attempt was admitted
  and ran or was rejected by policy
- session-layer receipt: a future host-owned aggregate view keyed by
  `SessionId` that points at ordered execution-attempt receipts and durable
  evidence lineage for that session

The session-layer receipt should summarize continuity across attempts, but it
must not replace the current attempt-scoped `ExecutionReceipt` and
`ExecutionRecord` boundary. Attempt-local policy decision, termination,
provenance, and emitted evidence remain durably owned by the execution-attempt
record even after session aggregation exists.

## Receipt Shape

The receipt engine should build on the current host-owned execution and
evidence record model and eventually aggregate it at the session layer.

Minimum future session-layer receipt contents:

- session identifier
- ordered execution-attempt receipt refs
- latest execution-attempt identifier
- harness or executable identity
- latest admission outcome
- latest execution mode outcome: warm, resumed, rehydrated, or cold
- latest status and termination detail
- accumulated evidence refs and artifact digests
- aggregate provenance timestamps and lineage

Minimum execution-attempt receipt and record contents stay attempt-scoped:

- execution-attempt identifier
- execution-attempt receipt URI
- resolved executable identity for the attempt
- final policy decision and granted authority for the attempt
- status and termination detail for the attempt
- emitted evidence refs and artifact digests for the attempt
- attempt-local provenance timestamps and lineage

## Evidence And Artifact Linkage

Receipts should include:

- evidence refs for host-persisted evidence records
- artifact hashes or resolved digests for executable identity
- execution metadata describing when and how the attempt ran

## Minimum Provenance Fields

- session ID
- execution ID
- trace/request correlation IDs where available
- resolved executable identity
- started/finished timestamps
- policy decision outcome
- parent/child lineage where applicable

## Replay And Audit Expectations

- Receipts must let an operator understand what was requested, what was
  admitted, how the session was materialized, and what evidence was produced.
- Replay or explanation must be grounded in durable refs, not chat logs or
  unverifiable summaries.
- A future session-layer receipt should aggregate execution-attempt receipts,
  not erase execution-attempt granularity.
- Attempt-local replay should still start from the execution-attempt receipt or
  execution record when the question is about one concrete run.

## Claims Vs Evidence

- claims: summaries, statuses, explanations, and policy decisions
- evidence: durable refs, stored metadata, content hashes, and host-owned
  records that substantiate those claims

Guild should distinguish the two explicitly. A receipt may claim that a session
was rehydrated; the evidence and provenance fields should show what durable
inputs made that claim true.
