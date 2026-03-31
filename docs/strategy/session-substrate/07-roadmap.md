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

## Immediate Focus

The next implementable focus after this pass is `M2 Shared Contract
Scaffolding`, followed by the design-heavy `M3 Harness Contract Design`.
