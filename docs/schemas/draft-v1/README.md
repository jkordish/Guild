# Guild Draft Schemas, v1.0.0

This bundle is the current draft schema surface for Guild's M3, M4, the older draft-example M5 proof path, the new M8c live proof bridge for the families the Rust runtime can honestly support today, one draft-local M6 token path, and one bounded M7 witness path.

It now covers five distinct layers:

- M3 hard-requirement precheck over `skill_contract` plus `runtime_guarantee`
- M4 invocation-specific admission over `skill_contract` plus `admission_request` plus one or more `runtime_guarantee` documents, producing an `execution_plan`
- M5 counterfactual minimization over an admissible `execution_plan` plus deterministic invocation inputs plus one `comparator_profile`, producing a `proof_record`
- M6 delegated capability-token issuance and verification over an admissible `execution_plan`, optional `proof_record`, explicit audience and resource bindings, and an optional parent token
- M7 witness generation and verification over an admissible `execution_plan`, optional `proof_record`, optional verified token basis, and bounded execution observations, producing a `witness_record` plus a `witness_verification_result`

This bundle is still draft. It is useful and executable, but it is not repo-wide canonical truth. The live Rust runtime vocabulary remains canonical, and this bundle now carries that vocabulary directly where the runtime already has stable semantics.

## M8c status

The live Rust capability-family surface is now the canonical runtime vocabulary for this repository.

The active canonical runtime families are:

- `http-request`
- `read-resource`
- `invoke-skill`
- `emit-evidence`
- `log-write`

`runtime_guarantee.supported_canonical_families` names that live surface.

`runtime_guarantee.supported_effect_classes` still exists, but only as a legacy draft-v1 compatibility surface for older bounded fixtures. It is not the canonical runtime truth surface.

Direct draft-v1 canonical family support now exists for:

- M4 admission and execution-plan representation
- M6 token issuance and verification
- M7 witness generation and verification
- scope-only negative-claim verification when the live runtime has complete relevant observation coverage

The remaining compatibility aliases are now explicit, bounded, and deprecated:

- `net.connect` -> `http-request`: deprecated narrowing-only compatibility alias for explicit HTTP(S) GET or HEAD scopes
- `component.invoke` -> `invoke-skill`: deprecated narrowing-only compatibility alias for declared dependency aliases
- `net.resolve` -> no safe direct live family mapping in the active runtime slice

The machine-readable status source for M8c is [`family_support_matrix.json`](./family_support_matrix.json).

Current live-alignment status is explicit:

- the live Rust runner now persists durable `authority_observations` for exercised and blocked attempts in `http-request`, `read-resource`, `invoke-skill`, `emit-evidence`, and `log-write`
- draft-v1 now carries those five live families directly in witnesses with no alias requirement
- scope-only negative claims are now supported for those five canonical families when coverage is complete
- there is still no fixed positive observed-fact claim vocabulary for any family, so positive claim verification remains unsupported even though witnesses carry the observed facts
- M5 is now live-backed only where the Rust runtime actually proves it:
  - `read-resource`: bounded live proof over immutable `guild://executions/` and `guild://objects/records/` roots only
  - `log-write`: live proof over the observed discrete log-level slice
  - `http-request`: bounded live proof only for one deterministic replay-fixtured `GET http://127.0.0.1:<port><exact-path>` loopback slice with no query and no redirects, under the normalized inspect-output comparator
  - `invoke-skill`: `not_proven`
  - `emit-evidence`: `not_proven`
- M6 now issues and verifies direct canonical family scopes, and it can consume live proofs where they exist, but it remains a draft-local HMAC token layer and does not justify runtime-general enforcement claims
- M8c now proves honest live end-to-end chains for `read-resource` and for the bounded `http-request` replay slice: plan -> bounded live proof -> proof-backed token -> proof-linked witness
- broader `http-request` shapes, plus `invoke-skill` and `emit-evidence`, still stay on explicit upper-bound-only token behavior and unlinked witness behavior because live proof is not yet honest for them

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
- Keep M7 honest: it records observed exercised authority and verifies narrow fixed claims against that observation. It does **not** infer absence from incomplete coverage, and it does **not** claim runtime-general completeness beyond the families whose live Rust observations are actually mapped safely into this draft vocabulary.

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
- `witness_verification_result.schema.json`
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
- required authority-selector support
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
- M6 remains separate from M7; token issuance does not by itself prove exercised authority

### M7 witness model

The current M7 layer is deliberately narrow and explicit:

- it consumes one admissible `execution_plan`
- it may consume one `proof_record`
- it may consume one token plus a verified token basis
- it consumes one bounded observation source
- it emits one `witness_record`
- it verifies witnesses and fixed claims through one `witness_verification_result`

Its semantics are intentionally strict:

- it records exercised authority separately from blocked attempted authority
- it distinguishes granted-but-unused authority only when coverage is sufficient to derive that fact honestly
- it tracks observation coverage per relevant effect family rather than treating coverage as one vague global bit
- absence claims require complete relevant coverage
- redaction may preserve some narrow claim checks, but if redaction removes needed facts then claim verification returns an explicit non-success result

Its current limits are also explicit:

- the bounded draft harnesses are still the only complete observation source for non-runtime-backed or non-canonical families in this milestone
- explicit observation fixtures may also be used for checked negative cases, mapping-limit cases, alias-deprecation cases, and blocked-attempt cases
- the live Rust runtime now publishes a durable per-effect `authority_observations` stream for `http-request`, `read-resource`, `invoke-skill`, `emit-evidence`, and `log-write`, and draft-v1 carries those families directly
- witness protection in draft-v1 is the same shared-secret HMAC-SHA256 MAC over canonical JSON claims used by M6, not a public signature or attestation mechanism
- M7 therefore remains useful but only partially runtime-general: the current five active canonical families have direct runtime-backed witness support at their actual live scope shapes, but the bundle still must fail closed for unmappable runtime-native families and still does not justify broader runtime-general completeness claims

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
- `witness_verification_result.schema.json`

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
- `examples/local-log-analyzer.within-envelope.witness.json`
- `examples/local-log-analyzer.out-of-envelope.witness.json`
- `examples/fetch-transform.coverage-limited.witness.json`
- `examples/fetch-transform.redacted-claim-blocked.witness.json`
- `examples/fetch-transform.blocked-attempt.witness.json`
- `examples/zero-authority.witness.json`
- `examples/cluster-rollout.witness.json`
- `examples/runtime-mapping-limited.witness.json`
- `examples/local-log-analyzer.runtime-mismatch.witness.json`
- `examples/runtime-http-read.contract.json`
- `examples/runtime-http-read.admit.request.json`
- `examples/runtime-http-read.invocation.json`
- `examples/runtime-http-read.unavailable.comparator.json`
- `examples/runtime-http-success.execution-record.json`
- `examples/runtime-http-redirect.contract.json`
- `examples/runtime-http-redirect.admit.request.json`
- `examples/runtime-http-redirect.invocation.json`
- `examples/runtime-http-redirect.execution-record.json`
- `examples/runtime-http-blocked.execution-record.json`
- `examples/runtime-read-resource.contract.json`
- `examples/runtime-read-resource.admit.request.json`
- `examples/runtime-read-resource.invocation.json`
- `examples/runtime-read-resource.execution-record.json`
- `examples/runtime-invoke-skill.contract.json`
- `examples/runtime-invoke-skill.admit.request.json`
- `examples/runtime-invoke-skill.invocation.json`
- `examples/runtime-invoke-skill.execution-record.json`
- `examples/runtime-emit-evidence-zero.contract.json`
- `examples/runtime-emit-evidence-zero.admit.request.json`
- `examples/runtime-emit-evidence.invocation.json`
- `examples/runtime-emit-evidence.execution-record.json`
- `examples/runtime-log-write.contract.json`
- `examples/runtime-log-write.admit.request.json`
- `examples/runtime-log-write.invocation.json`
- `examples/runtime-log-write.execution-record.json`

### Utilities

- `admission_engine.py`
- `minimization_engine.py`
- `compatibility_check.py`
- `validate_examples.py`
- `family_support_matrix.json`
- `minimization_core.py`
- `token_core.py`
- `token_engine.py`
- `witness_core.py`
- `witness_engine.py`
- `witness_examples.py`
- `compatibility_matrix.md`

## Status

This bundle remains a **proposal / draft contract surface**.

Two things are true at the same time:

- a portable component can declare broader enforcement requirements than the current runtime slice implements
- component portability is **not** the same thing as enforcement portability

Current mapping boundaries:

| Schema bundle term | Current repo term | Status |
|---|---|---|
| `component.wit_world` | `runtime.entrypoint` / active inspect world `guild-skill-inspect-v1` | bundled contracts now target the live inspect world explicitly |
| direct canonical `http-request` | `http-request` | direct canonical support in M4, M6, and M7 |
| direct canonical `read-resource` | `read-resource` | direct canonical support in M4, M6, and M7 |
| direct canonical `invoke-skill` | `invoke-skill` | direct canonical support in M4, M6, and M7 at the current alias-only runtime scope |
| direct canonical `emit-evidence` | `emit-evidence` | direct canonical support in M4, M6, and M7 |
| direct canonical `log-write` | `log-write` | direct canonical support in M4, M6, and M7 at the current level-only runtime scope |
| `component.invoke` | `invoke-skill` | deprecated narrowing compatibility mapping |
| `net.connect` | `http-request` | deprecated narrowing compatibility mapping; only explicit HTTP(S) GET or HEAD scopes map safely |
| `net.resolve` | `http-request` | unsupported; no safe direct live-family mapping |
| `fs.read`, `fs.write`, `fs.list` | `filesystem` | partial; active inspect runtime still rejects filesystem before guest start |
| `capability.delegate` | child-grant reduction plus host-owned delegation enforcement | related but split across policy and runtime layers |
| witness / proof records | `ExecutionRecord`, `EvidenceRecord`, `PolicyDecision`, host-owned receipts and evidence metadata | overlapping concepts, not one-to-one |
| `secret.read` | `get-secret` | partial; no live inspect enforcement or observation path yet |
| `clock.read` | `wall-clock` | partial; draft-v1 is less precise than the runtime family split |

Until those gaps are closed, this directory must stay explicitly labeled as draft.

The M6 token files in this directory are therefore draft control-plane artifacts. They bind authority to the chosen runtime id and guarantee digest already modeled by the draft examples, but they do **not** by themselves justify any runtime-general enforcement claim for the live Rust runtime.

The M7 witness files in this directory are bounded observed-authority artifacts. They can now prove narrow scope-only negative facts for the five active canonical runtime families when the recorded coverage is adequate, but they must not be described as runtime-general witness completeness.

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

### M7 witness protection status

The draft-v1 M7 witness path reuses that same narrow protection model:

- `witness_engine.py` MACs canonical JSON witness claims with HMAC-SHA256
- witness verification requires prior knowledge of the issuer id, key id, and shared secret
- this is still a shared-secret MAC, not public-key witness attestation
- redacted witness verification checks MACs, linkage, coverage semantics, and redaction hashes, but it does not claim zero-knowledge proofs or public transparency

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
- exact checked M7 witness output for within-envelope, out-of-envelope, coverage-limited, redacted-claim-blocked, blocked-attempt, delegation-chain, zero-authority, runtime-mapping-limited, and runtime-binding-mismatch cases
- witness verification success for authentic within-envelope, coverage-limited, zero-authority, and delegation-chain records
- witness verification fail-closed behavior for runtime-binding mismatch
- fixed-claim evaluation success for proof-backed token absence and bounded delegation claims
- explicit non-success for negative claims blocked by incomplete coverage or redaction
- deterministic repeated M7 witness generation and MAC output for identical inputs
- live-runtime alignment cases covering bounded live `read-resource` proof with proof-backed token and proof-linked witness, bounded live `http-request` proof with proof-backed token and proof-linked witness for the replay-fixtured loopback `GET` slice, unsupported redirect and no-replay `http-request` fail-closed behavior, exact live `log-write` family proof over the observed level slice, deterministic canonicalization, and explicit alias deprecation or rejection

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

# `validate_examples.py` also covers the checked M7 witness generation,
# verification, and claim-evaluation cases end to end.
```

## Next build target

The next honest follow-ons are:

1. vocabulary alignment with the repository's canonical capability-family surface, especially the currently unmapped live Rust inspect families
2. replacing the example-bounded M5 harness with a real runtime-general minimization/replay substrate
3. replacing the draft-local M6 and M7 shared-secret MAC harnesses with stronger repo-supported protection only if and when the runtime surface actually supports those claims
4. wiring any future M7 runtime-general witness flow to a real durable exercised-authority event stream rather than pretending today's bounded observation adapters are broader than they are
