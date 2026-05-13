# Run Plan Preview

Status: exploratory, xtask-only, preview-only, not execution.

This slice adds a non-executing Axiom Plan IR preview command:

```bash
cargo run -q -p xtask -- axiom-plan preview docs/strategy/axiom-plan-ir/examples/valid/basic-two-node-plan.json
```

JSON output is available with:

```bash
cargo run -q -p xtask -- axiom-plan preview docs/strategy/axiom-plan-ir/examples/valid/basic-two-node-plan.json --json
```

## Behavior

The preview command validates the Axiom plan exactly like
`axiom-plan validate` before rendering anything. Invalid plans print the same
diagnostic shape and exit nonzero. Valid plans render a pre-admission request
preview and plan trace only.

Preview rendering orders dependencies before dependents while preserving
document order for independent nodes. For each node, it shows:

- requested skill ref
- dependencies
- args as declared; references are not evaluated by preview
- requested grants as requested authority only
- expected outputs as planner expectations
- expected evidence as expectation only
- declared failure behavior
- `requestPreview`
- `previewTrace`

The JSON form uses top-level `kind: "axiom.plan_preview"` and
`status: "preview_only"`. It includes `plan`, `nodes`, `planTrace`, and
`limitations`.

## Boundary

This preview is not Guild admission. It is not Guild runtime truth. It does
not resolve skills, grant authority, execute nodes, create receipts, persist
evidence, call the registry, call the runner, use WIT, or read manifests. It
does not add a crate or change any runtime contract.

The preview intentionally uses boundary wording such as `would request`,
`would propose`, `pre-admission`, `not admitted`, `not granted`, and
`not executed`.

Requested grants are requested authority only. Expected evidence is
expectation only. Args are args as declared.

Guild remains canonical for skill resolution, admission, grant narrowing,
execution identity, receipts, evidence persistence, and inspect/explain truth.

## Skill Form

The current schema-admitted skill form is the string form:

```json
"skill": "skill://example/render-report@^0.1"
```

The preview renderer is defensive around object-form skill metadata if such a
value reaches preview internals during tests or future experiments. Object
form is not currently schema-admitted. If a defensive object-form render shows
a `resolved` value, it is labeled as plan-supplied resolved metadata,
pre-admission, not Guild resolution, and not verified by preview.

## Relationship

The validator answers whether an Axiom Plan IR document is locally well-shaped
enough to review. The preview answers what Guild request intent the plan would
propose before admission. Neither surface lowers into execution.

This keeps Axiom above Guild:

```text
Axiom Plan IR -> validator -> preview -> human review -> ordinary Guild path later
```

Any future executing path must still use ordinary Guild execution and produce
ordinary Guild durable records.

## Promotion Criteria

Promote this slice only if:

- the preview helps reviewers catch meaningful pre-admission plan mistakes
- the command stays xtask-only or moves behind an equally explicit
  non-executing review surface
- request preview wording remains clearly separate from Guild admission,
  granted authority, execution, receipts, and evidence persistence
- any lowerer still produces a non-executing review artifact before ordinary
  Guild execution

## Kill Criteria

Stop or remove the preview if:

- it starts calling Guild runtime, registry, admission, WIT, manifest, receipt,
  or evidence paths
- it claims skills were resolved, admitted, granted, executed, receipted, or
  evidenced
- it turns requested authority into effective authority
- it evaluates args references or treats expected evidence as produced
  evidence
- it widens Guild runtime support or session-substrate claims by prose alone
