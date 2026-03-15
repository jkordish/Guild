# ADR 0003: Guest ABI vs host record boundary

## Status

Accepted

## Context

Guild already had the right instinct: separate requested identity from resolved identity, keep the guest ABI smaller than the host runtime, and persist host-owned execution records. The problem was that the contract story was still soft in a few important places:

- the repo described WIT and Rust as if they should simply "match"
- host-owned execution state and guest-owned outputs were still easy to mentally collapse together
- manifest schema version, skill API version, and guest ABI version were still overloaded into one axis
- execution records persisted durable host truth, but the boundary between guest data and host enrichment was not frozen as an explicit platform rule

That ambiguity is dangerous in a contracts-first repository. Once translation drift becomes normal, contract bugs start getting explained away as implementation detail.

## Decision

Guild freezes the boundary model as follows:

1. `wit/guild-skill-v1.wit` is the canonical guest-wire contract.
2. Rust host types are the canonical durable platform model.
3. Translation between those layers is explicit, centralized, and tested.
4. The guest ABI stays intentionally small.
5. Host-owned durable records stay out of WIT return types.

Concretely, this means:

- guest-owned contract surface:
  - `ExecutionContext`
  - guest-visible capability grants
  - host imports such as `read-resource`, `emit-evidence`, and `invoke-dependency`
  - `SkillOutput`
  - `SkillError`
- host-owned runtime and durable surface:
  - `CallerRequest`
  - `ResolvedExecutionEnvelope`
  - `ExecutionReceipt`
  - `ExecutionRecord`
  - `EvidenceRecord`
  - `PolicyDecision`
  - termination or rejection detail
  - provenance, metrics, and Guild resource URIs

Additional required consequences:

- runner entrypoints accept only resolved execution envelopes
- `RequestedSkillRef` and `ResolvedSkillRef` remain separate public types
- `ExecutionRecord` may embed guest-authored `SkillOutput`, but the durable record itself remains host-owned
- `EvidenceRef` remains the guest-visible handle while `EvidenceRecord` is the host-owned durable metadata record behind it
- Guild runtime capability grants remain distinct from MCP transport authorization
- `ExecutionReceipt` and `ExecutionRecord` should stay simple enough to map to future MCP Tasks later, but Guild does not model MCP Tasks now

## Consequences

Positive:

- WIT remains a real ABI boundary instead of decorative schema
- host-owned policy, provenance, and durable identifiers no longer pressure the guest ABI to grow
- execution and evidence records become cleaner Guild-native resources
- manifest versioning becomes honest about the different version axes Guild actually carries

Negative:

- this is a breaking contract pass across shared types, manifests, examples, and tests
- translation logic must now be maintained deliberately instead of hand-waved
- durable record schemas become richer and therefore more explicit to evolve

## Relationship to prior ADRs

ADR 0002 remains accepted history and still records the original split between skill-authored output and host-owned execution records.

ADR 0003 is the authoritative refinement of that split. When `0002` and current code leave room for interpretation, this ADR wins.
