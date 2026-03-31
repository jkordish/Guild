# Session Substrate Roadmap

## Milestone Sequence

1. `M1 Session Vocabulary Freeze`
   Docs, ADR, README, AGENTS, machine-readable context, and drift guards align
   on session/harness/admission/receipt language.
2. `M2 Shared Contract Scaffolding`
   Add minimal session lifecycle types and runner trait seams without changing
   runtime behavior.
3. `M3 Harness Contract Design`
   Decide how harness identity, packaging, and runtime requirements relate to
   current skill manifests and resolved skill identity.
4. `M4 Session Persistence Model`
   Define durable session ID, persistence tiers, and wake/resume/rehydrate
   rules.
5. `M5 Session-Aware Admission`
   Extend admission from single execution requests to session invoke/wake
   decisions.
6. `M6 Session Receipt Aggregation`
   Add session-level receipt views while preserving existing execution-attempt
   records.

## Sequencing And Dependencies

- `M1` must land before deeper architectural work so future contributors stop
  optimizing for the old product framing by default.
- `M2` depends on `M1`, because the shared type names should match the frozen
  vocabulary.
- `M3` depends on `M2`, because harness contract work should reuse the same
  session and admission vocabulary rather than inventing another layer.
- `M4` depends on `M3`, because persistence and rehydration semantics require a
  stable understanding of what a harness is.
- `M5` depends on `M4`, because wake-time policy cannot be designed honestly
  without session lifecycle and persistence tiers.
- `M6` depends on `M5`, because session-level receipts need concrete admission
  and materialization outcomes to record.

## Status Snapshot

- `M1` through `M6` are now frozen in repo truth.
- Shared session vocabulary, contract scaffolding, lifecycle rules,
  admission-bridge vocabulary, and the session-receipt boundary are all landed.
- No post-`M6` milestone is accepted yet; the next step is to choose one
  bounded follow-on slice that stays honest about the still skill-first live
  runtime.

## Immediate Focus

The `M1` through `M6` design freeze is complete. The next planning pass should
choose the first post-`M6` follow-on slice that turns the remaining design
questions into bounded work without widening runtime claims by prose alone.

Likely candidates:

- specify the minimum `SessionBroker` persistence and re-proof contract needed
  for a real wake path
- define explicit recovery and reset semantics for `failed` and terminal
  session lineages
- decide when a session-layer receipt becomes concrete enough for a shared
  persisted type
