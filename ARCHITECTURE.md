# Guild Architecture

Status: Draft v0.1  
Scope: practical architecture for a local-first Guild implementation  
Audience: implementers, maintainers, reviewers, security and platform engineers

## 1. Overview

Guild is a local-first skill execution fabric.

Its architecture exists to preserve five properties end to end:

- requested identity is separate from executable identity
- guest execution is bounded by host authority
- execution attempts become durable records
- evidence becomes a durable reusable artifact
- later inspection and explanation are grounded in those durable artifacts

The architecture is intentionally boring in the good way. You want predictable state transitions, explicit boundaries, and forensic visibility, not clever agent sludge.

The key layering rule is now explicit:

- `wit/guild-skill-v1.wit` is the canonical guest-wire boundary
- Rust host types are the richer durable platform model
- translation between those layers is explicit and host-owned
- MCP transport authorization remains separate from Guild runtime capability grants

## 2. High-Level Component Model

A practical Guild implementation contains the following subsystems:

1. Registry / Resolver
2. Installer / Bundle Manager
3. Runtime Host
4. Execution Store
5. Evidence Store
6. Resource Backend
7. Policy Engine
8. Inspect / Explain Skill Layer

### 2.1 Registry / Resolver

Responsible for mapping `RequestedSkillRef` values to `ResolvedSkillRef` values and locating executable artifacts.

### 2.2 Installer / Bundle Manager

Responsible for installation, import, export, integrity verification, and local artifact indexing.

### 2.3 Runtime Host

Responsible for execution orchestration, capability enforcement, host-issued IDs, policy decisions, and runtime boundary mediation.

### 2.4 Execution Store

Responsible for durable persistence of `ExecutionRecord` objects.

### 2.5 Evidence Store

Responsible for durable persistence of evidence payloads plus evidence metadata and references.

### 2.6 Resource Backend

Responsible for host-mediated reads of persisted or external resources, ideally using the same backend semantics for host and guest access.

### 2.7 Policy Engine

Responsible for authorization decisions against skill refs, executable identities, capability families, and resource access.

### 2.8 Inspect / Explain Skill Layer

Responsible for reading durable execution and evidence artifacts and producing grounded summaries or explanations.

### 2.9 Current repository mapping

The current repository maps this architecture onto a small Rust workspace:

- `crates/guild-types`: shared contract types for identities, capabilities, execution, and evidence
- `crates/guild-manifest`: manifest model for source and installed skill metadata
- `crates/guild-registry`: local installer, local registry, bundle export and import, and Guild resource persistence
- `crates/guild-runner`: execution orchestration, capability evaluation, and runtime adapter boundary
- `crates/guild-mcp`: MCP-facing facade and proof examples
- `crates/guild-sdk-rust`: guest authoring support for Rust skills
- `wit/guild-skill-v1.wit`: guest and host ABI contract
- `examples/skills/`: runnable source skills used to prove the vertical slice

## 3. Logical Data Model

Guild has four core object classes:

### 3.1 Skill artifact metadata

Describes an installed executable artifact, including its resolved identity and compatibility metadata.

### 3.2 ExecutionRecord

Describes one execution attempt and its outcome.

### 3.3 Evidence object

Stores durable evidence content plus provenance metadata.

### 3.4 Linkage metadata

Stores relationships such as:

- requested ref -> resolved ref
- parent execution -> child execution
- execution -> evidence read
- execution -> evidence produced

The important thing is not the exact schema on day one. The important thing is that these are real objects and not dead strings scattered through logs like confetti after a bad conference keynote.

In the current repository this logical model is now split more explicitly into:

- `CallerRequest` for caller intent and requested identity
- `ResolvedExecutionEnvelope` for host-issued resolved execution input
- `ExecutionRecord` and `ExecutionReceipt` for durable execution truth
- `EvidenceRecord` plus `EvidenceRef` for host-owned evidence metadata and guest-visible handles
- distinct manifest schema, skill API, and guest ABI version axes

## 4. Reference Execution Flow

### 4.1 Step 1: Request ingress

Caller submits:

- requested skill ref
- input payload
- caller identity or execution context

The system has not yet selected what will run.

### 4.2 Step 2: Resolution

Registry resolves the requested ref to:

- concrete resolved skill ref
- executable artifact location
- compatibility and constraint metadata

This is the point where Guild becomes materially different from vague agent frameworks. A name is not enough. The system must know the exact thing it is about to run.

### 4.3 Step 3: Authorization and capability computation

Policy engine evaluates whether the request is allowed and computes the capability slice granted to the guest.

Possible outcomes:

- allowed
- allowed with narrowed capabilities
- rejected

If rejected, the runtime host still persists an `ExecutionRecord` with rejection outcome.

### 4.4 Step 4: Host-issued execution envelope

Runtime host creates:

- execution identifier
- execution context
- granted capability slice
- parent execution linkage if applicable

The guest does not mint any of this.

### 4.5 Step 5: Guest execution

Runtime host executes the skill within the runtime boundary.

The guest may:

- read resources through host-mediated calls
- write evidence through host-mediated calls
- invoke child skills through host-mediated calls
- return structured outputs or failure signals

### 4.6 Step 6: Persistence

Runtime host persists:

- terminal execution record
- evidence objects produced or referenced
- child execution linkage if present

### 4.7 Step 7: Later inspection

Inspect and explain skills query the durable stores and produce grounded explanations of what happened.

## 5. Trust Boundary

### 5.1 Host responsibilities

The host is the authority for:

- resolution
- policy
- capability enforcement
- execution IDs
- evidence IDs
- durable persistence
- audit metadata

### 5.2 Guest responsibilities

The guest is responsible for:

- executing skill logic
- requesting permitted operations
- returning outputs or errors

### 5.3 Hard boundary rule

Guests do not get to decide what they are, what they are allowed to do, or what durable identifiers count as authoritative.

That line matters. Once the guest can self-assert authority, the whole model starts smelling like a haunted house built from JSON.

## 6. Runtime Design

### 6.1 Wasm/WASI as reference runtime

Wasm/WASI is the preferred execution substrate because it gives Guild:

- sandboxing
- portable artifacts
- host-mediated capability exposure
- reduced ambient authority
- a clean guest/host separation

### 6.2 Runtime host interface

The runtime host SHOULD expose a narrow interface to the guest for operations such as:

- `read-resource`
- `write-evidence`
- `invoke-child`
- `emit-output`
- `emit-failure`

The exact ABI can evolve, but the shape should stay minimal and explicit.

### 6.3 Boundary discipline

Anything that changes durable system state or reaches outside the guest boundary SHOULD go through host APIs.

### 6.4 Current repository runtime

The current repository uses a Wasmtime-backed Wasm component adapter for the working slice. Primitive, explain, and composite example skills execute through `wit/guild-skill-v1.wit`, where the current host-facing operation names are `read-resource`, `emit-evidence`, `invoke-dependency`, and `log`, and skill output or failure is returned from `skill.run` rather than emitted through separate host calls.

## 7. Registry and Resolution Architecture

### 7.1 Requested refs

Human-meaningful references may be aliases, names, channels, or semantic identifiers.

### 7.2 Resolved refs

Resolved refs identify exact artifacts and SHOULD be digest-addressed.

### 7.3 Registry duties

The registry / resolver is responsible for:

- requested ref lookup
- alias/channel handling
- compatibility selection
- resolved identity issuance
- artifact lookup

### 7.4 Why this split exists

Without this split, you cannot reliably answer the question:

what exact thing ran?

And if you cannot answer that, you are not building infrastructure. You are building folklore.

## 8. Artifact Lifecycle Architecture

### 8.1 Install path

Install takes a source artifact and produces:

- a verified local artifact record
- resolved identity metadata
- executable placement within the Guild root

### 8.2 Export path

Export produces a portable bundle including:

- executable payload
- resolved identity metadata
- optional signatures or provenance metadata
- compatibility constraints

### 8.3 Import path

Import validates the bundle, verifies integrity, and installs it into the local Guild root.

### 8.4 Root portability

Two different Guild roots should be able to agree on artifact identity for the same bundle, even if their local storage layout differs.

### 8.5 Current repository bundle flow

The working bundle flow is built from installed executable state rather than source directories. Import verifies signature, trust, and bundled file digests before installation and writes host-owned verification metadata alongside imported installed records.

## 9. Execution Store Design

### 9.1 Required records

Execution store must persist all of:

- successful executions
- failed executions
- rejected executions

### 9.2 Why rejections matter

Rejections are not noise. They are part of the system truth.

If a skill was denied by policy, that matters just as much as a crash, especially in security-sensitive systems.

### 9.3 Parent-child graph

Execution store should support traversal across:

- parent -> child
- child -> parent
- execution -> evidence
- evidence -> execution

### 9.4 Suggested storage shape

Implementation is flexible, but the model should support:

- durable IDs
- timestamps
- outcome classes
- lineage edges
- audit metadata
- structured search/filtering

## 10. Evidence Store Design

### 10.1 Evidence as durable object

Evidence store is not just a cache. It is a durable artifact store.

### 10.2 Evidence contents

Evidence may contain:

- raw resource material
- derived summaries
- structured extraction results
- normalized observations
- explanation-ready snapshots

### 10.3 Evidence references

Evidence store issues host-owned references that can later be reused by inspect/explain skills.

### 10.4 Provenance

Evidence metadata should capture at least:

- producing execution
- read source if applicable
- creation time
- type/format
- integrity metadata if relevant

## 11. Resource Backend Architecture

### 11.1 Shared semantics

Host reads and guest `read-resource` calls should hit the same conceptual backend.

### 11.2 Why this matters

If host-side inspection sees one world and guest-side execution sees another, explanations become fake fast.

### 11.3 Backend responsibilities

Resource backend should provide:

- stable references
- controlled access
- explicit attribution
- optional caching/normalization
- policy-aware reads

## 12. Composite Skill Architecture

### 12.1 Composite control flow

Composite skills coordinate child skill invocations through the runtime host.

### 12.2 Host-mediated child calls

Child calls should not bypass the runtime host. The host must still:

- resolve child requested refs
- apply policy
- compute capability slices
- issue child execution IDs
- persist child outcomes

### 12.3 Composite success semantics

A composite skill may complete successfully even if a child fails or is rejected, provided its own logic handles that outcome.

### 12.4 Inspection goal

After the fact, the system must still make it easy to answer:

- which child skills ran?
- in what relation?
- under which resolved identities?
- which failed?
- which were rejected?
- which evidence objects were involved?

## 13. Inspect / Explain Architecture

### 13.1 Explain from artifacts

Explain skills are consumers of durable system truth.

### 13.2 Input sources

An explain skill may read:

- execution records
- evidence objects
- resource references
- policy results where permitted

### 13.3 Failure handling

Explain skills should operate over failed and rejected runs, not only clean successes.

### 13.4 Why this is a core feature

This is not bolt-on observability. It is the whole point. If the system cannot later explain what happened from durable artifacts, it falls back into prompt-era fog.

## 14. Policy Engine Architecture

### 14.1 Decision points

Policy should be enforced at:

- initial execution request
- resource read
- evidence write
- child invocation
- inspect/explain access

### 14.2 Policy inputs

A policy decision may consider:

- caller identity or caller class
- requested skill ref
- resolved skill ref
- artifact digest
- capability family
- resource identity
- evidence identity
- environment or root configuration

### 14.3 Durable denials

Rejected operations should be durable enough to support later audit.

## 15. Example Logical Layout of a Guild Root

```text
.guild/
  config/
    policy/
    runtime/
    registry/
  artifacts/
    installed/
    bundles/
  executions/
    by-id/
    indexes/
  evidence/
    objects/
    indexes/
  registry/
    requested/
    resolved/
  cache/
  temp/
```

This is illustrative, not normative. The important thing is that the root has clear homes for artifacts, records, evidence, and resolver metadata.

## 16. Sequence Sketch

```mermaid
sequenceDiagram
    participant Caller
    participant Resolver
    participant Policy
    participant Host
    participant Guest
    participant ExecStore
    participant EvidenceStore

    Caller->>Resolver: RequestedSkillRef + input
    Resolver-->>Host: ResolvedSkillRef + artifact
    Host->>Policy: authorize(request, resolved, caller)
    Policy-->>Host: allow/reject + capability slice
    alt rejected
        Host->>ExecStore: persist rejection record
        Host-->>Caller: execution receipt
    else allowed
        Host->>Guest: execute within boundary
        Guest->>Host: read-resource / write-evidence / invoke-child
        Host->>EvidenceStore: persist evidence as needed
        Host->>ExecStore: persist terminal execution record
        Host-->>Caller: execution receipt + output
    end
```

## 17. Deployment Shape

### 17.1 Local-first default

Guild should work as a local process or local service boundary without requiring a central hosted control plane.

### 17.2 Future decomposition

The architecture can later split across processes or services, but only if the core trust boundaries remain intact.

### 17.3 Good decomposition rule

Split implementation boundaries where they improve integrity, portability, or inspectability. Do not split them just to cosplay distributed systems.

## 18. Operational Benefits

This architecture buys you the things most agent stacks keep hand-waving about:

- reproducibility through immutable execution identity
- security through explicit capability mediation
- auditability through durable execution and evidence records
- composability without losing lineage
- explainability grounded in artifacts
- portability across local roots

That is why the architecture exists. Not because more components are fun. No one needs more components. People can barely survive the ones they already invented.

## 19. Current Repository Baseline

The current repository implements a real but intentionally narrow slice of this architecture:

- source skills install into local digest-pinned installed records
- the local registry resolves only against installed executable state
- the runtime host executes Wasm components through Wasmtime
- explicit caller-provided grants are enforced through typed capability evaluation
- successful, failed, and rejected resolved executions persist as durable execution records
- evidence persists as durable local objects and is readable through the same backend used by guest `read-resource`
- composite skills invoke declared child dependencies by alias through the same host boundary
- `guild.inspect` in `guild-mcp` rides that same registry, runner, and storage path

What is still deferred in this repo:

- a general policy engine beyond explicit grants
- full `plan` execution
- `apply` mode
- remote registry and publication infrastructure

## 20. Near-Term Build Priorities

Phase 1

- install/resolve/execute on immutable artifacts
- Wasm/WASI runtime boundary
- durable execution store
- durable evidence store
- primitive and composite execution support

Phase 2

- inspect/explain skills over persisted artifacts
- execution graph queries
- failure/rejection views
- resource backend unification

Phase 3

- signatures and provenance
- richer policy engine
- ecosystem distribution and trust tiers
- retention/governance controls

## 21. Bottom Line

Guild architecture is about making AI skill execution behave like a real system.

That means exact identity, bounded runtime authority, durable records, durable evidence, and later explanation that does not depend on remembering what the model "probably did."
