# ADR 0012: Capability policy layering model

Status: Accepted  
Date: 2026-03-17

For the current normative runtime contract, see `SPECS.md`, `wit/guild-skill-v1.wit`, and the core Rust runtime/types.
The frozen active-family registry now lives in `SPECS.md` section "Contract Surface v1 (core)".

## Context

ADR 0005 froze Guild's typed capability families and the honest active inspect
surface. ADR 0008 froze the small local policy evaluator and the separation
between caller-requested capabilities and host-granted capabilities.

What still lived too much in code and repo folklore was the layer boundary
between:

- manifest-declared capability requirements
- caller-requested capabilities
- host-owned policy evaluation
- final granted capabilities
- host-owned denials
- runtime enforcement of granted capabilities

Guild is now far enough along that future capability work needs that layering
written down explicitly before more families land.

## Decision

Guild uses a layered capability policy model. The layers are distinct and must
not be collapsed into a single "permission blob" concept.

The current repository model is:

1. `SkillManifest.capabilities` declares the capability requirements of the
   resolved skill and its installed dependency tree surface.
2. `CallerRequest.requested_capabilities` carries caller intent only.
3. The host loads local policy input from `policy.json` when present, otherwise
   from the built-in default local profile.
4. The host derives trust-sensitive execution metadata from installed state:
   `verification_state` and `trust_tier`.
5. The host selects exactly one named policy profile using actor and/or tenant
   bindings, or falls back to `default_profile`.
6. The host evaluates policy and produces a host-owned `PolicyDecision` plus the
   authoritative `granted_capabilities`.
7. The runtime executes only with `ResolvedExecutionEnvelope.granted_capabilities`
   and enforces those grants again at each host import boundary after projecting
   the host-owned grant model into the active inspect ABI.

The repository now uses the following terms precisely:

- Requirement: a manifest-declared `CapabilityRequirement`. This is skill-authored
  contract surface, not authority.
- Request: a caller-supplied `GrantedCapability` candidate in
  `CallerRequest.requested_capabilities`. This is policy input, not authority.
- Grant: a host-approved `GrantedCapability` in the resolved execution envelope
  and execution context. This is the only authority the guest receives.
- Denial: a host-owned rejection or reduction reason recorded in
  `PolicyDecision.reasons` or in host-owned termination metadata.
- Runtime enforcement: the host-side check that a concrete guest operation fits
  the already granted family-specific constraints.

The draft M3 schema bundle under `docs/schemas/draft-v1/` does not replace that terminology. If the bundle uses a broader effect-class vocabulary or names concepts differently, the host-owned requirement/request/grant/denial/enforcement layering in this ADR remains canonical until repo-wide alignment is finished.

The built-in default local profile is intentionally small:

- start from caller-requested capabilities
- clamp them to the declared capability surface of the resolved local dependency
  tree
- preserve required manifest capabilities when policy allows them
- reject execution before guest start if required capabilities are no longer
  covered

When named policy rules reduce that starting point, the current repository uses
typed family-specific ceilings rather than stringly matching. `cap` rules may
split one broader grant into a narrower same-family union, while `deny` rules
conservatively remove any grant that overlaps a denied typed ceiling.

Policy selection and validation are fail-closed:

- unreadable, unparseable, or invalid `policy.json` prevents policy load
- ambiguous profile binding matches reject execution
- missing referenced profiles reject execution
- invalid requested capability shapes reject execution

Child execution uses the same layering model again, starting from a reduced
subset:

- parent grants are reduced against the child manifest requirements on a
  family-by-family basis
- the resulting child request carries only that reduced subset
- the child then re-enters the same host policy path
- child authority may narrow again, but it must never widen beyond the parent
  grant

Capability policy is family-specific and typed where Guild semantics are stable.
The current active inspect families are:

- `http-request`
- `read-resource`
- `invoke-skill`
- `emit-evidence`
- `log-write`

Each family has its own constraint semantics. Guild does not use one generic
cross-family permission blob as its conceptual model.

Unsupported families are not "implicitly available" because they appear in
shared contracts. In the active Wasm inspect slice, unsupported families are
rejected before execution under the runtime surface allowlist, and their imports
are absent from `guild-skill-inspect-v1`.

Unsupported runtime surface remains distinct from policy denial:

- policy denial answers "the host chose not to grant this execution"
- unsupported runtime surface answers "the active inspect runtime does not
  expose this broader surface today"
- in the current repository, broader Guild component imports in the active
  inspect path are rejected as host-owned `unsupported-runtime-surface` rather
  than being recast as policy denials or generic runtime failures

Trust and verification stay separate from grants:

- verification answers what installed verification metadata exists
- trust tier answers how the local host currently classifies that install
- policy answers what authority the host grants now, given the request,
  requirements, profile, verification state, and trust tier

## Consequences

Positive:

- denial debugging now has one stable mental model
- future capability work has to define family-specific semantics instead of
  smuggling fields into an untyped permission bag
- trust-tier-aware policy remains host-owned rather than caller-defined
- nested execution keeps least authority as a first-class rule

Costs and limits:

- adding a new capability family now requires both typed contract surface and a
  corresponding policy story
- the current local policy layer is intentionally small and deterministic rather
  than a general policy platform
- unsupported families stay unavailable until the runtime actually supports
  them

## Explicit invariants

- `RequestedSkillRef` is not executable identity
- `ResolvedSkillRef` is the only executable identity
- manifest requirements are not grants
- caller requests are not grants
- `PolicyDecision` is host-owned durable metadata
- final grants are host-decided
- runtime host imports enforce granted capabilities, not caller intent
- child grants are subsets of parent grants
- guests never receive ambient authority through policy omission or ambiguity
- unsupported capability families in the active inspect slice fail before guest
  execution
- unsupported future imports do not appear in the active inspect guest ABI
- unsupported runtime surface stays distinct from policy denial in durable
  records and explain/debug flows

## Explicit non-goals / deferred work

- a generic policy DSL
- remote or distributed policy evaluation
- plan/apply policy semantics
- filesystem runtime support
- policy-specific MCP tools
- treating shared-contract future capability IDs as implemented runtime surface

## Cross-references

- `README.md`
- `SPECS.md`
- `ARCHITECTURE.md`
- `docs/adr/0005-capability-schema-and-active-inspect-profile.md`
- `docs/adr/0008-local-policy-evaluator.md`
- `crates/guild-types/src/lib.rs`
- `crates/guild-runner/src/lib.rs`
- `crates/guild-registry/src/lib.rs`
- `crates/guild-mcp/examples/inspect_policy_local.rs`
