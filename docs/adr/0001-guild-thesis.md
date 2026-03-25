# ADR 0001: Guild thesis

- Status: Accepted
- Date: 2026-03-15
- Owners: Guild maintainers

Historical note: this ADR records the accepted foundation behind Guild. For the
current project thesis, wording freeze, and first-reference-application
framing, use [`../project-positioning.md`](../project-positioning.md).

## Context

Most current AI tool and agent systems are structurally weak in the same predictable ways:

- the thing requested is not cleanly separated from the thing executed
- tools and skills often have weak identity and poor reproducibility
- execution, policy, storage, and explanation are collapsed into a single fuzzy runtime layer
- evidence is ephemeral prompt context instead of a durable object
- failures and policy denials are inconsistently preserved
- explanations are reconstructed from logs, memory, or vibes rather than grounded in durable artifacts

This creates systems that may demo well but are difficult to:

- secure
- audit
- reproduce
- debug
- trust in real operational settings

Guild exists because this pattern is not good enough.

## Decision

We will build Guild as a local-first runtime and artifact system for AI skills.

Guild will treat skills as real software units with:

- requested identity
- resolved immutable executable identity
- constrained host-managed execution
- durable host-owned execution records
- durable host-owned evidence objects
- structural support for inspection and explanation after the fact

Guild will also adopt the following foundation principles:

1. Rust is the platform core.
2. Wasm/WASI is the preferred execution substrate.
3. Skills receive host capabilities, not ambient authority.
4. Execution resolves to immutable executable identity.
5. Inspect, plan, and apply are separate modes.
6. Evidence, diagnostics, and provenance are required outputs and artifacts.
7. The MCP surface remains small and stable.
8. Contracts are treated as public product surface.

The central model is:

1. caller requests a skill using a human-meaningful reference
2. host resolves the request to an immutable executable artifact
3. host applies policy and computes a capability slice
4. guest executes inside a constrained boundary
5. host persists execution records and evidence
6. later skills or users inspect those durable artifacts to understand what happened

## Rationale

### 1. Requested identity must be separate from executable identity

A stable skill name is useful for people. It is not sufficient for execution.

If the system cannot tell us exactly which artifact ran, then reproducibility and auditability fall apart immediately.

### 2. Host-owned trust boundaries matter

Guests should not control policy, capability grants, durable identifiers, or other authority-bearing concerns.

Separating host authority from guest execution makes the system easier to secure and easier to reason about.

### 3. Evidence must be durable

Ephemeral tool output stuffed into prompts is not enough.

Evidence should be a durable reusable object so later inspection and explanation can operate on the same material the original execution used.

### 4. Failures and rejections are system truth

A rejected execution is not an absence of work. It is a meaningful outcome.

If the system does not persist failures and denials, it cannot support serious debugging, forensics, or audit.

### 5. Composition must preserve lineage

Composite skills are only useful if they do not erase traceability.

Parent and child executions must remain queryable as part of a durable execution graph.

### 6. Local-first is strategically correct

Guild should not require a hosted control plane to be useful.

A local-first architecture improves sovereignty, testability, portability, and operational sanity.

## Consequences

### Positive

- stronger reproducibility through immutable execution identity
- clearer security posture through explicit capability mediation
- durable audit and forensic trail
- explainability grounded in stored artifacts instead of narrative reconstruction
- better portability
- lower chance of tool-sprawl collapse
- better support for enterprise, platform, and security use cases
- stronger long-term compatibility discipline

### Costs

- more upfront systems design than a prompt-only tool stack
- extra persistence and runtime overhead
- authoring discipline is required
- capability design must be done carefully or authors will hate it
- import/export and provenance semantics must be treated seriously
- slower early demos
- more friction when adding new host capabilities
- stronger pressure to keep examples and docs aligned with code

### Neutral reality check

Guild does not make model outputs magically correct.
It makes the surrounding system sane enough to operate and inspect.
That is still a huge improvement.

## Alternatives Considered

### 1. Simple prompt-orchestrated tool calling

Rejected because it provides weak identity, weak auditability, and poor trust boundaries.

### 2. Centralized hosted agent registry as the primary model

Rejected as a requirement because it reduces portability and sovereignty and makes local development/testing worse.

### 3. Direct unrestricted plugin execution

Rejected because ambient authority is an avoidable security mistake.

### 4. Log-based explainability only

Rejected because logs are not the same thing as durable, host-owned execution and evidence artifacts.

### 5. MCP alone as the full solution

Rejected because MCP addresses interoperability and tool/resource access, not the complete packaging, resolution, execution, persistence, and inspection model Guild needs.

## Implications for Follow-On Work

This ADR implies the project should prioritize:

1. requested-to-resolved skill resolution
2. digest-pinned executable identity
3. Wasm/WASI execution boundary
4. durable execution records for success, failure, and rejection
5. durable evidence objects with stable references
6. composite execution lineage
7. inspect/explain workflows over persisted artifacts

It also implies a follow-on spec set covering:

- bundle format
- capability schema
- execution record schema
- evidence schema
- policy model
- retention and provenance

## Decision Summary

Guild will not be a fuzzy agent framework.

Guild will be a real execution substrate for AI skills: portable, inspectable, policy-bounded, and grounded in durable artifacts.

That is the thesis.
