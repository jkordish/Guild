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

## 2. High-Level Component Model

A practical Guild implementation contains the following subsystems:

1. Registry and Resolver
2. Installer and Bundle Manager
3. Runtime Host
4. Execution Store
5. Evidence Store
6. Resource Backend
7. Policy Engine
8. Inspect and Explain Skill Layer
9. MCP Facade

### 2.1 Registry and Resolver

Responsible for mapping `RequestedSkillRef` values to `ResolvedSkillRef` values and locating executable artifacts.

### 2.2 Installer and Bundle Manager

Responsible for installation, import, export, integrity verification, and local artifact indexing.

### 2.3 Runtime Host

Responsible for execution orchestration, capability enforcement, host-issued IDs, policy decisions, and runtime-boundary mediation.

### 2.4 Execution Store

Responsible for durable persistence of `ExecutionRecord` objects.

### 2.5 Evidence Store

Responsible for durable persistence of evidence payloads plus evidence metadata and references.

### 2.6 Resource Backend

Responsible for host-mediated reads of persisted or external resources, ideally using the same backend semantics for host and guest access.

### 2.7 Policy Engine

Responsible for authorization decisions against skill refs, executable identities, capability families, and resource access.

### 2.8 Inspect and Explain Skill Layer

Responsible for reading durable execution and evidence artifacts and producing grounded summaries or explanations.

### 2.9 MCP Facade

Responsible for exposing a small stable tool surface such as `guild.inspect`, `guild.plan`, and `guild.apply`.

## 3. Repository Mapping

The current repository maps the architecture onto a small Rust workspace:

- `crates/guild-types`: shared contract types for identities, capabilities, execution, and evidence
- `crates/guild-manifest`: manifest model for source and installed skill metadata
- `crates/guild-registry`: local installer, local registry, bundle export and import, and Guild resource persistence
- `crates/guild-runner`: execution orchestration, capability evaluation, and runtime adapter boundary
- `crates/guild-mcp`: MCP-facing facade and proof examples
- `crates/guild-sdk-rust`: guest authoring support for Rust skills
- `wit/guild-skill-v1.wit`: guest and host ABI contract
- `examples/skills/`: runnable source skills used to prove the vertical slice

## 4. Logical Data Model

Guild has four core object classes:

### 4.1 Skill artifact metadata

Describes an installed executable artifact, including resolved identity, staged artifact location, dependency snapshot, and compatibility metadata.

### 4.2 ExecutionRecord

Describes one execution attempt and its outcome.

### 4.3 Evidence object

Stores durable evidence content plus provenance metadata.

### 4.4 Linkage metadata

Stores relationships such as:

- requested ref to resolved ref
- parent execution to child execution
- execution to evidence read
- execution to evidence produced

The exact storage schema can evolve. The important part is that these are real host-owned objects, not dead strings scattered through logs.

## 5. Reference Execution Flow

### 5.1 Step 1: Request ingress

Caller submits:

- requested skill ref
- input payload
- caller identity or execution context

At this point the system has not yet selected what will run.

### 5.2 Step 2: Resolution

Registry resolves the request to:

- concrete resolved skill ref
- executable artifact location
- compatibility and constraint metadata

This is where Guild becomes materially different from vague agent frameworks. A name is not enough. The system must know the exact thing it is about to run.

### 5.3 Step 3: Authorization and capability computation

Policy engine evaluates whether the request is allowed and computes the capability slice granted to the guest.

Possible outcomes:

- allowed
- allowed with narrowed capabilities
- rejected

If rejected, the runtime host still persists an execution record with rejection outcome.

### 5.4 Step 4: Host-issued execution envelope

Runtime host creates:

- execution identifier
- execution context
- granted capability slice
- parent execution linkage when applicable

The guest does not mint any of this.

### 5.5 Step 5: Guest execution

Runtime host executes the skill within the runtime boundary.

The guest may:

- read resources through host-mediated calls
- write evidence through host-mediated calls
- invoke child skills through host-mediated calls
- return structured outputs or failure signals

### 5.6 Step 6: Persistence

Runtime host persists:

- terminal execution record
- evidence objects produced or referenced
- child execution linkage when present

### 5.7 Step 7: Later inspection

Inspect and explain skills query the durable stores and produce grounded explanations of what happened.

## 6. Trust Boundary

### 6.1 Host responsibilities

The host is the authority for:

- resolution
- policy
- capability enforcement
- execution IDs
- evidence IDs
- durable persistence
- audit metadata

### 6.2 Guest responsibilities

The guest is responsible for:

- executing skill logic
- requesting permitted operations
- returning outputs or errors

### 6.3 Hard boundary rule

Guests do not get to decide what they are, what they are allowed to do, or what durable identifiers count as authoritative.

Once the guest can self-assert authority, the whole model starts to collapse.

## 7. Runtime Design

### 7.1 Wasm/WASI as the reference runtime

Wasm/WASI is the preferred execution substrate because it gives Guild:

- sandboxing
- portable artifacts
- host-mediated capability exposure
- reduced ambient authority
- a clean guest-host separation

### 7.2 Runtime host interface

The runtime host SHOULD expose a narrow interface to the guest for operations such as:

- `read-resource`
- `emit-evidence`
- `invoke-dependency`
- `log`

The exact ABI can evolve, but the shape should stay minimal and explicit.

### 7.3 Boundary discipline

Anything that changes durable system state or reaches outside the guest boundary SHOULD go through host APIs.

### 7.4 Current repository runtime

The current repository uses a Wasmtime-backed Wasm component adapter for the working slice. Primitive, explain, and composite example skills execute through `wit/guild-skill-v1.wit`, with host imports enforcing typed capability constraints.

## 8. Registry and Resolution Architecture

### 8.1 Requested refs

Human-meaningful references may be aliases, names, channels, or semantic version selectors.

### 8.2 Resolved refs

Resolved refs identify exact artifacts and SHOULD be digest-addressed.

### 8.3 Registry duties

The registry and resolver are responsible for:

- requested ref lookup
- alias and channel handling
- compatibility selection
- resolved identity issuance
- artifact lookup

### 8.4 Why this split exists

Without this split, you cannot reliably answer the question: what exact thing ran?

If you cannot answer that, you are not building infrastructure.

### 8.5 Current repository registry behavior

The current repository uses a file-backed local registry. Source skills install into digest-pinned installed records, installed manifests pin child dependencies by alias, and execution resolves only against installed state.

## 9. Artifact Lifecycle Architecture

### 9.1 Install path

Install takes source material and produces:

- a verified local installed artifact record
- resolved identity metadata
- executable placement within the Guild root

### 9.2 Export path

Export produces a portable bundle including:

- executable payload
- resolved identity metadata
- signatures or local provenance metadata
- dependency snapshot information

### 9.3 Import path

Import validates the bundle, verifies integrity, verifies publisher trust, and installs it into the local Guild root.

### 9.4 Root portability

Two different Guild roots should be able to agree on artifact identity for the same bundle even if their local storage layout differs.

### 9.5 Current repository bundle flow

The working bundle flow is built from installed executable state rather than source directories. Import verifies signature, trust, and bundled file digests before installation and writes host-owned verification metadata alongside imported installed records.

## 10. Execution Store Design

### 10.1 Required records

Execution store MUST persist all of:

- successful executions
- failed executions
- rejected executions

### 10.2 Why rejections matter

Rejections are not noise. They are part of the system truth.

If a skill was denied by policy, that matters just as much as a crash in a security-sensitive system.

### 10.3 Parent-child graph

Execution store SHOULD support traversal across:

- parent to child
- child to parent
- execution to evidence
- evidence to execution

### 10.4 Suggested storage shape

Implementation is flexible, but the model should support:

- durable IDs
- timestamps
- outcome classes
- lineage edges
- audit metadata
- structured search and filtering

## 11. Evidence Store Design

### 11.1 Evidence as durable object

Evidence store is not just a cache. It is a durable artifact store.

### 11.2 Evidence contents

Evidence may contain:

- raw resource material
- derived summaries
- structured extraction results
- normalized observations
- explanation-ready snapshots

### 11.3 Evidence references

Evidence store issues host-owned references that can later be reused by inspect and explain skills.

### 11.4 Provenance

Evidence metadata SHOULD capture at least:

- producing execution
- read source when applicable
- creation time
- type or format
- integrity metadata when relevant

### 11.5 Current repository evidence backend

The current repository stores evidence durably in a local object store keyed by SHA-256 and exposes those objects through `guild://objects/sha256/{digest}` URIs.

## 12. Resource Backend Architecture

### 12.1 Shared semantics

Host reads and guest `read-resource` calls should hit the same conceptual backend.

### 12.2 Why this matters

If host-side inspection sees one world and guest-side execution sees another, explanations become fake fast.

### 12.3 Backend responsibilities

Resource backend should provide:

- stable references
- controlled access
- explicit attribution
- optional caching and normalization
- policy-aware reads

### 12.4 Current repository resource model

The current repository serves both MCP resource reads and guest-side `read-resource` calls from the same local execution and object store.

## 13. Composite Skill Architecture

### 13.1 Composite control flow

Composite skills coordinate child skill invocations through the runtime host.

### 13.2 Host-mediated child calls

Child calls SHOULD not bypass the runtime host. The host still:

- resolves child requested refs
- applies policy
- computes capability slices
- issues child execution IDs
- persists child outcomes

### 13.3 Composite success semantics

A composite skill MAY complete successfully even if a child fails or is rejected, provided its own logic handles that outcome.

### 13.4 Inspection goal

After the fact, the system must still make it easy to answer:

- which child skills ran
- in what relation
- under which resolved identities
- which failed
- which were rejected
- which evidence objects were involved

## 14. Inspect and Explain Architecture

### 14.1 Explain from artifacts

Explain skills are consumers of durable system truth.

### 14.2 Input sources

An explain skill may read:

- execution records
- evidence objects
- resource references
- policy results when permitted

### 14.3 Failure handling

Explain skills SHOULD operate over failed and rejected runs, not only clean successes.

### 14.4 Why this is a core feature

This is not bolt-on observability. It is part of the product claim. If the system cannot later explain what happened from durable artifacts, it falls back into prompt-era fog.

## 15. Policy Engine Architecture

### 15.1 Decision points

Policy SHOULD be enforced at:

- initial execution request
- resource read
- evidence write
- child invocation
- inspect and explain access

### 15.2 Policy inputs

A policy decision MAY consider:

- caller identity or caller class
- requested skill ref
- resolved skill ref
- artifact digest
- capability family
- resource identity
- evidence identity
- environment or root configuration

### 15.3 Durable denials

Rejected operations SHOULD be durable enough to support later audit.

### 15.4 Current repository policy shape

The working slice accepts explicit caller-provided grants and uses a shared typed capability evaluator rather than a full general policy engine.

## 16. Example Guild Root Layout

An illustrative local Guild root looks like this:

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
  trust/
    publishers/
  cache/
  temp/
```

This shape is illustrative, not normative. The important part is that the root has clear homes for artifacts, records, evidence, trust metadata, and resolver state.

## 17. Sequence Sketch

```text
Caller -> Resolver: RequestedSkillRef + input
Resolver -> Host: ResolvedSkillRef + artifact
Host -> Policy: authorize(request, resolved, caller)
Policy -> Host: allow/reject + capability slice

If rejected:
  Host -> Execution Store: persist rejection record
  Host -> Caller: execution receipt

If allowed:
  Host -> Guest: execute within boundary
  Guest -> Host: read-resource / emit-evidence / invoke-child
  Host -> Evidence Store: persist evidence as needed
  Host -> Execution Store: persist terminal execution record
  Host -> Caller: execution receipt + output
```

## 18. Deployment Shape

### 18.1 Local-first default

Guild SHOULD work as a local process or local service boundary without requiring a central hosted control plane.

### 18.2 Future decomposition

The architecture can later split across processes or services, but only if the core trust boundaries remain intact.

### 18.3 Good decomposition rule

Split implementation boundaries where they improve integrity, portability, or inspectability. Do not split them just to cosplay distributed systems.

## 19. Operational Benefits

This architecture buys the properties most agent stacks keep hand-waving about:

- reproducibility through immutable execution identity
- security through explicit capability mediation
- auditability through durable execution and evidence records
- composability without losing lineage
- explainability grounded in artifacts
- portability across local roots

That is why the architecture exists. Not because more components are fun.

## 20. Near-Term Build Priorities

### Phase 1

- install, resolve, and execute on immutable artifacts
- Wasm/WASI runtime boundary
- durable execution store
- durable evidence store
- primitive and composite execution support

### Phase 2

- inspect and explain skills over persisted artifacts
- execution graph queries
- failure and rejection views
- resource backend unification

### Phase 3

- signatures and provenance
- richer policy engine
- ecosystem distribution and trust tiers
- retention and governance controls

## 21. Bottom Line

Guild architecture is about making AI skill execution behave like a real system.

That means exact identity, bounded runtime authority, durable records, durable evidence, and later explanation that does not depend on remembering what the model probably did.
