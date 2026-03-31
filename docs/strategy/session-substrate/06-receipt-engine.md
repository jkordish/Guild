# Receipt Engine

## Receipt Shape

The receipt engine should build on the current host-owned execution and
evidence record model and eventually aggregate it at the session layer.

Minimum receipt contents:

- session identifier
- execution-attempt identifier
- harness or executable identity
- admission outcome
- execution mode outcome: warm, resumed, rehydrated, or cold
- status and termination detail
- evidence refs and artifact digests
- provenance timestamps and lineage

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
- A future session-layer receipt should not erase execution-attempt granularity.

## Claims Vs Evidence

- claims: summaries, statuses, explanations, and policy decisions
- evidence: durable refs, stored metadata, content hashes, and host-owned
  records that substantiate those claims

Guild should distinguish the two explicitly. A receipt may claim that a session
was rehydrated; the evidence and provenance fields should show what durable
inputs made that claim true.
