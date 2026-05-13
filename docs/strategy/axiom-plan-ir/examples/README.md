# Axiom Plan IR Examples

Status: docs fixtures only.

These examples exercise the exploratory JSON Schema in
[`../schema/axiom-plan-ir.schema.json`](../schema/axiom-plan-ir.schema.json).
They are not runtime fixtures, are not consumed by Guild, and do not prove that
Axiom Plan IR is implemented.

The valid examples show proposed pre-admission plan shapes. The invalid
examples show boundary cases for either the JSON Schema or a future semantic
validator.

## Valid Fixtures

- [`valid/basic-two-node-plan.json`](valid/basic-two-node-plan.json) - two
  zero-requested-grant nodes where the second depends on the first.
- [`valid/with-requested-grants.json`](valid/with-requested-grants.json) - one
  node requesting `read-resource` over canonical Guild execution and execution
  query roots. The grant is requested, not granted.
- [`valid/with-expected-evidence.json`](valid/with-expected-evidence.json) -
  one node declaring expected evidence. The declaration does not claim that any
  evidence record exists.

## Invalid Fixtures

- [`invalid/duplicate-node-id.json`](invalid/duplicate-node-id.json) - future
  semantic-validator failure. JSON Schema does not prove node ID uniqueness
  across array items.
- [`invalid/unknown-dependency.json`](invalid/unknown-dependency.json) - future
  semantic-validator failure. JSON Schema does not resolve dependency
  references.
- [`invalid/malformed-skill-ref.json`](invalid/malformed-skill-ref.json) -
  JSON Schema failure because the `skill` value does not match the exploratory
  skill-ref regex.
- [`invalid/granted-authority-claim.json`](invalid/granted-authority-claim.json)
  - JSON Schema failure because the node includes a forbidden runtime-authority
  claim outside Axiom-owned vocabulary.
