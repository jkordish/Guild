# M8 Real-Path Benchmark

This report measures the checked real path only. Supported and unsupported slices stay separate, bounded proof stays labeled bounded, and fallback or refusal stays explicit.

## Method

- Warmups per measured operation: 2
- Measured runs per operation: 10
- Live proof timing source: `crates/guild-runner/examples/live_proof_scenarios.rs benchmark mode`
- Admission/token/witness timing source: `docs/schemas/draft-v1 Python control-plane implementation`
- Live-runtime proof has no cache today. The draft M5 example cache is out of scope for this report.

## Supported Slices

| Slice | Family | Proof | Reduction | Narrowing | Proof-backed | Fallback | Refusal | Witness | Admission mean ms | Proof mean ms | Proof token mean ms | Fallback token mean ms | Refusal mean ms | Token verify mean ms | Witness gen mean ms | Witness verify mean ms |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| read-resource-immutable-guild-roots | read-resource | bounded_minimal | bounded | uri_prefix,resource_kind | 10 | 0 | 0 | proof_linked | 16.995 | 6785.507 | 21.111 | n/a | n/a | 31.977 | 81.070 | 126.434 |
| http-request-loopback-ip-get-explicit-port | http-request | bounded_minimal | bounded | none | 10 | 0 | 0 | proof_linked | 13.384 | 7362.278 | 21.793 | n/a | n/a | 34.393 | 84.675 | 133.471 |
| http-request-loopback-ip-get-default-port | http-request | bounded_minimal | bounded | none | 10 | 0 | 0 | proof_linked | 13.218 | 7366.571 | 23.936 | n/a | n/a | 32.580 | 81.646 | 126.266 |
| http-request-localhost-get-explicit-port | http-request | bounded_minimal | bounded | none | 10 | 0 | 0 | proof_linked | 14.352 | 7466.184 | 20.414 | n/a | n/a | 31.405 | 83.261 | 124.754 |
| http-request-localhost-head-explicit-port | http-request | bounded_minimal | bounded | none | 10 | 0 | 0 | proof_linked | 14.425 | 7371.429 | 21.198 | n/a | n/a | 31.852 | 83.985 | 127.120 |
| http-request-loopback-ip-head-explicit-port | http-request | bounded_minimal | bounded | none | 10 | 0 | 0 | proof_linked | 13.461 | 7416.143 | 20.014 | n/a | n/a | 31.001 | 80.792 | 123.043 |
| http-request-loopback-ip-head-default-port | http-request | bounded_minimal | bounded | none | 10 | 0 | 0 | proof_linked | 13.280 | 7329.966 | 21.361 | n/a | n/a | 31.076 | 88.061 | 127.760 |
| invoke-skill-single-child-zero-authority | invoke-skill | no_reduction | no_reduction | none | 10 | 0 | 0 | proof_linked | 12.972 | 10331.773 | 20.902 | n/a | n/a | 31.841 | 76.871 | 119.352 |
| log-write-observed-info-level | log-write | exact_minimal | exact | none | 0 | 0 | 0 | not_measured | 14.339 | 8976.147 | n/a | n/a | n/a | n/a | n/a | n/a |

## Unsupported Or Not Proven Slices

| Slice | Family | Proof | Reduction | Narrowing | Proof-backed | Fallback | Refusal | Witness | Admission mean ms | Proof mean ms | Proof token mean ms | Fallback token mean ms | Refusal mean ms | Token verify mean ms | Witness gen mean ms | Witness verify mean ms | Fail-closed reasons |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| http-request-redirect-driven-execution | http-request | not_proven | not_proven | none | 0 | 10 | 10 | unlinked | 13.598 | 3746.537 | n/a | 21.226 | 15.213 | 31.582 | 76.415 | 122.474 | HTTP_REDIRECTS_UNSUPPORTED |
| invoke-skill-multi-child-fan-out | invoke-skill | not_proven | not_proven | none | 0 | 10 | 10 | unlinked | 13.264 | 7683.914 | n/a | 19.973 | 15.335 | 31.087 | 74.267 | 112.136 | INVOKE_SKILL_MULTI_CHILD_UNSUPPORTED |
| emit-evidence-single-emission-replay-unavailable | emit-evidence | not_proven | not_proven | none | 0 | 10 | 10 | unlinked | 12.500 | 3000.557 | n/a | 20.267 | 14.674 | 31.106 | 81.842 | 124.294 | EMIT_EVIDENCE_REPLAY_UNAVAILABLE |

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
| http-request-no-replay-fixture | http-request | live_proof_search | HTTP_REPLAY_FIXTURE_REQUIRED | 3691.392 |
| read-resource-query-root-shrink-unsupported | read-resource | live_proof_search | LIVE_PROOF_BOUNDED,LIVE_SCOPE_SHRINK_UNSUPPORTED | 4344.146 |
| invoke-skill-child-authority-unsupported | invoke-skill | live_proof_search | INVOKE_SKILL_CHILD_AUTHORITY_UNSUPPORTED | 5877.281 |

## Notes

- The current checked real-path linked chain is `read-resource`, six bounded `http-request` slices, one bounded `invoke-skill` slice, and upper-bound fallback or unlinked witness behavior for the benchmarked unsupported slices.
- `log-write` is measured here through M4 plus M5 only. The repo has a real live proof slice for observed levels, but this benchmark does not claim a checked real-path M6 or M7 linkage slice for `log-write`.
- The measured reduction split is mixed by slice: `read-resource` really narrows from the admitted upper bound, the checked `http-request` and `invoke-skill` fixtures are already narrow enough that the proven authority does not shrink them further, and `log-write` is exact over an already narrow admitted level slice.
- The checked negative-claim probes are coverage-limited today. They come back as `not_provable` rather than synthetic successes or failures, and the matrix keeps that explicit per slice.
- The next real frontier is whichever unsupported rows you want to convert into bounded linked rows without broadening claims: `emit-evidence` exact sink or payload authority, broader `invoke-skill` shapes, and broader `http-request` hostname or replay coverage.
