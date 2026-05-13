# Axiom Plan IR Examples

Status: docs fixtures only.

These examples exercise the exploratory JSON Schema and docs-local validator in
[`../schema/axiom-plan-ir.schema.json`](../schema/axiom-plan-ir.schema.json).
They are not runtime fixtures, are not consumed by Guild, and do not prove that
Axiom Plan IR is implemented.

The valid examples show proposed pre-admission plan shapes. The invalid
examples show boundary cases for JSON Schema and semantic validator checks.

## Valid Fixtures

- [`valid/basic-two-node-plan.json`](valid/basic-two-node-plan.json) - two
  zero-requested-grant nodes where the second depends on the first.
- [`valid/with-requested-grants.json`](valid/with-requested-grants.json) - one
  node requesting `read-resource` over canonical Guild execution and execution
  query roots. The grant is requested, not granted.
- [`valid/with-expected-evidence.json`](valid/with-expected-evidence.json) -
  one node declaring expected evidence. The declaration does not claim that any
  evidence record exists.
- [`valid/with-references.json`](valid/with-references.json) - shallow
  `$input.*` and dependency-accessible `$<nodeId>.*` references.

## Invalid Fixtures

- [`invalid/duplicate-node-id.json`](invalid/duplicate-node-id.json) -
  duplicate node ID semantic failure.
- [`invalid/unknown-dependency.json`](invalid/unknown-dependency.json) -
  dependency reference to a missing node.
- [`invalid/malformed-skill-ref.json`](invalid/malformed-skill-ref.json) -
  malformed exploratory skill ref.
- [`invalid/granted-authority-claim.json`](invalid/granted-authority-claim.json)
  - forbidden runtime-authority claim outside Axiom-owned vocabulary.
- [`invalid/cycle.json`](invalid/cycle.json) - dependency cycle semantic
  failure.
- [`invalid/bad-reference.json`](invalid/bad-reference.json) - unknown and
  unsupported shallow reference forms.
- [`invalid/missing-required-field.json`](invalid/missing-required-field.json)
  - schema failure for a node missing required `skill`.
