# Guild M3 Schemas, v1.0.0

This is the first locked schema cut for Guild’s authority-admission system.

## Design stance

- Use **JSON Schema 2020-12**.
- Keep the schema name `skill_contract` for roadmap continuity, but the subject inside the record is a **portable executable component**, not product-marketing mush.
- Treat enums as part of the protocol. Do **not** replace them with free-form strings unless you enjoy turning admission logic into soup.
- Keep forward compatibility in `extensions`, not in random top-level keys.

## Core records

- `skill_contract.schema.json`
- `runtime_guarantee.schema.json`
- `proof_record.schema.json`
- `witness_record.schema.json`
- shared definitions in `common.schema.json`

## Why `authority_ceiling` is in `skill_contract`

You asked for required effects and forbidden effects. That is not enough for minimization.
The planner needs a **declared maximum grant envelope** to start from. That is what `authority_ceiling` is.

Think of the three sets like this:

- `required_effects`: effects the component must retain
- `forbidden_effects`: effects the component may never obtain
- `authority_ceiling`: the largest admissible grant set the planner may consider before minimization

Without `authority_ceiling`, the minimizer turns into a philosophy seminar.

## Ordered comparisons used by admission

These orderings are not enforced by JSON Schema itself. The admission engine must implement them.

### `execution_isolation_assurance`
`none < best_effort < strong`

### `filesystem_isolation_class`
`none < path_filter < preopen_only < virtual_fs < os_sandbox`

### `network_policy_granularity`
`none < binary < domain < host_port < url`

### `witness_level`
`summary < decision < hostcall < full`

## Comparison rules

- For ordered fields, the runtime must be **at least** the requested minimum.
- For policy-mode fields expressed as `required_mode`, the runtime must list that mode in `supported_modes`.
- For `component.wit_world`, the runtime must explicitly publish `component_model_support.wit_worlds`.
- Omitted or unknown `wit_worlds` support fails closed. Empty arrays are allowed and mean the runtime currently publishes support for no WIT worlds.
- For witness support, the runtime must support:
  - at least the requested witness level
  - at least one acceptable tamper-evidence mode
  - at least one acceptable signature mode
  - every required boolean capability

## Files included

### Schemas
- `common.schema.json`
- `skill_contract.schema.json`
- `runtime_guarantee.schema.json`
- `proof_record.schema.json`
- `witness_record.schema.json`

### Examples
- `examples/local-log-analyzer.contract.json`
- `examples/zero-authority.contract.json`
- `examples/fetch-transform.contract.json`
- `examples/cluster-rollout.contract.json`
- `examples/wasmtime-strict.runtime.json`
- `examples/node-wasi-basic.runtime.json`
- `examples/local-log-analyzer.proof.json`
- `examples/cluster-rollout.witness.json`

### Utilities
- `validate_examples.py`
- `compatibility_matrix.md`

## Status

This bundle is a **proposal / draft contract surface** for M3. It is not repo-wide normative truth until the capability-family vocabulary is aligned in the corresponding SPECS.md and ARCHITECTURE.md update.

This patch keeps the bundle in draft status on purpose. The schemas now fail closed on omitted WIT-world support, but the schema bundle still uses an effect-class vocabulary that is broader than the current canonical runtime capability-family surface implemented in the repository.

## Current mapping to repo norms

The current repository product surface is the host-owned typed capability-family model in `SPECS.md` and `ARCHITECTURE.md`. The schema bundle remains a draft vocabulary mapped onto that surface.

| Schema bundle term | Current repo term | Status |
|---|---|---|
| `component.wit_world` | `runtime.entrypoint` / active inspect world `guild-skill-inspect-v1` | related but not identical |
| `component.invoke` | `invoke-skill` | close mapping |
| `net.connect`, `net.resolve` | `http-request` | not equivalent; repo runtime is narrower and host-mediated |
| `fs.read`, `fs.write`, `fs.list` | `filesystem` | related, but active inspect runtime still rejects filesystem before guest start |
| `capability.delegate` | child-grant reduction plus host-owned delegation enforcement | related but split across policy and runtime layers |
| witness / proof records | `ExecutionRecord`, `EvidenceRecord`, `PolicyDecision`, host-owned receipts and evidence metadata | overlapping concepts, not one-to-one |
| no direct schema effect-class for `read-resource` | `read-resource` | unmapped in this draft bundle |
| no direct schema effect-class for `emit-evidence` | `emit-evidence` | unmapped in this draft bundle |
| no direct schema effect-class for `log-write` | `log-write` | unmapped in this draft bundle |

Two things are true at the same time:

- a portable component can declare broader enforcement requirements than the current runtime slice implements
- component portability is **not** the same thing as enforcement portability

If a runtime does not explicitly publish support for the required WIT world or effect vocabulary, admission must deny rather than infer support.

## Locked decisions in this cut

1. `kind` + `version` are required on every top-level record.
2. Digests are structured as `{ algorithm, value }`, not as an opaque string, and digest length is bound to the declared algorithm.
3. Delegation is explicit, typed, and bounded.
4. Witnessing is a first-class schema concern, not an afterthought bolted onto logs.
5. Runtime guarantees describe **what can be enforced**, not just what someone hopes is true. Component-model compatibility, explicit WIT-world publication, and required effect-class support are admission checks, not advisory metadata.

## Deliberate omissions

These are not in v1 on purpose:

- embedded policy DSLs
- arbitrary expression evaluators
- free-form comparator programs
- open-ended resource kinds
- implicit ambient authority

Those can come back later if they earn their keep.

## Validation status

All bundled example files validate cleanly against the bundled schemas **when run with the directory-local validation dependencies installed**.

`compatibility_check.py` also regenerates the derived compatibility matrix and asserts the fail-closed negative probes for omitted and unsupported `wit_worlds` support.

### Reproducible validation

```bash
python3 -m venv .venv
. .venv/bin/activate
pip install -r requirements.txt
python3 validate_examples.py
python3 compatibility_check.py
```

## Next build target

Use these schemas to implement:
1. runtime guarantee publication
2. contract ingestion
3. deterministic compatibility checks
4. proof-record creation from shadow-run minimization
5. witness-record emission on hostcalls
