# AGENTS.md

Guild is a contracts-first repository. Treat architecture, types, manifests,
receipts, and execution boundaries as product surface.

## Current North Star

Guild is evolving into a trusted session substrate for isolated harness execution: the admission controller, session broker, and receipt engine that lets callers target durable sessions instead of ephemeral sandboxes.

## Pivot In One Paragraph

Today, Guild honestly ships a skill-first, inspect-first trust chain: explicit
capability review, bounded execution, durable execution records, durable
evidence records, and a thin CLI/MCP surface. The next evolution is not to
throw that away, but to build on it. Guild should preserve the useful packaging
and capability ideas while moving the user-facing abstraction from “portable
skill/runtime” to “durable session for isolated harness execution.” Sessions
become the product surface; sandbox lifecycle stays internal.

## Glossary

- `capability`: host-mediated authority granted to a harnessed session
- `harness`: the isolated execution abstraction Guild admits, brokers, and
  receipts
- `session`: the durable host-owned unit callers address above any runtime
  instance
- `admission controller`: the policy boundary that decides allow, deny,
  ask-human, or stricter isolation
- `session broker`: the host-owned component that resumes, rehydrates, or
  cold-starts a session
- `receipt`: the durable host-owned record of admission, execution, evidence,
  and outcome
- `rehydration`: rebuilding a valid session materialization from durable
  session state and artifacts
- `execution mode`: the session materialization outcome: `warm`, `resumed`,
  `rehydrated`, or `cold`

## What Guild Is Becoming

- A durable session substrate for isolated harness execution
- A host-owned policy and admission boundary for invoking or waking sessions
- A receipt and evidence system that explains what happened across attempts
- A place where harness identity, capability policy, and durable records stay
  explicit

## What Guild Is Not Trying To Be

- A generic agent operating system
- A broad workflow engine that hides trust boundaries
- A promise that arbitrary session resume or snapshot restore already exists
- A product that treats sandbox lifecycle as the user-facing primitive
- A giant MCP tool surface or a thin wrapper over other runtimes

## Read These First

- `README.md`
- `docs/strategy/session-substrate/00-umbrella-epic.md`
- `docs/strategy/session-substrate/07-roadmap.md`
- `docs/strategy/session-substrate/tasks.md`
- `SPECS.md`
- `ARCHITECTURE.md`
- `docs/adr/0020-evolve-guild-toward-a-trusted-session-substrate-for-isolated-harness-execution.md`
- `docs/adr/README.md`

Compatibility links still exist at `docs/project-positioning.md`,
`docs/contracts.md`, and `docs/architecture.md`.

## Repo Navigation

- `crates/guild-types`: shared data contracts and identifiers
- `crates/guild-manifest`: current skill manifest model
- `crates/guild-runner`: runtime boundary and future home of session/admission
  seams
- `crates/guild-registry`: installed state, persistence, and transport
- `crates/guild-mcp`: CLI, MCP server, and user-facing presentation
- `wit/`: current guest ABI surface
- `docs/strategy/session-substrate/`: strategic direction and milestone docs
- `docs/adr/`: accepted architecture decisions
- `.agents/skills/`: repo-scoped Codex helper skills
- `MEMORY.md`: durable repo state summary
- `WORKING_MEMORY.md`: timestamped short-horizon task log

## Build And Test

Primary verification sweep:

```bash
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo run -q -p xtask -- draft-v1 truth check
cargo run -q -p xtask -- project-positioning check
```

Focused proof suites:

```bash
cargo test -p guild-mcp --test guild_cli --test codex_workflow --test mcp_server_stdio
cargo test -p guild-runner --test live_proofs -- --nocapture
```

## Safe Architecture Change Rules

- Keep host truth and guest ABI truth distinct.
- Keep session lifecycle and sandbox lifecycle distinct.
- Preserve the small MCP surface.
- Do not widen runtime support by prose alone.
- Do not add fake lifecycle managers, fake snapshots, or speculative manifest
  contracts.
- If a change affects contracts or trust boundaries, update code, docs, and ADRs
  together.

## Current Milestone

`M1 Session Vocabulary Freeze`: codify the session-substrate direction across
repo entrypoints, ADRs, drift guards, and minimal shared scaffolding.

## Next Likely Tasks

- Retarget the project-positioning drift checker to the new direction
- Add shared session lifecycle types in `guild-types`
- Add interface-only `AdmissionController` and `SessionBroker` traits in
  `guild-runner`
- Decide how Harness relates to current skill manifests and transport identity
- Define what survives resume vs rehydration vs cold start

## Open Questions / Unresolved Bets

- What stable identifier should a durable session use?
- How should Harness relate to the current skill packaging model?
- Which wake-time checks must rerun for secrets, mounts, and network policy?
- Should receipts aggregate at the session layer, execution-attempt layer, or
  both?
- What should Guild persist as durable session state versus rebuildable harness
  state?
