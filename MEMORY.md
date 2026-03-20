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
- evidence metadata is now directly readable as a first-class companion resource under `guild://objects/records/{evidence_record_id}/metadata` while the existing evidence-record URI still dereferences payload bytes
- Guild now runs as a real MCP server over stdio, not just an internal façade with MCP-shaped concepts
- Guild now has one current `guild init` operator setup path for persistent local/Codex wiring, while `guild codex` remains the deterministic dogfood and smoke surface against the real stdio server
- Trusted repos can now load thin workflow-oriented Codex skills from `.agents/skills` for incident triage, policy-denial debugging, bundle verification checks, and execution-tree investigation without widening Guild's public MCP surface
- Guild now also has three inspect-only authority-debug example skills over durable execution records: `explain-capability-denial`, `diff-execution-authority`, and `explain-http-authority`
- Guild now has a real bounded `http-request` capability family in the active inspect slice
- Guild can now export and import the same signed installed bundles either as native bundle directories, local OCI image layouts, or OCI registry artifacts
- the draft schema bundle under `docs/schemas/draft-v1/` now has a real fail-closed M4 admission layer with `admission_request` and safe upper-bound `execution_plan` artifacts plus checked `admit` / `downgrade` / `migrate` / `refuse` examples, while remaining explicitly draft vocabulary and leaving execution plans unsigned by default unless they are later signed through the existing publisher/trust model
- MCP resource reads and guest-side `read-resource` calls use the same local backend
- bounded execution-query resources and templates now derive from that same local backend, so persisted executions can be discovered without already knowing an exact execution URI
- a resource-aware explain skill can read stored execution and evidence artifacts through the Wasm host boundary, including failed and rejected records
- a resource-aware query-summary skill can read bounded execution-query resources through the Wasm host boundary and return deterministic structured reports
- deterministic Codex-oriented MCP-path smoke flows now exercise `explain-execution`, `explain-execution-tree`, `recent-failure-triage`, and `policy-denial-debug` through the real stdio server
- top-level unsuccessful inspect calls return host-issued execution receipts pointing at persisted `guild://executions/...` records
- supported inspect-slice capability families now use typed host-enforced constraints
- the active inspect slice now has one centralized host-to-guest projection layer with contract tests
- the canonical primitive HTTP proof skill is `inspect-http-json`, exercised through `guild.inspect` against a deterministic local server
- unsupported capability families are rejected before execution in the active inspect slice
- broader Guild component imports in the active inspect runtime path now fail closed as host-owned `unsupported-runtime-surface` rejections instead of generic runtime load failures
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
- The remaining unsupported-import ambiguity in the active inspect slice is now closed: broader capability imports stay absent from the inspect ABI, broader Guild component imports are preflighted, and unsupported runtime surface is persisted distinctly from policy denial and operational runtime failure.
- Guild now also has a real stdio MCP server surface over that same runtime and storage path, with one honest public tool (`guild.inspect`) plus durable Guild resources.
- Guild is now straightforward to connect to Codex over that same stdio surface: `guild init` creates the default local root and prints the current Codex wiring, `guild init --global` or `guild init --project` write persistent Codex config explicitly, and `guild codex bootstrap` / `scenario` / `smoke` remain the deterministic repo-local dogfood path.
- Guild can now use persisted execution records to answer practical operator questions about authority: why one execution was denied or reduced, how two executions differed, and whether one candidate loopback/IP-literal HTTP request fits a stored grant without performing the request.
- The checked-in repo skills under `.agents/skills` now package those realistic Codex workflows as thin wrappers around the same shared scenario helpers and Guild MCP resources.
- The draft schema bundle now has its own real M3/M4 validation story: `compatibility_check.py` remains a hard-requirement precheck, `admission_engine.py` derives safe upper-bound plans for one invocation, and the checked validation path stays in the bundle-local Python scripts rather than in the Rust workspace test sweep.
- The repository now also has a real reusable execution-plan signing path through the same Ed25519 publisher identities and trusted-publisher records already used for signed bundles; M4 plan generation itself stays unsigned by default.

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
- Added new inspect-only authority-debug skills that read stored execution resources and explain denied/reduced capability state, compare two executions' granted authority, and dry-run stored HTTP authority without performing the request.
- Made resolved execution attempts durable on success, failure, and rejection with host-owned termination metadata.
- Added persisted execution receipts on top-level failure/rejection so callers can immediately address the stored execution URI.
- Replaced loose capability constraint handling with typed constraints plus one shared host-side evaluator.
- Added portable local bundle export/import built from installed executable records, including composite dependency closure export/import.
- Added OCI image layout export/import as an additional transport mapping for those same signed installed bundles, preserving the existing local trust/signature verification path by reconstructing the native signed bundle semantics before installation.
- Added OCI registry push/pull for that same OCI-mapped signed installed bundle transport, still reconstructing the native signed bundle semantics and re-running the local trust/signature verification path before installation.
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
- Added the `guild codex` workflow surface that bootstraps a fresh local Guild root for Codex dogfooding, installs the recommended example skills, prints the exact stdio launch/config snippets for the real `guild mcp serve --stdio` path, and runs deterministic smoke flows through the same shared helper path used by the checked-in examples.
- Added `guild codex scenario` with deterministic `recent-failure-triage`, `policy-denial-debug`, and `execution-tree` setup flows that return subject/query URIs plus one recommended Codex ask string for each scenario.
- Added a real host-mediated `http-request` capability family with typed host enforcement and a Wasmtime-backed outbound HTTP path behind the existing Guild guest ABI.
- Added the primitive `inspect-http-json` example skill plus a local deterministic HTTP proof flow and regression coverage for denial, timeout, response-size, and nested child-grant reduction behavior.
- Extended the local policy proof flow so it now contrasts trusted vs restricted imported HTTP authority under named profiles and uses `explain-execution` to summarize the persisted host-owned denial.
- Mapped successful inspect calls to MCP tool results with `structuredContent`, text compatibility output, and execution/evidence resource links.
- Mapped unsuccessful inspect executions to MCP tool errors with `isError: true` while preserving persisted execution receipt and record information.
- Added deterministic Codex-oriented MCP-path dogfood flows for `explain-execution`, `explain-execution-tree`, `recent-failure-triage`, and `policy-denial-debug`, plus startup/config regression coverage for the documented stdio workflow.
- Added repo-scoped Codex skills under `.agents/skills` that wrap the shared scenario helpers for incident triage, policy-denial debugging, bundle verification checks, and execution-tree investigation.
- Reworked `inspect_policy_local`, `explain_recent_failures_local`, and `explain_execution_tree_local` to reuse the same shared scenario prep helpers that now power the Codex-facing CLI flows.
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
- OCI image layouts are also built from installed executable state, not source directories.
- A signed bundle contains the installed manifest, staged Wasm artifact, staged support files, explicit digests for bundled files, a bundle index identifying the root skill and included installs, and a detached signature envelope.
- The OCI mapping stores that same signed bundle index as the root manifest config blob, stores the detached signature as a dedicated OCI layer, stores each bundled installed file as its own OCI blob layer, and identifies the root skill through descriptor annotations in `index.json`.
- OCI registry transport pushes and pulls that same OCI-mapped artifact through a registry reference; the pull path validates the OCI image index, root manifest, and referenced blob digests before reconstructing the signed bundle payload locally.
- Import verifies bundle structure, publisher trust, signature validity, and bundled digests before copying anything into the target registry.
- OCI import first verifies image layout structure plus blob descriptor size/digest integrity, then reconstructs the native signed bundle payload and runs the same publisher-trust, signature, and bundled-digest verification flow before copying anything into the target registry.
- OCI registry pull/import first verifies remote OCI structure plus pulled blob digests, then reconstructs the native signed bundle payload and runs the same publisher-trust, signature, and bundled-digest verification flow before copying anything into the target registry.
- Imported skills become normal installed records under the target registry's `installed/...` tree.
- Imported execution does not require the original source tree or a local rebuild.
- Imported verified installs carry host-owned verification metadata in registry-side sidecars.

### Execute

- The real runtime path is `WasmtimeRuntimeAdapter` using the active inspect world `guild-skill-inspect-v1` defined in `wit/guild-skill-v1.wit`.
- `ExecutionContext` carries explicit `CapabilityGrantSet` data into the guest together with a host-minted durable execution ID.
- The runner still executes only resolved refs and still globally rejects `apply`.
- The host now evaluates typed constraints for `read-resource`, `invoke-skill`, `emit-evidence`, and `log-write`.
- The runner now projects the richer durable host execution model into the inspect guest ABI explicitly through one centralized inspect projection layer before guest start.
- Inspect guest `ExecutionContext` is a bounded subset that intentionally omits `mode`, while the current active family grant shapes for `http-request`, `read-resource`, `invoke-skill`, `emit-evidence`, and `log-write` project fully.
- Unsupported capability families in the broader shared contract are rejected before execution in the active inspect slice, and unsupported imports are absent from the active inspect guest ABI itself.
- Broader Guild component imports in the active inspect path are rejected during runtime-load preflight as host-owned `unsupported-runtime-surface` outcomes instead of generic component-instantiation failures.
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
- Evidence metadata for those same emissions persists as first-class JSON resources at `guild://objects/records/.../metadata`.
- Parent execution records retain host-owned child execution metadata and child execution URIs.
- Failed and rejected execution records carry host-owned `termination` metadata and may omit `SkillOutput`.
- Top-level unsuccessful inspect calls still return errors, but those errors now carry a receipt URI for the persisted execution record.
- Evidence records retain per-emission metadata plus `produced_by_execution` linkage even when multiple executions emit the same payload digest.
- MCP can read execution resources, evidence-record payload URIs, evidence-record metadata URIs, and underlying payload blobs from the same local store.
- Guests can now read allowed Guild URIs through `read-resource` when granted typed `uri_prefixes` plus `resource_kinds`.
- `read-resource` authorization now parses Guild URIs and canonical scope roots like `guild://executions/`, `guild://objects/records/`, and `guild://objects/sha256/` before matching.
- Malformed or ambiguous Guild URIs fail closed instead of being normalized or accepted through permissive prefix logic.

### MCP server surface

- `guild-mcp-server` can be launched as a stdio MCP subprocess against a local Guild root.
- `guild init` provides the supported local Codex setup path by creating the local root and printing or writing the exact cwd-independent `codex mcp add ... -- <command>` and `config.toml` snippets for that same stdio server, while `guild codex` prepares deterministic scenario roots and runs helper-level smoke flows against already prepared roots.
- `guild codex bootstrap` installs the query and authority-debug example skills so the local dogfood root is ready for manual operator-style follow-up inspection without widening the public MCP surface.
- `guild codex scenario --scenario recent-failure-triage|policy-denial-debug|execution-tree --json` returns deterministic execution/query URIs plus one recommended Codex ask string for that workflow.
- Repo-scoped Codex helpers now live under `.agents/skills` and stay intentionally thin by wrapping `guild codex scenario --json` instead of adding new top-level Guild tools.
- The active public MCP tool surface is intentionally minimal: one tool, `guild.inspect`.
- `tools/list` publishes honest input and output schemas derived from the existing Guild-facing types.
- `tools/call` for `guild.inspect` executes through the same `GuildMcpFacade -> registry -> runner -> Wasmtime` path as the direct Rust façade.
- Successful MCP tool results include `structuredContent`, a text compatibility block, and resource links to the persisted execution record and emitted evidence records.
- Unsuccessful inspect executions that reached a real resolved execution attempt are surfaced as MCP tool errors with preserved persisted execution record identity instead of opaque protocol crashes.
- MCP `resources/read` exposes execution records, bounded execution-query results, evidence-record payloads, evidence-record metadata resources, and digest-addressed blobs through the same local resource backend Guild already used internally.
- MCP `resources/templates/list` now exposes canonical Guild URI templates for execution records, bounded execution-query resources, evidence-record payloads, evidence-record metadata, and raw blobs.
- MCP `resources/list` remains intentionally narrow and honest by listing only a bounded recent view of execution records.

### Example flows

Canonical local proof commands:

```bash
cargo run -p guild-mcp --bin guild -- codex bootstrap --registry-root target/dev-local-registry/codex-local --reset
cargo run -p guild-mcp --bin guild -- codex print-config --registry-root target/dev-local-registry/codex-local
cargo run -p guild-mcp --bin guild -- codex scenario --registry-root target/dev-local-registry/codex-local --scenario recent-failure-triage --json
cargo run -p guild-mcp --bin guild -- codex scenario --registry-root target/dev-local-registry/codex-local --scenario policy-denial-debug --json
cargo run -p guild-mcp --bin guild -- codex scenario --registry-root target/dev-local-registry/codex-local --scenario execution-tree --json
cargo run -p guild-mcp --bin guild -- codex smoke --registry-root target/dev-local-registry/codex-local --flow explain-execution
cargo run -p guild-mcp --bin guild -- codex smoke --registry-root target/dev-local-registry/codex-local --flow explain-execution-tree
cargo run -p guild-mcp --bin guild -- codex smoke --registry-root target/dev-local-registry/codex-local --flow recent-failure-triage
cargo run -p guild-mcp --bin guild -- codex smoke --registry-root target/dev-local-registry/codex-local --flow policy-denial-debug
cargo run -p guild-mcp --example inspect_local
cargo run -p guild-mcp --example inspect_http_json_local
cargo run -p guild-mcp --example inspect_policy_local
cargo run -p guild-mcp --example inspect_composite_local
cargo run -p guild-mcp --example explain_execution_local
cargo run -p guild-mcp --example explain_execution_tree_local
cargo run -p guild-mcp --example explain_failure_local
cargo run -p guild-mcp --example explain_recent_failures_local
cargo run -p guild-mcp --example codex_explain_execution_local
cargo run -p guild-mcp --example codex_explain_execution_tree_local
cargo run -p guild-mcp --example export_import_local
cargo run -p guild-mcp --example export_import_oci_local
cargo run -p guild-mcp --example export_import_composite_local
cargo run -p guild-mcp --example export_import_composite_oci_local
cargo run -p guild-mcp --example signed_import_failures_local
cargo run -p guild-mcp --example signed_import_oci_failures_local
cargo run -p guild-mcp --example push_pull_oci_registry_local
cargo run -p guild-mcp --example push_pull_composite_oci_registry_local
cargo run -p guild-mcp --example signed_pull_oci_registry_failures_local
cargo run -p guild-mcp --example mcp_stdio_local
```

What they prove:

- `inspect_local`: install `hello-inspect`, execute it, read back stored execution + evidence
- `inspect_http_json_local`: start a deterministic local HTTP server, install `inspect-http-json`, run one bounded allowed request through `guild.inspect`, then run one denied host-mismatch request and read back both persisted execution records
- `inspect_policy_local`: prepare the shared `policy-denial-debug` scenario in one local Guild root, show trusted and restricted imported execution resources, then run `explain-execution`, `explain-capability-denial`, `diff-execution-authority`, and `explain-http-authority` against the persisted execution URIs
- `inspect_composite_local`: install `hello-inspect`, install `hello-composite`, execute composite inspect, read back parent + child + child evidence
- `explain_execution_local`: install `hello-inspect`, produce a stored execution URI, install `explain-execution`, then run a resource-aware skill against that stored execution through the Wasm host boundary
- `explain_execution_tree_local`: prepare the shared `execution-tree` scenario, then run `explain-execution-tree` against the stored root execution URI through the same host-mediated resource path
- `explain_failure_local`: trigger a persisted rejected execution, capture its receipt URI, then run `explain-execution` against that stored unsuccessful record
- `explain_recent_failures_local`: prepare the shared `recent-failure-triage` scenario, read `guild://queries/executions/failures/recent/10` through the host resource backend, run `summarize-execution-query` against that same query URI through the Wasm host boundary, and then follow up with `explain-execution` for one stored failure
- `guild codex scenario --scenario recent-failure-triage --json`: prepare a deterministic recent-failures root and return the query URI plus stored subject execution URIs for Codex or local follow-up inspection
- `guild codex scenario --scenario policy-denial-debug --json`: prepare a deterministic imported-bundle trust/policy-denial root and return the denied receipt, trusted/restricted comparison receipts, and candidate HTTP URLs for follow-up inspection
- `guild codex scenario --scenario execution-tree --json`: prepare a deterministic composite lineage root and return the stored root execution URI for follow-up tree inspection
- `guild codex smoke --flow explain-execution`: reuse a prepared Codex dogfood root, launch the real stdio MCP server through the documented helper-generated command shape, then execute the `hello-inspect -> explain-execution` flow and read both execution resources back through `resources/read`
- `guild codex smoke --flow explain-execution-tree`: reuse that same prepared Codex dogfood root, launch the same real stdio MCP server path, then execute the `hello-composite -> explain-execution-tree` flow and read both execution resources back through `resources/read`
- `guild codex smoke --flow recent-failure-triage`: reuse a prepared Codex dogfood root, launch the real stdio MCP server through the documented helper-generated command shape, summarize the recent-failures query through MCP, and read the same stored query and execution resources back through `resources/read`
- `guild codex smoke --flow policy-denial-debug`: reuse a prepared Codex dogfood root, launch the same real stdio MCP server path, then execute the denial, authority-diff, and HTTP-authority explain flows through MCP and read the same stored execution resources back through `resources/read`
- `codex_explain_execution_local`: bootstrap the recommended Codex dogfood skill set, launch the real stdio MCP server using the same `cargo run` command the helper prints for Codex, then execute the `hello-inspect -> explain-execution` flow through MCP and read both execution resources back through `resources/read`
- `codex_explain_execution_tree_local`: bootstrap the recommended Codex dogfood skill set, launch the real stdio MCP server using that same documented stdio path, then execute the `hello-composite -> explain-execution-tree` flow through MCP and read both execution resources back through `resources/read`
- `export_import_local`: install `hello-inspect` into registry A, generate a local publisher identity, export a signed installed bundle, trust that publisher in fresh registry B, import, resolve by `RequestedSkillRef`, and execute without rebuilding
- `export_import_oci_local`: export the same installed signed bundle payload as an OCI image layout, trust/import it in fresh registry B, resolve by `RequestedSkillRef`, and execute without rebuilding
- `export_import_composite_local`: export `hello-composite` together with its installed dependency closure as a signed bundle, trust the publisher in fresh registry B, and execute the composite plus child entirely from imported installed records
- `export_import_composite_oci_local`: export that same installed dependency closure as an OCI image layout, trust/import it in fresh registry B, and execute the composite plus child entirely from imported installed records
- `signed_import_failures_local`: prove both untrusted-publisher rejection and tampered-bundle rejection before unsafe executable state is installed
- `signed_import_oci_failures_local`: prove the same untrusted/tampered fail-closed behavior for OCI image layout import before unsafe executable state is installed
- `push_pull_oci_registry_local`: publish the same installed signed bundle payload through a local OCI registry, trust/pull it in fresh registry B, resolve by `RequestedSkillRef`, and execute without rebuilding
- `push_pull_composite_oci_registry_local`: publish `hello-composite` together with its installed dependency closure through a local OCI registry, trust/pull it in fresh registry B, and execute the composite plus child entirely from pulled installed records
- `signed_pull_oci_registry_failures_local`: prove the same untrusted/tampered fail-closed behavior for OCI registry pull/import before unsafe executable state is installed
- `mcp_stdio_local`: launch `guild-mcp-server` as a subprocess, initialize over stdio JSON-RPC, list tools, call `guild.inspect`, and read back the returned execution/evidence URIs through MCP resources

Each command uses its own cleaned subdirectory under `target/dev-local-registry/`, so repeated local runs stay deterministic and do not overwrite another proof flow's stored execution ids.

## Gaps

Still intentionally missing or narrow:

- no `plan` execution path yet
- `apply` remains globally gated off
- no remote registry or publication flow
- no remote signatures, transparency logs, or trust/publication metadata beyond the local offline trust store
- no remote or distributed policy beyond the local host-owned evaluator
- no broad policy language beyond the current typed local `policy.json` profile model
- no MCP subscriptions, list-changed notifications, or HTTP transport
- no search, indexing, or query layer over stored executions/evidence
- no arbitrary filesystem or non-Guild URI reads from guests
- no guest-side write/update resource API beyond evidence emission
- no workflow/orchestration DSL

Current sharp edges worth remembering:

- capability hardening and policy remain intentionally narrow to the currently implemented capability families and the current local rule model, not a general enterprise policy language
- the local store is honest and useful, but still not a broader storage platform
- pre-resolution request/lookup failures are still not persisted in this milestone
- persistence failures themselves still surface as direct errors; Guild does not yet write provisional/in-progress records
- unsuccessful records now have a consistent host-owned rejection path for authorization denials, but still not a broader incident taxonomy or retry/orchestration system

## Next Steps

The clean next milestones after Codex dogfooding are:

1. Keep using the current stdio surface for real work
   - treat the new Codex workflow as the source of ergonomics truth before widening the platform
   - prefer shaving setup friction and clarifying receipts/resources over adding fresh substrate

2. Expand capability enforcement deliberately
   - add more typed families only when there is a real host operation behind them
   - keep nested grant reduction conservative and explicit

3. Build on portability
   - treat installed bundles as the local transport unit future publication can build on
   - keep import/export focused on installed executable state instead of source packages

4. Prepare for richer artifact reuse
   - build on the current execution/evidence resource model before adding any search or subscription surface
   - keep MCP and guest reads on the same backend
   - keep the public MCP tool surface small rather than drifting into one-tool-per-skill sprawl

5. Only then widen outward
   - richer local policy profiles and trust tiers
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
cargo run -p guild-mcp --bin guild -- codex bootstrap --registry-root target/dev-local-registry/codex-local --reset
cargo run -p guild-mcp --bin guild -- codex print-config --registry-root target/dev-local-registry/codex-local
cargo run -p guild-mcp --bin guild -- codex scenario --registry-root target/dev-local-registry/codex-local --scenario recent-failure-triage --json
cargo run -p guild-mcp --bin guild -- codex scenario --registry-root target/dev-local-registry/codex-local --scenario policy-denial-debug --json
cargo run -p guild-mcp --bin guild -- codex scenario --registry-root target/dev-local-registry/codex-local --scenario execution-tree --json
cargo run -p guild-mcp --bin guild -- codex smoke --registry-root target/dev-local-registry/codex-local --flow explain-execution
cargo run -p guild-mcp --bin guild -- codex smoke --registry-root target/dev-local-registry/codex-local --flow explain-execution-tree
cargo run -p guild-mcp --bin guild -- codex smoke --registry-root target/dev-local-registry/codex-local --flow recent-failure-triage
cargo run -p guild-mcp --bin guild -- codex smoke --registry-root target/dev-local-registry/codex-local --flow policy-denial-debug
cargo run -p guild-mcp --example inspect_local
cargo run -p guild-mcp --example inspect_http_json_local
cargo run -p guild-mcp --example inspect_policy_local
cargo run -p guild-mcp --example inspect_composite_local
cargo run -p guild-mcp --example explain_execution_local
cargo run -p guild-mcp --example explain_execution_tree_local
cargo run -p guild-mcp --example explain_failure_local
cargo run -p guild-mcp --example explain_recent_failures_local
cargo run -p guild-mcp --example codex_explain_execution_local
cargo run -p guild-mcp --example codex_explain_execution_tree_local
cargo run -p guild-mcp --example export_import_local
cargo run -p guild-mcp --example export_import_oci_local
cargo run -p guild-mcp --example export_import_composite_local
cargo run -p guild-mcp --example export_import_composite_oci_local
cargo run -p guild-mcp --example signed_import_failures_local
cargo run -p guild-mcp --example signed_import_oci_failures_local
cargo run -p guild-mcp --example push_pull_oci_registry_local
cargo run -p guild-mcp --example push_pull_composite_oci_registry_local
cargo run -p guild-mcp --example signed_pull_oci_registry_failures_local
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
- the documented Codex stdio config shape and helper-generated `codex mcp add` workflow
- deterministic MCP-path Codex dogfood flows for `explain-execution` and `explain-execution-tree`
- strict workspace lint verification via `cargo clippy --workspace --all-targets --all-features -- -W clippy::pedantic -W clippy::cargo -W clippy::future_not_send`
- MCP tool-error semantics preserving persisted execution receipts instead of collapsing them into raw protocol failures
- bounded recent execution resource listing, bounded execution-query resource reads, and canonical Guild URI resource templates
- resource-aware explain skill execution against stored successful, failed, and rejected artifacts
- resource-aware query-summary skill execution against bounded execution-query resources discovered through the same backend
- documented primitive and composite portability proof flows using separate registry A / bundle / registry B roots
- documented negative trust proof flow for untrusted and tampered signed bundles
- operator-facing root resolution now defaults to `--registry-root` > `GUILD_REGISTRY_ROOT` > `~/.guild`, with no cwd-local `.guild/` fallback and no `target/dev-local-registry/...` operator default
- read-only operator commands now open existing registry state without creating a fresh default root, while write-oriented commands can honestly initialize the selected root
- `guild init` is now the single current local bootstrap path for creating the selected Guild root and optionally folding in Codex setup writes, and the unreleased extra `guild-codex` binary has been removed so the repo presents one supported CLI path
- `guild init` is now the explicit persistent Codex integration workflow: it creates the selected Guild root, prints the real stdio launch command / `codex mcp add ...` registration / TOML snippet, and can idempotently update `~/.codex/config.toml` and `.codex/config.toml` against the running `guild` binary
- ADR 0019, README, command-language docs, testing docs, and AGENTS guidance now describe `guild` as the first-class Cargo-installable local operator tool with persistent `~/.guild` defaults, while deterministic proofs and examples still use explicit temp or `target/dev-local-registry/...` roots so CI and local verification never touch a developer home directory
