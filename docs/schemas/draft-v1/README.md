# Guild Draft Schemas, v1.0.0

This bundle is the current draft schema surface for Guild's M3 and M4 admission work.

It now covers two distinct layers:

- M3 hard-requirement precheck over `skill_contract` plus `runtime_guarantee`
- M4 invocation-specific admission over `skill_contract` plus `admission_request` plus one or more `runtime_guarantee` documents, producing an `execution_plan`

This bundle is still draft. It is useful and now executable, but it is not repo-wide canonical truth until the schema vocabulary and the repository's implemented capability-family surface are aligned.

## Design stance

- Use **JSON Schema 2020-12**.
- Keep the schema name `skill_contract` for roadmap continuity, but the subject inside the record is a **portable executable component**, not product-marketing mush.
- Treat enums as part of the protocol. Do **not** replace them with free-form strings unless you enjoy turning admission logic into soup.
- Keep forward compatibility in `extensions`, not in random top-level keys.
- Fail closed on omitted or unknown runtime guarantees.
- Keep hard contract requirements separate from request-time narrowing.
- Keep M4 honest: it derives a safe upper bound for one invocation. It does **not** minimize that bound.

## Core records

- `skill_contract.schema.json`
- `runtime_guarantee.schema.json`
- `admission_request.schema.json`
- `execution_plan.schema.json`
- `proof_record.schema.json`
- `witness_record.schema.json`
- shared definitions in `common.schema.json`

## Why `authority_ceiling` is in `skill_contract`

You asked for required effects and forbidden effects. That is not enough for admission planning.

The planner needs a **declared maximum grant envelope** to start from. That is what `authority_ceiling` is.

Think of the three sets like this:

- `required_effects`: effects the component must retain
- `forbidden_effects`: effects the component may never obtain
- `authority_ceiling`: the largest admissible grant set M4 may consider before any later minimization phase

Without `authority_ceiling`, the planner turns into a philosophy seminar.

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

## Admission model

### Hard requirements

The hard-requirement path is shared by `compatibility_check.py` and `admission_engine.py`.

It currently enforces:

- component-model compatibility
- explicit WIT-world publication
- required effect-class support
- required-effect scope enforceability
- ordered and mode-based runtime guarantee thresholds
- witness-support minimums

If a runtime omits a required guarantee or publishes an unknown value, the result is fail-closed denial.

### Request-time narrowing

M4 then evaluates the invocation request against the contract ceiling and the selected runtime:

- requested authority may be narrowed to a stricter granted set
- denied requested authority is explicit and reason-coded
- denied requested authority does **not** automatically imply refusal
- refusal happens only when hard requirements fail or no safe upper-bound plan can be derived

### Decision outcomes

`execution_plan` uses one exact decision enum:

- `admit`
- `downgrade`
- `migrate`
- `refuse`

The important distinction is:

- `compatibility_matrix.md` is a hard-requirement precheck artifact
- `execution_plan` is the M4 admission artifact for a specific invocation

Migration is runtime reselection, not silent relaxation.

### M4 does not do M5 work

This bundle now emits a safe upper-bound `execution_plan`.

It still does **not** do:

- counterfactual shadow execution
- authority minimization
- proof-record creation from minimization trials
- silent runtime widening because a narrow request is hard to enforce

That is later work.

## Files included

### Schemas

- `common.schema.json`
- `skill_contract.schema.json`
- `runtime_guarantee.schema.json`
- `admission_request.schema.json`
- `execution_plan.schema.json`
- `proof_record.schema.json`
- `witness_record.schema.json`

### Examples

- `examples/local-log-analyzer.contract.json`
- `examples/zero-authority.contract.json`
- `examples/fetch-transform.contract.json`
- `examples/cluster-rollout.contract.json`
- `examples/wasmtime-strict.runtime.json`
- `examples/node-wasi-basic.runtime.json`
- `examples/zero-authority.admit.request.json`
- `examples/zero-authority.admit.plan.json`
- `examples/zero-authority.migrate.request.json`
- `examples/zero-authority.migrate.plan.json`
- `examples/fetch-transform.downgrade.request.json`
- `examples/fetch-transform.downgrade.plan.json`
- `examples/cluster-rollout.refuse.request.json`
- `examples/cluster-rollout.refuse.plan.json`
- `examples/local-log-analyzer.proof.json`
- `examples/cluster-rollout.witness.json`

### Utilities

- `admission_engine.py`
- `compatibility_check.py`
- `validate_examples.py`
- `compatibility_matrix.md`

## Status

This bundle remains a **proposal / draft contract surface**.

Two things are true at the same time:

- a portable component can declare broader enforcement requirements than the current runtime slice implements
- component portability is **not** the same thing as enforcement portability

Current mapping boundaries:

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

Until those gaps are closed, this directory must stay explicitly labeled as draft.

## Signing status

The bundle now includes schema hooks for an optional `plan_signature`, but checked-in M4 execution plans are **unsigned**.

That is deliberate.

The wider repository already has real Ed25519 signing for installed bundles, but this draft bundle does not yet have a reused, verifiable generic plan-signing path. Until that exists, the M4 plan artifacts must not be described as signed.

## Validation status

All bundled examples validate cleanly against the bundled schemas when run with the directory-local validation dependencies installed.

`validate_examples.py` now verifies:

- schema validation for the bundled contracts, runtimes, requests, plans, proof, and witness examples
- exact expected-plan output for the `admit`, `downgrade`, `migrate`, and `refuse` admission examples
- deterministic repeated execution-plan output for the same inputs
- explicit negative probes for omitted and invalid runtime guarantees

`compatibility_check.py` regenerates the derived hard-requirement compatibility matrix and asserts the fail-closed negative probes for omitted and unsupported `wit_worlds` support.

### Reproducible validation

```bash
python3 -m venv .venv
. .venv/bin/activate
pip install -r requirements.txt
python3 validate_examples.py
python3 compatibility_check.py
python3 admission_engine.py \
  --contract examples/zero-authority.contract.json \
  --request examples/zero-authority.migrate.request.json \
  --runtime examples/node-wasi-basic.runtime.json \
  --runtime examples/wasmtime-strict.runtime.json
```

## Next build target

The next honest follow-ons are:

1. vocabulary alignment with the repository's canonical capability-family surface
2. real reusable plan-signing support, if the repository grows a verifiable generic signing path
3. later M5 minimization and proof generation on top of the M4 upper-bound plan
