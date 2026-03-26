# M9 Claim-To-Evidence Map

This map is meant to let a reviewer trace:

`claim concept -> measured artifact -> validating test -> scenario/example -> codepath`

The measured source set is [`benchmark_matrix.json`](../schemas/draft-v1/benchmark_matrix.json), [`m8-real-path-benchmark.md`](../benchmarking/m8-real-path-benchmark.md), [`family_support_matrix.json`](../schemas/draft-v1/family_support_matrix.json), [`SPECS.md`](../../SPECS.md), and the live-proof runner code under [`crates/guild-runner/`](../../crates/guild-runner/).

## c1-primary-method-fail-closed-admission-bounded-proof-linkage

- Benchmark evidence:
  - `benchmark_matrix.json.slices[read-resource-immutable-guild-roots]`
  - `benchmark_matrix.json.slices[http-request-loopback-ip-get-explicit-port]`
  - `benchmark_matrix.json.slices[http-request-loopback-ip-get-default-port]`
  - `benchmark_matrix.json.slices[http-request-localhost-get-explicit-port]`
  - `benchmark_matrix.json.slices[http-request-localhost-get-default-port]`
  - `benchmark_matrix.json.slices[http-request-localhost-head-explicit-port]`
  - `benchmark_matrix.json.slices[http-request-localhost-head-default-port]`
  - `benchmark_matrix.json.slices[http-request-loopback-ip-head-explicit-port]`
  - `benchmark_matrix.json.slices[http-request-loopback-ip-head-default-port]`
  - `benchmark_matrix.json.slices[invoke-skill-single-child-zero-authority]`
  - `benchmark_matrix.json.slices[http-request-redirect-driven-execution]`
  - `benchmark_matrix.json.slices[invoke-skill-multi-child-fan-out]`
  - `benchmark_matrix.json.slices[emit-evidence-single-emission-replay-unavailable]`
  - `benchmark_matrix.json.checked_fail_closed_walls[http-request-no-replay-fixture]`
  - `benchmark_matrix.json.checked_fail_closed_walls[read-resource-query-root-shrink-unsupported]`
  - `benchmark_matrix.json.checked_fail_closed_walls[invoke-skill-child-authority-unsupported]`
- Support-matrix evidence:
  - `family_support_matrix.json.families.read-resource.layers.live_minimization_proof`
  - `family_support_matrix.json.families.http-request.layers.live_minimization_proof`
  - `family_support_matrix.json.families.invoke-skill.layers.live_minimization_proof`
  - `family_support_matrix.json.families.emit-evidence.layers.live_minimization_proof`
  - `family_support_matrix.json.families.log-write.layers.live_minimization_proof`
- Validating tests:
  - `crates/guild-runner/tests/live_proofs.rs::read_resource_live_proof_is_bounded_and_live_linkable`
  - `crates/guild-runner/tests/live_proofs.rs::http_request_live_proof_is_bounded_with_replay`
  - `crates/guild-runner/tests/live_proofs.rs::http_request_live_proof_is_bounded_with_replay_for_default_port_shape`
  - `crates/guild-runner/tests/live_proofs.rs::http_request_live_proof_is_bounded_with_replay_for_localhost_explicit_port_shape`
  - `crates/guild-runner/tests/live_proofs.rs::http_request_live_proof_is_bounded_with_replay_for_localhost_default_port_shape`
  - `crates/guild-runner/tests/live_proofs.rs::http_request_live_proof_is_bounded_with_replay_for_localhost_head_explicit_port_shape`
  - `crates/guild-runner/tests/live_proofs.rs::http_request_live_proof_is_bounded_with_replay_for_localhost_head_default_port_shape`
  - `crates/guild-runner/tests/live_proofs.rs::http_request_live_proof_is_bounded_with_replay_for_head_shape`
  - `crates/guild-runner/tests/live_proofs.rs::http_request_live_proof_is_bounded_with_replay_for_head_default_port_shape`
  - `crates/guild-runner/tests/live_proofs.rs::invoke_skill_live_proof_is_bounded_for_single_zero_authority_child`
  - `crates/guild-runner/tests/live_proofs.rs::http_request_live_proof_stays_not_proven_for_redirect_shape`
  - `crates/guild-runner/tests/live_proofs.rs::invoke_skill_live_proof_stays_not_proven_for_multi_child_shape`
  - `crates/guild-runner/tests/live_proofs.rs::emit_evidence_live_proof_stays_not_proven_for_single_sink_replay_unavailable`
- Scenario and example anchors:
  - `crates/guild-runner/examples/live_proof_scenarios.rs` scenarios `read-resource-bounded`, `http-request-bounded`, `http-request-default-port-bounded`, `http-request-localhost-bounded`, `http-request-localhost-default-port-bounded`, `http-request-localhost-head-bounded`, `http-request-localhost-head-default-port-bounded`, `http-request-head-bounded`, `http-request-head-default-port-bounded`, `invoke-skill-single-child-bounded`, `http-request-redirect-unsupported`, `invoke-skill-multi-child-unsupported`, `emit-evidence-single-sink-replay-unavailable`
- Codepaths:
  - `crates/guild-runner/src/live_proof.rs`
  - `crates/guild-draft-truth/src/benchmark.rs`
  - `crates/guild-draft-truth/src/support_matrix.rs`

## c2-family-specific-bounded-proof-slices

- Benchmark evidence:
  - `benchmark_matrix.json.slices[read-resource-immutable-guild-roots]`
  - `benchmark_matrix.json.slices[http-request-loopback-ip-get-explicit-port]`
  - `benchmark_matrix.json.slices[http-request-loopback-ip-get-default-port]`
  - `benchmark_matrix.json.slices[http-request-localhost-get-explicit-port]`
  - `benchmark_matrix.json.slices[http-request-localhost-get-default-port]`
  - `benchmark_matrix.json.slices[http-request-localhost-head-explicit-port]`
  - `benchmark_matrix.json.slices[http-request-localhost-head-default-port]`
  - `benchmark_matrix.json.slices[http-request-loopback-ip-head-explicit-port]`
  - `benchmark_matrix.json.slices[http-request-loopback-ip-head-default-port]`
  - `benchmark_matrix.json.slices[invoke-skill-single-child-zero-authority]`
  - `benchmark_matrix.json.slices[log-write-observed-info-level]`
- Support-matrix evidence:
  - `family_support_matrix.json.families.read-resource.proven_slices[immutable-guild-execution-object-record-roots]`
  - `family_support_matrix.json.families.http-request.proven_slices[loopback-ip-get-explicit-port]`
  - `family_support_matrix.json.families.http-request.proven_slices[loopback-ip-get-default-port]`
  - `family_support_matrix.json.families.http-request.proven_slices[localhost-get-explicit-port]`
  - `family_support_matrix.json.families.http-request.proven_slices[localhost-get-default-port]`
  - `family_support_matrix.json.families.http-request.proven_slices[localhost-head-explicit-port]`
  - `family_support_matrix.json.families.http-request.proven_slices[localhost-head-default-port]`
  - `family_support_matrix.json.families.http-request.proven_slices[loopback-ip-head-explicit-port]`
  - `family_support_matrix.json.families.http-request.proven_slices[loopback-ip-head-default-port]`
  - `family_support_matrix.json.families.invoke-skill.proven_slices[single-child-zero-authority-inspect-child]`
  - `family_support_matrix.json.families.log-write.proven_slices[observed-discrete-levels]`
- Validating tests:
  - `crates/guild-runner/tests/live_proofs.rs`
  - `crates/guild-runner/tests/http_requests.rs`
  - `crates/guild-runner/tests/composition.rs::single_child_invoke_fixture_persists_exact_child_digest_binding`

## c3-deterministic-replay-or-comparator-basis

- Benchmark evidence:
  - `benchmark_matrix.json.checked_fail_closed_walls[http-request-no-replay-fixture]`
  - `benchmark_matrix.json.checked_fail_closed_walls[read-resource-query-root-shrink-unsupported]`
  - `benchmark_matrix.json.checked_fail_closed_walls[invoke-skill-child-authority-unsupported]`
- Support-matrix evidence:
  - `family_support_matrix.json.families.http-request.layers.live_minimization_proof`
  - `family_support_matrix.json.families.invoke-skill.proven_slices[single-child-zero-authority-inspect-child]`
  - `family_support_matrix.json.families.emit-evidence.not_proven_shapes[single-emission-local-object-store-replay-mismatch]`
- Validating tests:
  - `crates/guild-runner/tests/live_proofs.rs::http_request_live_proof_stays_not_proven_without_replay`
  - `crates/guild-runner/tests/live_proofs.rs::http_request_live_proof_stays_not_proven_for_unsupported_comparator`
  - `crates/guild-runner/tests/live_proofs.rs::invoke_skill_live_proof_stays_not_proven_for_child_authority`
  - `crates/guild-runner/tests/live_proofs.rs::invoke_skill_live_proof_stays_not_proven_for_unsupported_comparator`
  - `crates/guild-runner/tests/live_proofs.rs::emit_evidence_live_proof_stays_not_proven_for_single_sink_replay_unavailable`
- Scenario and example anchors:
  - `crates/guild-runner/examples/live_proof_scenarios.rs` scenarios `http-request-no-replay`, `read-resource-query-unsupported`, `invoke-skill-child-authority-unsupported`, `emit-evidence-single-sink-replay-unavailable`
- Codepaths:
  - `crates/guild-runner/src/live_proof.rs`

## c4-proof-backed-token-issuance-versus-explicit-upper-bound-fallback

- Benchmark evidence:
  - `benchmark_matrix.json.questions.issuance_modes`
  - `benchmark_matrix.json.slices[http-request-redirect-driven-execution]`
  - `benchmark_matrix.json.slices[invoke-skill-multi-child-fan-out]`
  - `benchmark_matrix.json.slices[emit-evidence-single-emission-replay-unavailable]`
- Support-matrix evidence:
  - `family_support_matrix.json.families.read-resource.layers.plan_proof_token_linkage`
  - `family_support_matrix.json.families.http-request.layers.plan_proof_token_linkage`
  - `family_support_matrix.json.families.invoke-skill.layers.plan_proof_token_linkage`
  - `family_support_matrix.json.families.emit-evidence.layers.plan_proof_token_linkage`
  - `family_support_matrix.json.families.log-write.layers.plan_proof_token_linkage`
- Validating tests:
  - `crates/guild-runner/tests/live_proofs.rs::http_request_live_proof_stays_not_proven_for_redirect_shape`
  - `crates/guild-runner/tests/live_proofs.rs::invoke_skill_live_proof_stays_not_proven_for_multi_child_shape`
  - `crates/guild-runner/tests/live_proofs.rs::emit_evidence_live_proof_stays_not_proven_for_single_sink_replay_unavailable`
- Codepaths:
  - `crates/guild-draft-truth/src/benchmark.rs`
  - `docs/schemas/draft-v1/README.md`

## c5-proof-linked-witness-generation-versus-unlinked-witnesses

- Benchmark evidence:
  - `benchmark_matrix.json.slices[*].witness_linkage_status`
  - `benchmark_matrix.json.questions.negative_claims`
- Support-matrix evidence:
  - `family_support_matrix.json.families.read-resource.layers.proof_witness_linkage`
  - `family_support_matrix.json.families.http-request.layers.proof_witness_linkage`
  - `family_support_matrix.json.families.invoke-skill.layers.proof_witness_linkage`
  - `family_support_matrix.json.families.emit-evidence.layers.proof_witness_linkage`
  - `family_support_matrix.json.families.log-write.layers.proof_witness_linkage`
- Validating tests:
  - `crates/guild-runner/tests/inspect_slice.rs::emitted_evidence_is_deduped_by_digest_and_resources_are_readable`
  - `crates/guild-runner/tests/inspect_slice.rs::emit_evidence_denials_are_host_owned_rejections`
  - `crates/guild-runner/tests/http_requests.rs::localhost_happy_path_records_resolution_binding`
  - `crates/guild-runner/tests/composition.rs::composite_skill_invokes_child_and_records_host_owned_metadata`
- Codepaths:
  - `crates/guild-runner/src/live_proof.rs`
  - `docs/schemas/draft-v1/README.md`

## c6-explicit-fail-closed-unsupported-walls-and-reason-codes

- Benchmark evidence:
  - `benchmark_matrix.json.questions.fail_closed_walls`
  - `benchmark_matrix.json.checked_fail_closed_walls[*]`
- Support-matrix evidence:
  - `family_support_matrix.json.families.emit-evidence.not_proven_shapes[*]`
  - `family_support_matrix.json.families.invoke-skill.not_proven_shapes[*]`
  - `family_support_matrix.json.families.read-resource.not_proven_shapes[execution-query-shrink]`
- Validating tests:
  - `crates/guild-runner/tests/http_requests.rs::redirect_is_denied_when_following_is_not_granted`
  - `crates/guild-runner/tests/http_requests.rs::redirect_target_must_still_be_granted`
  - `crates/guild-runner/tests/composition.rs::child_capabilities_must_be_satisfied_by_parent_grants`
  - `crates/guild-runner/tests/composition.rs::unsupported_capability_grants_are_rejected_before_execution`
  - `crates/guild-runner/tests/inspect_slice.rs::unsupported_manifest_capabilities_are_rejected_before_execution`
  - `crates/guild-runner/tests/inspect_slice.rs::filesystem_grants_are_rejected_before_execution`
- Codepaths:
  - `crates/guild-runner/src/live_proof.rs`
  - `crates/guild-runner/tests/http_requests.rs`
  - `crates/guild-runner/tests/composition.rs`

## c7-machine-readable-support-frontier-and-benchmark-surface

- Measured artifacts:
  - `docs/schemas/draft-v1/family_support_matrix.json`
  - `docs/schemas/draft-v1/benchmark_matrix.json`
  - `docs/benchmarking/m8-real-path-benchmark.md`
- Generator and checker codepaths:
  - `crates/guild-draft-truth/src/support_matrix.rs`
  - `crates/guild-draft-truth/src/benchmark.rs`
  - `xtask/src/main.rs`

## c8-proof-only-log-write-slice-without-real-path-downstream-linkage

- Benchmark evidence:
  - `benchmark_matrix.json.slices[log-write-observed-info-level]`
- Support-matrix evidence:
  - `family_support_matrix.json.families.log-write.proven_slices[observed-discrete-levels]`
  - `family_support_matrix.json.families.log-write.layers.plan_proof_token_linkage`
  - `family_support_matrix.json.families.log-write.layers.proof_witness_linkage`
- Validating tests:
  - `crates/guild-runner/tests/live_proofs.rs::log_write_live_proof_reduces_to_observed_levels_and_leaves_emit_evidence_residual`
- Scenario and example anchors:
  - `crates/guild-runner/examples/live_proof_scenarios.rs` scenario `log-write-reduced`
- Codepaths:
  - `crates/guild-runner/src/live_proof.rs`

## c9-real-path-log-write-downstream-linkage

- Boundary evidence:
  - `benchmark_matrix.json.slices[log-write-observed-info-level].token_linkage_status = not_measured_on_real_path`
  - `benchmark_matrix.json.slices[log-write-observed-info-level].witness_linkage_status = not_measured_on_real_path`
  - `family_support_matrix.json.families.log-write.layers.token_issuance_basis = supported`
  - `family_support_matrix.json.families.log-write.layers.witness_generation = supported`
- Drafting consequence:
  - the surrounding control-plane pieces exist
  - the checked benchmark still does not justify a present-tense real-path linkage claim

## c10-not-claimable-yet-surfaces

- Boundary evidence:
  - `family_support_matrix.json.families.emit-evidence.layers.live_minimization_proof = not_proven`
  - `family_support_matrix.json.families.emit-evidence.layers.plan_proof_token_linkage = not_proven`
  - `family_support_matrix.json.families.emit-evidence.layers.proof_witness_linkage = not_proven`
  - `family_support_matrix.json.families.invoke-skill.not_proven_shapes[*]`
  - `family_support_matrix.json.families.http-request.not_proven_shapes[*]`
- Validating tests:
  - `crates/guild-runner/tests/live_proofs.rs::emit_evidence_live_proof_stays_not_proven_for_single_sink_replay_unavailable`
  - `crates/guild-runner/tests/live_proofs.rs::invoke_skill_live_proof_stays_not_proven_for_multi_child_shape`
  - `crates/guild-runner/tests/live_proofs.rs::invoke_skill_live_proof_stays_not_proven_for_child_authority`
  - `crates/guild-runner/tests/live_proofs.rs::http_request_live_proof_stays_not_proven_for_redirect_shape`
  - `crates/guild-runner/tests/live_proofs.rs::http_request_live_proof_stays_not_proven_without_replay`

## Packet Integration

The claim ladder is in [m9-claim-ladder.md](./m9-claim-ladder.md). The exclusions memo is in [m9-non-claims.md](./m9-non-claims.md). The figure source set is in [m9-figures.md](./m9-figures.md). The machine-readable packet manifest that ties these references together is [m9-packet-manifest.json](./m9-packet-manifest.json).
