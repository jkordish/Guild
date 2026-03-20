# Guild Draft Schemas, v1.0.0

This bundle is the current draft schema surface for Guild's M3, M4, one bounded M5 proof path, and one draft-local M6 token path.

It now covers four distinct layers:

- M3 hard-requirement precheck over `skill_contract` plus `runtime_guarantee`
- M4 invocation-specific admission over `skill_contract` plus `admission_request` plus one or more `runtime_guarantee` documents, producing an `execution_plan`
- M5 counterfactual minimization over an admissible `execution_plan` plus deterministic invocation inputs plus one `comparator_profile`, producing a `proof_record`
- M6 delegated capability-token issuance and verification over an admissible `execution_plan`, optional `proof_record`, explicit audience and resource bindings, and an optional parent token

This bundle is still draft. It is useful and now executable, but it is not repo-wide canonical truth until the schema vocabulary and the repository's implemented capability-family surface are aligned.

## Design stance

- Use **JSON Schema 2020-12**.
- Keep the schema name `skill_contract` for roadmap continuity, but the subject inside the record is a **portable executable component**, not product-marketing mush.
- Treat enums as part of the protocol. Do **not** replace them with free-form strings unless you enjoy turning admission logic into soup.
- Keep forward compatibility in `extensions`, not in random top-level keys.
- Fail closed on omitted or unknown runtime guarantees.
- Keep hard contract requirements separate from request-time narrowing.
- Keep M4 honest: it derives a safe upper bound for one invocation. It does **not** minimize that bound.
- Keep M5 honest: it may only preserve or reduce the M4 upper bound, it may never widen authority, and it must not claim exact minimality unless the explored search model actually proves it.
- Keep M6 honest: it may materialize only the M4 upper bound or a proof-backed M5 subset, it may never widen either one, and it must say plainly whether the draft path is using a MAC or a signature.

## Core records

- `skill_contract.schema.json`
- `runtime_guarantee.schema.json`
- `admission_request.schema.json`
- `execution_plan.schema.json`
- `comparator_profile.schema.json`
- `proof_record.schema.json`
- `delegated_capability_token.schema.json`
- `token_verification_result.schema.json`
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

### M4 still does not do M5 work

`admission_engine.py` still stops at the safe upper-bound `execution_plan`.

It still does **not**:

- minimize authority inline during admission
- widen runtime or authority semantics to rescue a narrow request
- claim that compatibility precheck alone is admission

M5 now happens only in the separate `minimization_engine.py` path, and only after M4 has already produced an admissible plan.

### M5 minimization model

The current M5 layer is deliberately narrow:

- it consumes one admissible `execution_plan`
- it runs deterministic counterfactual trials against one explicit invocation fixture
- it uses one explicit `comparator_profile`
- it emits one `proof_record`
- it caches conservatively by exact plan/runtime/comparator/input identity

It is also deliberately limited:

- it is **example-bounded**, not runtime-general
- it only has real replay/proof harnesses for the bundled draft examples
- exact discrete grant elimination is exhaustive only over the finite grant subsets it actually explores
- scope shrinkers are bounded observed-effect projections, so accepted shrink results are reported as `bounded_minimal`, not exact
- comparator failure or unavailability yields `not_proven`, not silent success

The current proof-status vocabulary is:

- `exact_minimal`
- `bounded_minimal`
- `reduced`
- `no_reduction`
- `not_proven`

### M6 delegated capability token model

The current M6 layer is deliberately narrow:

- it consumes one admissible `execution_plan`
- it may consume one `proof_record`
- it requires explicit holder binding
- it binds the token to one invocation call chain and one chosen runtime
- it emits either one `delegated_capability_token` or one structured refusal result
- it verifies tokens through one `token_verification_result`

Its default stance is the stricter honest one:

- root issuance is proof-backed by default
- if no acceptable M5 proof exists, issuance refuses by default
- M4 upper-bound issuance only happens when the caller explicitly enables it, and the token is marked `issuance_basis: m4_upper_bound`
- zero-authority invocations emit an explicit empty-capability token rather than an ambiguous "no token required" result
- root tokens are non-pass-through by default
- child issuance is explicit, bounded, and must narrow or preserve scope, audience, runtime binding, expiry, and delegation depth
- presenting a parent token directly to an unintended downstream consumer fails closed

The current M6 layer is also deliberately limited:

- cryptographic protection is a draft-local HMAC-SHA256 MAC over canonical JSON, not a public-key signature
- the verifier must already know the issuer id, key id, and shared secret
- replay protection is local verifier-side state keyed by token id plus chain identity; it is not distributed replay protection
- revocation is a local verifier denylist and issuer-epoch hook; it is not distributed revocation
- the token layer is still draft/example-bounded because the runtime vocabulary alignment problem that limits M5 also limits any stronger M6 enforcement claim
- M6 does not implement the later M7 witness layer; it only leaves room for future witness linkage

## Files included

### Schemas

- `common.schema.json`
- `skill_contract.schema.json`
- `runtime_guarantee.schema.json`
- `admission_request.schema.json`
- `execution_plan.schema.json`
- `comparator_profile.schema.json`
- `proof_record.schema.json`
- `delegated_capability_token.schema.json`
- `token_verification_result.schema.json`
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
- `examples/fetch-transform.no-reduction.request.json`
- `examples/fetch-transform.no-reduction.plan.json`
- `examples/cluster-rollout.refuse.request.json`
- `examples/cluster-rollout.refuse.plan.json`
- `examples/cluster-rollout.admit.request.json`
- `examples/cluster-rollout.admit.plan.json`
- `examples/local-log-analyzer.admit.request.json`
- `examples/local-log-analyzer.admit.plan.json`
- `examples/local-log-analyzer.invocation.json`
- `examples/local-log-analyzer.canonical-json.comparator.json`
- `examples/local-log-analyzer.unavailable.comparator.json`
- `examples/local-log-analyzer.proof.json`
- `examples/local-log-analyzer.cache-hit.proof.json`
- `examples/local-log-analyzer.comparator-unavailable.proof.json`
- `examples/local-log-analyzer.proof-backed.root-token.json`
- `examples/fetch-transform.invocation.json`
- `examples/fetch-transform.postconditions.comparator.json`
- `examples/fetch-transform.bounded.comparator.json`
- `examples/fetch-transform.no-reduction.proof.json`
- `examples/fetch-transform.bounded.proof.json`
- `examples/fetch-transform.upper-bound-refusal.json`
- `examples/zero-authority.invocation.json`
- `examples/zero-authority.pure.comparator.json`
- `examples/zero-authority.proof.json`
- `examples/zero-authority.empty-token.json`
- `examples/cluster-rollout.root-token.json`
- `examples/cluster-rollout.child-token.json`
- `examples/cluster-rollout.witness.json`

### Utilities

- `admission_engine.py`
- `minimization_engine.py`
- `compatibility_check.py`
- `validate_examples.py`
- `minimization_core.py`
- `token_core.py`
- `token_engine.py`
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

The M6 token files in this directory are therefore draft control-plane artifacts. They bind authority to the chosen runtime id and guarantee digest already modeled by the draft examples, but they do **not** by themselves justify any runtime-general enforcement claim for the live Rust runtime.

## Cryptographic status

### M4 plan signing status

The bundle now includes a real `plan_signature` shape plus a reusable signing path through Guild's existing publisher identity and trust-store model.

That is deliberate.

Three things are true at once:

- checked-in M4 execution-plan examples are still **unsigned**
- `admission_engine.py` still emits unsigned plans by default
- `guild trust sign-plan` and `guild trust verify-plan` can now sign and verify those plans later using the same Ed25519 publisher identities and trusted publisher records already used for signed bundles

So the M4 plan artifacts must not be described as automatically signed, but they also are no longer blocked on a fake or decorative signing story.

### M6 token protection status

The M6 token path uses a different and narrower mechanism:

- `delegated_capability_token` carries inline canonicalization and protection metadata
- `token_engine.py` computes an HMAC-SHA256 MAC over canonical JSON claims
- that MAC is verified only by verifiers that already share the issuer secret

That means:

- this is issuer/verifier shared-secret protection
- this is **not** public verifiability
- this is **not** a detached signature workflow
- this is a draft control-plane mechanism for the bundled harness, not final distributed attestation

## Validation status

All bundled examples validate cleanly against the bundled schemas when run with the directory-local validation dependencies installed.

`validate_examples.py` now verifies:

- schema validation for the bundled contracts, runtimes, comparator profiles, requests, plans, proof, and witness examples
- exact expected-plan output for the `admit`, `downgrade`, `migrate`, and `refuse` admission examples
- deterministic repeated execution-plan output for the same inputs
- exact checked proof output for the bundled M5 reduction, no-reduction, bounded-minimal, zero-authority, comparator-unavailable, and cache-hit cases
- strict cache-bypass probes when runtime, comparator, or plan identity changes
- explicit negative probes for omitted and invalid runtime guarantees
- exact checked M6 token output for proof-backed root issuance, explicit upper-bound issuance, delegated child issuance, upper-bound refusal, and zero-authority empty-token issuance
- verification success and fail-closed denial for replay, wrong audience, wrong holder, passthrough attempts, chain mismatch, runtime mismatch, broadening, and expiry cases
- deterministic repeated M6 issuance for identical claims and key material

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
python3 minimization_engine.py \
  --plan examples/local-log-analyzer.admit.plan.json \
  --contract examples/local-log-analyzer.contract.json \
  --request examples/local-log-analyzer.admit.request.json \
  --runtime examples/wasmtime-strict.runtime.json \
  --invocation-input examples/local-log-analyzer.invocation.json \
  --comparator-profile examples/local-log-analyzer.canonical-json.comparator.json \
  --created-at 2026-03-20T12:10:00Z \
  --cache-dir /tmp/guild-m5-cache
python3 token_engine.py issue-root \
  --plan examples/local-log-analyzer.admit.plan.json \
  --contract examples/local-log-analyzer.contract.json \
  --proof examples/local-log-analyzer.proof.json \
  --holder-id urn:guild:service:local-log-analyzer \
  --issuer-id urn:guild:issuer:draft-control-plane:v1 \
  --key-id draft-hmac-2026-03 \
  --shared-secret guild-draft-shared-secret-2026-03 \
  --issuer-epoch 3 \
  --issued-at 2026-03-20T13:00:00Z \
  --token-id urn:guild:token:local-log-analyzer:root:v1
python3 token_engine.py verify \
  --token examples/cluster-rollout.child-token.json \
  --issuer-id urn:guild:issuer:draft-control-plane:v1 \
  --key-id draft-hmac-2026-03 \
  --shared-secret guild-draft-shared-secret-2026-03 \
  --verification-time 2026-03-20T13:05:20Z \
  --holder-id urn:guild:service:kube-api-client \
  --runtime-guarantee-id urn:guild:runtime:wasmtime-strict:v1 \
  --plan examples/cluster-rollout.admit.plan.json \
  --contract examples/cluster-rollout.contract.json \
  --parent-token examples/cluster-rollout.root-token.json \
  --audience cluster-prod \
  --resource-binding-json '{"effect_class":"net.connect","audience":"cluster-prod","resource":"https://kube-api.prod.example.internal/apis/apps/"}' \
  --chain-link urn:guild:actor:ops-user \
  --chain-link urn:guild:workflow:cluster-rollout \
  --chain-link urn:guild:token:cluster-rollout:root:v1 \
  --chain-link urn:guild:service:kube-api-client \
  --replay-state-dir /tmp/guild-m6-replay

guild trust generate \
  --publisher-id local.example \
  --display-name "Local Example" \
  --output /tmp/guild-plan-signer.json
guild --registry-root /tmp/guild-plan-registry trust add \
  --identity-file /tmp/guild-plan-signer.json
guild trust sign-plan \
  --plan examples/zero-authority.admit.plan.json \
  --identity-file /tmp/guild-plan-signer.json \
  --output /tmp/zero-authority.admit.signed.plan.json
guild --registry-root /tmp/guild-plan-registry trust verify-plan \
  --plan /tmp/zero-authority.admit.signed.plan.json
```

## Next build target

The next honest follow-ons are:

1. vocabulary alignment with the repository's canonical capability-family surface
2. replacing the example-bounded M5 harness with a real runtime-general minimization/replay substrate
3. replacing the draft-local M6 shared-secret token harness with the eventual repo-supported runtime-attestation and revocation story, if and when the runtime surface actually supports those claims
4. later M7 witness materialization and exercised-authority verification on top of the still-separate M4/M5/M6 outputs
