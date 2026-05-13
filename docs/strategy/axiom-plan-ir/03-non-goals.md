# Axiom Plan IR Non-Goals

Status: exploratory guardrails.

Axiom Plan IR is intentionally narrow. It is a planning and review language
above Guild, not a runtime below Guild.

## Non-Goals

- Axiom is not a new runtime.
- Axiom is not a Wasm executor.
- Axiom does not replace Guild manifests.
- Axiom does not define skill ABI.
- Axiom does not own capability enforcement below the composition layer.
- Axiom does not widen active Guild capability families.
- Axiom does not make sessions or harnesses normative before Guild does.
- Axiom does not replace `guild run`, `guild why`, `guild get`, or `guild verify`.
- Axiom is not a generic workflow engine.

## Composite-Skill Guardrails

- Axiom does not replace Guild composite skill semantics.
- Axiom must not create an alternate child-invocation path.
- If an Axiom plan lowers to child skill calls, those calls still go through Guild's host-mediated invocation path.

## Practical Boundary

If Axiom cannot stay above Guild as a docs, validator, and lowerer layer, the
idea should stop. The useful boundary is a reviewable pre-admission plan that
makes AI intent clearer without changing who owns authority or truth.
