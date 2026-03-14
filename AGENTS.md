# AGENTS.md

This file is for coding agents and human contributors acting like coding agents on short sleep.

Guild is a **contracts-first** repository. Treat architecture, types, manifests, and execution boundaries as product surface, not internal implementation trivia.

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
   - Registry, policy, runner, and MCP façade live in Rust.
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
   - Prefer stable façade tools like `guild.search`, `guild.describe`, `guild.inspect`, `guild.plan`, `guild.apply`.
   - Do not expose every skill as a top-level MCP tool.

8. **Contract changes are multi-file changes.**
   If you change a contract, update all relevant surfaces:
   - `crates/guild-types`
   - `crates/guild-manifest`
   - `wit/guild-skill-v1.wit`
   - `docs/contracts.md`
   - example manifests if affected

## Repository map

- `crates/guild-types`: core shared structs and enums
- `crates/guild-manifest`: manifest model
- `crates/guild-runner`: runtime abstraction and execution boundary
- `crates/guild-registry`: publication, lookup, resolution model
- `crates/guild-mcp`: MCP-facing names and façade concepts
- `crates/guild-sdk-rust`: authoring trait for Rust skills
- `wit/`: platform ABI contract
- `docs/`: architecture, contracts, ADRs, roadmap
- `examples/`: example skills and sample manifests

## Change rules

### When adding a new host capability
You must:
- document the capability in `docs/contracts.md`
- add the type-level representation
- explain the security boundary
- describe how policy grants or denies it
- add or update an ADR if the capability changes platform shape

### When changing execution semantics
You must:
- update the request/result types
- update the WIT world if the ABI changes
- update examples
- call out compatibility impact in the relevant doc or ADR

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
- do not make "temporary" network or secret access permanent by inertia
- do not add a workflow DSL in the first phase
- do not bypass the contracts because a demo needs to work today

Demos rot fast. Platform mistakes rot slower and cost more.
