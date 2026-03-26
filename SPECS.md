# Guild Specification

Status: Draft v0.1  
Scope: core normative contract for Guild implementations  
Audience: runtime implementers, registry authors, skill authors, security reviewers, platform engineers

## 1. Purpose

Guild defines a local-first execution and artifact model for AI skills.

For current project framing and repository vocabulary, use [`docs/project-positioning.md`](docs/project-positioning.md). That document is explanatory and strategic; it does not override the normative contract in this specification.

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

This is the central claim of Guild:

AI skills should be treated like real software units with identity, runtime constraints, receipts, and evidence, not as informal prompt-era behavior.

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

### 4.1 Source Of Truth

Guild does not treat every repo surface as equally normative.

For runtime contracts, the normative sources are:

- `SPECS.md`
- `wit/guild-skill-v1.wit`
- the core Rust runtime and type surfaces, especially `crates/guild-types` and `crates/guild-runner`

Those sources own the live runtime vocabulary, execution boundary, durable record model, and active inspect-slice contract. If an explanatory document, generated artifact, or older rationale note disagrees with them, those runtime-contract sources win.

For the bounded draft proof/control-plane harness, the normative sources are the schemas, checked examples, and Rust-native truth tooling under `docs/schemas/draft-v1/` together with the shared Rust generator code that validates and regenerates that bundle. That draft harness is normative only for the draft harness itself. It is not the primary runtime-contract definition for Guild.

The following repo surfaces are not primary runtime-contract definitions:

- ADRs are rationale and history. They explain why decisions were made, but they do not replace current runtime-contract ownership in `SPECS.md`, WIT, or the core Rust runtime/types.
- `docs/project-positioning.md`, `docs/roadmap.md`, and roadmap epic docs are explanatory framing and planning surfaces. They do not define runtime semantics or widen support by prose.
- Generated support, compatibility, and benchmark artifacts are measured or derived outputs. They must reflect current repo-backed truth exactly, but they are not hidden sources of truth.
- `README.md`, `ARCHITECTURE.md`, `docs/testing.md`, and other explanatory docs are derived documentation unless they explicitly point back to a normative source.

This pass stays fail-closed:

- unsupported and `not_proven` slices remain explicit
- generated outputs must not widen support by wording alone
- explanatory docs must not smooth over runtime or draft-harness boundaries
- Contract Surface v1 (core) freezes only the mature surfaces listed below

### 4.2 Contract Surface v1 (core)

This milestone freezes only the runtime-contract surfaces that are already real,
stable enough to be normative now, and mechanically guardable against drift.

Included in Contract Surface v1 (core):

- Guild runtime resource URI grammar and canonical root registry
- `GuildResourceScope` normalization and rejection rules for those roots
- the active live runtime capability-family registry
- support-status and linkage vocabulary used by the frozen surface and checked outputs
- executable identity semantics from requested reference through resolved artifact and host-minted execution identity
- runtime-vs-draft source-of-truth boundaries where they affect contract interpretation

Excluded from Contract Surface v1 (core):

- broader OCI artifact profile freeze
- broader remote trust and distribution semantics
- public CLI output as a normative API contract
- broader `apply` and effects semantics beyond already explicit normative material
- deeper prose-lint drift checks beyond exact marker blocks and structured lists
- broader future-facing capability-universe freeze outside the active live runtime family set

Included surfaces are frozen now because they already have concrete truth in the
runtime parser, the core Rust types, the active runner surface, or WIT, and
they can be checked fail-closed. Excluded surfaces stay deferred because the
current repository truth is still transport-specific, draft-local, broader than
the active live path, or not yet precise enough to freeze without overclaiming.

### 4.3 Contract Index

- URIs and canonical resource roots: this document section 4.4 plus `crates/guild-types`
- active live runtime family registry: this document section 4.5 plus `crates/guild-runner`
- support and linkage vocabulary: this document section 4.6 plus `crates/guild-types`
- executable identity semantics: this document section 4.7 plus `crates/guild-types` and `crates/guild-runner`
- guest ABI and active inspect world: section 8 plus `wit/guild-skill-v1.wit`
- draft control-plane harness only: section 12.7 plus `docs/schemas/draft-v1/`

### 4.4 Guild URI Grammar And Resource Roots

The canonical runtime parser and normalizer live in `crates/guild-types`. The
runtime contract is the exact accepted and rejected local Guild URI surface
implemented there. This document freezes the canonical roots and accepted query
forms; the Rust parser remains the authoritative executable definition.

<!-- contract-surface-v1-core:uri-roots:start -->
Canonical runtime resource roots:
- `guild://executions/`
- `guild://objects/sha256/`
- `guild://objects/records/`
- `guild://queries/executions/`

Accepted execution-query forms:
- `guild://queries/executions/recent/{limit}`
- `guild://queries/executions/failures/recent/{limit}`
- `guild://queries/executions/by-status/{status}/{limit}` where `{status}` is one of `succeeded`, `failed`, `partial`, `rejected`
- `guild://queries/executions/by-skill/{namespace}/{name}/{limit}`
<!-- contract-surface-v1-core:uri-roots:end -->

The frozen runtime rules are:

- `GuildResourceScope::parse` accepts only the exact canonical roots above. Missing trailing slashes, broader prefixes, and unknown roots are rejected.
- `GuildResourceUri::parse` accepts only concrete execution, object-blob, object-record, object-record-metadata, and execution-query URIs under those roots.
- `guild://objects/records/{evidence_record_id}/metadata` is a supported concrete URI and remains in the same `guild://objects/records/` scope root as the payload-dereference URI.
- execution identifiers, evidence-record identifiers, and `by-skill` namespace or name segments are percent-decoded; malformed percent encoding is rejected rather than normalized loosely.
- object blob digests must be lowercase hexadecimal under `guild://objects/sha256/{digest}`.
- execution-query limits remain bounded to `1..=50`.
- unsupported roots, unsupported query paths, and malformed concrete URIs fail closed.

### 4.5 Canonical Capability Family Registry

The frozen core family registry is the active live runtime family set only. It
does not freeze the broader future-facing capability universe carried elsewhere
in shared Rust types, draft schemas, or broader WIT package surface.

<!-- contract-surface-v1-core:families:start -->
Frozen active live runtime families:
- `http-request`
- `read-resource`
- `invoke-skill`
- `emit-evidence`
- `log-write`
<!-- contract-surface-v1-core:families:end -->

The frozen family rules are:

- the names above are the canonical family spellings for the current live runtime surface
- inclusion in this registry does not imply broad or exact support across every slice; slice status remains separate and may still be `bounded` or `not_proven`
- draft-v1 compatibility aliases such as `net.connect` or `component.invoke` are not canonical family names in this frozen surface
- broader future-facing families such as filesystem, cache, secret, or clock remain typed contract vocabulary only where already present; they are not part of the Contract Surface v1 (core) family freeze

### 4.6 Support-Status Vocabulary

The frozen vocabulary below is closed for this milestone. These words have
distinct meanings and MUST NOT be collapsed into a softer or broader support
claim.

- `supported`: the named live runtime surface or checked slice is implemented as claimed on the checked path
- `bounded`: the named surface is supported only within an explicitly narrower checked envelope
- `partial`: only a narrowing compatibility or incomplete mapping exists; this is not direct canonical support
- `unsupported`: the named surface is not supported on the current live path and stays fail-closed
- `not_proven`: there is not yet an honest live proof basis for the claimed slice
- `proof_backed`: token or explanation basis comes from an acceptable proof rather than only an upper-bound plan
- `upper_bound_fallback`: issuance or explanation fell back to the admitted upper bound rather than a proof-backed reduction
- `proof_linked`: witness or downstream linkage is backed by an acceptable proof chain on the checked path
- `unlinked`: witness or downstream output exists without proof linkage
- `refused`: the host rejected the execution or issuance attempt rather than widening authority
- `coverage_limited`: the checked observation coverage is insufficient for the requested verification claim
- `unverifiable`: the available material cannot support safe verification

Some checked outputs also use explicit residual terms for benchmark or linkage
reporting. Those spellings remain checked and stable, but they are output terms
rather than a claim that the broader live runtime surface is generally
supported.

<!-- contract-surface-v1-core:status-vocabulary:start -->
Frozen support status spellings:
- `supported`
- `bounded`
- `partial`
- `unsupported`
- `not_proven`

Frozen linkage and presentation spellings:
- `proof_backed` -> CLI `proof-backed`
- `upper_bound_fallback` -> CLI `upper-bound`
- `proof_linked` -> CLI `linked`
- `unlinked` -> CLI `unlinked`
- `refused` -> CLI `refused`

Explicit checked-output residual terms:
- `not_measured_on_real_path`
- `fallback_unlinked`
- `proof_only`
- `coverage_limited`
- `unverifiable`
- `not_provable`
- `coverage_limited_or_unverifiable`
<!-- contract-surface-v1-core:status-vocabulary:end -->

### 4.7 Executable Identity Semantics

Requested identity, resolved identity, and durable execution identity are
distinct layers. The core Rust types are the executable truth for those layers,
and this section freezes their normative meaning.

<!-- contract-surface-v1-core:identity:start -->
Frozen executable identity terms:
- requested identity: `RequestedSkillRef` fields `key`, `version_req`
- resolved identity: `ResolvedSkillRef` fields `key`, `version`, `digest`
- host-minted durable execution identity field: `execution_id`
- non-authoritative caller correlation fields: `request_id`, `trace_id`
<!-- contract-surface-v1-core:identity:end -->

The frozen identity rules are:

- `RequestedSkillRef` is caller intent only. It is not executable identity.
- `ResolvedSkillRef` is the executable identity that actually ran. The resolved digest is normative for the executable artifact.
- the current inspect runtime path also depends normatively on the selected runtime entrypoint and guest ABI identity; in this repository that means `guild-skill-inspect-v1` where the active inspect slice requires it
- durable execution identity is host-minted and persists through the receipt and execution record
- caller request or correlation fields may be preserved for observability, but they do not control durable execution identity
- implementation-language package metadata such as Cargo package version is not Guild requested or resolved identity

### 4.8 Deferred From Contract Surface v1 (core)

This milestone does not freeze the following surfaces:

- OCI profile details beyond the current already-shipped signed-bundle transport behavior, because the broader transport profile is still transport-specific and not yet narrow enough for a stable general freeze
- remote trust and distribution semantics, because the repository truth is still local-first and host-owned
- public CLI output as a normative API, because the CLI remains an operator surface rather than a stable external contract
- broader `apply` and effects semantics, because `apply` remains intentionally deferred
- broader capability-universe semantics outside the active live runtime family set, because the current repository still distinguishes typed future vocabulary from active live runtime support
- paragraph-level prose drift enforcement, because this pass only claims exact-list and exact-marker checks where the repo truth is structured enough to support them safely

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

A packaged skill payload suitable for sandboxed execution.

### 5.7 ExecutionRecord

A durable host-owned record describing a single execution attempt and its outcome.

### 5.8 Evidence

A durable host-owned object model representing material read, derived, or produced in support of execution or explanation.

### 5.9 EvidenceRef

A host-issued stable reference to a persisted evidence record.

### 5.10 EvidenceRecord

A durable host-owned metadata record describing one persisted evidence emission and linking it to the underlying blob identity.

### 5.11 CallerRequest

A caller-facing request object carrying requested identity plus caller intent and inputs.

Caller-supplied request identifiers are correlation data only. They are not durable execution record identifiers.

### 5.12 ResolvedExecutionEnvelope

A host-enriched execution object carrying resolved identity, granted capabilities, policy decision, and runtime linkage.

### 5.13 Capability slice

A typed, host-defined permission set granted to a guest for a particular execution. In the current Rust implementation this is represented by `CapabilityGrantSet`.

### 5.14 PolicyDecision

A host-owned authorization result describing whether execution was allowed,
reduced, or rejected, together with the selected local policy profile and the
host-owned trust metadata considered during grant evaluation.

### 5.15 Host

The trusted runtime authority responsible for resolution, policy, capability enforcement, identifier issuance, and persistence.

### 5.16 Guest

The executing skill payload running within the runtime boundary.

### 5.17 Guild root

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

## 8. Boundary Layering

Guild freezes the guest and host contract boundary as follows:

- `wit/guild-skill-v1.wit` is the canonical guest-wire contract package.
- the active inspect ABI world in that package is `guild-skill-inspect-v1`.
- Rust host types are the canonical durable platform model.
- Translation between those layers MUST be explicit and tested.
- projection from the richer host model into the active inspect ABI MUST be host-owned, named, and fail-closed.
- the active inspect projection boundary MUST be centralized rather than scattered through runtime call paths.
- Guest-visible types SHOULD stay small and focused on execution context, capability imports, `SkillOutput`, and `SkillError`.
- Host-owned records such as `ExecutionRecord`, `ExecutionReceipt`, `EvidenceRecord`, `PolicyDecision`, provenance, and termination detail MUST NOT be pushed into WIT return types.
- Guild runtime capability grants MUST remain distinct from MCP transport authorization.
- unsupported future capability families MUST NOT appear as available host imports in the active inspect world.
- broader Guild component imports outside the active inspect allowlist MUST either be impossible by construction in the active inspect runtime path or be rejected by the host as `unsupported-runtime-surface` before guest execution continues.

In the active inspect slice, the current projection contract is:

- inspect guest `ExecutionContext` is a bounded subset of the richer host execution model and carries host-minted execution identity, trace/tenant IDs, resolved skill identity, input hash, `now_utc`, budget, and guest-visible granted capabilities only
- `ExecutionContext.mode` is intentionally omitted because `guild-skill-inspect-v1` is inspect-only by world contract
- current active grant projections for `http-request`, `read-resource`, `invoke-skill`, `emit-evidence`, and `log-write` are full at the current guest-visible grant-shape level
- host-owned request intent, `PolicyDecision`, provenance, termination detail, durable evidence metadata, and child lineage remain host truth rather than guest ABI truth
- explain/debug flows that need full truth MUST read host-owned records or Guild resources rather than inferring that state from guest-visible context

## 9. Current Repository Baseline

This repository already implements a narrow local inspect-oriented slice of the specification:

- source manifests install into digest-pinned installed manifests plus staged artifacts
- source and installed manifests now carry distinct `manifest_schema_version`, `skill_api_version`, and `runtime.guest_abi_version` axes
- Guild skill identity and transport tags are driven by the Guild manifest version and resolved artifact identity, not by implementation-language package-manager metadata such as the Cargo package version used to build a CLI or guest crate
- execution resolves only against installed executable state
- the host now models caller intent separately from resolved execution envelopes and durable execution records
- durable execution record identifiers are minted by the host rather than accepted from callers
- `guild.inspect` runs through a real Wasmtime-backed Wasm component runtime
- resolved execution attempts persist on success, failure, and rejection
- evidence persists as durable host-owned objects behind host-issued per-emission `EvidenceRef` values plus host-loadable `EvidenceRecord` metadata
- evidence payload blobs remain content-addressed by digest and distinct from evidence-record identity
- evidence metadata is also readable as first-class JSON resources under `guild://objects/records/{evidence_record_id}/metadata`, while `guild://objects/records/{evidence_record_id}` remains the existing payload-dereference URI
- signed local bundle export and import verifies trust, signature, and bundled file digests before installation
- local OCI image layout transport maps those same signed installed-bundle semantics onto OCI descriptors and blobs without changing execution identity or trust rules
- OCI registry transport pushes and pulls that same OCI-mapped signed installed bundle through a remote OCI registry without changing Guild's local trust or signature verification rules on import
- composite skills invoke declared child dependencies by alias through the host boundary
- local source installs stage and validate before an atomic move into place
- requested resolution fails closed if a single key and version maps to multiple installed digests
- supported typed capability families in the active Wasm inspect slice are the frozen active live runtime families listed in section 4.5
- inspect-mode Wasm skills now declare `runtime.entrypoint = guild-skill-inspect-v1` together with `runtime.guest_abi_version = guild-skill-inspect-v1`
- the active Wasm inspect runtime instantiates only the `guild-skill-inspect-v1` world, so unsupported capability imports such as secret, cache, clock, and filesystem surface are absent from the active inspect guest ABI
- the active Wasm inspect runtime also preflights Guild component imports and allows only `guild:skill/inspect-types@1.0.0` and `guild:skill/inspect-host@1.0.0`; broader Guild import surfaces fail closed as host-owned `unsupported-runtime-surface` rejections rather than degrading into generic component instantiation failures
- the shared host-side capability vocabulary now also includes an explicit typed `filesystem` family, but the active Wasm inspect slice still rejects filesystem before execution and the inspect guest ABI does not expose filesystem imports in this milestone
- caller-requested capabilities are evaluated through a host-owned local policy layer before execution
- the current repository loads an optional `policy.json` from the Guild root and otherwise uses a built-in default local policy profile
- local policy now selects a named profile by actor and/or tenant, then evaluates grants against host-owned verification state and local trust tier metadata
- local policy `cap` rules may reduce a broader grant into a narrower same-family union, while `deny` rules conservatively remove any grant that overlaps a denied typed ceiling
- bounded local execution-query resources expose deterministic views over persisted execution records through the same host-mediated resource backend used by guest `read-resource` and MCP `resources/read`
- the host-owned projection into `guild-skill-inspect-v1` is explicit, centralized in the runner, and covered by contract tests
- the inspect guest `ExecutionContext` is a bounded projection that intentionally omits `mode`, while durable request/policy/provenance state remains host-owned
- the current active grant shapes for `http-request`, `read-resource`, `invoke-skill`, `emit-evidence`, and `log-write` are all projected fully into the inspect guest ABI

The current repository does not yet implement full `plan` mode, remote or distributed policy, or `apply` mode.

The current repository now also exposes a real stdio MCP server surface over that same runtime:

- stdio transport only in this milestone
- one public tool, `guild.inspect`
- `guild.inspect` tool annotations now truthfully advertise that inspect execution is not read-only, not idempotent, not destructive, and open-world in the MCP hint sense because the real inspect path persists durable records and may reach bounded external HTTP targets
- `resources/list` as a bounded discovery catalog: canonical recent-query entry points first, then recent execution resources, then recent evidence-metadata resources; plus `resources/read` and `resources/templates/list` for durable Guild URIs and bounded execution-query resources
- `tools/list`, `resources/list`, and `resources/templates/list` accept opaque cursor-based pagination and return `nextCursor` when more results remain
- list pagination stays endpoint-scoped, deterministic, and bounded; cursors do not widen authorization or bypass the existing bounded discovery-catalog behavior
- no subscriptions, no list-changed notifications, and no HTTP transport in this milestone

Unsuccessful `guild.inspect` executions that reached a real resolved execution attempt MUST be surfaced over MCP as tool execution errors while preserving the persisted execution record and receipt URI.

The current repository also exposes one thin local operator CLI over that same runtime and registry behavior:

- `guild` is the canonical Cargo-installable operator binary for the current implementation
- operator-facing root selection resolves as `--registry-root <path>`, then `GUILD_REGISTRY_ROOT`, then `~/.guild`
- there is no cwd-local `.guild/` default and no `target/dev-local-registry/...` operator default
- read-only commands do not silently create a missing root, while write-oriented commands may create the selected root when they are already performing real local mutation
- `guild init` is the current persistent local bootstrap workflow: it creates the selected root, prints the exact `guild mcp serve --stdio` wiring for the running `guild` binary, and may explicitly and idempotently update global or project Codex config files
- `guild codex` remains the deterministic repo-local scenario and smoke surface for bootstrap, config-printing, scenario preparation, and stdio smoke flows; it is not the normal persistent operator setup path

## 10. Identity and Resolution

### 9.1 Requested skill identity

Guild MUST permit callers to refer to skills using stable human-meaningful requested references.

### 9.2 Resolution requirement

Before execution begins, the host MUST resolve a `RequestedSkillRef` to a `ResolvedSkillRef`.

### 9.3 Immutable execution identity

A `ResolvedSkillRef` MUST correspond to an immutable executable artifact. Digest-pinned identity is the preferred form.

### 9.4 Execution binding

Execution MUST record the resolved identity that actually ran.

### 9.5 Durable execution identifier ownership

Durable execution record identifiers MUST be minted by the host. Callers MAY supply correlation or request identifiers, but those values MUST NOT control durable execution record identity.

### 9.6 Mutable aliases

Implementations MAY support aliases such as stable, approved, or channel-like selectors, but execution MUST still bind to a concrete immutable resolved artifact.

Implementation-language package metadata, such as Cargo package versions, MUST NOT be treated as requested or resolved Guild skill identity. Execution identity MUST continue to derive from the Guild manifest contract and the resolved executable artifact selected by the host.

### 9.7 Requested resolution ambiguity

If a requested reference would match multiple installed digests for the same selected skill key and version, the host MUST use an explicit deterministic policy. Silent tie-breaking by scan order, directory order, or incidental path order is not conformant.

The current repository policy is to reject such resolution as ambiguous unless the caller already names an exact resolved digest.

### 9.8 Resolution visibility

The resolved identity SHOULD be inspectable after execution.

## 11. Artifact Packaging and Lifecycle

### 10.1 Installation

Guild SHOULD support local installation of executable artifacts into host-managed records.

Install flows MUST NOT require destructive pre-deletion of existing installed state.

### 10.2 Export

Guild SHOULD support exporting installed artifacts as portable bundles.

The current repository supports two local transport mappings for installed executable state:

- a native signed installed-bundle directory
- an OCI image layout that carries the same signed installed-bundle semantics as OCI blobs and descriptors

The current repository also supports OCI registry transport for that same installed executable state by pushing and pulling the OCI-mapped signed bundle through a registry reference.

### 10.3 Import

Guild SHOULD support importing bundles into a fresh Guild root while preserving executable identity.

Import from any supported transport mapping MUST reconstruct normal installed executable state under the target Guild root without requiring source trees or local rebuilds.

### 10.4 Integrity verification

Imported artifacts MUST be integrity-verifiable. Verification failure MUST prevent execution.

### 10.5 Host authority on import

The host MAY accept, reject, quarantine, or reclassify imported bundles. Import does not imply trust.

OCI image layout transport MUST NOT bypass the host's trust or signature verification rules. If OCI image layout is supported, its import path MUST still verify layout structure, bundled digests, and the current host-owned signature and publisher-trust metadata before installation.

OCI registry transport MUST NOT bypass the host's trust or signature verification rules. If OCI registry transport is supported, its pull/import path MUST still verify pulled OCI manifest structure, pulled blob digests, the reconstructed signed bundle digest, and the current host-owned signature and publisher-trust metadata before installation.

### 10.6 Installed execution source

Execution SHOULD resolve from host-managed installed state rather than directly from mutable source directories.

### 10.7 Atomic local source installs

Local source installs SHOULD stage into temporary host-managed state, validate there, and then move into installed state atomically. Install failure MUST NOT destroy previously working installed artifacts for the same requested key and version.

## 12. Runtime Boundary

### 11.1 Constrained execution

Guild SHOULD execute skills inside a constrained runtime boundary. Wasm/WASI is the reference design target.

### 11.2 No ambient authority

Guests MUST NOT implicitly inherit unrestricted host filesystem, process, network, or secret access.

### 11.3 Host mediation

Sensitive operations MUST flow through host-defined interfaces.

### 12.4 MCP Integration

#### 12.4.1 MCP role

Guild MAY expose its local runtime and durable resource model through MCP, but that MCP layer MUST remain a façade over the existing Guild runtime boundary rather than a second execution engine.

#### 12.4.2 Transport scope

The current repository implements MCP over stdio only. Streamable HTTP, subscriptions, and list-changed notifications are not part of the current milestone and MUST NOT be advertised unless actually implemented.

#### 12.4.3 Tool surface

The public MCP tool surface SHOULD remain small and stable. The current repository surface is one primary tool, `guild.inspect`, rather than one top-level tool per installed skill.

The current repository advertises `guild.inspect` with truthful MCP hints for the active inspect slice: `readOnlyHint = false`, `destructiveHint = false`, `idempotentHint = false`, and `openWorldHint = true`. These annotations are metadata hints for clients, not security controls, and they MUST reflect the actual inspect/runtime/storage behavior rather than the name of the tool.

#### 12.4.4 MCP resources

Guild durable execution and evidence artifacts MAY be exposed through MCP resources, but resource URIs, resource contents, and linkage metadata remain host-owned Guild concepts. MCP resource access does not replace Guild runtime capability enforcement inside guest execution.

Where Guild exposes MCP list operations, the current repository uses opaque cursor-based pagination with deterministic ordering over already-bounded result sets. `resources/list` is a bounded discovery catalog over persisted Guild resources rather than a broad discovery or search interface.

The current repository `resources/list` contract is:

- the first listed URI is the canonical recent-executions query resource, `guild://queries/executions/recent/10`
- the second listed URI is the canonical recent-failures query resource, `guild://queries/executions/failures/recent/10`
- those query entry points are followed by a bounded recent-execution slice and then a bounded recent evidence-metadata slice from the same selected Guild root
- evidence payload URIs and raw blob URIs remain readable through `resources/read` and discoverable through `resources/templates/list` or inspect-result links, but they are not listed by default in `resources/list`

#### 12.4.5 Tool error semantics

Malformed protocol requests, unknown methods, and invalid tool arguments SHOULD use protocol error semantics.

Business or runtime failures from a real `guild.inspect` execution attempt SHOULD use MCP tool-result error semantics and preserve the persisted Guild execution receipt or record rather than collapsing that information into an opaque protocol error.

### 11.4 Runtime portability

The runtime design SHOULD minimize host-specific coupling so skill artifacts remain portable.

## 13. Capability Model

### 12.1 Typed capabilities

Capability grants SHOULD be represented as typed capability slices rather than vague global access.

### 12.2 Least privilege

The host MUST grant only the capabilities needed for the execution after applying host-owned policy.

Caller-requested capabilities are policy input, not final authority. The host MAY reduce or reject caller intent before guest execution begins.

### 12.3 Capability families

The frozen core live-runtime family registry for this milestone is defined in
section 4.5.

Shared Rust types, broader WIT package surfaces, or draft harnesses MAY still
carry broader future-facing capability vocabulary, but those names are not part
of Contract Surface v1 (core) unless they also appear in section 4.5.

If a capability family is supported on a given runtime path, it MUST be
enforced by the host.

### 12.4 Denied capability use

If a guest attempts an ungranted operation, the host MUST deny it in a way that can be represented durably.

Authorization denials MUST remain host-owned outcomes. A capability denial MUST NOT be silently recast as a guest-domain skill failure.

### 12.4a Unsupported runtime surface

If execution reaches a broader or currently unsupported runtime surface in the active inspect path, the host MUST classify that outcome distinctly from policy denial and from operational runtime failure.

In the current repository this is a host-owned rejected execution with termination detail classified as `unsupported-runtime-surface`. The deferred filesystem contract currently uses the more specific host-owned code `filesystem-runtime-not-supported` inside that same unsupported-runtime-surface classification.

### 12.5 No self-escalation

Guests MUST NOT mint, widen, or self-approve their own capabilities.

### 12.6 Current typed families in this repository

The current repository enforces typed constraints for:

- `filesystem` (host-side contract only; rejected before execution in the active Wasm inspect slice): `preopened_roots`, where each root declares `name`, `guest_path_prefix`, `host_path`, and `operations`
- `http-request`: `allowed_schemes`, `allowed_hosts`, `allowed_host_suffixes`, `allowed_ports`, `allowed_methods`, `allowed_path_prefixes`, `max_timeout_ms`, `max_response_bytes`, `follow_redirects`, `max_redirects`, `allow_loopback`, `allow_link_local`, `allow_private_networks`, `allow_ip_literals`
- `read-resource`: `uri_prefixes`, `resource_kinds`
- `invoke-skill`: `aliases`
- `emit-evidence`: `max_bytes`, `audiences`, `redactions`
- `log-write`: `levels`

Those are the current typed product names in shared host-side contracts. Unknown fields, wrong-family constraint shapes, empty scoped lists, and vague `filesystem` entries without explicit root contracts are validation errors.

For `filesystem`, `CapabilityAccess::Read` roots may only declare `read` operations, while `CapabilityAccess::Write` roots may only declare `write`, `create`, or `append` operations. The contract exists so manifests, caller intent, and local policy can represent filesystem authority intentionally, but the current guest ABI does not expose filesystem imports and the active Wasm inspect slice MUST reject any manifest or granted filesystem capability before guest execution.

Unsupported runtime surface outcomes are not policy denials. If policy allowed the request but the active inspect runtime still rejects a broader capability family or broader Guild import surface, the persisted `PolicyDecision` remains the host-owned policy truth while the termination detail carries the host-owned unsupported-runtime-surface classification.

For `read-resource`, `uri_prefixes` are the canonical local Guild scope roots
frozen in section 4.4 rather than arbitrary string prefixes. Authorization
MUST compare parsed Guild URIs against those canonical scopes rather than using
loose raw-string prefix matching. The object-record scope covers both
`guild://objects/records/{evidence_record_id}` payload URIs and
`guild://objects/records/{evidence_record_id}/metadata` metadata URIs.

For `http-request`, the current repository exposes a bounded request/response model rather than ambient networking. The host MUST parse absolute HTTP or HTTPS URLs, reject embedded credentials, enforce typed scheme/host/domain-suffix/port/path/method constraints before dispatch, and clamp timeout and response-size limits to host-owned bounds. Redirect following MUST remain disabled unless the granted capability explicitly enables it with a bounded `max_redirects`, and every redirected hop MUST pass the same host-owned authorization path before dispatch. Loopback, link-local, private-network, and raw IP-literal destinations MUST fail closed unless the granted capability explicitly allows those destination classes. Authorization denials MUST remain host-owned. The current inspect slice supports bodyless `GET` and `HEAD` requests only, does not expose arbitrary request headers or request-body streaming, and returns a bounded typed response body to the guest.

Shared contracts may mention broader capability families for future phases, but the active inspect slice MUST either prune unsupported families from the executable surface or reject them before execution. The current repository chooses preflight rejection.

### 12.7 M3/M4/M5/M6/M7 schema-bundle mapping and draft status

The draft schema bundle under `docs/schemas/draft-v1/` is still a draft M3/M4/M5/M6/M7 contract vocabulary. It is useful for tightening admission, minimization, token-materialization, and witness-verification semantics, but it is not the canonical product vocabulary for the current repository.

Contract Surface v1 (core) does not freeze those broader draft control-plane
semantics. It freezes only the runtime-facing surfaces listed in section 4.2.

The stricter interpretation wins:

- runtime guarantees MUST be explicit rather than inferred
- omitted or unknown guarantees MUST fail closed
- component portability MUST NOT be presented as enforcement portability
- hard-requirement compatibility precheck MUST NOT be presented as the full admission decision
- M4 `execution_plan` artifacts MUST be described as safe upper-bound invocation plans, not minimized authority proofs
- M5 proof outputs MUST preserve their non-binary status vocabulary rather than being collapsed into a fake minimal/non-minimal story
- M6 token outputs MUST be described as invocation-bound delegated capability tokens, not as runtime-general enforcement receipts or witness records
- M7 witness outputs MUST be described as bounded observed-authority records rather than runtime-general attestations
- denied requested authority MAY yield a downgrade rather than a refusal when hard requirements still hold

For M8c, the live Rust vocabulary is the canonical runtime vocabulary for this repository:

- `runtime_guarantee.supported_canonical_families` MUST be treated as the authoritative live-runtime family list
- `supported_effect_classes` MAY remain as a legacy draft-v1 compatibility surface, but it MUST NOT be described as the canonical live-runtime truth surface
- the currently active canonical runtime families are `http-request`, `read-resource`, `invoke-skill`, `emit-evidence`, and `log-write`
- draft-v1 direct canonical family support status MUST be described per family and per layer rather than as one undifferentiated runtime-general claim blob
- `docs/schemas/draft-v1/family_support_matrix.json` is the machine-readable status source for that per-family or per-layer draft-bundle view

Current live proof scope in M8c is intentionally narrow:

- `read-resource` MUST be described only as bounded live-proof-backed, and only for the immutable `guild://executions/` and `guild://objects/records/` scope roots the live Rust path actually explores today
- `log-write` MAY be described as live-proof-supported only for the observed discrete log-level slice the live Rust search actually proves
- `http-request` MUST be described only as bounded live-proof-backed for eight deterministic replay-fixtured shapes over `http`: loopback IP `GET` with an explicit port, loopback IP `GET` with the implicit default HTTP port, loopback IP `HEAD` with an explicit port, loopback IP `HEAD` with the implicit default HTTP port, `localhost` `GET` with an explicit port, `localhost` `GET` with the implicit default HTTP port, `localhost` `HEAD` with an explicit port, and `localhost` `HEAD` with the implicit default HTTP port. Each hostname slice is supported only when the proof basis binds the literal host string, the effective port, resolved addresses and address families, loopback-only resolution semantics, and replay invalidation inputs for that binding. All eight slices stay exact observed path-prefix, query-free, redirect-free, and normalized-inspect-comparator-only.
- `invoke-skill` MUST be described only as bounded live-proof-backed for two exact zero-authority inspect slices: one exercised declared alias resolved through the installed dependency snapshot to one exact child digest, and one exact two-child same-alias fan-out in deterministic order under that same digest-pinned `guild-skill-inspect-v1` boundary
- broader `http-request` shapes, including other hostname forms, query or fragment components, redirects, multiple exercised requests outside the checked replay-backed slice set, and `https`, broader `invoke-skill` shapes including dynamic or broader resolution, broader multi-child fan-out, recursion, child-side authority use, and non-inspect child targets, plus broader or legacy `emit-evidence` flows beyond the exact single-emission fixed local-object-store slice, MUST remain explicitly `not_proven` for live proof and MUST NOT be described as proof-backed for token or witness linkage. For `emit-evidence`, one exact single-emission fixed local-object-store slice is now honest: the runtime binds the sink explicitly, the dedicated comparator replays that exact slice, and downstream proof linkage carries the exact sink and payload digest as a host-owned binding rather than widening the guest authority surface. The canonical `emit-evidence` authority shape remains coarse on purpose, so broader flows still stay fail-closed.

The repository now also carries a slice-aware measured benchmark artifact at `docs/schemas/draft-v1/benchmark_matrix.json` with the paired report at `docs/benchmarking/m8-real-path-benchmark.md`. Those artifacts are descriptive, not normative, but they MUST stay aligned with the checked repository truth:

- supported proof-linked slices measured today are exactly one bounded `read-resource` immutable-root slice, eight bounded `http-request` replay-fixtured `http` slices, two bounded `invoke-skill` zero-authority inspect slices, and one exact single-emission `emit-evidence` fixed-sink slice
- the only measured proof-only slice is one exact `log-write` observed `info`-level slice through M4 plus M5 only; the benchmark MUST NOT claim a checked real-path M6 or M7 `log-write` linkage slice
- the benchmarked unsupported slices are exactly redirect `http-request` and replay-unavailable `emit-evidence`, each with explicit default refusal, explicit upper-bound fallback issuance only when allowed, and unlinked witness behavior
- the benchmarked extra fail-closed walls are exactly unsupported `http-request` no-replay, `read-resource` execution-query shrink, and `invoke-skill` child-authority use
- the current checked benchmark measures M4 admission at about `12.5` to `17.0 ms`, M5 proof search at about `6.8 s` for `read-resource`, `7.3` to `7.5 s` for the supported `http-request` slices, `10.3 s` for the supported `invoke-skill` slice, `9.0 s` for the measured `log-write` slice, and `3.0` to `7.7 s` for the benchmarked unsupported slices or walls
- the checked negative-claim probes currently remain coverage-limited on every measured non-`log-write` slice and MUST NOT be reported as synthetic successes

Current mapping boundaries:

| `docs/schemas/draft-v1/` term | Current repository term | Contract status |
|---|---|---|
| `component.wit_world` | active inspect world `guild-skill-inspect-v1` plus host-owned runtime-entrypoint checks | bundled contracts now target the live inspect world explicitly |
| direct canonical `http-request` | `http-request` | direct canonical support in the draft M4, M6, and M7 layers |
| direct canonical `read-resource` | `read-resource` | direct canonical support in the draft M4, M6, and M7 layers |
| direct canonical `invoke-skill` | `invoke-skill` | direct canonical support in the draft M4, M6, and M7 layers at the current alias-only runtime scope |
| direct canonical `emit-evidence` | `emit-evidence` | direct canonical support in the draft M4, M6, and M7 layers |
| direct canonical `log-write` | `log-write` | direct canonical support in the draft M4, M6, and M7 layers at the current level-only runtime scope |
| `component.invoke` | `invoke-skill` | deprecated narrowing compatibility mapping |
| `net.connect` | `http-request` | deprecated narrowing compatibility mapping; only explicit HTTP(S) GET or HEAD scopes map safely |
| `net.resolve` | `http-request` | unsupported; the live runtime does not expose a standalone DNS-resolution family |
| `fs.read`, `fs.write`, `fs.list` | `filesystem` | partial; still rejected before guest execution in the active inspect slice |
| `secret.read` | `get-secret` | partial; no live inspect enforcement or observation path yet |
| `clock.read` | `wall-clock` | partial; draft term is less precise than the runtime family split |
| `capability.delegate` | host-owned child-grant reduction and delegation enforcement | related but split across policy and runtime semantics |

Until those vocabulary gaps are closed across the repository, `docs/schemas/draft-v1/` MUST stay explicitly labeled as draft/proposal surface rather than being described as normative repo truth.

Within that draft bundle, the current M4 surface is:

- `admission_request.schema.json` for invocation-specific caller intent
- `execution_plan.schema.json` for the resulting admission artifact
- `admission_engine.py` for deterministic request evaluation against one contract and one or more runtime guarantees

Those M4 plan artifacts are still unsigned by default in this milestone, but they are no longer blocked on fake signing language. The repository now has a real reusable Ed25519 sign/verify path for execution plans through the existing publisher identity and trusted-publisher model. `admission_engine.py` does not sign automatically, and checked-in examples remain unsigned unless a caller explicitly signs them later, so unsigned plans MUST NOT be described as already signed.

The current draft-bundle M5 surface is narrower still:

- `comparator_profile.schema.json` for deterministic comparator identity and inputs
- `proof_record.schema.json` for minimization outputs and cache identity
- `minimization_engine.py` plus `minimization_core.py` for counterfactual proof generation over an already-admissible M4 plan

That M5 path has hard limits:

- it MUST only preserve or reduce the M4 upper bound
- it MUST fail closed on comparator failure, comparator unavailability, runtime mismatch, or replay-harness gaps
- it MUST NOT claim runtime-general minimization because live proof is now real only for the narrow M8c families the Rust runtime actually supports today, while the older draft harness remains example-only for everything else
- it MUST distinguish `exact_minimal`, `bounded_minimal`, `reduced`, `no_reduction`, and `not_proven`

The current draft-bundle M6 surface is narrower than a full attestation system:

- `delegated_capability_token.schema.json` for root and child capability-token claims
- `token_verification_result.schema.json` for explicit allow or deny verification output
- `token_engine.py` plus `token_core.py` for draft-local issuance and verification

That M6 path has hard limits:

- it MUST NEVER issue authority outside the admissible M4 `execution_plan`
- it MUST issue from the M5 final authority subset when an acceptable proof exists
- it MUST refuse issuance by default when no acceptable proof exists unless the caller explicitly enables upper-bound issuance
- it MUST mark upper-bound issuance explicitly as `issuance_basis: m4_upper_bound`
- it MUST emit an explicit empty-capability token for zero-authority issuance rather than an ambiguous implicit no-op
- it MUST bind issued tokens to an explicit holder, one invocation call chain, and the chosen runtime identity published by the applicable M4 plan
- it MUST default to non-pass-through behavior
- child issuance MUST require an explicit parent token and MUST remain a subset of both the parent token and the applicable M4 or M5 authority envelope
- child issuance MUST NOT broaden scope, audience, runtime binding, expiry, or delegation depth
- verification MUST fail closed on schema mismatch, unknown issuer, unknown key id, invalid cryptographic protection, replay, expiry, not-before violation, audience mismatch, holder mismatch, runtime mismatch, call-chain mismatch, parent mismatch, or parent-child broadening
- the current draft implementation MUST be described as HMAC MAC protection over canonical JSON claims, not as public-key signatures
- the current replay and revocation story MUST be described as local verifier-state behavior only, not as distributed replay protection or distributed revocation
- M6 MUST NOT be presented as the later M7 witness layer

The current draft-bundle M7 surface is also intentionally narrow:

- `witness_record.schema.json` for exercised-authority witness records
- `witness_verification_result.schema.json` for explicit witness and fixed-claim verification results
- `witness_engine.py` plus `witness_core.py` for bounded witness generation, verification, and fixed claim checks

That M7 path has hard limits:

- it MUST record exercised authority separately from blocked attempted authority and granted-but-unused authority
- it MUST NOT widen any M4, M5, or M6 envelope during witness generation
- it MUST fail closed on invalid or missing MACs, unknown issuer or key id, linkage mismatch, runtime mismatch, audience mismatch, holder mismatch, call-chain mismatch, unsupported observation sources, malformed summaries, or insufficient coverage for the requested claim
- absence claims MUST require complete relevant coverage
- partial coverage MAY support positive observed facts, but it MUST NOT be treated as proof that nothing else happened
- unmapped or lossy runtime-native observations MUST produce `coverage_limited` or `unverifiable` outcomes, not silent success
- redaction MUST NOT be described as verified when it removes facts required for the requested claim
- the current draft implementation MUST be described as HMAC MAC protection over canonical JSON claims, not as public-key signatures or public attestation
- the current live Rust runtime now persists durable per-effect `authority_observations` for `http-request`, `read-resource`, `invoke-skill`, `emit-evidence`, and `log-write`
- `ExecutionRecord` JSON surfaces MUST also carry `authority_observations_recorded` so legacy executions that predate observation capture stay distinguishable from explicitly recorded empty observation lists
- the current witness path MAY describe runtime-backed exercised-authority and absence claims only where that live stream maps safely into the draft-v1 vocabulary
- for M8c, draft-v1 now carries the live canonical `http-request`, `read-resource`, `invoke-skill`, `emit-evidence`, and `log-write` families directly in witness generation and verification
- scope-only negative claims MAY now be supported for those five families when the relevant live observation coverage is complete
- proof-linked witnesses MUST remain limited to real live-runtime proofs. In this milestone that means bounded live `read-resource` linkage is real, bounded live `http-request` linkage is real only for the deterministic replay-fixtured loopback IP `GET` and `HEAD` slices with either an explicit port or the implicit default HTTP port plus exact `localhost` `GET` and `HEAD` with either an explicit port or the implicit default HTTP port and deterministic loopback-only resolution bindings, bounded live `invoke-skill` linkage is real only for the exact single-child zero-authority inspect slice and the exact two-child same-alias zero-authority inspect slice, bounded live `emit-evidence` linkage is real only for one exact single-emission fixed local-object-store slice with a carried host exact binding, broader `http-request`, `invoke-skill`, and `emit-evidence` shapes stay explicitly unlinked, and `log-write` linkage is honest only when a real live proof record is supplied
- positive observed facts MAY be carried in witness records under partial or complete coverage, but the current fixed claim vocabulary still does not expose per-family positive observed-fact claim types
- unmappable runtime-native families or semantics MUST still fail closed as `coverage_limited`, `unverifiable`, or explicit verification failure rather than being silently accepted

## 14. Execution Semantics

### 13.1 Execution attempt

Every top-level invocation MUST create a host-recognized execution attempt.

### 13.2 Outcome classes

Every execution attempt MUST terminate as one of at least:

- success
- failure
- rejection

### 13.3 Persistence

These outcomes MUST be durably representable as `ExecutionRecord` resources.

### 13.4 Host-issued receipt

A top-level execution SHOULD return a host-issued receipt suitable for locating the durable execution record.

Receipts SHOULD expose the host-issued durable execution URI rather than a caller-chosen identifier.

### 13.5 Composite executions

If composite skills are supported, child skill calls MUST create child execution attempts with durable parent-child linkage.

### 13.6 Rejection semantics

Policy rejection MUST be a real observable outcome, not a silent short circuit.

### 13.7 Mode separation

`inspect`, `plan`, and `apply` are distinct execution modes. Implementations MUST NOT smuggle mutation into `inspect` or `plan`.

### 13.8 Current repository mode support

The current repository implements `inspect` end to end, defers `plan`, and globally rejects `apply`.

## 15. ExecutionRecord Schema Requirements

A minimally useful `ExecutionRecord` MUST contain:

- execution identifier
- caller request or correlation identifier if supplied
- requested skill reference
- resolved skill reference
- parent execution identifier if applicable
- start timestamp
- terminal timestamp
- outcome class
- status summary or failure classification
- granted capability slice or reference thereto
- evidence references produced, and any persisted read-attribution metadata if the implementation records reads durably
- policy decision metadata sufficient for audit

The start and terminal timestamps in durable execution records MUST be host-stamped rather than guest-authored placeholders.

Implementations MAY include richer diagnostics, logs, structured outputs, lineage edges, or timing breakdowns.

## 16. Evidence Model

### 15.1 Durable evidence

Evidence MUST be persistable as a durable host-owned object.

### 15.2 Stable evidence references

Persisted evidence MUST be addressable by `EvidenceRef` values issued by the host.

An `EvidenceRef` SHOULD identify an evidence record for a single emission event rather than only the underlying blob digest.

### 15.3 Evidence linkage

The system SHOULD preserve which executions produced each evidence object.

Implementations MAY additionally preserve durable read attribution. The current repository does not yet persist a read-set in `ExecutionRecord`.

### 15.4 Shared read backend

If both host-side and guest-side reads exist, they SHOULD resolve through the same underlying durable backend.

### 15.5 Explain reuse

Evidence persisted during execution MUST be available for later inspect and explain flows when policy allows.

### 15.6 Provenance

Evidence SHOULD preserve provenance or chain-of-custody metadata sufficient for later analysis.

### 15.7 Blob identity and record identity

Implementations SHOULD distinguish content-addressed evidence blob identity from host-issued evidence-record identity.

If multiple executions emit the same payload digest, implementations MAY deduplicate the underlying blob storage, but they MUST preserve distinct evidence-record identity and per-emission metadata.

The current repository uses distinct canonical URIs for those two evidence-record read shapes:

- `guild://objects/records/{evidence_record_id}` returns the emitted payload bytes using the evidence record as a stable dereference handle
- `guild://objects/records/{evidence_record_id}/metadata` returns serialized host-owned `EvidenceRecord` metadata for that same emission
- `guild://objects/sha256/{digest}` returns the raw content-addressed blob bytes

## 17. Inspect and Explain

### 16.1 Supported workload type

Guild SHOULD support inspect and explain skills as first-class workloads.

### 16.2 Artifact grounding

Inspect and explain behavior SHOULD be grounded in durable execution and evidence artifacts, not just transient conversational recall.

### 16.3 Failed and rejected executions

Failed and rejected executions MUST remain readable to inspect and explain flows when policy permits.

### 16.4 Truthfulness boundary

Explain skills MUST NOT claim certainty beyond what durable artifacts support.

## 18. Composite Skill Requirements

### 17.1 Durable lineage

Parent-child execution relationships MUST be durable and queryable.

### 17.2 Child identity preservation

Each child execution MUST preserve its own requested and resolved identity.

### 17.3 Mixed outcomes

A composite skill MAY succeed despite failed or rejected child executions, but those child records MUST remain inspectable.

### 17.4 Capability narrowing

Implementations SHOULD prefer narrowing capability grants across child calls rather than widening them.

## 19. Policy Model

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

### 18.3 Policy inputs and outputs

A policy evaluator SHOULD be able to consider at least:

- caller identity such as `actor_id`
- tenant identity such as `tenant_id`
- the target skill identity
- caller-requested capabilities
- manifest-declared required capabilities
- installed verification or trust metadata when available

A host policy decision SHOULD distinguish:

- the caller-requested capability set
- the final granted capability set
- an outcome of allowed, reduced, or rejected
- the policy profile used for the decision
- the host-owned verification state and local trust tier considered
- host-owned reason codes sufficient for later explanation

For the current repository's user-facing docs and help, these same stages are
described with one stable authority lifecycle vocabulary:

- declared authority: capabilities declared by the installed manifest
- requested authority: caller-requested grants for one run
- granted authority: the final capability slice the host policy allows for that run
- effective at runtime: the authority the guest can actually exercise during execution

This explanatory vocabulary does not widen the normative model above; the
host-owned policy decision and final granted capability set remain canonical.

### 18.4 Durable denial record

Policy rejection SHOULD produce a durable record suitable for later inspection and audit.

In the current repository, authorization denials across runner checks and supported host imports are represented as host-owned rejected executions rather than guest-authored failures.

For supported runtime-side HTTP failures after authorization, the current repository distinguishes host-owned authorization rejections from bounded transport/runtime failures such as timeout or oversized response bodies. Those latter failures persist as unsuccessful executions without being reclassified as capability denials.

The current host-side `filesystem` contract may appear in manifests, caller-requested capabilities, and local `policy.json` profiles, but those surfaces MUST NOT enable guest filesystem runtime support in the active inspect slice. When filesystem survives policy evaluation into a final grant, the runner MUST reject it before guest start as a host-owned validation outcome.

The current repository uses a local file-backed `policy.json` with named
profiles plus a built-in default profile when the file is absent. Profile
selection is host-owned, actor/tenant scoped, and fail-closed when configuration
is unreadable, invalid, or ambiguous for a given execution.

### 18.5 Safety precedence

When policy conflicts with convenience, the host MUST prefer policy.

## 20. Resource Access Semantics

### 19.1 Host-mediated reads

Reads of persisted or external resources MUST be mediated by the host.

Local Guild resource authorization SHOULD parse supported Guild URIs into typed execution/blob/evidence-record/evidence-record-metadata/query forms before matching them against granted scopes. Malformed or ambiguous Guild URIs SHOULD be rejected rather than normalized loosely.

### 19.2 Explicit attribution

Resource access SHOULD be attributable to execution attempts.

### 19.3 Auditability

The system SHOULD be able to answer: what did this execution read, and under which capability grant?

### 19.4 Bounded query resources

Implementations MAY expose bounded query resources derived from persisted host-owned artifacts, but those resources MUST remain canonical host-issued URIs rather than a free-form search language.

If query resources are supported:

- they MUST remain deterministic and bounded by explicit host-owned limits
- they MUST return structured host-owned results that point back to canonical execution or evidence URIs
- guest `read-resource` and host or MCP resource reads MUST observe the same query backend result for the same query URI
- authorization MUST remain fail-closed and SHOULD require an explicit query scope rather than implicitly reusing exact-record execution or object grants

The current repository supports bounded execution-query resources only. It exposes recent, failure, by-status, and by-skill execution views under `guild://queries/executions/...`, keeps result limits in the closed range `1..=50`, orders results deterministically by host-stamped execution timestamps plus execution ID, and does not implement full-text search, arbitrary boolean query DSLs, subscriptions, list-changed notifications, or broader evidence-query resources in this milestone.

## 21. Security Properties

A strong Guild implementation SHOULD provide all of the following:

- immutable execution identity
- no ambient authority
- host-issued execution and evidence identifiers
- durable policy denials
- integrity-verifiable import and export
- forensic support through durable records and evidence

Guild does not guarantee that the guest makes perfect decisions. It guarantees that the system around the guest is explicit, inspectable, and governable.

## 22. Conformance

A system may call itself Guild-conformant only if it supports all of the following:

1. request-to-resolution before execution
2. immutable executable identity
3. host-mediated constrained execution
4. durable execution records for success, failure, and rejection
5. durable evidence with host-issued references
6. host-owned capability enforcement
7. artifact-grounded inspection of prior executions

If composite skills are supported, durable parent-child linkage is also REQUIRED.

## 23. Extension Points

Future specs or ADRs SHOULD define:

- remote publication and registry mapping for installed bundles
- signatures and provenance chains
- richer evidence query surfaces and notification models
- retention and garbage collection
- capability schema registry and versioning
- replay semantics
- federation across Guild roots
- human approval or interruption semantics

## 24. Bottom Line

Guild exists to force AI skill execution out of the swamp of informal orchestration and into a real software contract.

That means immutable identity, constrained execution, durable records, durable evidence, and later explanation grounded in what actually happened.

Everything else is secondary.
