use anyhow::{Result, bail};
use serde_json::json;

use crate::ArtifactMode;
use crate::benchmark;
use crate::compatibility;
use crate::schemas::{validate_examples, validate_instance};
use crate::support_matrix;
use crate::surface::{
    verify_active_runtime_example_alignment, verify_contract_surface_v1_spec_markers,
    verify_doc_truth_markers, verify_removed_truth_entrypoints,
};
use crate::util::{draft_v1_dir, read_json};

const EXAMPLES: &[(&str, &str)] = &[
    (
        "skill_contract.schema.json",
        "examples/local-log-analyzer.contract.json",
    ),
    (
        "skill_contract.schema.json",
        "examples/zero-authority.contract.json",
    ),
    (
        "skill_contract.schema.json",
        "examples/fetch-transform.contract.json",
    ),
    (
        "skill_contract.schema.json",
        "examples/cluster-rollout.contract.json",
    ),
    (
        "skill_contract.schema.json",
        "examples/runtime-http-read.contract.json",
    ),
    (
        "skill_contract.schema.json",
        "examples/runtime-http-read-default-port.contract.json",
    ),
    (
        "skill_contract.schema.json",
        "examples/runtime-http-localhost.contract.json",
    ),
    (
        "skill_contract.schema.json",
        "examples/runtime-http-localhost-default-port.contract.json",
    ),
    (
        "skill_contract.schema.json",
        "examples/runtime-http-localhost-head.contract.json",
    ),
    (
        "skill_contract.schema.json",
        "examples/runtime-http-localhost-head-default-port.contract.json",
    ),
    (
        "skill_contract.schema.json",
        "examples/runtime-http-head.contract.json",
    ),
    (
        "skill_contract.schema.json",
        "examples/runtime-http-head-default-port.contract.json",
    ),
    (
        "skill_contract.schema.json",
        "examples/runtime-http-redirect.contract.json",
    ),
    (
        "skill_contract.schema.json",
        "examples/runtime-read-resource.contract.json",
    ),
    (
        "skill_contract.schema.json",
        "examples/runtime-invoke-skill.contract.json",
    ),
    (
        "skill_contract.schema.json",
        "examples/runtime-emit-evidence-zero.contract.json",
    ),
    (
        "skill_contract.schema.json",
        "examples/runtime-emit-evidence-exact.contract.json",
    ),
    (
        "skill_contract.schema.json",
        "examples/runtime-log-write.contract.json",
    ),
    (
        "runtime_guarantee.schema.json",
        "examples/wasmtime-strict.runtime.json",
    ),
    (
        "runtime_guarantee.schema.json",
        "examples/node-wasi-basic.runtime.json",
    ),
    (
        "comparator_profile.schema.json",
        "examples/local-log-analyzer.canonical-json.comparator.json",
    ),
    (
        "comparator_profile.schema.json",
        "examples/local-log-analyzer.unavailable.comparator.json",
    ),
    (
        "comparator_profile.schema.json",
        "examples/fetch-transform.postconditions.comparator.json",
    ),
    (
        "comparator_profile.schema.json",
        "examples/fetch-transform.bounded.comparator.json",
    ),
    (
        "comparator_profile.schema.json",
        "examples/zero-authority.pure.comparator.json",
    ),
    (
        "comparator_profile.schema.json",
        "examples/runtime-http-read.unavailable.comparator.json",
    ),
    (
        "proof_record.schema.json",
        "examples/local-log-analyzer.proof.json",
    ),
    (
        "proof_record.schema.json",
        "examples/local-log-analyzer.cache-hit.proof.json",
    ),
    (
        "proof_record.schema.json",
        "examples/local-log-analyzer.comparator-unavailable.proof.json",
    ),
    (
        "proof_record.schema.json",
        "examples/fetch-transform.no-reduction.proof.json",
    ),
    (
        "proof_record.schema.json",
        "examples/fetch-transform.bounded.proof.json",
    ),
    (
        "proof_record.schema.json",
        "examples/zero-authority.proof.json",
    ),
    (
        "witness_record.schema.json",
        "examples/cluster-rollout.witness.json",
    ),
    (
        "witness_record.schema.json",
        "examples/local-log-analyzer.within-envelope.witness.json",
    ),
    (
        "witness_record.schema.json",
        "examples/local-log-analyzer.out-of-envelope.witness.json",
    ),
    (
        "witness_record.schema.json",
        "examples/fetch-transform.coverage-limited.witness.json",
    ),
    (
        "witness_record.schema.json",
        "examples/fetch-transform.redacted-claim-blocked.witness.json",
    ),
    (
        "witness_record.schema.json",
        "examples/fetch-transform.blocked-attempt.witness.json",
    ),
    (
        "witness_record.schema.json",
        "examples/zero-authority.witness.json",
    ),
    (
        "witness_record.schema.json",
        "examples/runtime-mapping-limited.witness.json",
    ),
    (
        "witness_record.schema.json",
        "examples/local-log-analyzer.runtime-mismatch.witness.json",
    ),
    (
        "admission_request.schema.json",
        "examples/zero-authority.admit.request.json",
    ),
    (
        "admission_request.schema.json",
        "examples/zero-authority.migrate.request.json",
    ),
    (
        "admission_request.schema.json",
        "examples/fetch-transform.downgrade.request.json",
    ),
    (
        "admission_request.schema.json",
        "examples/fetch-transform.no-reduction.request.json",
    ),
    (
        "admission_request.schema.json",
        "examples/local-log-analyzer.admit.request.json",
    ),
    (
        "admission_request.schema.json",
        "examples/cluster-rollout.refuse.request.json",
    ),
    (
        "admission_request.schema.json",
        "examples/cluster-rollout.admit.request.json",
    ),
    (
        "admission_request.schema.json",
        "examples/runtime-http-read.admit.request.json",
    ),
    (
        "admission_request.schema.json",
        "examples/runtime-http-read-default-port.admit.request.json",
    ),
    (
        "admission_request.schema.json",
        "examples/runtime-http-localhost.admit.request.json",
    ),
    (
        "admission_request.schema.json",
        "examples/runtime-http-localhost-default-port.admit.request.json",
    ),
    (
        "admission_request.schema.json",
        "examples/runtime-http-localhost-head.admit.request.json",
    ),
    (
        "admission_request.schema.json",
        "examples/runtime-http-localhost-head-default-port.admit.request.json",
    ),
    (
        "admission_request.schema.json",
        "examples/runtime-http-head.admit.request.json",
    ),
    (
        "admission_request.schema.json",
        "examples/runtime-http-head-default-port.admit.request.json",
    ),
    (
        "admission_request.schema.json",
        "examples/runtime-http-redirect.admit.request.json",
    ),
    (
        "admission_request.schema.json",
        "examples/runtime-read-resource.admit.request.json",
    ),
    (
        "admission_request.schema.json",
        "examples/runtime-invoke-skill.admit.request.json",
    ),
    (
        "admission_request.schema.json",
        "examples/runtime-emit-evidence-zero.admit.request.json",
    ),
    (
        "admission_request.schema.json",
        "examples/runtime-emit-evidence-exact.admit.request.json",
    ),
    (
        "admission_request.schema.json",
        "examples/runtime-log-write.admit.request.json",
    ),
    (
        "execution_plan.schema.json",
        "examples/zero-authority.admit.plan.json",
    ),
    (
        "execution_plan.schema.json",
        "examples/zero-authority.migrate.plan.json",
    ),
    (
        "execution_plan.schema.json",
        "examples/fetch-transform.downgrade.plan.json",
    ),
    (
        "execution_plan.schema.json",
        "examples/fetch-transform.no-reduction.plan.json",
    ),
    (
        "execution_plan.schema.json",
        "examples/local-log-analyzer.admit.plan.json",
    ),
    (
        "execution_plan.schema.json",
        "examples/cluster-rollout.refuse.plan.json",
    ),
    (
        "execution_plan.schema.json",
        "examples/cluster-rollout.admit.plan.json",
    ),
    (
        "delegated_capability_token.schema.json",
        "examples/local-log-analyzer.proof-backed.root-token.json",
    ),
    (
        "delegated_capability_token.schema.json",
        "examples/cluster-rollout.root-token.json",
    ),
    (
        "delegated_capability_token.schema.json",
        "examples/cluster-rollout.child-token.json",
    ),
    (
        "delegated_capability_token.schema.json",
        "examples/zero-authority.empty-token.json",
    ),
];

pub fn run(mode: ArtifactMode) -> Result<()> {
    verify_example_schemas()?;
    verify_invalid_runtime_probes()?;
    verify_active_runtime_example_alignment()?;
    verify_removed_truth_entrypoints()?;
    verify_doc_truth_markers()?;
    verify_contract_surface_v1_spec_markers()?;

    match mode {
        ArtifactMode::Check => {
            support_matrix::run(ArtifactMode::Check)?;
            compatibility::run(ArtifactMode::Check)?;
            benchmark::run(ArtifactMode::Check)?;
            println!("draft-v1 Rust-native truth checks completed cleanly.");
            Ok(())
        }
        ArtifactMode::Write => {
            support_matrix::run(ArtifactMode::Write)?;
            compatibility::run(ArtifactMode::Write)?;
            benchmark::run(ArtifactMode::Write)?;
            println!("draft-v1 Rust-native truth artifacts regenerated.");
            Ok(())
        }
    }
}

fn verify_example_schemas() -> Result<()> {
    let failures = validate_examples(EXAMPLES)?;
    if !failures.is_empty() {
        bail!(
            "draft-v1 example schema validation failed:\n - {}",
            failures.join("\n - ")
        );
    }
    Ok(())
}

fn verify_invalid_runtime_probes() -> Result<()> {
    let base_runtime = read_json(&draft_v1_dir().join("examples/wasmtime-strict.runtime.json"))?;
    let mut missing_granularity = base_runtime.clone();
    missing_granularity
        .as_object_mut()
        .expect("runtime object")
        .remove("network_policy_granularity");
    let missing_errors = validate_instance("runtime_guarantee.schema.json", &missing_granularity)?;
    if missing_errors.is_empty() {
        bail!(
            "negative probe failed: omitted runtime network_policy_granularity unexpectedly passed schema validation"
        );
    }

    let mut unknown_granularity = base_runtime;
    unknown_granularity
        .as_object_mut()
        .expect("runtime object")
        .insert("network_policy_granularity".to_owned(), json!("super-url"));
    let unknown_errors = validate_instance("runtime_guarantee.schema.json", &unknown_granularity)?;
    if unknown_errors.is_empty() {
        bail!(
            "negative probe failed: unknown runtime network_policy_granularity unexpectedly passed schema validation"
        );
    }
    Ok(())
}
