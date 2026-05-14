# Preview Goldens

Status: docs-local, xtask-only, pre-admission output-stability hardening.

This slice adds checked-in goldens for Axiom Plan IR preview output:

```bash
cargo run -q -p xtask -- axiom-plan check-goldens
```

The command stays above Guild. It validates the same docs-local examples as
inputs, renders preview or diagnostic output, and compares that output with
files under [`goldens/`](goldens/). It does not execute Axiom plans and does
not call Guild runtime, admission, registry resolution, authority grants,
receipts, evidence, WIT, or manifests.

## Coverage

The checked goldens cover:

- human preview output for the basic two-node plan
- JSON preview output for the basic two-node plan
- human preview output for a requested-grants plan
- JSON preview output for a requested-grants plan
- diagnostic JSON output for a malformed skill ref
- diagnostic JSON output for a forbidden granted-authority claim

The preview goldens intentionally pin the boundary wording that keeps Axiom a
pre-admission planning artifact: `would request`, `would propose`,
`pre-admission`, `not admitted`, `not granted`, `not executed`,
`no Guild resolution`, `no receipt creation`, and `no evidence persistence`.

The diagnostic goldens pin stable semantic diagnostic codes for cases where
schema output alone is too broad to review confidently, including
`axiom.malformed_skill_ref` and `axiom.forbidden_runtime_truth_field`.

## Non-Coverage

This slice does not cover:

- Axiom execution
- Guild skill availability checks
- Guild registry resolution
- Guild admission or policy reduction
- granted or effective authority
- receipt creation
- evidence persistence
- WIT, manifests, runtime contracts, or session-substrate behavior
- full workspace verification wiring

## Commands

Check the committed goldens:

```bash
cargo run -q -p xtask -- axiom-plan check-goldens
make axiom-plan-goldens
```

Refresh the committed golden files after an intentional preview wording or
diagnostic-shape change:

```bash
cargo run -q -p xtask -- axiom-plan check-goldens --update
```

## Update Rules

`--update` may rewrite only files under
`docs/strategy/axiom-plan-ir/goldens/`. It must not rewrite examples, schemas,
prose docs, or unrelated fixtures.

Golden changes should be reviewed as product-surface changes. If a golden
changes because the preview boundary or diagnostic code changed, update the
matching tests and docs in the same change. Do not use `--update` to bless
accidental drift.

The JSON preview goldens use `serde_json::to_string_pretty(...)` with a final
newline. Diagnostic goldens normalize `sourcePath` to repo-relative paths and
sort diagnostics by path, code, severity, and message. Non-ASCII characters in
diagnostic messages are rendered as ASCII escape sequences to keep the
checked-in files stable across terminals and diff tools.

## Promotion Criteria

Promote this surface only if:

- the goldens catch meaningful preview or diagnostic drift that semantic tests
  alone would miss
- the preview remains clearly pre-admission and non-executing
- requested grants remain requested authority only
- diagnostics keep stable codes that reviewers can rely on
- the command remains local to Axiom docs and xtask hardening

## Kill Criteria

Stop or remove this surface if:

- the goldens start encouraging large snapshot churn
- preview output claims Guild resolution, admission, authority grants,
  execution, receipts, or evidence persistence
- the command calls Guild runtime, registry, admission, WIT, manifest, receipt,
  or evidence paths
- update mode can rewrite files outside `goldens/`
- maintaining exact output makes the boundary less clear than focused semantic
  tests
