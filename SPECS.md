# Guild Specification

Status: Draft v0.1  
Scope: core normative contract for Guild implementations  
Audience: runtime implementers, registry authors, skill authors, security reviewers, platform engineers

## 1. Purpose

Guild defines a local-first execution and artifact model for AI skills.

Its purpose is to make skills:

- portable across Guild roots
- inspectable after execution
- policy-bounded at runtime
- reproducible through immutable execution identity
- traceable through durable execution and evidence records

Guild is not a prompt convention, not a hosted agent brand, and not a loose orchestration pattern. It is a concrete system contract.

## 2. Core Thesis

A Guild-conformant system MUST turn a user or system request for a skill into a governed execution pipeline:

1. accept a human-meaningful skill request
2. resolve it to an immutable executable identity
3. execute it within a constrained host-managed boundary
4. persist what happened as durable host-owned records
5. permit later inspection and explanation grounded in those records

The central claim of Guild is simple: AI skills should be treated like real software units with identity, runtime constraints, receipts, and evidence, not as informal prompt-era behavior.

## 3. Non-Goals

Guild does not try to solve the following directly:

- universal semantic correctness of skill outputs
- model truthfulness in the general case
- centralized SaaS orchestration as a requirement
- arbitrary unrestricted plugin execution
- replacing transport or resource protocols such as MCP
- UX concerns unrelated to artifact identity, execution, or inspection

Guild MAY integrate with models, MCP servers, and orchestration systems, but those integrations are not the core contract.

## 4. Normative Language

The key words MUST, MUST NOT, SHOULD, SHOULD NOT, and MAY in this document are to be interpreted as normative requirements.

## 5. Terms

### 5.1 Skill

A software unit that can be requested, resolved, executed, and inspected under Guild.

### 5.2 Primitive skill

A skill whose execution is not defined in terms of child skill orchestration.

### 5.3 Composite skill

A skill whose execution may invoke child skills while preserving durable lineage.

### 5.4 RequestedSkillRef

A human-facing reference used by a caller to ask for a skill.

### 5.5 ResolvedSkillRef

An immutable reference identifying the exact executable artifact selected for execution.

### 5.6 Executable artifact

A packaged installed skill payload suitable for constrained execution.

### 5.7 ExecutionRecord

A durable host-owned record describing a single execution attempt and its outcome.

### 5.8 Evidence

A durable host-owned object representing material read, derived, or produced in support of execution or explanation.

### 5.9 EvidenceRef

A host-issued stable reference to a persisted evidence object.

### 5.10 Capability slice

A typed, host-defined permission set granted to a guest for a particular execution. In the current Rust implementation this is represented by `CapabilityGrantSet`.

### 5.11 Host

The trusted runtime authority responsible for resolution, policy, capability enforcement, identifier issuance, and persistence.

### 5.12 Guest

The executing skill payload running within the runtime boundary.

### 5.13 Guild root

A local logical root containing installed artifacts, metadata, execution records, evidence, and configuration.

## 6. Core Invariants

A Guild-conformant system MUST preserve the following invariants:

1. Requested identity is not executable identity. A requested reference MUST be resolved before execution.
2. Executable identity is immutable. A resolved reference MUST name the exact artifact that ran.
3. Trust-sensitive authority remains with the host. Guests MUST NOT self-assert execution IDs, evidence IDs, policy grants, or durable authority.
4. Execution attempts are durable events. Success, failure, and rejection MUST all be representable as persisted records.
5. Evidence is a first-class durable object. Evidence used or produced by execution MUST be persistable and referenceable.
6. Composition does not erase lineage. Parent and child execution relationships MUST be recoverable.
7. Capability access is explicit. Guests MUST only receive capabilities granted by the host.

## 7. System Model

Guild operates as a five-stage pipeline:

1. Request: caller submits `RequestedSkillRef` plus inputs.
2. Resolve: host resolves the request to a `ResolvedSkillRef` bound to an immutable artifact.
3. Authorize: host evaluates policy and computes the granted capability slice.
4. Execute: guest runs inside the constrained runtime boundary.
5. Persist and Inspect: host persists execution and evidence artifacts and later supports inspection or explanation.

## 8. Current Repository Baseline

This repository already implements a narrow local inspect-oriented slice of the specification:

- source manifests install into digest-pinned installed manifests plus staged artifacts
- execution resolves only against installed executable state
- `guild.inspect` runs through a real Wasmtime-backed Wasm component runtime
- resolved execution attempts persist on success, failure, and rejection
- evidence persists as durable host-owned objects behind `EvidenceRef` values
- signed local bundle export and import verifies trust, signature, and bundled file digests before installation
- composite skills invoke declared child dependencies by alias through the host boundary
- supported typed capability families are currently `read-resource`, `invoke-skill`, `emit-evidence`, and `log-write`

The current repository does not yet implement full `plan` mode, a general policy engine, or `apply` mode.

## 9. Identity and Resolution

### 9.1 Requested skill identity

Guild MUST permit callers to refer to skills using stable human-meaningful requested references.

### 9.2 Resolution requirement

Before execution begins, the host MUST resolve a `RequestedSkillRef` to a `ResolvedSkillRef`.

### 9.3 Immutable execution identity

A `ResolvedSkillRef` MUST correspond to an immutable executable artifact. Digest-pinned identity is the preferred form.

### 9.4 Execution binding

Execution MUST record the resolved identity that actually ran.

### 9.5 Mutable aliases

Implementations MAY support aliases such as channels, approvals, or environment-specific selectors, but execution MUST still bind to a concrete immutable resolved artifact.

### 9.6 Resolution visibility

The resolved identity SHOULD remain inspectable after execution.

## 10. Artifact Packaging and Lifecycle

### 10.1 Installation

Guild SHOULD support local installation of executable artifacts into host-managed records.

### 10.2 Export

Guild SHOULD support exporting installed artifacts as portable bundles.

### 10.3 Import

Guild SHOULD support importing bundles into a fresh Guild root while preserving executable identity.

### 10.4 Integrity verification

Imported artifacts MUST be integrity-verifiable. Verification failure MUST prevent installation or execution.

### 10.5 Host authority on import

The host MAY accept, reject, quarantine, or reclassify imported bundles. Import does not imply trust.

### 10.6 Installed execution source

Execution SHOULD resolve from host-managed installed state rather than directly from mutable source directories.

## 11. Runtime Boundary

### 11.1 Constrained execution

Guild SHOULD execute skills inside a constrained runtime boundary. Wasm/WASI is the reference design target.

### 11.2 No ambient authority

Guests MUST NOT implicitly inherit unrestricted host filesystem, process, network, or secret access.

### 11.3 Host mediation

Sensitive operations MUST flow through host-defined interfaces.

### 11.4 Runtime portability

The runtime design SHOULD minimize host-specific coupling so skill artifacts remain portable.

## 12. Capability Model

### 12.1 Typed capabilities

Capability grants SHOULD be represented as typed capability slices rather than vague global access.

### 12.2 Least privilege

The host MUST grant only the capabilities required for the execution.

### 12.3 Capability families

Implementations MAY define capability families such as:

- `read-resource`
- `invoke-skill`
- `emit-evidence`
- `log-write`
- `http-request`
- `get-secret`

If supported, those capabilities MUST be enforced by the host.

### 12.4 Denied capability use

If a guest attempts an ungranted operation, the host MUST deny it in a way that can be represented durably.

### 12.5 No self-escalation

Guests MUST NOT mint, widen, or self-approve their own capabilities.

### 12.6 Current typed families in this repository

The current repository enforces typed constraints for:

- `read-resource`: `uri_prefixes`, `resource_kinds`
- `invoke-skill`: `aliases`
- `emit-evidence`: `max_bytes`, `audiences`, `redactions`
- `log-write`: `levels`

Unknown fields, wrong-family constraint shapes, and empty scoped lists are validation errors.

## 13. Execution Semantics

### 13.1 Execution attempt

Every top-level invocation MUST create a host-recognized execution attempt once a resolved skill has been selected.

### 13.2 Outcome classes

Every execution attempt MUST terminate as one of at least:

- success
- failure
- rejection

### 13.3 Persistence

Those outcomes MUST be durably representable as `ExecutionRecord` resources.

### 13.4 Host-issued receipt

A top-level execution SHOULD return a host-issued receipt suitable for locating the durable execution record.

### 13.5 Composite executions

If composite skills are supported, child skill calls MUST create child execution attempts with durable parent-child linkage.

### 13.6 Rejection semantics

Policy rejection MUST be representable as a durable terminal outcome, not as a silent short circuit.

### 13.7 Mode separation

`inspect`, `plan`, and `apply` are distinct execution modes. Implementations MUST NOT smuggle mutation into `inspect` or `plan`.

### 13.8 Current repository mode support

The current repository implements `inspect` end to end, defers `plan`, and globally rejects `apply`.

## 14. ExecutionRecord Requirements

A minimally useful `ExecutionRecord` MUST contain enough host-owned data to answer what was requested, what actually ran, how it ended, and what durable artifacts were involved.

At minimum, implementations SHOULD preserve:

- execution identifier
- requested skill reference
- resolved skill reference
- parent execution identifier when applicable
- start timestamp
- terminal timestamp
- outcome class
- status summary or failure classification
- granted capability slice or a durable reference to it
- evidence references read or produced
- policy decision metadata sufficient for audit

Implementations MAY include richer diagnostics, logs, structured outputs, lineage edges, or timing breakdowns.

## 15. Evidence Model

### 15.1 Durable evidence

Evidence MUST be persistable as a durable host-owned object.

### 15.2 Stable evidence references

Persisted evidence MUST be addressable by `EvidenceRef` values issued by the host.

### 15.3 Evidence linkage

The system SHOULD preserve which executions read or produced each evidence object.

### 15.4 Shared read backend

If both host-side and guest-side reads exist, they SHOULD resolve through the same underlying durable backend.

### 15.5 Explain reuse

Evidence persisted during execution MUST be available for later inspect and explain flows when policy allows.

### 15.6 Provenance

Evidence SHOULD preserve provenance or chain-of-custody metadata sufficient for later analysis.

### 15.7 Current repository URI forms

The current repository issues durable local URIs of the form:

- `guild://executions/{execution_id}`
- `guild://objects/sha256/{digest}`

## 16. Inspect and Explain

### 16.1 Supported workload type

Guild SHOULD support inspect and explain skills as first-class workloads.

### 16.2 Artifact grounding

Inspect and explain behavior SHOULD be grounded in durable execution and evidence artifacts, not only transient conversational recall.

### 16.3 Failed and rejected executions

Failed and rejected executions MUST remain readable to inspect and explain flows when policy permits.

### 16.4 Truthfulness boundary

Explain skills MUST NOT claim certainty beyond what durable artifacts support.

## 17. Composite Skill Requirements

### 17.1 Durable lineage

Parent-child execution relationships MUST be durable and queryable.

### 17.2 Child identity preservation

Each child execution MUST preserve its own requested and resolved identity.

### 17.3 Mixed outcomes

A composite skill MAY succeed despite failed or rejected child executions, but those child records MUST remain inspectable.

### 17.4 Capability narrowing

Implementations SHOULD prefer narrowing capability grants across child calls rather than widening them.

## 18. Policy Model

### 18.1 Host-owned enforcement

Policy decisions MUST be made by the host.

### 18.2 Policy targets

Policy MAY be expressed against:

- requested skill refs
- resolved skill refs
- executable digests
- caller identity or caller class
- capability families
- resource access
- evidence access

### 18.3 Durable denial record

Policy rejection SHOULD produce a durable record suitable for later inspection and audit.

### 18.4 Safety precedence

When policy conflicts with convenience, the host MUST prefer policy.

## 19. Resource Access Semantics

### 19.1 Host-mediated reads

Reads of persisted or external resources MUST be mediated by the host.

### 19.2 Explicit attribution

Resource access SHOULD be attributable to execution attempts.

### 19.3 Auditability

The system SHOULD be able to answer what a given execution read and under which capability grant.

## 20. Security Properties

A strong Guild implementation SHOULD provide all of the following:

- immutable execution identity
- no ambient authority
- host-issued execution and evidence identifiers
- durable policy denials
- integrity-verifiable import and export
- forensic support through durable records and evidence

Guild does not guarantee that the guest makes perfect decisions. It guarantees that the system around the guest is explicit, inspectable, and governable.

## 21. Conformance

A system may call itself Guild-conformant only if it supports all of the following:

1. request-to-resolution before execution
2. immutable executable identity
3. host-mediated constrained execution
4. durable execution records for success, failure, and rejection
5. durable evidence with host-issued references
6. host-owned capability enforcement
7. artifact-grounded inspection of prior executions

If composite skills are supported, durable parent-child linkage is also REQUIRED.

## 22. Extension Points

Future specs or ADRs SHOULD define:

- bundle manifest format
- signatures and provenance chains
- execution and evidence query model
- retention and garbage collection
- capability schema registry and versioning
- replay semantics
- federation across Guild roots
- human approval or interruption semantics

## 23. Bottom Line

Guild exists to force AI skill execution out of the swamp of informal orchestration and into a real software contract.

That means immutable identity, constrained execution, durable records, durable evidence, and later explanation grounded in what actually happened.

Everything else is secondary.
