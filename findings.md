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
- evidence persists as durable local Guild objects with host-issued `EvidenceRef` values
- MCP resource reads and guest-side `read-resource` calls use the same local backend
- a resource-aware explain skill can read stored execution and evidence artifacts through the Wasm host boundary, including failed and rejected records
- top-level unsuccessful inspect calls return host-issued execution receipts pointing at persisted `guild://executions/...` records
- supported inspect-slice capability families now use typed host-enforced constraints
- signed bundle import now verifies local trust, signature validity, and bundled digests before installation

The trust boundary remains intact:

- callers ask for requested refs
- the registry resolves to digest-pinned executable refs
- the runner executes only resolved refs
- skills return `SkillOutput`
- the host owns `ExecutionRecord`, evidence storage, URIs, and child execution metadata

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
- Made resolved execution attempts durable on success, failure, and rejection with host-owned termination metadata.
- Added persisted execution receipts on top-level failure/rejection so callers can immediately address the stored execution URI.
- Replaced loose capability constraint handling with typed constraints plus one shared host-side evaluator.
- Added portable local bundle export/import built from installed executable records, including composite dependency closure export/import.
- Added local publisher identities, signed bundle export, local trust-store verification on import, and host-owned verification metadata for imported installs.

## Current Functionality

### Install and resolve

- Source manifests and installed manifests are distinct lifecycle stages.
- `LocalSourceInstaller` builds Wasm component skills locally, computes digests, stages artifacts, and writes installed manifests.
- Installed executable directories can now be exported as signed portable local bundles and imported into a fresh local registry root.
- `LocalRegistry` resolves only installed executable records for execution.

### Portability

- Portable bundles are built from installed executable state, not source directories.
- A signed bundle contains the installed manifest, staged Wasm artifact, staged support files, explicit digests for bundled files, a bundle index identifying the root skill and included installs, and a detached signature envelope.
- Import verifies bundle structure, publisher trust, signature validity, and bundled digests before copying anything into the target registry.
- Imported skills become normal installed records under the target registry's `installed/...` tree.
- Imported execution does not require the original source tree or a local rebuild.
- Imported verified installs carry host-owned verification metadata in registry-side sidecars.

### Execute

- The real runtime path is `WasmtimeRuntimeAdapter` using `wit/guild-skill-v1.wit`.
- `ExecutionContext` carries explicit `CapabilityGrantSet` data into the guest.
- The runner still executes only resolved refs and still globally rejects `apply`.
- The host now evaluates typed constraints for `read-resource`, `invoke-skill`, `emit-evidence`, and `log-write`.

### Compose

- Composite skills declare dependencies by alias in source manifests.
- Installed manifests pin dependency aliases to `ResolvedSkillRef`.
- Guests invoke children by alias through `invoke-dependency`, not by arbitrary ref.
- Child execution goes through the same registry + runner + Wasmtime path as top-level execution.
- Child grants are reduced from parent grants and child requirements.

### Persist and read artifacts

- Resolved execution attempts persist under `guild://executions/...` whether they succeed, fail, or are rejected.
- Evidence objects persist under `guild://objects/sha256/...`.
- Parent execution records retain host-owned child execution metadata and child execution URIs.
- Failed and rejected execution records carry host-owned `termination` metadata and may omit `SkillOutput`.
- Top-level unsuccessful inspect calls still return errors, but those errors now carry a receipt URI for the persisted execution record.
- MCP can read execution and evidence resources from the same local store.
- Guests can now read allowed Guild URIs through `read-resource` when granted typed `uri_prefixes` plus `resource_kinds`.

### Example flows

Canonical local proof commands:

```bash
cargo run -p guild-mcp --example inspect_local
cargo run -p guild-mcp --example inspect_composite_local
cargo run -p guild-mcp --example explain_execution_local
cargo run -p guild-mcp --example explain_failure_local
cargo run -p guild-mcp --example export_import_local
cargo run -p guild-mcp --example export_import_composite_local
cargo run -p guild-mcp --example signed_import_failures_local
```

What they prove:

- `inspect_local`: install `hello-inspect`, execute it, read back stored execution + evidence
- `inspect_composite_local`: install `hello-inspect`, install `hello-composite`, execute composite inspect, read back parent + child + child evidence
- `explain_execution_local`: install `hello-inspect`, produce a stored execution URI, install `explain-execution`, then run a resource-aware skill against that stored execution through the Wasm host boundary
- `explain_failure_local`: trigger a persisted rejected execution, capture its receipt URI, then run `explain-execution` against that stored unsuccessful record
- `export_import_local`: install `hello-inspect` into registry A, generate a local publisher identity, export a signed installed bundle, trust that publisher in fresh registry B, import, resolve by `RequestedSkillRef`, and execute without rebuilding
- `export_import_composite_local`: export `hello-composite` together with its installed dependency closure as a signed bundle, trust the publisher in fresh registry B, and execute the composite plus child entirely from imported installed records
- `signed_import_failures_local`: prove both untrusted-publisher rejection and tampered-bundle rejection before unsafe executable state is installed

Each command uses its own cleaned subdirectory under `target/dev-local-registry/`, so repeated local runs stay deterministic and do not overwrite another proof flow's stored execution ids.

## Gaps

Still intentionally missing or narrow:

- no `plan` execution path yet
- `apply` remains globally gated off
- no remote registry or publication flow
- no remote signatures, transparency logs, or trust/publication metadata beyond the local offline trust store
- no full policy engine beyond explicit caller-provided grants
- no subscriptions, notifications, or change streams for resources
- no search, indexing, or query layer over stored executions/evidence
- no arbitrary filesystem or non-Guild URI reads from guests
- no guest-side write/update resource API beyond evidence emission
- no workflow/orchestration DSL

Current sharp edges worth remembering:

- capability hardening is still intentionally narrow to the currently implemented capability families, not a general policy language
- the local store is honest and useful, but still not a broader storage platform
- pre-resolution request/lookup failures are still not persisted in this milestone
- persistence failures themselves still surface as direct errors; Guild does not yet write provisional/in-progress records
- unsuccessful records currently use a simple host-owned failure model, not a broader incident taxonomy or retry/orchestration system

## Next Steps

The clean next milestones are:

1. Improve resource-aware skills
   - add one more explain/debug skill that consumes stored execution trees more deeply
   - keep it inspect-only and deterministic

2. Expand capability enforcement deliberately
   - add more typed families only when there is a real host operation behind them
   - keep nested grant reduction conservative and explicit

3. Build on portability
   - treat installed bundles as the local transport unit future publication can build on
   - keep import/export focused on installed executable state instead of source packages

4. Prepare for richer artifact reuse
   - build on the current execution/evidence resource model before adding any search or subscription surface
   - keep MCP and guest reads on the same backend

5. Only then widen outward
   - policy evaluation
   - remote registries/publication
   - eventually `plan`
   - much later, carefully gated `apply`

## Coverage

Current proof and validation commands:

```bash
cargo fmt --all
cargo test --workspace
cargo run -p guild-mcp --example inspect_local
cargo run -p guild-mcp --example inspect_composite_local
cargo run -p guild-mcp --example explain_execution_local
cargo run -p guild-mcp --example explain_failure_local
cargo run -p guild-mcp --example export_import_local
cargo run -p guild-mcp --example export_import_composite_local
cargo run -p guild-mcp --example signed_import_failures_local
```

Regression coverage now includes:

- local build/install producing staged Wasm artifacts and digest-pinned installed records
- signed bundle export/import for primitive and composite installed skills
- local Ed25519 publisher identity generation and trust-store verification
- bundle metadata identifying the exported root skill and included installed records
- detached bundle signature metadata and bundled file digests
- registry resolution returning executable `ResolvedSkillRef` records from installed manifests
- reinstall digest changes when guest artifacts change
- missing staged artifacts failing closed
- missing/tampered bundle content failing closed on import
- source-only manifests not being treated as executable installs
- primitive and composite Wasmtime execution
- imported primitive and composite execution through the same Wasmtime-backed path without rebuild
- imported verified installs carrying host-owned verification metadata
- alias-scoped child invocation and undeclared alias rejection
- child grant reduction and child budget reduction
- persisted top-level and child execution records on success, failure, and rejection once a resolved execution attempt exists
- durable evidence storage and host-issued evidence refs
- guest-side `read-resource` authorization and failure modes
- shared backend consistency between MCP resource reads and guest resource reads
- resource-aware explain skill execution against stored successful, failed, and rejected artifacts
- documented primitive and composite portability proof flows using separate registry A / bundle / registry B roots
- documented negative trust proof flow for untrusted and tampered signed bundles
