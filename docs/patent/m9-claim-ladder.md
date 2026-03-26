# M9 Technical Claim Ladder

This is a technical claim ladder for counsel. It is not a set of formal patent claims. Each concept is tagged with one of these drafting statuses:

- `measured_supported_now`
- `architecture_supported_but_not_yet_measured`
- `not_claimable_yet`

## C1 Primary Method: Fail-Closed Upper-Bound Admission With Bounded Live-Proof Linkage

- Claim id: `c1-primary-method-fail-closed-admission-bounded-proof-linkage`
- Status: `measured_supported_now`
- Technical concept: receive a requested portable component invocation, compute a fail-closed upper-bound admission result, attempt bounded live proof only for measured family-specific slices, and carry proof state into downstream token and witness linkage only where that proof exists.
- Measured scope now: `read-resource-immutable-guild-roots`, the eight measured `http-request` replay-backed slices, and `invoke-skill-single-child-zero-authority`, plus explicit fallback or refusal behavior for the measured unsupported slices and walls.
- Not included: runtime-general authority minimization, broad `emit-evidence` proof, broad outbound HTTP proof, or broad child-call-graph proof.

## C2 Family-Specific Bounded Proof Slices

- Claim id: `c2-family-specific-bounded-proof-slices`
- Status: `measured_supported_now`
- Technical concept: preserve a family-specific measured frontier rather than one undifferentiated proof claim.
- Measured scope now:
  - `read-resource` over immutable execution and object-record roots
  - eight bounded `http-request` replay-backed slices
  - one bounded `invoke-skill` single-child zero-authority slice
  - one exact `log-write` proof-only slice
- Not included: any claim that one proof basis covers all families or all shapes inside a family.

## C3 Deterministic Replay Or Comparator Basis

- Claim id: `c3-deterministic-replay-or-comparator-basis`
- Status: `measured_supported_now`
- Technical concept: bound proof to deterministic replay or comparator configurations that are explicit enough to justify the measured slice and fail closed when those preconditions are absent.
- Measured scope now:
  - normalized inspect comparator for `read-resource` and the measured `http-request` slices
  - child-aware normalized inspect comparator for the measured `invoke-skill` slice
  - single-sink comparator exists for `emit-evidence`, but the measured replay still fails closed
- Not included: generic replay sufficiency or replay without the measured comparator and fixture conditions.

## C4 Proof-Backed Token Issuance Versus Explicit Upper-Bound Fallback

- Claim id: `c4-proof-backed-token-issuance-versus-explicit-upper-bound-fallback`
- Status: `measured_supported_now`
- Technical concept: downstream issuance preserves whether the basis was live proof or only the admitted upper bound.
- Measured scope now:
  - proof-backed issuance `10/10` for the supported proof-linked slices
  - explicit default refusal `10/10` and explicit upper-bound fallback `10/10` for the benchmarked unsupported slices when fallback is enabled
- Not included: runtime-general delegated-token enforcement or any claim that fallback issuance was still proof-backed.

## C5 Proof-Linked Witness Generation Versus Unlinked Witnesses

- Claim id: `c5-proof-linked-witness-generation-versus-unlinked-witnesses`
- Status: `measured_supported_now`
- Technical concept: witness output preserves whether it is linked to a real proof chain or is merely an observed but unlinked record.
- Measured scope now:
  - proof-linked witnesses for the supported proof-linked slices
  - unlinked witnesses for the benchmarked unsupported slices
- Not included: general positive factual witness claims, or broad absence claims where coverage stays limited.

## C6 Explicit Fail-Closed Unsupported Walls And Reason Codes

- Claim id: `c6-explicit-fail-closed-unsupported-walls-and-reason-codes`
- Status: `measured_supported_now`
- Technical concept: unsupported shapes and missing proof prerequisites are part of the measured surface and are recorded with stable reason codes instead of being silently widened away.
- Measured scope now:
  - redirect-driven `http-request`
  - multi-child `invoke-skill`
  - replay-unavailable `emit-evidence`
  - no-replay HTTP proof search
  - read-resource query-root shrink
  - invoke-skill child-authority use
- Not included: any claim that the system auto-recovers those shapes into proof-backed linkage.

## C7 Machine-Readable Support Frontier And Benchmark Surface

- Claim id: `c7-machine-readable-support-frontier-and-benchmark-surface`
- Status: `measured_supported_now`
- Technical concept: the measured claim frontier is itself machine-readable and tied to checked repo artifacts rather than to narrative-only prose.
- Measured scope now:
  - per-family and per-layer status in `family_support_matrix.json`
  - per-slice and per-wall measured behavior in `benchmark_matrix.json`
  - human-readable benchmark report in `m8-real-path-benchmark.md`
- Not included: any claim that generated artifacts replace the runtime contract source of truth.

## C8 Proof-Only `log-write` Slice Without Real-Path Downstream Linkage

- Claim id: `c8-proof-only-log-write-slice-without-real-path-downstream-linkage`
- Status: `measured_supported_now`
- Technical concept: an exact live proof slice can still be important even when the measured real path does not claim downstream token or witness linkage.
- Measured scope now: `log-write-observed-info-level` through M4 plus M5 only.
- Not included: any claim that the checked benchmark already proves M6 or M7 real-path linkage for `log-write`.

## C9 Real-Path `log-write` Downstream Linkage

- Claim id: `c9-real-path-log-write-downstream-linkage`
- Status: `architecture_supported_but_not_yet_measured`
- Technical concept: the surrounding control-plane pieces exist, but the checked benchmark does not currently claim a real-path proof-backed token or witness linkage slice for `log-write`.
- Measured boundary now: the support matrix marks direct canonical control-plane support, while the benchmark keeps linkage `not_measured_on_real_path`.
- Not included: any present-tense claim that this linkage is already measured on the checked path.

## C10 Not-Claimable-Yet Surfaces

- Claim id: `c10-not-claimable-yet-surfaces`
- Status: `not_claimable_yet`
- Technical concept: preserve the next frontier explicitly instead of overclaiming it.
- Not-claimable-yet items now:
  - proof-backed `emit-evidence` linkage
  - broader `invoke-skill` call graphs
  - broader outbound HTTP proof beyond the eight measured slices
  - runtime-general proof across all families
  - runtime-general delegated-token enforcement
  - broad positive factual witness claims

## Drafting Bias

If counsel needs a narrow independent concept, C1 is the safest measured center. If counsel needs dependent-style narrowing, C2 through C8 are the measured layers. C9 and C10 should stay outside any present-tense "already proven now" framing.
