# Session Substrate Backlog

## 1. Freeze repo entrypoints on the session-substrate thesis

Intent: make the new direction visible from the first files contributors read.

Likely file touchpoints: `README.md`, `AGENTS.md`, `docs/project-positioning.md`, `docs/roadmap.md`, `docs/adr/README.md`

Acceptance notes: a fresh reader can find the new thesis and the strategy docs
from all major entrypoints without reading issue history.

## 2. Add ADR 0020 for the evolution to session substrate

Intent: record the old framing, new framing, and what remains stable.

Likely file touchpoints: `docs/adr/0020-*.md`, `docs/adr/README.md`

Acceptance notes: ADR clearly distinguishes shipped truth, new framing, and
deferred work.

## 3. Replace the current project-positioning drift guard with session-substrate checks

Intent: make the repo mechanically protect the new direction instead of the old
playbook/starter-set thesis.

Likely file touchpoints: `crates/guild-draft-truth/src/project_positioning.rs`, `xtask/src/main.rs`

Acceptance notes: `cargo run -q -p xtask -- project-positioning check` passes
and validates the new required docs/links/phrases.

## 4. Update doc regressions that pin the old thesis

Intent: keep tests aligned with the new direction while preserving current
runtime claims.

Likely file touchpoints: `crates/guild-mcp/tests/guild_cli.rs`

Acceptance notes: tests stop requiring the old “playbook is the application”
framing and instead require the new session/harness links and wording.

## 5. Add shared session lifecycle types

Intent: create a compile-safe vocabulary for planned session work.

Likely file touchpoints: `crates/guild-types/src/lib.rs`

Acceptance notes: add `SessionId`, `SessionState`, `SessionMaterializationMode`,
`ResumePolicy`, and `RehydratePolicy` with serde/jsonschema coverage and no
behavioral wiring.

## 6. Add runner trait seams for session coordination

Intent: create clear future boundaries without changing current execution flow.

Likely file touchpoints: `crates/guild-runner/src/lib.rs`, `crates/guild-runner/src/session.rs`

Acceptance notes: `AdmissionController` and `SessionBroker` traits compile,
have rustdoc, and are not yet wired into the active path.

## 7. Decide whether Harness gets a Rust type before a manifest contract

Intent: avoid implying a stable packaging boundary too early.

Likely file touchpoints: `docs/strategy/session-substrate/03-harness-abstraction.md`, `SPECS.md`, `ARCHITECTURE.md`

Acceptance notes: repo clearly records whether harness remains docs-first or
gains a small shared type next.

## 8. Define session identity minting and persistence ownership

Intent: determine what host-owned durable identifier a future session uses.

Likely file touchpoints: `crates/guild-types/src/lib.rs`, `ARCHITECTURE.md`, `SPECS.md`

Acceptance notes: one canonical minting/ownership rule is written down before
session runtime work starts.

## 9. Define session lifecycle transitions precisely

Intent: remove ambiguity between active, suspended, rehydration-required, and
terminated paths.

Likely file touchpoints: `docs/strategy/session-substrate/04-session-substrate.md`, `SPECS.md`, `ARCHITECTURE.md`

Acceptance notes: transitions and failure fallbacks are specific enough to
implement without guessing.

## 10. Define what survives resume vs rehydration vs cold start

Intent: separate durable host truth from rebuildable harness state.

Likely file touchpoints: `docs/strategy/session-substrate/04-session-substrate.md`, `ARCHITECTURE.md`

Acceptance notes: reconnecting services, invalid snapshots, and cold-start
fallback are all covered explicitly.

## 11. Define invoke-time versus wake-time admission checks

Intent: avoid smuggling policy assumptions into future session wake logic.

Likely file touchpoints: `docs/strategy/session-substrate/05-admission-controller.md`, `SPECS.md`

Acceptance notes: secrets, mounts, network policy, and runtime selection all
have a declared invoke/wake check boundary.

## 12. Decide how current PolicyDecision maps to future admission outputs

Intent: bridge existing allow/reduce/reject decisions to
allow/deny/ask-human/elevate-isolation.

Likely file touchpoints: `crates/guild-types/src/lib.rs`, `docs/strategy/session-substrate/05-admission-controller.md`

Acceptance notes: repo records whether new outcomes extend or wrap the current
policy model.

## 13. Define session-level receipt aggregation

Intent: clarify how the future receipt engine relates to existing execution and
evidence records.

Likely file touchpoints: `docs/strategy/session-substrate/06-receipt-engine.md`, `crates/guild-types/src/lib.rs`

Acceptance notes: session receipt vs execution-attempt receipt boundary is
explicit.

## 14. Decide whether receipt envelopes belong in guild-types now

Intent: add only data that has a real near-term home.

Likely file touchpoints: `crates/guild-types/src/lib.rs`, `docs/strategy/session-substrate/06-receipt-engine.md`

Acceptance notes: either add a minimal `ReceiptEnvelope` or record clearly why
it stays deferred.

## 15. Reframe project-positioning from canonical thesis to compatibility bridge

Intent: preserve stable links while moving the strategic center of gravity.

Likely file touchpoints: `docs/project-positioning.md`, `README.md`, `docs/contracts.md`, `docs/architecture.md`

Acceptance notes: old links still work, but new readers are routed to the new
strategy stack.

## 16. Add one repo-scoped agent direction skill

Intent: give future Codex sessions a short, reliable strategic orientation.

Likely file touchpoints: `.agents/skills/guild-direction/SKILL.md`, `AGENTS.md`

Acceptance notes: skill is short, points at the right files, and does not
duplicate the long-form docs.
