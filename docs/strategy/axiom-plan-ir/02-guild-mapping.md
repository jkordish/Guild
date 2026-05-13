# Axiom Plan IR To Guild Mapping

Status: exploratory mapping, not an implementation contract.

Guild executes; Axiom plans. The mapping below describes how an Axiom Plan IR
document could lower into ordinary Guild concepts without bypassing Guild's
host-mediated execution boundary.

| Axiom concept | Guild concept |
| --- | --- |
| Axiom skill | Guild requested skill ref |
| Axiom resolved node | Guild resolved skill ref + digest |
| Axiom args | Guild skill input JSON |
| Axiom requested grants | Caller-requested grants |
| Axiom policy precheck | Guild admission/policy input |
| Axiom node execution | Guild execution record |
| Axiom plan trace | Guild execution records, receipts, and evidence refs |
| Axiom output refs | `guild://executions/...` and `guild://objects/...` refs |

## Canonical Guild Roots

An Axiom plan may mention canonical Guild roots as requested inputs or expected
refs, but it does not own those resources. The relevant roots include:

- `guild://executions/`
- `guild://objects/sha256/`
- `guild://objects/records/`
- `guild://queries/executions/`

Those roots remain Guild resource roots. A plan may ask to read or produce refs
under them, but Guild policy and storage decide whether any actual execution
may access or create them.

## Lowering Rules

Axiom must not bypass Guild host-mediated execution. A lowerer can prepare a
reviewable run plan, but every concrete skill invocation still has to enter
Guild through the same resolution, admission, policy, runtime, receipt, and
evidence paths as any other Guild execution.

Composite or child skill invocation must still go through Guild. If an Axiom
plan lowers to child skill calls, those calls still go through Guild's
host-mediated invocation path. Axiom cannot create an alternate child-invocation
channel, cannot reuse parent authority without Guild policy, and cannot bypass
the installed dependency snapshot or resolved skill identity.

Guild durable records remain canonical execution truth. A plan trace can help a
human understand what the planner expected, but the truth of what was admitted,
what was granted, what ran, what was blocked, what evidence was persisted, and
what refs can be inspected comes from Guild execution records, receipts, and
evidence refs.
