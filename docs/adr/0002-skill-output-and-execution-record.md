# ADR 0002: Split skill output from execution records

## Status

Accepted

## Note

ADR 0003 refines the boundary frozen here. ADR 0002 remains the historical split decision; ADR 0003 is the authoritative statement of the WIT guest ABI versus durable host record layering model.
For the current normative runtime contract, see `SPECS.md`, `wit/guild-skill-v1.wit`, and the core Rust runtime/types.

## Context

Guild originally modeled a single execution result shape across Rust types, the WIT ABI, and host-facing docs. That shape mixed together:

- caller intent and resolved execution state
- skill-authored output
- host-owned facts such as status, metrics, and provenance

That made digest-pinned execution optional in public types, hid granted capabilities from the execution contract, and let the ABI drift from the Rust model.

## Decision

Guild will separate execution into four layers:

1. requested intent via `RequestedSkillRef`
2. resolved execution via `ResolvedSkillRef`, `ResolvedExecutionEnvelope`, `ExecutionContext`, and `CapabilityGrantSet`
3. skill-authored output via `SkillOutput` and `SkillError`
4. host-owned records via `ExecutionRecord`, `ExecutionStatus`, `ExecutionMetrics`, and `Provenance`

Additional consequences of this split:

- the runner only accepts resolved skill refs
- manifests declare supported execution modes through `ModePolicy`
- the WIT ABI returns `skill-output` from `skill.run`
- child invocation is typed and returns host-owned execution records
- evidence stays inside `SkillOutput`; raw evidence side channels are removed
- persistent execution records and evidence objects remain host-owned even when skills emit evidence refs

## Consequences

Positive:

- digest-pinned execution becomes enforceable by type
- the ABI and Rust contracts describe the same trust boundary
- granted capabilities become explicit input to execution
- skills can no longer fabricate host-owned provenance and metrics

Negative:

- this is a breaking contract change across Rust types, WIT, docs, and examples
- future runner implementations must construct execution records explicitly
- manifest authors now need to declare mode support instead of relying on prose
