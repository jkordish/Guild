# ADR Index

ADRs record rationale and accepted decisions. They are not the current normative contract surface.

For how Guild behaves today, use `SPECS.md` and `ARCHITECTURE.md`.
For the frozen core runtime-contract surfaces in this milestone, see `SPECS.md` section "Contract Surface v1 (core)".
For the current long-term direction, use
[`../strategy/session-substrate/00-umbrella-epic.md`](../strategy/session-substrate/00-umbrella-epic.md)
and ADR `0020`. `project-positioning.md` remains a compatibility bridge for the
prior framing. ADR `0001-guild-thesis.md` is historical rationale, not the
current framing source.

## Current ADRs

- `0001-guild-thesis.md` - accepted historical foundation for local-first, Rust-core, Wasm-first, contract-first Guild execution; current framing now lives in `docs/project-positioning.md`
- `0002-skill-output-and-execution-record.md` - accepted split between skill-authored output and host-owned execution records
- `0003-guest-abi-vs-host-record-boundary.md` - accepted rule that WIT is the guest ABI, Rust types are the durable host model, and translation is explicit
- `0004-installed-bundle-format.md` - accepted current local installed-state bundle format, signature envelope, import verification, and installed portability rules
- `0005-capability-schema-and-active-inspect-profile.md` - accepted typed capability model and honest active inspect capability surface
- `0006-execution-record-schema.md` - accepted durable execution record shape, receipt model, persisted-attempt boundary, and child lineage rules
- `0007-evidence-record-schema.md` - accepted split between blob identity and per-emission evidence-record identity
- `0008-local-policy-evaluator.md` - accepted local host-owned policy evaluation, `policy.json` profile shape, and separation between requested and granted capabilities
- `0009-oci-image-layout-mapping.md` - accepted OCI image layout mapping for the existing signed installed-bundle transport without changing Guild's local trust model
- `0010-oci-registry-transport.md` - accepted OCI registry push and pull for the existing signed installed-bundle transport without changing Guild's local trust model
- `0011-bounded-artifact-query-resources.md` - accepted bounded execution-query resources over the canonical persisted execution store without widening the public MCP tool surface
- `0012-capability-policy-layering-model.md` - accepted parent policy ADR defining requirements vs requests vs grants vs denials vs runtime enforcement
- `0013-read-resource-policy-family.md` - accepted per-family policy contract for canonical Guild resource reads and bounded query resources
- `0014-invoke-skill-policy-family.md` - accepted per-family policy contract for alias-scoped composite invocation and child grant reduction
- `0015-emit-evidence-policy-family.md` - accepted per-family policy contract for host-mediated evidence emission and per-emission identity
- `0016-log-write-policy-family.md` - accepted per-family policy contract for explicit severity-scoped guest logging
- `0017-http-request-policy-family.md` - accepted per-family policy contract for bounded outbound HTTP authority in the active inspect slice
- `0018-filesystem-policy-contract-not-yet-implemented.md` - accepted design-only guardrail ADR for future filesystem policy semantics without implying runtime support
- `0019-thin-guild-cli.md` - accepted first-class `guild` CLI contract for local install, read-only grant templates, run, read, diagnostics, transport, trust, focused help topics, and MCP command workflows
- `0020-evolve-guild-toward-a-trusted-session-substrate-for-isolated-harness-execution.md` - accepted evolution from portable-skill/runtime-first framing toward session, harness, admission, and receipt language without discarding current shipped trust surfaces
- `0021-adopt-the-effect-kernel-as-guilds-mutation-truth-boundary.md` - proposed adoption of the pure effect kernel as Guild's exact external-mutation, receipt, custody, and non-repeating recovery boundary

## Backlog

The next ADRs should focus on execution, trust, and receipt shape that is still intentionally narrow or deferred:

1. Retention and evidence query surfaces
2. Remote publication and trust

## Working Rule

If a change materially alters trust boundaries, installed portability, runtime semantics, or durable record shape, it should land with a new ADR or an explicit update to an existing one.
