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
| read-resource-immutable-guild-roots | read-resource | bounded_minimal | bounded | uri_prefix,resource_kind | 10 | 0 | 0 | proof_linked | 0.014 | 7016.869 | 0.021 | n/a | n/a | 0.302 | 0.285 | 0.184 |
| http-request-loopback-ip-get-explicit-port | http-request | bounded_minimal | bounded | none | 10 | 0 | 0 | proof_linked | 0.018 | 7384.317 | 0.028 | n/a | n/a | 0.390 | 0.292 | 0.220 |
| http-request-loopback-ip-get-default-port | http-request | bounded_minimal | bounded | none | 10 | 0 | 0 | proof_linked | 0.018 | 7322.058 | 0.046 | n/a | n/a | 0.321 | 0.362 | 0.221 |
| http-request-localhost-get-explicit-port | http-request | bounded_minimal | bounded | none | 10 | 0 | 0 | proof_linked | 0.022 | 7358.744 | 0.028 | n/a | n/a | 0.373 | 0.292 | 0.225 |
| http-request-localhost-get-default-port | http-request | bounded_minimal | bounded | none | 10 | 0 | 0 | proof_linked | 0.018 | 7447.842 | 0.034 | n/a | n/a | 0.322 | 0.351 | 0.232 |
| http-request-localhost-head-explicit-port | http-request | bounded_minimal | bounded | none | 10 | 0 | 0 | proof_linked | 0.020 | 7320.192 | 0.028 | n/a | n/a | 0.332 | 0.344 | 0.241 |
| http-request-localhost-head-default-port | http-request | bounded_minimal | bounded | none | 10 | 0 | 0 | proof_linked | 0.024 | 7637.809 | 0.028 | n/a | n/a | 0.389 | 0.302 | 0.248 |
| http-request-loopback-ip-head-explicit-port | http-request | bounded_minimal | bounded | none | 10 | 0 | 0 | proof_linked | 0.023 | 7448.314 | 0.028 | n/a | n/a | 0.329 | 0.430 | 0.274 |
| http-request-loopback-ip-head-default-port | http-request | bounded_minimal | bounded | none | 10 | 0 | 0 | proof_linked | 0.026 | 7548.674 | 0.027 | n/a | n/a | 0.394 | 0.297 | 0.226 |
| invoke-skill-single-child-zero-authority | invoke-skill | no_reduction | no_reduction | none | 10 | 0 | 0 | proof_linked | 0.017 | 10439.893 | 0.021 | n/a | n/a | 0.273 | 0.260 | 0.184 |
| invoke-skill-two-child-same-alias-zero-authority | invoke-skill | no_reduction | no_reduction | none | 10 | 0 | 0 | proof_linked | 0.013 | 15214.595 | 0.019 | n/a | n/a | 0.296 | 0.239 | 0.202 |
| emit-evidence-single-emission-exact-local-object-store | emit-evidence | exact_minimal | exact | none | 10 | 0 | 0 | proof_linked | 0.016 | 4851.549 | 0.022 | n/a | n/a | 0.338 | 0.257 | 0.195 |
| log-write-observed-info-level | log-write | exact_minimal | exact | none | 0 | 0 | 0 | not_measured | 0.013 | 9098.226 | n/a | n/a | n/a | n/a | n/a | n/a |

## Unsupported Or Not Proven Slices

| Slice | Family | Proof | Reduction | Narrowing | Proof-backed | Fallback | Refusal | Witness | Admission mean ms | Proof mean ms | Proof token mean ms | Fallback token mean ms | Refusal mean ms | Token verify mean ms | Witness gen mean ms | Witness verify mean ms | Fail-closed reasons |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| http-request-redirect-driven-execution | http-request | not_proven | not_proven | none | 0 | 10 | 10 | unlinked | 0.021 | 3691.515 | n/a | 0.027 | 4119.516 | 0.316 | 0.341 | 0.119 | HTTP_REDIRECTS_UNSUPPORTED |
| emit-evidence-single-emission-replay-unavailable | emit-evidence | not_proven | not_proven | none | 0 | 10 | 10 | unlinked | 0.019 | 2975.605 | n/a | 0.020 | 3518.764 | 0.293 | 0.245 | 0.119 | EMIT_EVIDENCE_REPLAY_UNAVAILABLE |

## Negative Claims

| Slice | Family | Success | Fail | Coverage limited | Unsupported raw |
| --- | --- | --- | --- | --- | --- |
| read-resource-immutable-guild-roots | read-resource | 0 | 0 | 3 | 0 |
| http-request-loopback-ip-get-explicit-port | http-request | 0 | 0 | 3 | 0 |
| http-request-loopback-ip-get-default-port | http-request | 0 | 0 | 3 | 0 |
| http-request-localhost-get-explicit-port | http-request | 0 | 0 | 3 | 0 |
| http-request-localhost-get-default-port | http-request | 0 | 0 | 3 | 0 |
| http-request-localhost-head-explicit-port | http-request | 0 | 0 | 3 | 0 |
| http-request-localhost-head-default-port | http-request | 0 | 0 | 3 | 0 |
| http-request-loopback-ip-head-explicit-port | http-request | 0 | 0 | 3 | 0 |
| http-request-loopback-ip-head-default-port | http-request | 0 | 0 | 3 | 0 |
| invoke-skill-single-child-zero-authority | invoke-skill | 0 | 0 | 3 | 0 |
| invoke-skill-two-child-same-alias-zero-authority | invoke-skill | 0 | 0 | 3 | 0 |
| http-request-redirect-driven-execution | http-request | 0 | 0 | 3 | 0 |
| emit-evidence-single-emission-exact-local-object-store | emit-evidence | 0 | 0 | 3 | 0 |
| emit-evidence-single-emission-replay-unavailable | emit-evidence | 0 | 0 | 3 | 0 |
| log-write-observed-info-level | log-write | 0 | 0 | 0 | 0 |

## Additional Fail-Closed Walls

| Wall | Family | Stage | Reasons | Proof mean ms |
| --- | --- | --- | --- | --- |
| http-request-no-replay-fixture | http-request | live_proof_search | HTTP_REPLAY_FIXTURE_REQUIRED | 3653.466 |
| read-resource-query-root-shrink-unsupported | read-resource | live_proof_search | LIVE_PROOF_BOUNDED,LIVE_SCOPE_SHRINK_UNSUPPORTED | 4190.073 |
| invoke-skill-child-authority-unsupported | invoke-skill | live_proof_search | INVOKE_SKILL_CHILD_AUTHORITY_UNSUPPORTED | 5884.496 |

## Notes

- The current checked real-path linked chain is `read-resource`, eight bounded `http-request` slices, two bounded `invoke-skill` slices, one exact single-emission `emit-evidence` slice with a carried host exact binding, and explicit upper-bound fallback or unlinked witness behavior for the benchmarked unsupported slices.
- `log-write` is still measured here through M4 plus M5 only. The repo has a real live proof slice for observed levels, but this benchmark does not claim a checked real-path M6 or M7 linkage slice for `log-write`.
- The measured reduction split is still mixed by slice: `read-resource` really narrows from the admitted upper bound, the checked `http-request` and `invoke-skill` fixtures are already narrow enough that the proven authority does not shrink them further, and `log-write` is exact over an already narrow admitted level slice.
- The checked negative-claim probes remain coverage-limited on the checked path. They stay `not_provable` rather than being rewritten into synthetic success or failure.
- The remaining frontier is still whichever unsupported rows you want to convert into bounded linked rows without broadening claims: broader `emit-evidence` shapes beyond the exact single-emission fixed-sink slice, broader `invoke-skill` shapes, and broader `http-request` hostname or replay coverage.
