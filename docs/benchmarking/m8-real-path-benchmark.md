# M8 Real-Path Benchmark

This report measures the checked real path only. Supported and unsupported slices stay separate, bounded proof stays labeled bounded, and fallback or refusal stays explicit.

## Method

- Warmups per measured operation: 2
- Measured runs per operation: 10
- Live proof timing source: `crates/guild-runner/examples/live_proof_scenarios.rs benchmark mode`
- Admission/token/witness timing source: `crates/guild-draft-truth Rust-native internal benchmark generator`
- Live-runtime proof has no cache today. The older draft-example M5 cache remains out of scope for this report.

## Supported Slices

| Slice | Family | Proof | Reduction | Narrowing | Proof-backed | Fallback | Refusal | Witness | Admission mean ms | Proof mean ms | Proof token mean ms | Fallback token mean ms | Refusal mean ms | Token verify mean ms | Witness gen mean ms | Witness verify mean ms |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| read-resource-immutable-guild-roots | read-resource | bounded_minimal | bounded | uri_prefix,resource_kind | 10 | 0 | 0 | proof_linked | 0.014 | 6921.821 | 0.027 | n/a | n/a | 0.357 | 0.302 | 0.253 |
| http-request-loopback-ip-get-explicit-port | http-request | bounded_minimal | bounded | none | 10 | 0 | 0 | proof_linked | 0.015 | 7489.842 | 0.031 | n/a | n/a | 0.323 | 0.295 | 0.239 |
| http-request-loopback-ip-get-default-port | http-request | bounded_minimal | bounded | none | 10 | 0 | 0 | proof_linked | 0.021 | 7825.029 | 0.028 | n/a | n/a | 0.354 | 0.354 | 0.225 |
| http-request-localhost-get-explicit-port | http-request | bounded_minimal | bounded | none | 10 | 0 | 0 | proof_linked | 0.016 | 7382.886 | 0.033 | n/a | n/a | 0.324 | 0.292 | 0.274 |
| http-request-localhost-head-explicit-port | http-request | bounded_minimal | bounded | none | 10 | 0 | 0 | proof_linked | 0.017 | 7489.827 | 0.031 | n/a | n/a | 0.337 | 0.309 | 0.239 |
| http-request-loopback-ip-head-explicit-port | http-request | bounded_minimal | bounded | none | 10 | 0 | 0 | proof_linked | 0.018 | 7420.664 | 0.064 | n/a | n/a | 0.319 | 0.293 | 0.262 |
| http-request-loopback-ip-head-default-port | http-request | bounded_minimal | bounded | none | 10 | 0 | 0 | proof_linked | 0.018 | 7387.621 | 0.051 | n/a | n/a | 0.333 | 0.312 | 0.260 |
| invoke-skill-single-child-zero-authority | invoke-skill | no_reduction | no_reduction | none | 10 | 0 | 0 | proof_linked | 0.012 | 10370.417 | 0.027 | n/a | n/a | 0.269 | 0.251 | 0.183 |
| log-write-observed-info-level | log-write | exact_minimal | exact | none | 0 | 0 | 0 | not_measured | 0.012 | 9041.587 | n/a | n/a | n/a | n/a | n/a | n/a |

## Unsupported Or Not Proven Slices

| Slice | Family | Proof | Reduction | Narrowing | Proof-backed | Fallback | Refusal | Witness | Admission mean ms | Proof mean ms | Proof token mean ms | Fallback token mean ms | Refusal mean ms | Token verify mean ms | Witness gen mean ms | Witness verify mean ms | Fail-closed reasons |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| http-request-redirect-driven-execution | http-request | not_proven | not_proven | none | 0 | 10 | 10 | unlinked | 0.020 | 3662.286 | n/a | 0.026 | 4204.651 | 0.316 | 0.285 | 0.109 | HTTP_REDIRECTS_UNSUPPORTED |
| invoke-skill-multi-child-fan-out | invoke-skill | not_proven | not_proven | none | 0 | 10 | 10 | unlinked | 0.011 | 7667.103 | n/a | 0.022 | 8313.553 | 0.273 | 0.288 | 0.163 | INVOKE_SKILL_MULTI_CHILD_UNSUPPORTED |
| emit-evidence-single-emission-replay-unavailable | emit-evidence | not_proven | not_proven | none | 0 | 10 | 10 | unlinked | 0.015 | 3007.173 | n/a | 0.026 | 3527.528 | 0.284 | 0.283 | 0.117 | EMIT_EVIDENCE_REPLAY_UNAVAILABLE |

## Negative Claims

| Slice | Family | Success | Fail | Coverage limited | Unsupported raw |
| --- | --- | --- | --- | --- | --- |
| read-resource-immutable-guild-roots | read-resource | 0 | 0 | 3 | 0 |
| http-request-loopback-ip-get-explicit-port | http-request | 0 | 0 | 3 | 0 |
| http-request-loopback-ip-get-default-port | http-request | 0 | 0 | 3 | 0 |
| http-request-localhost-get-explicit-port | http-request | 0 | 0 | 3 | 0 |
| http-request-localhost-head-explicit-port | http-request | 0 | 0 | 3 | 0 |
| http-request-loopback-ip-head-explicit-port | http-request | 0 | 0 | 3 | 0 |
| http-request-loopback-ip-head-default-port | http-request | 0 | 0 | 3 | 0 |
| invoke-skill-single-child-zero-authority | invoke-skill | 0 | 0 | 3 | 0 |
| http-request-redirect-driven-execution | http-request | 0 | 0 | 3 | 0 |
| invoke-skill-multi-child-fan-out | invoke-skill | 0 | 0 | 3 | 0 |
| emit-evidence-single-emission-replay-unavailable | emit-evidence | 0 | 0 | 3 | 0 |
| log-write-observed-info-level | log-write | 0 | 0 | 0 | 0 |

## Additional Fail-Closed Walls

| Wall | Family | Stage | Reasons | Proof mean ms |
| --- | --- | --- | --- | --- |
| http-request-no-replay-fixture | http-request | live_proof_search | HTTP_REPLAY_FIXTURE_REQUIRED | 3821.467 |
| read-resource-query-root-shrink-unsupported | read-resource | live_proof_search | LIVE_PROOF_BOUNDED,LIVE_SCOPE_SHRINK_UNSUPPORTED | 4272.835 |
| invoke-skill-child-authority-unsupported | invoke-skill | live_proof_search | INVOKE_SKILL_CHILD_AUTHORITY_UNSUPPORTED | 5877.952 |

## Notes

- The current checked real-path linked chain is `read-resource`, six bounded `http-request` slices, one bounded `invoke-skill` slice, and explicit upper-bound fallback or unlinked witness behavior for the benchmarked unsupported slices.
- `log-write` is still measured here through M4 plus M5 only. The repo has a real live proof slice for observed levels, but this benchmark does not claim a checked real-path M6 or M7 linkage slice for `log-write`.
- The measured reduction split is still mixed by slice: `read-resource` really narrows from the admitted upper bound, the checked `http-request` and `invoke-skill` fixtures are already narrow enough that the proven authority does not shrink them further, and `log-write` is exact over an already narrow admitted level slice.
- The checked negative-claim probes remain coverage-limited on the checked path. They stay `not_provable` rather than being rewritten into synthetic success or failure.
- The remaining frontier is still whichever unsupported rows you want to convert into bounded linked rows without broadening claims: `emit-evidence` exact sink or payload authority, broader `invoke-skill` shapes, and broader `http-request` hostname or replay coverage.
