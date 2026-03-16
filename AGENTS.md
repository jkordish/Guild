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
- supported capability families now use typed constraints enforced by one shared host-side evaluator
- only the active inspect-slice capability families `read-resource`, `invoke-skill`, `emit-evidence`, and `log-write` are actually executable; unsupported families fail before execution
- resolved execution attempts persist under local Guild URIs on success, failure, and rejection with host-minted durable IDs and host-stamped timestamps
- evidence emitted through the Wasm boundary persists as content-addressed blobs plus host-issued per-emission evidence records
- `read-resource` authorization uses canonical parsed Guild URI scopes rather than loose raw string prefix checks
- `guild.inspect` in `guild-mcp` rides that same path
- `guild-mcp-server` now exposes that same inspect/runtime/resource model over real stdio MCP
- installed skills can be exported as signed portable bundles, verified against a local trust store, and imported into fresh Guild roots without rebuilding

Preferred local proof commands:

```bash
cargo run -p guild-mcp --example inspect_local
cargo run -p guild-mcp --example inspect_composite_local
cargo run -p guild-mcp --example explain_execution_local
cargo run -p guild-mcp --example explain_failure_local
cargo run -p guild-mcp --example export_import_local
cargo run -p guild-mcp --example export_import_composite_local
cargo run -p guild-mcp --example signed_import_failures_local
cargo run -p guild-mcp --example mcp_stdio_local
```

Those commands are the canonical local install workflows: they build the example source skills, install them into command-specific cleaned subdirectories under `target/dev-local-registry/`, resolve them, and execute them. The source manifests no longer require manual artifact digest updates.
They also prove the storage layer by reading back persisted execution and evidence resources, `explain_execution_local` proves that a Wasm guest can consume those same Guild URIs through a host-mediated `read-resource` capability, and `explain_failure_local` proves that unsuccessful resolved executions now persist durable host-owned records that can be explained after the fact.
`export_import_local` and `export_import_composite_local` now prove signed bundle portability with an explicit local publisher identity plus local trust-store import verification, while `signed_import_failures_local` proves that untrusted or tampered bundles fail closed before installation.
The current working capability families are `read-resource`, `invoke-skill`, `emit-evidence`, and `log-write`, all with typed constraints rather than ad hoc JSON matching. Caller request IDs are correlation only, not durable execution IDs, and `EvidenceRef` values now identify evidence-record URIs rather than raw blob digests.

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
