# M9 Measured Technical Delta Memo

This memo is anchored to the checked repo artifacts [`benchmark_matrix.json`](../schemas/draft-v1/benchmark_matrix.json), [`m8-real-path-benchmark.md`](../benchmarking/m8-real-path-benchmark.md), [`family_support_matrix.json`](../schemas/draft-v1/family_support_matrix.json), and the frozen contract vocabulary in [`SPECS.md`](../../SPECS.md). It is written as technical drafting material, not as a legal conclusion.

## What The System Does Technically

1. The host computes an invocation-specific upper-bound admission result. That result is an allowed ceiling, not a proof that the ceiling is minimal.
2. The runner attempts live proof only for slices where the repo has an honest basis for replay or deterministic comparison.
3. If live proof succeeds, the downstream draft control-plane can mark the result as proof-backed and can link witnesses to that proof.
4. If live proof is unavailable or fails closed, the system does not invent narrower authority. It either refuses, or only issues an explicit upper-bound fallback token, and any witness remains unlinked.

The measured distinction is therefore between:

- upper-bound admission
- bounded live proof
- proof-backed downstream linkage
- explicit fallback or refusal

Those stages are separate in the repo and should stay separate in any draft built from this packet.

## Measured Frontier By Family

| Family | Live proof status now | Token basis now | Witness linkage now | Measured scope now | Explicit limits now |
| --- | --- | --- | --- | --- | --- |
| `read-resource` | `bounded` | `proof_backed` on one bounded slice | `proof_linked` on one bounded slice | Immutable `guild://executions/` and `guild://objects/records/` roots only | Query resources and broader shrink models remain outside the measured envelope |
| `http-request` | `bounded` | `proof_backed` on eight replay-backed slices | `proof_linked` on eight replay-backed slices | Loopback IP `GET` and `HEAD`, explicit or default port; `localhost` `GET` and `HEAD`, explicit or default port with deterministic loopback-only resolution binding; no query; no redirects; one exercised request; deterministic comparator | Redirects, `https`, multiple requests, other hostname forms, and query or fragment components stay outside the measured envelope |
| `invoke-skill` | `bounded` | `proof_backed` on one single-child slice | `proof_linked` on one single-child slice | One declared alias, one exact child digest, `guild-skill-inspect-v1`, deterministic child input, zero child authority, zero nested child executions | Multi-child, recursion, child authority, broader resolution, and non-inspect child targets remain outside the measured envelope |
| `log-write` | `supported` for proof only | `not_measured_on_real_path` for linkage | `not_measured_on_real_path` for linkage | One exact observed `info`-level slice through M4 plus M5 only | The checked benchmark does not claim real-path M6 or M7 linkage here |
| `emit-evidence` | `not_proven` | `upper_bound_fallback` only in the checked unsupported slice | `unlinked` only in the checked unsupported slice | The checked benchmark covers replay-unavailable single-emission behavior as an unsupported slice | No proof-backed `emit-evidence` linkage is claimable now |

## Where Authority Narrows And Where It Does Not

- `read-resource` is the clearest measured narrowing case. The checked path reduces the admitted authority on `uri_prefix` and `resource_kind`.
- The measured `http-request` proof-backed slices are already narrow fixtures. The proof is bounded and real, but the proven authority does not shrink further in those fixtures.
- The measured `invoke-skill` slice is also already narrow. The proof result is `no_reduction`, not a broader minimization story.
- The measured `log-write` slice is exact over an already narrow admitted level slice, but the checked real path stops at proof only.
- `emit-evidence` remains `not_proven`, so the repo does not honestly narrow the authority beyond the admitted upper bound on the checked path.

## Proof-Backed Issuance Versus Fallback Or Refusal

- The supported proof-linked slices issue proof-backed tokens `10/10`.
- The benchmarked unsupported slices refuse by default `10/10`.
- The benchmarked unsupported slices also issue explicit upper-bound fallback tokens `10/10` when fallback is enabled.
- The unsupported slices do not silently collapse those two outcomes. Refusal and fallback are both visible in the benchmark artifact.

The measured unsupported slices are:

- `http-request-redirect-driven-execution`
- `invoke-skill-multi-child-fan-out`
- `emit-evidence-single-emission-replay-unavailable`

## Witness Linkage Versus Unlinked Witnesses

- `read-resource`, the eight measured `http-request` slices, and the one measured `invoke-skill` slice produce proof-linked witnesses on the checked path.
- `log-write` is not currently measured as a proof-linked witness path on the real path.
- The benchmarked unsupported slices still generate witnesses, but those witnesses are explicitly unlinked.
- The checked negative-claim probes remain coverage-limited on every measured non-`log-write` slice, so the repo does not claim runtime-general absence checking.

## Explicit Fail-Closed Walls

| Wall or unsupported slice | Family | Measured path | Reason codes |
| --- | --- | --- | --- |
| `http-request-redirect-driven-execution` | `http-request` | refusal then upper-bound fallback | `HTTP_REDIRECTS_UNSUPPORTED` |
| `invoke-skill-multi-child-fan-out` | `invoke-skill` | refusal then upper-bound fallback | `INVOKE_SKILL_MULTI_CHILD_UNSUPPORTED` |
| `emit-evidence-single-emission-replay-unavailable` | `emit-evidence` | refusal then upper-bound fallback | `EMIT_EVIDENCE_REPLAY_UNAVAILABLE` |
| `http-request-no-replay-fixture` | `http-request` | fail closed during live proof search | `HTTP_REPLAY_FIXTURE_REQUIRED` |
| `read-resource-query-root-shrink-unsupported` | `read-resource` | fail closed during live proof search | `LIVE_PROOF_BOUNDED`, `LIVE_SCOPE_SHRINK_UNSUPPORTED` |
| `invoke-skill-child-authority-unsupported` | `invoke-skill` | fail closed during live proof search | `INVOKE_SKILL_CHILD_AUTHORITY_UNSUPPORTED` |

## Measured Overheads

The checked benchmark is slice-aware and keeps supported, unsupported, and fail-closed-wall cases separate. The values below follow the checked report units as written in [`m8-real-path-benchmark.md`](../benchmarking/m8-real-path-benchmark.md).

### Supported And Proof-Only Slices

| Slice | Admission mean ms | Proof mean ms | Proof token mean ms | Token verify mean ms | Witness gen mean ms | Witness verify mean ms |
| --- | --- | --- | --- | --- | --- | --- |
| `read-resource-immutable-guild-roots` | `0.022` | `7000.947` | `0.021` | `0.305` | `0.318` | `0.186` |
| `http-request-loopback-ip-get-explicit-port` | `0.015` | `7459.956` | `0.032` | `0.354` | `0.377` | `0.239` |
| `http-request-loopback-ip-get-default-port` | `0.015` | `7570.034` | `0.029` | `0.373` | `0.313` | `0.228` |
| `http-request-localhost-get-explicit-port` | `0.016` | `7617.252` | `0.050` | `0.331` | `0.296` | `0.227` |
| `http-request-localhost-get-default-port` | `0.018` | `7436.796` | `0.028` | `0.367` | `0.290` | `0.237` |
| `http-request-localhost-head-explicit-port` | `0.022` | `7356.991` | `0.035` | `0.332` | `0.291` | `0.241` |
| `http-request-localhost-head-default-port` | `0.019` | `7428.505` | `0.028` | `0.326` | `0.347` | `0.243` |
| `http-request-loopback-ip-head-explicit-port` | `0.019` | `7412.078` | `0.039` | `0.372` | `0.296` | `0.219` |
| `http-request-loopback-ip-head-default-port` | `0.019` | `7369.590` | `0.032` | `0.325` | `0.302` | `0.229` |
| `invoke-skill-single-child-zero-authority` | `0.013` | `10353.470` | `0.020` | `0.273` | `0.286` | `0.189` |
| `log-write-observed-info-level` | `0.013` | `8996.703` | `n/a` | `n/a` | `n/a` | `n/a` |

### Unsupported Slices And Fail-Closed Walls

| Slice or wall | Admission mean ms | Proof mean ms | Fallback token mean ms | Refusal mean ms |
| --- | --- | --- | --- | --- |
| `http-request-redirect-driven-execution` | `0.021` | `3724.381` | `0.024` | `4184.741` |
| `invoke-skill-multi-child-fan-out` | `0.012` | `7791.796` | `0.020` | `8302.056` |
| `emit-evidence-single-emission-replay-unavailable` | `0.039` | `2994.840` | `0.022` | `3537.115` |
| `http-request-no-replay-fixture` | `n/a` | `3702.955` | `n/a` | `n/a` |
| `read-resource-query-root-shrink-unsupported` | `n/a` | `4236.320` | `n/a` | `n/a` |
| `invoke-skill-child-authority-unsupported` | `n/a` | `6043.337` | `n/a` | `n/a` |

The multi-second cost is in live proof search, not in admission or downstream token or witness processing. That cost is part of the technical story and should stay visible.

## Practical Drafting Consequence

The strongest technical story is not "Guild proves least authority everywhere." The strongest measured story is narrower:

- the host computes an upper bound and treats it honestly as an upper bound
- bounded live proof exists only for explicit measured slices
- proof-backed downstream linkage exists only for those slices
- unsupported and not-proven shapes fail closed into refusal, fallback, or unlinked witness behavior

That narrower story is what the repo actually proves today, and it is the safest drafting center of gravity for M9.
