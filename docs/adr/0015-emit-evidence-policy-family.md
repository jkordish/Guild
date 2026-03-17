# ADR 0015: Emit-evidence policy family

Status: Accepted  
Date: 2026-03-17

## Context

Guild already has a real evidence store, host-issued evidence-record identity,
and a guest host import that can emit evidence during execution.

What needed freezing is the policy meaning of that family before future write
surfaces start to appear.

## Decision

The `emit-evidence` family authorizes host-mediated evidence emission only.
It does not mean arbitrary object storage access.

The guest may ask for one emission by providing an `EvidenceEmissionRequest`
containing:

- payload bytes
- MIME type
- optional title
- audience
- redaction class
- optional freshness label

The current typed policy dimensions are:

- `max_bytes`
- `audiences`
- `redactions`

Authorization is host-owned and per emission:

- no `emit-evidence` grant means denial
- payloads above `max_bytes` are denied
- audiences outside the granted set are denied
- redaction classes outside the granted set are denied
- denials remain host-owned and are persisted as rejected executions rather than
  guest-authored failures

The current denial taxonomy is intentionally concrete:

- `emit-evidence-not-granted`
- `emit-evidence-too-large`
- `emit-evidence-audience-not-granted`
- `emit-evidence-redaction-not-granted`

An allowed emission follows the durable evidence model already frozen in ADR
0007:

1. the host stores or reuses the content-addressed payload blob
2. the host mints a fresh evidence-record ID and URI for this emission
3. the host records per-emission metadata plus producing execution linkage
4. the host returns a host-issued `EvidenceRef` pointing at the evidence-record
   URI

Blob identity and evidence-record identity remain separate:

- identical payloads may share blob storage
- identical payloads must not silently collapse per-emission evidence-record
  identity
- `EvidenceRef` authority remains tied to the host-issued evidence-record URI,
  not the blob digest alone

The skill is not allowed to fabricate durable evidence handles:

- `SkillOutput.evidence` must correspond to the host-issued refs actually emitted
  during execution
- guest-authored output that names evidence refs the host did not issue is
  invalid

Safe defaults in the current repository are:

- no family grant means no evidence emission authority
- omitted optional family constraints are only unbounded within this evidence
  family, not outside it
- even with an unbounded `emit-evidence` grant, the guest still cannot enumerate,
  update, or delete stored evidence

Nested child behavior is subset-only:

- child evidence emission constraints are reduced from the parent grant and the
  child requirement
- child audiences, redactions, and max-bytes ceilings may narrow only

## Consequences

Positive:

- Guild keeps evidence as a durable, host-owned write boundary
- per-emission lineage survives blob deduplication
- denial debugging is clearer because size, audience, and redaction each have
  explicit semantics

Costs and limits:

- the family is intentionally narrower than a general object-store API
- there is no mutable evidence update path
- retention, quotas, and richer evidence query surfaces remain deferred

## Explicit invariants

- `emit-evidence` is not arbitrary storage access
- evidence refs are host-issued
- same payload digest does not imply same evidence-record identity
- guests cannot self-assert durable evidence refs
- child evidence authority cannot widen beyond the parent grant

## Explicit non-goals / deferred work

- evidence update or delete APIs
- arbitrary object-store reads or writes
- retention policy
- richer write-style artifact families
- remote object stores

## Cross-references

- `SPECS.md`
- `ARCHITECTURE.md`
- `docs/adr/0007-evidence-record-schema.md`
- `docs/adr/0012-capability-policy-layering-model.md`
- `crates/guild-types/src/lib.rs`
- `crates/guild-runner/src/lib.rs`
- `crates/guild-runner/tests/inspect_slice.rs`
- `crates/guild-runner/tests/composition.rs`
- `examples/skills/explain-execution/README.md`

