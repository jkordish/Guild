# Contracts

Guild starts from contracts, not implementation folklore.

This document describes the core contracts that must stay aligned across:

- Rust types in `crates/guild-types`
- manifest types in `crates/guild-manifest`
- the ABI in `wit/guild-skill-v1.wit`
- example manifests and fixtures

## 1. Identity contract

Every skill has four identities:

- **logical identity**: `namespace + name`
- **API identity**: semantic version
- **artifact identity**: immutable digest
- **ABI identity**: platform contract version

Rules:

- execution happens by digest, not by floating version
- dependency snapshots pin digests
- caches should key on digest + canonicalized input + granted capabilities + mode

## 2. Manifest contract

The manifest describes everything needed to resolve and reason about a skill before running it.

Required categories:

- key and version
- runtime kind and ABI
- input and output schema URIs
- behavior metadata
- capabilities
- dependencies
- publisher and package metadata
- tests and examples

## 3. Execution contract

Every run should normalize to the same shape:

- execution request
- execution context
- execution result
- diagnostics
- evidence
- effects
- provenance

Execution modes:

- `inspect`
- `plan`
- `apply`

`apply` exists conceptually from day one, but should remain heavily gated until idempotency, approvals, and audit paths are real.

## 4. Capability contract

Skills do not get broad host access by default.

Capabilities are declared by the skill and granted by policy. Initial capability families:

- `http-request`
- `read-resource`
- `invoke-skill`
- `get-secret`
- `cache-read`
- `cache-write`
- `log-write`
- `monotonic-clock`
- `wall-clock`

Each capability may carry constraints such as:

- allowed hosts
- allowed HTTP methods
- timeouts
- named secret handles
- invocation depth

## 5. Evidence contract

Evidence is first-class output.

Each evidence item should carry:

- URI
- optional digest
- audience
- redaction class
- optional freshness metadata

Evidence should be referenceable independently of the human-readable summary.

## 6. Composition contract

Composite skills should be possible without inventing a giant workflow language in the first week.

Initial composition posture:

- support child skill invocation
- inherit reduced budgets
- propagate trace context
- pin dependency digests
- forbid cycles and accidental unbounded recursion

## 7. MCP contract

MCP is the external interface, not the internal execution model.

Guild should expose a stable façade rather than one tool per skill.

Expected stable tools:

- `guild.search`
- `guild.describe`
- `guild.inspect`
- `guild.plan`
- `guild.apply`

## 8. Source of truth rules

If the ABI changes, update:
- `wit/guild-skill-v1.wit`
- `crates/guild-types`
- `docs/contracts.md`

If manifest fields change, update:
- `crates/guild-manifest`
- examples in `examples/`
- `docs/contracts.md`

If execution semantics change, update:
- `crates/guild-types`
- runner interfaces
- docs and ADRs where relevant
