# ADR 0007: Evidence record schema

## Status

Accepted

## Context

Guild's current evidence model is already split in code:

- digest-addressed blob storage for payload bytes
- host-issued per-emission evidence records for durable metadata
- guest-visible `EvidenceRef` values that identify evidence-record URIs

That split is one of the repository's important trust and audit boundaries. Without an explicit ADR, future cleanup pressure could easily collapse evidence back into digest-only blobs and erase per-emission lineage.

## Decision

Guild's durable evidence model has two layers:

- `EvidenceBlobRecord` for content-addressed payload storage
- `EvidenceRecord` for host-owned per-emission metadata

Blob identity and evidence-record identity are distinct.

The current blob layer is defined as follows:

- payload bytes are stored under `guild://objects/sha256/{digest}`
- the blob record is keyed by payload digest
- the current blob metadata schema is `EvidenceBlobRecord { uri, sha256, size_bytes }`
- identical payloads may reuse the same blob storage

The current evidence-record layer is defined as follows:

- each emission gets a new host-issued evidence record ID and URI under `guild://objects/records/{evidence_record_id}`
- the current metadata schema is `EvidenceRecord { uri, blob_uri, mime_type, sha256, size_bytes, title, audience, redaction, freshness, produced_by_execution }`
- `produced_by_execution` stores the producing execution ID in the current implementation

Evidence emission is per-emission, not per-digest:

- the host first ensures the underlying blob exists or validates the existing blob metadata
- the host then mints a fresh evidence-record ID for the emission
- two executions emitting the same payload digest do not share evidence-record identity
- two emissions from the same execution also do not share evidence-record identity unless the host explicitly reuses the same record, which the current implementation does not do

`EvidenceRef` is the guest-visible durable handle and identifies the evidence-record URI, not the raw blob URI.

`ExecutionRecord.emitted_evidence` stores host-loaded `EvidenceRecord` values for produced evidence, so execution records preserve per-emission metadata and linkage rather than only digest labels.

Current read semantics are intentionally split:

- host-side `load_evidence_record(uri)` returns `EvidenceRecord` metadata
- reading `guild://objects/records/{evidence_record_id}` through the current Guild resource backend dereferences the record to the underlying payload bytes
- reading `guild://objects/sha256/{digest}` returns the raw blob payload bytes

This means an evidence-record URI is a durable Guild resource in the current model, but its current resource-read behavior is payload dereference rather than serialized metadata readout.

## Consequences

Positive:

- Guild can deduplicate payload storage without collapsing per-emission lineage
- execution records can point at durable evidence metadata rather than bare digests
- explain and inspect flows can reuse emitted evidence through stable host-issued references
- title, audience, redaction, freshness, and producing execution metadata stay attached to each emission

Costs and current limits:

- the model stores more objects than a blob-only design
- current resource reads do not yet expose `EvidenceRecord` metadata directly as a JSON resource payload
- the current schema records producing execution lineage but not a richer read-attribution graph

## Explicit invariants

- evidence-record IDs and evidence-record URIs are host-issued
- blob identity is content-addressed by payload digest
- evidence-record identity is per emission and must not silently collapse across executions
- `EvidenceRef` identifies the evidence-record URI rather than only the blob digest
- same payload digest does not imply same durable evidence record
- per-emission metadata and lineage remain attached to evidence records even when blob storage is deduplicated

## Explicit non-goals / deferred work

- search or indexing over evidence objects
- richer evidence query APIs
- non-local object stores
- update-in-place or mutable evidence APIs
- durable read-lineage beyond current producing-execution linkage

## Cross-references

- `README.md`
- `SPECS.md`
- `ARCHITECTURE.md`
- `MEMORY.md`
- `docs/adr/0003-guest-abi-vs-host-record-boundary.md`
- `docs/adr/0006-execution-record-schema.md`
- `crates/guild-types/src/lib.rs`
- `crates/guild-registry/src/lib.rs`
- `crates/guild-runner/src/lib.rs`
- `crates/guild-mcp/examples/explain_execution_local.rs`
- `crates/guild-runner/tests/inspect_slice.rs`
- `crates/guild-runner/tests/resource_reads.rs`
