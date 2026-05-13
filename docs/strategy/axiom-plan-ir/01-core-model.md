# Axiom Plan IR Core Model

Status: proposed, non-normative, and not implemented.

The concepts below describe an exploratory planning shape only. They do not
change Guild manifests, Guild runtime contracts, guest ABI, admission policy,
receipt storage, evidence storage, or CLI/MCP behavior.

For the docs-local schema spike and fixture examples, see
[`05-schema-and-examples.md`](05-schema-and-examples.md).

## Proposed Concepts

| Concept | Meaning |
| --- | --- |
| Program | A complete `axiom.plan` document describing one requested skill composition. |
| Node | One planned skill invocation in the composition graph. |
| Skill reference | A requested `skill://` ref for the skill the node wants Guild to resolve. |
| Arguments | JSON input the node proposes to pass as the Guild skill input JSON. |
| Dependency edges | Directed graph edges that say one node depends on another node's completion, output, or evidence ref. |
| Requested grants | Capability grants the planner asks Guild to consider during admission. |
| Expected outputs | Names, shapes, or refs the planner expects the node to produce. |
| Expected evidence | Evidence refs or evidence classes the planner expects the node to emit or make inspectable. |
| Failure behavior | Advisory behavior such as stop, continue, or skip dependents if a node fails. |
| Policy precheck | A non-authoritative check that the plan appears to request known Guild families and canonical roots. |
| Plan explanation | Human-readable rationale for why the graph exists and what it is expected to accomplish. |

Requested grants are policy input only. They are not granted authority. Guild
may deny the request, reduce it, ask for human review in a future host-owned
surface, or route it through stricter isolation if that exists later. The plan
cannot grant itself access.

## Example

This `axiom.plan` shape follows the docs-local version-1 schema shape, but it is
still only illustrative and is not accepted by the current Guild runtime. The
schema spike narrows the fixture shape separately in
[`05-schema-and-examples.md`](05-schema-and-examples.md).

```json
{
  "kind": "axiom.plan",
  "version": "1",
  "name": "explain one execution",
  "nodes": [
    {
      "id": "explainSubject",
      "skill": "skill://example/explain-execution@^0.1",
      "args": {
        "executionUri": "guild://executions/<execution-id>",
        "includeFirstEvidence": true
      },
      "dependsOn": [],
      "requestedGrants": [
        {
          "family": "read-resource",
          "constraints": {
            "uriPrefixes": [
              "guild://executions/"
            ],
            "resourceKinds": [
              "execution"
            ]
          },
          "purpose": "Read one execution record for a future Guild explanation request."
        }
      ],
      "expectedOutputs": [
        {
          "name": "explanation",
          "kind": "json",
          "description": "Planner expectation for a JSON explanation payload."
        }
      ],
      "expectedEvidence": [
        {
          "kind": "execution-summary",
          "audience": "user",
          "refRoot": "guild://objects/records/",
          "description": "Planner expectation only; not a claim that evidence exists."
        }
      ],
      "failureBehavior": "stopPlan"
    }
  ]
}
```

## Interpretation

The example asks to read one stored execution resource with a bounded
`read-resource` request over `guild://executions/`. That request is only an
input to Guild policy. A validator may reject malformed graph structure before
lowering, but only Guild can resolve the skill, narrow or deny grants, execute
the skill, mint execution identity, persist receipts, persist evidence, and
explain what happened later.
