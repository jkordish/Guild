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
| read-resource-immutable-guild-roots | read-resource | bounded_minimal | bounded | uri_prefix,resource_kind | 10 | 0 | 0 | proof_linked | 0.015 | 6899.103 | 0.022 | n/a | n/a | 0.292 | 0.266 | 0.184 |
| http-request-loopback-ip-get-explicit-port | http-request | bounded_minimal | bounded | none | 10 | 0 | 0 | proof_linked | 0.015 | 7408.547 | 0.031 | n/a | n/a | 0.372 | 0.371 | 0.224 |
| http-request-loopback-ip-get-default-port | http-request | bounded_minimal | bounded | none | 10 | 0 | 0 | proof_linked | 0.020 | 7480.698 | 0.029 | n/a | n/a | 0.338 | 0.290 | 0.227 |
| http-request-localhost-get-explicit-port | http-request | bounded_minimal | bounded | none | 10 | 0 | 0 | proof_linked | 0.019 | 7314.193 | 0.028 | n/a | n/a | 0.339 | 0.299 | 0.307 |
| http-request-localhost-get-default-port | http-request | bounded_minimal | bounded | none | 10 | 0 | 0 | proof_linked | 0.035 | 7505.201 | 0.032 | n/a | n/a | 0.332 | 0.352 | 0.243 |
| http-request-localhost-head-explicit-port | http-request | bounded_minimal | bounded | none | 10 | 0 | 0 | proof_linked | 0.018 | 7558.367 | 0.029 | n/a | n/a | 0.325 | 0.302 | 0.229 |
| http-request-localhost-head-default-port | http-request | bounded_minimal | bounded | none | 10 | 0 | 0 | proof_linked | 0.018 | 7448.939 | 0.029 | n/a | n/a | 0.333 | 0.355 | 0.243 |
| http-request-loopback-ip-head-explicit-port | http-request | bounded_minimal | bounded | none | 10 | 0 | 0 | proof_linked | 0.019 | 7635.972 | 0.037 | n/a | n/a | 0.324 | 0.313 | 0.221 |
| http-request-loopback-ip-head-default-port | http-request | bounded_minimal | bounded | none | 10 | 0 | 0 | proof_linked | 0.019 | 7430.322 | 0.029 | n/a | n/a | 0.328 | 0.301 | 0.221 |
| invoke-skill-single-child-zero-authority | invoke-skill | no_reduction | no_reduction | none | 10 | 0 | 0 | proof_linked | 0.013 | 10363.662 | 0.025 | n/a | n/a | 0.272 | 0.247 | 0.204 |
| invoke-skill-two-child-same-alias-zero-authority | invoke-skill | no_reduction | no_reduction | none | 10 | 0 | 0 | proof_linked | 0.012 | 15389.453 | 0.038 | n/a | n/a | 0.306 | 0.256 | 0.183 |
| log-write-observed-info-level | log-write | exact_minimal | exact | none | 0 | 0 | 0 | not_measured | 0.016 | 8934.525 | n/a | n/a | n/a | n/a | n/a | n/a |

## Unsupported Or Not Proven Slices

| Slice | Family | Proof | Reduction | Narrowing | Proof-backed | Fallback | Refusal | Witness | Admission mean ms | Proof mean ms | Proof token mean ms | Fallback token mean ms | Refusal mean ms | Token verify mean ms | Witness gen mean ms | Witness verify mean ms | Fail-closed reasons |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| http-request-redirect-driven-execution | http-request | not_proven | not_proven | none | 0 | 10 | 10 | unlinked | 0.021 | 3701.789 | n/a | 0.025 | 4199.318 | 0.322 | 0.299 | 0.122 | HTTP_REDIRECTS_UNSUPPORTED |
| emit-evidence-single-emission-replay-unavailable | emit-evidence | not_proven | not_proven | none | 0 | 10 | 10 | unlinked | 0.014 | 3024.378 | n/a | 0.023 | 3522.523 | 0.316 | 0.284 | 0.121 | EMIT_EVIDENCE_REPLAY_UNAVAILABLE |

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
| emit-evidence-single-emission-replay-unavailable | emit-evidence | 0 | 0 | 3 | 0 |
| log-write-observed-info-level | log-write | 0 | 0 | 0 | 0 |

## Additional Fail-Closed Walls

| Wall | Family | Stage | Reasons | Proof mean ms |
| --- | --- | --- | --- | --- |
| http-request-no-replay-fixture | http-request | live_proof_search | HTTP_REPLAY_FIXTURE_REQUIRED | 3742.373 |
| read-resource-query-root-shrink-unsupported | read-resource | live_proof_search | LIVE_PROOF_BOUNDED,LIVE_SCOPE_SHRINK_UNSUPPORTED | 4200.051 |
| invoke-skill-child-authority-unsupported | invoke-skill | live_proof_search | INVOKE_SKILL_CHILD_AUTHORITY_UNSUPPORTED | 5977.197 |

## Notes

- The current checked real-path linked chain is `read-resource`, eight bounded `http-request` slices, two bounded `invoke-skill` slices, and explicit upper-bound fallback or unlinked witness behavior for the benchmarked unsupported slices.
- `log-write` is still measured here through M4 plus M5 only. The repo has a real live proof slice for observed levels, but this benchmark does not claim a checked real-path M6 or M7 linkage slice for `log-write`.
- The measured reduction split is still mixed by slice: `read-resource` really narrows from the admitted upper bound, the checked `http-request` and `invoke-skill` fixtures are already narrow enough that the proven authority does not shrink them further, and `log-write` is exact over an already narrow admitted level slice.
- The checked negative-claim probes remain coverage-limited on the checked path. They stay `not_provable` rather than being rewritten into synthetic success or failure.
- The remaining frontier is still whichever unsupported rows you want to convert into bounded linked rows without broadening claims: `emit-evidence` exact sink or payload authority, broader `invoke-skill` shapes, and broader `http-request` hostname or replay coverage.
