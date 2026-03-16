# Findings

## Summary

Guild now has a real local inspect-only skills fabric, not just a contract sketch.

What is materially real today:

- source skills install into digest-pinned executable records
- installed skills can be exported as signed portable local bundles and imported into a fresh Guild root
- `RequestedSkillRef` resolves to `ResolvedSkillRef` before execution
- skills execute through the real Wasmtime-backed Wasm component path
- primitive and composite inspect skills run end to end
- resolved top-level and child execution attempts persist as host-owned `ExecutionRecord` resources on success, failure, and rejection
- durable execution IDs are host-minted, collision-resistant, and protected against silent overwrite
- evidence persists as durable local Guild objects with distinct blob identity and per-emission evidence-record identity
- Guild now runs as a real MCP server over stdio, not just an internal façade with MCP-shaped concepts
- MCP resource reads and guest-side `read-resource` calls use the same local backend
- a resource-aware explain skill can read stored execution and evidence artifacts through the Wasm host boundary, including failed and rejected records
- top-level unsuccessful inspect calls return host-issued execution receipts pointing at persisted `guild://executions/...` records
- supported inspect-slice capability families now use typed host-enforced constraints
- unsupported capability families are rejected before execution in the active inspect slice
- `read-resource` authorization now matches parsed canonical Guild URI scopes instead of loose raw string prefixes
- same-version / different-digest requested resolution now fails closed instead of silently picking an artifact
- local source installs are staged and atomic instead of destructive pre-delete operations
- durable execution records now carry host-stamped start and finish timestamps
- signed bundle import now verifies local trust, signature validity, and bundled digests before installation
- the stricter workspace pedantic/cargo/future-not-send Clippy pass is now clean across code, examples, and test harnesses

The trust boundary remains intact:

- callers ask for requested refs
- the registry resolves to digest-pinned executable refs
- the runner executes only resolved refs
- skills return `SkillOutput`
- the host owns durable execution identity, `ExecutionRecord`, evidence storage, URIs, timestamps, denial classification, and child execution metadata

## Status Snapshot

Where the repository is now:

- Guild has completed a real local inspect-first vertical slice, not just a type skeleton.
- Phase 1 of the roadmap is materially done: local install, resolve, execute, persist, evidence, and `guild.inspect` all work end to end.
- Parts of Phase 2 are already real: composite execution, alias-scoped child invocation, and durable child lineage are working.
- Parts of Phase 3 are already real: signed installed bundles, local publisher identity, local trust-store verification, and fail-closed import checks are implemented.
- The integrity-hardening pass is complete for the current inspect-only substrate: host-minted execution IDs, create-only execution persistence, split evidence record identity, ambiguity rejection, atomic installs, unified host-owned denials, honest inspect capability surface, canonical Guild URI authz, and host-stamped timestamps are all in place.
- Guild now also has a real stdio MCP server surface over that same runtime and storage path, with one honest public tool (`guild.inspect`) plus durable Guild resources.

What this means in practice:

- Guild is already a credible local-first execution fabric for inspect-mode skills.
- The current public surface is intentionally narrow, but it is real, test-backed, and contract-shaped rather than demo glue.
- The remaining work is mostly about broadening capability carefully and filling in deferred modes and distribution concerns, not proving the core architecture exists.

## What We Have Done

- Established a clean execution boundary between `RequestedSkillRef` and `ResolvedSkillRef`.
- Split skill-authored `SkillOutput` from host-owned `ExecutionRecord`.
- Added a local source-to-installed lifecycle so normal development no longer depends on manually copying Wasm artifacts or pasting digests into source manifests.
- Switched the working runtime path to real Wasmtime-backed Wasm component execution.
- Added inspect-only primitive and composite example skills.
- Implemented alias-scoped child invocation through the Wasm host boundary with host-owned child execution metadata.
- Added a local execution store and durable evidence/object store with Guild URIs.
- Wired MCP resource reads to the same local execution/evidence backend.
- Implemented guest-side `read-resource` with explicit `read-resource` capability enforcement.
- Added a new inspect-only `explain-execution` skill that reads stored execution/evidence resources and returns a structured report.
- Added a new inspect-only `explain-execution-tree` skill that walks persisted execution lineage with bounded traversal and summarizes evidence across the tree without inlining payloads.
- Made resolved execution attempts durable on success, failure, and rejection with host-owned termination metadata.
- Added persisted execution receipts on top-level failure/rejection so callers can immediately address the stored execution URI.
- Replaced loose capability constraint handling with typed constraints plus one shared host-side evaluator.
- Added portable local bundle export/import built from installed executable records, including composite dependency closure export/import.
- Added local publisher identities, signed bundle export, local trust-store verification on import, and host-owned verification metadata for imported installs.
- Hardened execution identity so durable execution IDs are host-minted UUIDv7 values rather than caller-controlled IDs or process-local counters.
- Made execution persistence create-only so stored execution records cannot be silently overwritten by duplicate IDs.
- Split evidence payload blob identity from evidence-record identity and changed `EvidenceRef` to point at host-issued evidence-record URIs.
- Added explicit ambiguity rejection when a requested key and version map to multiple installed digests.
- Reworked local source installs to stage, validate, and atomically move digest directories into place without deleting sibling installs.
- Unified host authorization denials so runner checks and supported Wasm host imports persist as host-owned rejected executions instead of guest-domain failures.
- Enforced an honest active inspect runtime slice by rejecting unsupported capability families before execution.
- Replaced raw-prefix `read-resource` authorization with parsed canonical Guild URI scope matching and fail-closed URI validation.
- Stamped durable execution provenance with real host-generated UTC start and finish timestamps across top-level and child records.
- Added a real stdio MCP server entrypoint with honest initialize/capabilities, one public tool (`guild.inspect`), Guild URI resources, and resource templates.
- Mapped successful inspect calls to MCP tool results with `structuredContent`, text compatibility output, and execution/evidence resource links.
- Mapped unsuccessful inspect executions to MCP tool errors with `isError: true` while preserving persisted execution receipt and record information.
- Raised crate-level lint strictness across the workspace to `clippy::all`, `clippy::pedantic`, `clippy::cargo`, and `clippy::perf`, then resolved the resulting warnings with API docs, `#[must_use]`, safer numeric conversions, smaller helper boundaries, and more explicit error handling.
- Added honest crate package metadata for the local workspace crates so `clippy::cargo` checks now pass on descriptions, repository/readme linkage, keywords, and categories.
- Followed through on the stricter test/example pass by cleaning the remaining `clippy::pedantic`, `clippy::cargo`, and `clippy::future_not_send` warnings in the MCP stdio harnesses and registry/runner test fixtures instead of suppressing them.
- Kept two intentionally narrow Clippy exceptions:
  - `clippy::multiple_crate_versions` remains allowed at crate roots because the active Wasmtime/Cranelift dependency graph currently pulls incompatible `hashbrown` major lines upstream.
  - `clippy::struct_excessive_bools` remains allowed only for the MCP `ToolAnnotations` wire struct because it mirrors the external MCP protocol shape directly.

## Current Functionality

### Install and resolve

- Source manifests and installed manifests are distinct lifecycle stages.
- `LocalSourceInstaller` builds Wasm component skills locally, computes digests, stages artifacts, validates them, and atomically writes installed manifests without pre-deleting existing version state.
- Installed executable directories can now be exported as signed portable local bundles and imported into a fresh local registry root.
- `LocalRegistry` resolves only installed executable records for execution.
- Requested semver-style resolution now fails closed with an explicit ambiguity error if the selected key and version exist under multiple installed digests.

### Portability

- Portable bundles are built from installed executable state, not source directories.
- A signed bundle contains the installed manifest, staged Wasm artifact, staged support files, explicit digests for bundled files, a bundle index identifying the root skill and included installs, and a detached signature envelope.
- Import verifies bundle structure, publisher trust, signature validity, and bundled digests before copying anything into the target registry.
- Imported skills become normal installed records under the target registry's `installed/...` tree.
- Imported execution does not require the original source tree or a local rebuild.
- Imported verified installs carry host-owned verification metadata in registry-side sidecars.

### Execute

- The real runtime path is `WasmtimeRuntimeAdapter` using `wit/guild-skill-v1.wit`.
- `ExecutionContext` carries explicit `CapabilityGrantSet` data into the guest together with a host-minted durable execution ID.
- The runner still executes only resolved refs and still globally rejects `apply`.
- The host now evaluates typed constraints for `read-resource`, `invoke-skill`, `emit-evidence`, and `log-write`.
- Unsupported capability families in the broader shared contract are rejected before execution in the active inspect slice.
- Host authorization denials across runner checks, `read-resource`, `emit-evidence`, `invoke-dependency`, and `log` are classified through one host-owned rejection model.
- Durable successful, failed, and rejected execution records now carry host-stamped start and finish timestamps.

### Compose

- Composite skills declare dependencies by alias in source manifests.
- Installed manifests pin dependency aliases to `ResolvedSkillRef`.
- Guests invoke children by alias through `invoke-dependency`, not by arbitrary ref.
- Child execution goes through the same registry + runner + Wasmtime path as top-level execution.
- Child grants are reduced from parent grants and child requirements.

### Persist and read artifacts

- Resolved execution attempts persist under `guild://executions/...` whether they succeed, fail, or are rejected.
- Execution persistence is create-only, so duplicate durable IDs fail closed instead of overwriting prior records.
- Evidence payload blobs persist under `guild://objects/sha256/...`.
- Evidence emissions persist under distinct host-issued record URIs at `guild://objects/records/...`.
- Parent execution records retain host-owned child execution metadata and child execution URIs.
- Failed and rejected execution records carry host-owned `termination` metadata and may omit `SkillOutput`.
- Top-level unsuccessful inspect calls still return errors, but those errors now carry a receipt URI for the persisted execution record.
- Evidence records retain per-emission metadata plus `produced_by_execution` linkage even when multiple executions emit the same payload digest.
- MCP can read execution resources, evidence-record URIs, and underlying payload blobs from the same local store.
- Guests can now read allowed Guild URIs through `read-resource` when granted typed `uri_prefixes` plus `resource_kinds`.
- `read-resource` authorization now parses Guild URIs and canonical scope roots like `guild://executions/`, `guild://objects/records/`, and `guild://objects/sha256/` before matching.
- Malformed or ambiguous Guild URIs fail closed instead of being normalized or accepted through permissive prefix logic.

### MCP server surface

- `guild-mcp-server` can be launched as a stdio MCP subprocess against a local Guild root.
- The active public MCP tool surface is intentionally minimal: one tool, `guild.inspect`.
- `tools/list` publishes honest input and output schemas derived from the existing Guild-facing types.
- `tools/call` for `guild.inspect` executes through the same `GuildMcpFacade -> registry -> runner -> Wasmtime` path as the direct Rust façade.
- Successful MCP tool results include `structuredContent`, a text compatibility block, and resource links to the persisted execution record and emitted evidence records.
- Unsuccessful inspect executions that reached a real resolved execution attempt are surfaced as MCP tool errors with preserved persisted execution record identity instead of opaque protocol crashes.
- MCP `resources/read` exposes execution records, evidence-record payloads, and digest-addressed blobs through the same local resource backend Guild already used internally.
- MCP `resources/templates/list` now exposes canonical Guild URI templates for execution records, evidence records, and raw blobs.
- MCP `resources/list` remains intentionally narrow and honest by listing only a bounded recent view of execution records.

### Example flows

Canonical local proof commands:

```bash
cargo run -p guild-mcp --example inspect_local
cargo run -p guild-mcp --example inspect_composite_local
cargo run -p guild-mcp --example explain_execution_local
cargo run -p guild-mcp --example explain_execution_tree_local
cargo run -p guild-mcp --example explain_failure_local
cargo run -p guild-mcp --example export_import_local
cargo run -p guild-mcp --example export_import_composite_local
cargo run -p guild-mcp --example signed_import_failures_local
cargo run -p guild-mcp --example mcp_stdio_local
```

What they prove:

- `inspect_local`: install `hello-inspect`, execute it, read back stored execution + evidence
- `inspect_composite_local`: install `hello-inspect`, install `hello-composite`, execute composite inspect, read back parent + child + child evidence
- `explain_execution_local`: install `hello-inspect`, produce a stored execution URI, install `explain-execution`, then run a resource-aware skill against that stored execution through the Wasm host boundary
- `explain_execution_tree_local`: install `hello-inspect` and `hello-composite`, produce a stored parent/child execution tree, install `explain-execution-tree`, then walk that stored lineage through the same host-mediated resource path
- `explain_failure_local`: trigger a persisted rejected execution, capture its receipt URI, then run `explain-execution` against that stored unsuccessful record
- `export_import_local`: install `hello-inspect` into registry A, generate a local publisher identity, export a signed installed bundle, trust that publisher in fresh registry B, import, resolve by `RequestedSkillRef`, and execute without rebuilding
- `export_import_composite_local`: export `hello-composite` together with its installed dependency closure as a signed bundle, trust the publisher in fresh registry B, and execute the composite plus child entirely from imported installed records
- `signed_import_failures_local`: prove both untrusted-publisher rejection and tampered-bundle rejection before unsafe executable state is installed
- `mcp_stdio_local`: launch `guild-mcp-server` as a subprocess, initialize over stdio JSON-RPC, list tools, call `guild.inspect`, and read back the returned execution/evidence URIs through MCP resources

Each command uses its own cleaned subdirectory under `target/dev-local-registry/`, so repeated local runs stay deterministic and do not overwrite another proof flow's stored execution ids.

## Gaps

Still intentionally missing or narrow:

- no `plan` execution path yet
- `apply` remains globally gated off
- no remote registry or publication flow
- no remote signatures, transparency logs, or trust/publication metadata beyond the local offline trust store
- no full policy engine beyond explicit caller-provided grants
- no MCP subscriptions, list-changed notifications, or HTTP transport
- no search, indexing, or query layer over stored executions/evidence
- no arbitrary filesystem or non-Guild URI reads from guests
- no guest-side write/update resource API beyond evidence emission
- no workflow/orchestration DSL

Current sharp edges worth remembering:

- capability hardening is still intentionally narrow to the currently implemented capability families, not a general policy language
- the local store is honest and useful, but still not a broader storage platform
- pre-resolution request/lookup failures are still not persisted in this milestone
- persistence failures themselves still surface as direct errors; Guild does not yet write provisional/in-progress records
- unsuccessful records now have a consistent host-owned rejection path for authorization denials, but still not a broader incident taxonomy or retry/orchestration system

## Next Steps

The clean next milestones after integrity hardening are:

1. Expand capability enforcement deliberately
   - add more typed families only when there is a real host operation behind them
   - keep nested grant reduction conservative and explicit

2. Build on portability
   - treat installed bundles as the local transport unit future publication can build on
   - keep import/export focused on installed executable state instead of source packages

4. Prepare for richer artifact reuse
   - build on the current execution/evidence resource model before adding any search or subscription surface
   - keep MCP and guest reads on the same backend
   - keep the public MCP tool surface small rather than drifting into one-tool-per-skill sprawl

5. Only then widen outward
   - policy evaluation
   - remote registries/publication
   - eventually `plan`
   - much later, carefully gated `apply`

6. Keep integrity work local and surgical
   - add replay/idempotency semantics only when they are explicit and testable
   - add install GC or replacement policy only when it cannot reintroduce destructive ambiguity

## Coverage

Current proof and validation commands:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -W clippy::pedantic -W clippy::cargo -W clippy::future_not_send
cargo run -p guild-mcp --example inspect_local
cargo run -p guild-mcp --example inspect_composite_local
cargo run -p guild-mcp --example explain_execution_local
cargo run -p guild-mcp --example explain_execution_tree_local
cargo run -p guild-mcp --example explain_failure_local
cargo run -p guild-mcp --example export_import_local
cargo run -p guild-mcp --example export_import_composite_local
cargo run -p guild-mcp --example signed_import_failures_local
cargo run -p guild-mcp --example mcp_stdio_local
```

Regression coverage now includes:

- local build/install producing staged Wasm artifacts and digest-pinned installed records
- signed bundle export/import for primitive and composite installed skills
- local Ed25519 publisher identity generation and trust-store verification
- bundle metadata identifying the exported root skill and included installed records
- detached bundle signature metadata and bundled file digests
- registry resolution returning executable `ResolvedSkillRef` records from installed manifests
- reinstall digest changes when guest artifacts change
- requested same-version multi-digest ambiguity failing closed instead of silently choosing a digest
- missing staged artifacts failing closed
- missing/tampered bundle content failing closed on import
- source-only manifests not being treated as executable installs
- failed source reinstalls preserving previously working installed digests
- primitive and composite Wasmtime execution
- imported primitive and composite execution through the same Wasmtime-backed path without rebuild
- imported verified installs carrying host-owned verification metadata
- alias-scoped child invocation and undeclared alias rejection
- child grant reduction and child budget reduction
- caller request IDs not controlling durable execution IDs
- duplicate durable execution record persistence being rejected
- persisted top-level and child execution records on success, failure, and rejection once a resolved execution attempt exists
- durable evidence blob storage and host-issued per-emission evidence refs
- distinct evidence-record URIs for repeated emissions of the same payload digest
- guest-side `read-resource` authorization and failure modes
- canonical Guild URI scope validation and parsed URI authorization matching
- host-owned denial classification for authorization failures across runner and import paths
- unsupported capability families failing before execution in the active inspect slice
- durable provenance timestamps on successful, failed, rejected, and child records
- shared backend consistency between MCP resource reads and guest resource reads
- real stdio MCP initialize/tools/resources flows against a subprocess server
- strict workspace lint verification via `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- MCP tool-error semantics preserving persisted execution receipts instead of collapsing them into raw protocol failures
- bounded recent execution resource listing and canonical Guild URI resource templates
- resource-aware explain skill execution against stored successful, failed, and rejected artifacts
- documented primitive and composite portability proof flows using separate registry A / bundle / registry B roots
- documented negative trust proof flow for untrusted and tampered signed bundles
