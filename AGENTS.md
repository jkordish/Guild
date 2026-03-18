# AGENTS.md

This file is for coding agents and human contributors acting like coding agents on short sleep.

Guild is a **contracts-first** repository. Treat architecture, types, manifests, and execution boundaries as product surface, not internal implementation trivia.

## Canonical docs

Read these first and treat them as the current human-facing source of truth:

- `README.md`: repo entrypoint and current proof path
- `SPECS.md`: normative contract and conformance language
- `ARCHITECTURE.md`: practical system view and trust boundaries
- `docs/adr/README.md`: decision log and ADR backlog
- `docs/roadmap.md`: phase ordering

The files at `docs/contracts.md` and `docs/architecture.md` are compatibility wrappers kept for stable links.

## Tracking files

- `MEMORY.md` is the durable repo state / final tracking / conclusions file worth carrying forward.
- `WORKING_MEMORY.md` is the timestamped short-term task log and is safe to prune or rewrite later.
- When work is in progress, create timestamped entries in `WORKING_MEMORY.md` as you go so short-horizon context does not get lost between edits.

## Research workflow

- Prefer loaded MCP servers before web search whenever they can answer the question.
- Use local repo docs and code first for Guild-specific truth, then use MCP servers for external library, platform, or ecosystem context.
- Use Context7 first for library/framework/package documentation and API usage patterns.
- Use the relevant domain MCP next when available, for example OpenAI docs MCP for OpenAI product questions or GitHub MCP for repository and PR context.
- Web search is allowed only when the loaded MCP servers do not have what you need, or when you need extra clarification or current external context after checking them.
- Do not jump straight to browser search when Context7 or another loaded MCP server can answer the question well enough.

## What this repo is optimizing for

- portable skills
- capability-scoped execution
- stable contracts
- small MCP surface
- auditable results
- boring, trustworthy infrastructure

## Hard invariants

These are not suggestions.

1. **The Rust core is the platform boundary.**
   - Registry, policy, runner, and MCP facade live in Rust.
   - Keep unsafe behavior and broad host access out of the default path.

2. **WASM is the preferred execution format.**
   - External process or container runtimes may exist later.
   - Do not make them the architectural center.

3. **Skills never receive ambient authority.**
   - No raw filesystem access.
   - No raw environment access.
   - No unrestricted outbound network.
   - No subprocess spawning as a casual escape hatch.
   - New capabilities must be host-mediated and policy-gated.

4. **Execution resolves to immutable digests.**
   - Humans may ask for semver.
   - The system executes concrete artifacts.

5. **Inspect, plan, and apply are distinct.**
   - `inspect`: gather and explain
   - `plan`: compute intended effects without mutation
   - `apply`: mutation, only once audit and idempotency exist
   - Never smuggle mutation into inspect or plan.

6. **Evidence is part of the contract.**
   - Skills return structured output plus evidence and diagnostics.
   - Do not optimize for paragraph-only responses.

7. **The MCP surface stays small.**
   - Prefer stable facade tools like `guild.search`, `guild.describe`, `guild.inspect`, `guild.plan`, `guild.apply`.
   - Do not expose every skill as a top-level MCP tool.

8. **Contract changes are multi-file changes.**
   If you change a contract, update all relevant surfaces:
   - `crates/guild-types`
   - `crates/guild-manifest`
   - `wit/guild-skill-v1.wit`
   - `SPECS.md`
   - `ARCHITECTURE.md` when execution shape or trust boundaries change
   - example manifests if affected

9. **Host truth and guest ABI truth are not the same thing.**
   - Durable records stay canonical for policy, provenance, requested-vs-granted state, and evidence metadata.
   - The active inspect guest ABI only receives the named host-owned projection.
   - If you change inspect-visible data shapes, update the centralized projection layer, its tests, and the docs together.

## Repository map

- `SPECS.md`: normative repository contract
- `ARCHITECTURE.md`: practical system view
- `crates/guild-types`: core shared structs and enums
- `crates/guild-manifest`: manifest model
- `crates/guild-runner`: runtime abstraction and execution boundary
- `crates/guild-registry`: publication, lookup, resolution model
- `crates/guild-mcp`: MCP-facing names, stdio server, and facade concepts
- `crates/guild-sdk-rust`: authoring trait for Rust skills
- `wit/`: platform ABI contract
- `docs/adr/`: accepted and proposed architectural decisions
- `docs/roadmap.md`: phased build priorities
- `examples/`: example skills and sample manifests

## Current working baseline

The repository now has a real local inspect-only path:

- source manifests install into digest-pinned executable records under the local registry root
- source installs stage and validate before an atomic move into installed state
- `RequestedSkillRef` resolves through the local file-backed registry
- requested same-version multi-digest resolution now fails closed as ambiguous
- installed manifests and staged artifact digests are validated before execution
- the runner builds `ExecutionContext` with explicit grants
- primitive and composite example skills execute through the Wasmtime-backed Wasm runtime adapter
- composite skills invoke declared child dependencies by alias through the host boundary
- caller-requested capabilities now flow through a local host-owned policy evaluator before they become granted capabilities
- supported capability families now use typed constraints enforced by one shared host-side evaluator
- the active inspect-slice capability families `http-request`, `read-resource`, `invoke-skill`, `emit-evidence`, and `log-write` are actually executable; unsupported families fail before execution
- the shared host-side capability vocabulary now also includes an explicit typed deferred `filesystem` family, and the active inspect slice rejects filesystem before guest start rather than implying guest file IO exists
- inspect-mode Wasm skills now target the dedicated `guild-skill-inspect-v1` world; broader future imports must stay out of inspect manifests and the active inspect runtime path
- the active inspect host-to-guest projection now lives in one explicit runner layer; inspect guest `ExecutionContext` is a bounded subset, while the current five active family grant shapes project fully
- resolved execution attempts persist under local Guild URIs on success, failure, and rejection with host-minted durable IDs and host-stamped timestamps
- evidence emitted through the Wasm boundary persists as content-addressed blobs plus host-issued per-emission evidence records
- `read-resource` authorization uses canonical parsed Guild URI scopes rather than loose raw string prefix checks
- bounded execution-query resources and templates now derive from the same persisted execution backend seen by guest `read-resource` and MCP `resources/read`
- `guild.inspect` in `guild-mcp` rides that same path
- `guild-mcp-server` now exposes that same inspect/runtime/resource model over real stdio MCP
- `guild-codex` now bootstraps a local Codex dogfood root, installs the recommended example skills, and prints the exact stdio Codex MCP config for the real `guild-mcp-server`
- a resource-aware `explain-execution-tree` skill can walk stored parent/child execution lineage with bounded traversal and optional evidence descriptors through the same host-mediated path
- a resource-aware `summarize-execution-query` skill can consume bounded execution-query resources and return deterministic structured summaries through the same host-mediated path
- deterministic Codex-oriented MCP-path smoke flows now exercise `explain-execution` and `explain-execution-tree` through the real stdio server without widening the public MCP surface
- installed skills can be exported as signed portable bundles, verified against a local trust store, and imported into fresh Guild roots without rebuilding

Preferred local proof commands:

```bash
cargo run -p guild-mcp --bin guild-codex -- bootstrap --registry-root target/dev-local-registry/codex-local --reset
cargo run -p guild-mcp --bin guild-codex -- print-config --registry-root target/dev-local-registry/codex-local
cargo run -p guild-mcp --example inspect_local
cargo run -p guild-mcp --example inspect_http_json_local
cargo run -p guild-mcp --example inspect_policy_local
cargo run -p guild-mcp --example filesystem_rejection_local
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

Those commands are the canonical local install workflows: they build the example source skills, install them into command-specific cleaned subdirectories under `target/dev-local-registry/`, resolve them, and execute them. The source manifests no longer require manual artifact digest updates.
They also prove the storage layer by reading back persisted execution and evidence resources, `explain_execution_local` proves that a Wasm guest can consume those same Guild URIs through a host-mediated `read-resource` capability, `explain_execution_tree_local` proves that a resource-aware inspect skill can walk a persisted parent/child execution tree deterministically, and `explain_failure_local` proves that unsuccessful resolved executions now persist durable host-owned records that can be explained after the fact.
`explain_recent_failures_local` proves that bounded execution-query resources can discover persisted failed and rejected executions without already knowing an exact execution URI, and that a Wasm guest can consume those same query results through a scoped `read-resource` grant.
`guild-codex -- bootstrap` is the supported local setup path for Codex dogfooding: it creates a fresh local Guild root, installs the example skills used by the recommended Codex flows, and prints both the `codex mcp add ... -- <command>` registration command and the matching `.codex/config.toml` snippet for the real stdio server.
`codex_explain_execution_local` and `codex_explain_execution_tree_local` prove those same explain/debug flows over the real stdio MCP server with a deterministic local MCP client harness, which is the honest CI-safe stand-in for a full authenticated Codex session.
`export_import_local`, `export_import_oci_local`, `export_import_composite_local`, `export_import_composite_oci_local`, `push_pull_oci_registry_local`, and `push_pull_composite_oci_registry_local` now prove signed-bundle portability across native, OCI layout, and OCI registry transport with explicit local trust verification, while `signed_import_failures_local`, `signed_import_oci_failures_local`, and `signed_pull_oci_registry_failures_local` prove that untrusted or tampered imports fail closed before installation.
`inspect_http_json_local` proves the bounded `http-request` host capability, `inspect_policy_local` proves that a local `policy.json` can reduce or deny caller-requested capabilities before guest execution, and `filesystem_rejection_local` proves that the explicit host-side filesystem contract still fails closed before guest start in the active inspect slice. The current working executable capability families are `http-request`, `read-resource`, `invoke-skill`, `emit-evidence`, and `log-write`, all with typed constraints rather than ad hoc JSON matching. Caller request IDs are correlation only, not durable execution IDs, and `EvidenceRef` values now identify evidence-record URIs rather than raw blob digests.

## Change rules

### When adding a new host capability
You must:
- document the capability in `SPECS.md`
- add the type-level representation
- explain the security boundary
- describe how policy grants or denies it
- add or update an ADR if the capability changes platform shape

### When changing execution semantics
You must:
- update the request and result types
- update the WIT world if the ABI changes
- update examples
- call out compatibility impact in `SPECS.md` and the relevant ADR

### When changing installed portability or bundle flow
You must:
- build transport units from installed executable state, not source directories
- preserve digest pinning and dependency alias snapshots
- keep imported execution source-independent
- verify signature, trust, and bundled digests before installation
- keep verification metadata host-owned
- update the local proof examples, `SPECS.md`, and `ARCHITECTURE.md` together

### When touching apply mode
Default posture: do less.
- require idempotency thinking
- require audit thinking
- require approval thinking
- assume failure and retries are normal, because they are

## Preferred implementation style

- Keep crates small and explicit.
- Prefer narrow traits over giant god objects.
- Prefer data-first contracts over magical builders.
- Prefer immutable identifiers and explicit provenance.
- Prefer typed wrappers and enums over stringly glue.
- Keep optionality honest. Avoid "future proof" blobs that destroy meaning.

## Review checklist

Before considering a change ready, verify:

- Does it preserve the trust model?
- Does it keep the ABI smaller or clearer?
- Does it reduce, not increase, ambient authority?
- Does it make evidence and provenance easier, not harder?
- Does it avoid turning MCP into a tool explosion?
- Does it update docs and examples alongside code?

## Do not do these things

- do not add raw shell execution because it feels convenient
- do not add runtime-resolution of floating versions
- do not stuff policy into stringly JSON when a type belongs in Rust
- do not make temporary network or secret access permanent by inertia
- do not add a workflow DSL in the first phase
- do not bypass the contracts because a demo needs to work today

Demos rot fast. Platform mistakes rot slower and cost more.
