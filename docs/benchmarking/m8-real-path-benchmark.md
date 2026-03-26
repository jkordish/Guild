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
| read-resource-immutable-guild-roots | read-resource | bounded_minimal | bounded | uri_prefix,resource_kind | 10 | 0 | 0 | proof_linked | 0.020 | 7005.052 | 0.022 | n/a | n/a | 0.351 | 0.268 | 0.191 |
| http-request-loopback-ip-get-explicit-port | http-request | bounded_minimal | bounded | none | 10 | 0 | 0 | proof_linked | 0.015 | 7514.163 | 0.049 | n/a | n/a | 0.328 | 0.352 | 0.223 |
| http-request-loopback-ip-get-default-port | http-request | bounded_minimal | bounded | none | 10 | 0 | 0 | proof_linked | 0.016 | 7485.099 | 0.029 | n/a | n/a | 0.339 | 0.294 | 0.267 |
| http-request-localhost-get-explicit-port | http-request | bounded_minimal | bounded | none | 10 | 0 | 0 | proof_linked | 0.015 | 7518.554 | 0.033 | n/a | n/a | 0.373 | 0.311 | 0.225 |
| http-request-localhost-get-default-port | http-request | bounded_minimal | bounded | none | 10 | 0 | 0 | proof_linked | 0.018 | 7463.798 | 0.028 | n/a | n/a | 0.333 | 0.305 | 0.270 |
| http-request-localhost-head-explicit-port | http-request | bounded_minimal | bounded | none | 10 | 0 | 0 | proof_linked | 0.018 | 7482.216 | 0.028 | n/a | n/a | 0.389 | 0.298 | 0.226 |
| http-request-localhost-head-default-port | http-request | bounded_minimal | bounded | none | 10 | 0 | 0 | proof_linked | 0.019 | 7458.403 | 0.028 | n/a | n/a | 0.341 | 0.297 | 0.236 |
| http-request-loopback-ip-head-explicit-port | http-request | bounded_minimal | bounded | none | 10 | 0 | 0 | proof_linked | 0.019 | 7501.686 | 0.027 | n/a | n/a | 0.327 | 0.292 | 0.223 |
| http-request-loopback-ip-head-default-port | http-request | bounded_minimal | bounded | none | 10 | 0 | 0 | proof_linked | 0.018 | 7385.771 | 0.029 | n/a | n/a | 0.351 | 0.306 | 0.224 |
| invoke-skill-single-child-zero-authority | invoke-skill | no_reduction | no_reduction | none | 10 | 0 | 0 | proof_linked | 0.013 | 10380.703 | 0.020 | n/a | n/a | 0.337 | 0.234 | 0.181 |
| invoke-skill-two-child-same-alias-zero-authority | invoke-skill | no_reduction | no_reduction | none | 10 | 0 | 0 | proof_linked | 0.012 | 15264.775 | 0.021 | n/a | n/a | 0.328 | 0.290 | 0.188 |
| log-write-observed-info-level | log-write | exact_minimal | exact | none | 0 | 0 | 0 | not_measured | 0.020 | 9064.789 | n/a | n/a | n/a | n/a | n/a | n/a |

## Unsupported Or Not Proven Slices

| Slice | Family | Proof | Reduction | Narrowing | Proof-backed | Fallback | Refusal | Witness | Admission mean ms | Proof mean ms | Proof token mean ms | Fallback token mean ms | Refusal mean ms | Token verify mean ms | Witness gen mean ms | Witness verify mean ms | Fail-closed reasons |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| http-request-redirect-driven-execution | http-request | not_proven | not_proven | none | 0 | 10 | 10 | unlinked | 0.026 | 3667.899 | n/a | 0.026 | 4187.642 | 0.328 | 0.295 | 0.118 | HTTP_REDIRECTS_UNSUPPORTED |
| emit-evidence-single-emission-replay-unavailable | emit-evidence | not_proven | not_proven | none | 0 | 10 | 10 | unlinked | 0.014 | 3069.193 | n/a | 0.023 | 3507.546 | 0.297 | 0.294 | 0.118 | EMIT_EVIDENCE_REPLAY_UNAVAILABLE |

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
| http-request-no-replay-fixture | http-request | live_proof_search | HTTP_REPLAY_FIXTURE_REQUIRED | 3676.879 |
| read-resource-query-root-shrink-unsupported | read-resource | live_proof_search | LIVE_PROOF_BOUNDED,LIVE_SCOPE_SHRINK_UNSUPPORTED | 4197.478 |
| invoke-skill-child-authority-unsupported | invoke-skill | live_proof_search | INVOKE_SKILL_CHILD_AUTHORITY_UNSUPPORTED | 5931.420 |

## Notes

- The current checked real-path linked chain is `read-resource`, eight bounded `http-request` slices, two bounded `invoke-skill` slices, and explicit upper-bound fallback or unlinked witness behavior for the benchmarked unsupported slices.
- `log-write` is still measured here through M4 plus M5 only. The repo has a real live proof slice for observed levels, but this benchmark does not claim a checked real-path M6 or M7 linkage slice for `log-write`.
- The measured reduction split is still mixed by slice: `read-resource` really narrows from the admitted upper bound, the checked `http-request` and `invoke-skill` fixtures are already narrow enough that the proven authority does not shrink them further, and `log-write` is exact over an already narrow admitted level slice.
- The checked negative-claim probes remain coverage-limited on the checked path. They stay `not_provable` rather than being rewritten into synthetic success or failure.
- The remaining frontier is still whichever unsupported rows you want to convert into bounded linked rows without broadening claims: `emit-evidence` exact sink or payload authority, broader `invoke-skill` shapes, and broader `http-request` hostname or replay coverage.
