# ADR 0008: Local Policy Evaluator

Status: Accepted  
Date: 2026-03-16

## Context

Guild already had typed capability families, host-owned denials, and narrow
least-authority child grant reduction. The remaining weakness was that
caller-requested capabilities still sat too close to the final granted slice.

That made the authority model less trustworthy than the rest of the substrate:

- caller intent and host authority were too easy to confuse
- durable execution records could show granted capabilities without making the
  host policy step explicit
- installed verification metadata existed, but could not yet influence local
  capability activation
- future trust tiers and distribution work had no concrete local policy hook to
  build on

Guild needs a real host-owned policy step now, but it does not need a giant
policy platform.

## Decision

Guild now uses a small local host-owned policy evaluator before execution.

The repository adopts these rules:

1. `guild.inspect` and other caller-facing paths still accept
   `requested_capabilities`, but those values are policy input only.
2. The host evaluates local policy and produces a durable `PolicyDecision`
   before guest execution.
3. The runner executes only with the final `granted_capabilities` computed by
   policy.
4. Policy outcomes are `allowed`, `reduced`, or `rejected`.
5. Policy reasons are host-owned structured metadata, not guest-authored text.
6. Child execution starts from the parent-derived subset and then goes through
   the same host policy path again, so policy can narrow but never widen child
   authority.

## Current repository shape

The current repository uses one small local-first policy source:

- an optional `policy.json` at the Guild root
- a built-in default local policy profile when that file is absent
- fail-closed behavior when `policy.json` exists but cannot be read, parsed, or
  validated

The current typed policy config is intentionally small:

- `format_version`
- `default_profile`
- `profiles`
- `bindings`

Rules are also intentionally small:

- exact-match selectors for `skills`, `publisher_ids`, host-owned
  `trust_tiers`, and host-owned `verification_states`
- actor and tenant profile selection through named `bindings`
- effects of either `deny` or `cap`
- an `applies_to` target of `requested`, `required`, or `any`
- typed capability ceilings expressed with the existing capability grant model

This is not a general policy language. It is a local deterministic reduction
layer.

## Default profile

The built-in default profile keeps current inspect-mode proof flows working
without requiring extra setup:

- start from caller-requested capabilities
- clamp them to the declared capability surface of the resolved local
  dependency tree
- reject before guest execution if the currently executing skill's manifest
  requirements are no longer satisfied

That means:

- callers cannot activate undeclared capability families just by asking
- composite flows keep enough declared authority available for child execution
  while still remaining bounded by typed host evaluation
- policy remains host-owned even when no custom `policy.json` is present

## Trust-aware execution metadata

Installed verification metadata and trusted publisher identity now feed a
derived host-owned local trust tier before execution.

The current repository uses these host-owned values:

- `verification_state`
- `trust_tier`
- `profile_name`

This keeps trust verification and policy decision separate:

- verification answers "what trust metadata does this install carry?"
- trust-tier derivation answers "how does the host classify that install right
  now?"
- policy answers "given that metadata and the selected profile, what authority
  does the host allow now?"

## Consequences

Positive:

- requested capabilities are now clearly separated from granted capabilities
- policy decisions are durable and explainable
- authority is materially more host-owned than before
- trust metadata now has one real local path to matter at execution time
- the MCP surface stays small because policy remains a host concern rather than
  a new tool family

Costs and current limits:

- local policy is intentionally simple and exact-match oriented
- `policy.json` is a repository-local config file, not a distributed control
  plane
- the current rule model only reduces or removes authority; it does not invent
  grants the caller never requested
- this ADR does not define plan/apply policy behavior

## Explicit invariants

- `RequestedSkillRef` is still not executable identity
- `ResolvedSkillRef` is still the only executable identity
- `PolicyDecision` remains host-owned durable metadata
- guests never decide their final authority
- nested child grants remain subsets of parent grants
- invalid or unreadable local policy fails closed

## Non-goals

- remote policy distribution
- OPA, Cedar, or another external policy engine
- multi-tenant SaaS authorization infrastructure
- a general enterprise policy language
- widening the public MCP surface with policy-specific tools
- changing the guest ABI to expose policy mechanics directly

## Cross-references

- `README.md`
- `SPECS.md`
- `ARCHITECTURE.md`
- `MEMORY.md`
- `docs/adr/0005-capability-schema-and-active-inspect-profile.md`
- `docs/adr/0006-execution-record-schema.md`
- `crates/guild-types/src/lib.rs`
- `crates/guild-registry/src/lib.rs`
- `crates/guild-runner/src/lib.rs`
- `crates/guild-mcp/src/lib.rs`
- `crates/guild-mcp/examples/inspect_policy_local.rs`
