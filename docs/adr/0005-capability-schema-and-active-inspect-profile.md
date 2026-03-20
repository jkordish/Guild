# ADR 0005: Capability schema and active inspect profile

## Status

Accepted

## Context

Guild's contracts already define a broader capability universe than the current working runtime slice actually supports.

That is fine only if the active surface stays brutally honest.

The repository now has real host-enforced capability evaluation, real denial persistence, and real nested child-grant reduction. What was missing was an ADR that freezes the current capability model so future work does not confuse:

- manifest-declared requirements with host-owned grants
- shared contract vocabulary with active runtime support
- host-owned denials with guest-domain failures
- future policy ambitions with the current inspect implementation

## Decision

Guild uses explicit host capabilities instead of ambient authority.

The current capability model is defined as follows:

- skills declare capability requirements in manifests as `CapabilityRequirement`
- callers may ask for capabilities in `CallerRequest.requested_capabilities`
- the host decides the authoritative granted slice in `ResolvedExecutionEnvelope.granted_capabilities`
- the guest receives only the granted slice through `ExecutionContext.granted_capabilities`

The active typed constraint families in the current inspect implementation are:

- `HttpRequestConstraints`
- `ReadResourceConstraints`
- `InvokeDependencyConstraints`
- `EmitEvidenceConstraints`
- `LogConstraints`

These map to the currently active inspect capability families:

- `http-request` with `read` access
- `read-resource` with `read` access
- `invoke-skill` with `invoke` access
- `emit-evidence` with `write` access
- `log-write` with `write` access

The draft schema bundle under `docs/schemas/draft-v1/` is not a replacement vocabulary for those current product terms. Where the bundle uses broader or differently named effect classes such as `component.invoke`, `net.connect`, or `fs.*`, the repository still treats the host-owned capability-family model above as canonical and keeps the schema bundle explicitly marked draft until that mapping is closed repo-wide.

The current Wasm inspect runtime rejects unsupported capability families before execution.

The active inspect guest ABI is now a dedicated world, `guild-skill-inspect-v1`.
That world exposes only the active inspect imports and only the active capability
IDs. Secret, cache, clock, and filesystem imports are not part of the inspect
guest ABI.

The active inspect runtime now also preflights Guild component imports before
instantiation. In the current repository, only
`guild:skill/inspect-types@1.0.0` and `guild:skill/inspect-host@1.0.0` are
allowed through that path. Broader Guild import surfaces such as
`guild:skill/types@1.0.0` or `guild:skill/host@1.0.0` are rejected by the host
as `unsupported-runtime-surface` before guest execution continues.

The active inspect runner also owns one explicit host-to-guest projection layer.
In the current repository that means:

- inspect guest `ExecutionContext` is a bounded subset and intentionally omits `mode`
- current active grant projections for `http-request`, `read-resource`, `invoke-skill`, `emit-evidence`, and `log-write` are all full at the current guest-visible grant-shape level
- host-owned request intent, `PolicyDecision`, provenance, and durable evidence metadata remain outside the guest ABI and are read through durable records or Guild resources when needed

That preflight rule applies to both:

- manifest-declared capability requirements
- execution-time grants

Broader capability IDs and host-side vocabulary may still exist in shared Rust
types or broader WIT contract surface for future phases. They are not part of
the active inspect profile unless the runtime actually supports them. In
particular, the current inspect slice activates one bounded outbound network
family, `http-request`, but still does not activate secret, cache, clock, or
filesystem capability families in the active inspect guest ABI.

Manifest requirements and host grants stay separate:

- manifests describe what a skill says it needs
- grants describe what the host actually allows for a specific execution
- execution is rejected when required capabilities are not covered by the granted slice
- guest code does not self-assert durable authority by naming capabilities it was not granted

The host enforces typed constraint coverage, not loose string matching:

- `http-request` may constrain schemes, exact hosts, domain suffixes, ports, methods, path prefixes, redirect following, timeout, response size, and explicit risky-destination allowances such as loopback, private-network, link-local, and raw IP literals
- `read-resource` may constrain `uri_prefixes` and `resource_kinds`
- `invoke-skill` may constrain dependency aliases
- `emit-evidence` may constrain `max_bytes`, `audiences`, and `redactions`
- `log-write` may constrain permitted severities

`http-request` is now real in the active inspect profile:

- the guest ABI exposes the full active HTTP grant shape used by the inspect runtime
- the host implementation uses `wasmtime-wasi-http` behind a thin translation layer
- the host parses absolute URLs and enforces typed scheme, host, domain-suffix, port, path, method, timeout, size, redirect, and risky-destination constraints before or during dispatch as appropriate
- child execution receives only a reduced `http-request` grant derived from the parent slice
- the public MCP surface stays at one tool, `guild.inspect`; HTTP is exercised through skill execution rather than new MCP tools

`read-resource` scope matching is canonical and typed:

- manifest and grant `uri_prefixes` must use canonical Guild scope roots
- the current supported roots are `guild://executions/`, `guild://objects/records/`, and `guild://objects/sha256/`
- resource authorization parses concrete Guild URIs into typed execution, evidence-record, or blob forms before matching
- malformed URIs and non-canonical scope roots fail closed

Child invocation narrows authority rather than widening it:

- composite execution derives child grants from parent grants plus child manifest requirements
- reduction is family-specific and typed
- if a required child capability cannot be reduced from the parent grant, invocation is denied
- the current model does not let a child expand beyond the parent's granted scope

Denials are host-owned:

- grant-validation failures are host rejections
- unsupported active-surface capability use is a host rejection
- unsupported broader Guild import surface in the active inspect runtime path is
  a host-owned `unsupported-runtime-surface` rejection, not a generic runtime
  trap or policy denial
- supported host-import denials remain host-classified outcomes with host-owned codes and details
- denials are not reclassified as guest-authored skill failures for durable record purposes

This ADR does not define a general policy engine.
It freezes the current capability schema and the current active inspect profile.

## Consequences

Positive:

- the active runtime surface is honest about what actually works today
- host authority stays separate from skill-authored requests
- composite execution preserves least-authority behavior through typed child-grant reduction
- `read-resource` authorization now matches Guild URI semantics instead of permissive string prefixes
- the inspect slice now has one standards-aligned outbound HTTP family with real host behavior instead of a stubbed or implied network surface

Costs and current limits:

- the current inspect surface is intentionally smaller than the shared contract vocabulary
- adding a new capability family now requires real host behavior, typed constraints, and explicit documentation
- callers can request capabilities, but the final granted slice is now decided by the host-owned local policy evaluator described in ADR 0008 rather than by caller intent alone

## Explicit invariants

- skills never receive ambient filesystem, environment, unrestricted network, or subprocess authority through the default path
- bounded `http-request` authority is host-mediated and grant-scoped, not ambient outbound networking
- manifest requirements are skill-authored declarations, not grants
- grants and denials are host-owned
- unsupported capability families in the active Wasm inspect slice are rejected before execution
- unsupported future imports are absent from `guild-skill-inspect-v1`
- broader Guild component imports in the active inspect path fail closed as
  host-owned `unsupported-runtime-surface`
- `read-resource` authorization uses canonical parsed Guild URI scopes
- child execution narrows grants; it does not widen them

## Explicit non-goals / deferred work

- defining a general policy language or policy engine
- implying that unsupported shared-contract capability families are production-ready
- activating broader network, secret, cache, or clock capability families beyond bounded `http-request` in the inspect slice
- deciding capability behavior for `plan` mode
- deciding capability behavior for `apply` mode

## Cross-references

- `README.md`
- `SPECS.md`
- `ARCHITECTURE.md`
- `MEMORY.md`
- `docs/adr/0001-guild-thesis.md`
- `docs/adr/0003-guest-abi-vs-host-record-boundary.md`
- `docs/adr/0012-capability-policy-layering-model.md`
- `docs/adr/0013-read-resource-policy-family.md`
- `docs/adr/0014-invoke-skill-policy-family.md`
- `docs/adr/0015-emit-evidence-policy-family.md`
- `docs/adr/0016-log-write-policy-family.md`
- `docs/adr/0017-http-request-policy-family.md`
- `docs/adr/0018-filesystem-policy-contract-not-yet-implemented.md`
- `wit/guild-skill-v1.wit`
- `crates/guild-types/src/lib.rs`
- `crates/guild-manifest/src/lib.rs`
- `crates/guild-runner/src/lib.rs`
- `crates/guild-runner/src/inspect_projection.rs`
- `crates/guild-runner/tests/inspect_slice.rs`
- `crates/guild-runner/tests/resource_reads.rs`
- `crates/guild-runner/tests/composition.rs`
