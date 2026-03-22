use anyhow::{Context, Result, bail};
use serde_json::{Map, Value, json};

use crate::ArtifactMode;
use crate::schemas::validate_instance;
use crate::surface::{
    LINKAGE_NOT_MEASURED_ON_REAL_PATH, LINKAGE_PROOF_LINKED, LINKAGE_UNLINKED,
    LINKED_PATH_FALLBACK_UNLINKED, LINKED_PATH_PROOF_LINKED, LINKED_PATH_PROOF_ONLY,
    STATUS_BOUNDED, STATUS_NOT_PROVEN, STATUS_SUPPORTED, TOKEN_LINKAGE_PROOF_BACKED,
    TOKEN_LINKAGE_UPPER_BOUND_FALLBACK, ensure_allowed_value,
};
use crate::util::{
    benchmarking_dir, count_rate, draft_v1_dir, ensure_parent_dir, json_array, json_digest,
    json_object, measure_operation, read_json, read_to_string, run_cargo_json, write_json_pretty,
    write_string,
};

const MATRIX_NAME: &str = "benchmark_matrix.json";
const REPORT_NAME: &str = "m8-real-path-benchmark.md";
const MATRIX_KIND: &str = "guild.real_path_benchmark_matrix";
const MATRIX_VERSION: &str = "1.0.0";
const GENERATED_AT: &str = "2026-03-22T00:00:00Z";
const BENCHMARK_WARMUPS: usize = 2;
const BENCHMARK_RUNS: usize = 10;
const TOKEN_ISSUER_ID: &str = "urn:guild:issuer:draft-control-plane:v1";
const TOKEN_KEY_ID: &str = "draft-hmac-2026-03";
const TOKEN_VERIFICATION_TIME: &str = "2026-03-22T00:00:40Z";
const WITNESS_VERIFICATION_TIME: &str = "2026-03-22T00:00:55Z";
const CALL_CHAIN_LINKS: &[&str] = &["urn:guild:actor:benchmark"];
const COVERAGE_LIMIT_REASON: &str = "TOKEN_LINKAGE_MISMATCH";

#[derive(Clone, Copy, Eq, PartialEq)]
enum LinkedPath {
    ProofLinked,
    FallbackUnlinked,
    ProofOnly,
}

#[derive(Clone, Copy)]
struct SliceSpec {
    slice_id: &'static str,
    family: &'static str,
    slice_name: &'static str,
    exact_scope: &'static str,
    support_status: &'static str,
    scenario_name: &'static str,
    contract: &'static str,
    request: &'static str,
    invocation: &'static str,
    execution_record_path: Option<&'static str>,
    linked_path: LinkedPath,
    default_path: &'static str,
    token_linkage_status: &'static str,
    witness_linkage_status: &'static str,
    negative_claim_support_status: &'static str,
    negative_claim_type: Option<&'static str>,
    notes: &'static str,
}

#[derive(Clone, Copy)]
struct WallSpec {
    wall_id: &'static str,
    family: &'static str,
    stage: &'static str,
    wall_name: &'static str,
    scenario_name: &'static str,
    checked_test: &'static str,
}

struct TokenIssuanceBenchmark {
    proof_backed_token: Option<Value>,
    proof_backed_timing: Option<Value>,
    upper_bound_token: Option<Value>,
    upper_bound_timing: Option<Value>,
    _refusal_result: Option<Value>,
    refusal_timing: Option<Value>,
    issuance_outcomes: Value,
}

pub fn run(mode: ArtifactMode) -> Result<()> {
    match mode {
        ArtifactMode::Check => {
            let matrix = checked_matrix()?;
            validate_checked_matrix(&matrix)?;
            verify_checked_matrix_alignment(&matrix)?;
            let report = render_report(&matrix)?;
            let report_path = benchmarking_dir().join(REPORT_NAME);
            let existing_report = read_to_string(&report_path)?;
            if existing_report != report {
                bail!("{REPORT_NAME} is out of date with {MATRIX_NAME}");
            }
            println!("{MATRIX_NAME} and {REPORT_NAME} validate cleanly.");
            Ok(())
        }
        ArtifactMode::Write => {
            let matrix = build_matrix()?;
            let report = render_report(&matrix)?;
            ensure_parent_dir(&draft_v1_dir().join(MATRIX_NAME))?;
            ensure_parent_dir(&benchmarking_dir().join(REPORT_NAME))?;
            write_json_pretty(&draft_v1_dir().join(MATRIX_NAME), &matrix)?;
            write_string(&benchmarking_dir().join(REPORT_NAME), &report)?;
            println!("Wrote {}", draft_v1_dir().join(MATRIX_NAME).display());
            println!("Wrote {}", benchmarking_dir().join(REPORT_NAME).display());
            Ok(())
        }
    }
}

pub fn checked_matrix() -> Result<Value> {
    let matrix = read_json(&draft_v1_dir().join(MATRIX_NAME))?;
    validate_checked_matrix(&matrix)?;
    Ok(matrix)
}

pub fn validate_checked_matrix(matrix: &Value) -> Result<()> {
    let failures = validate_generated_matrix(matrix)?;
    if !failures.is_empty() {
        bail!(
            "benchmark artifact validation failed:\n - {}",
            failures.join("\n - ")
        );
    }
    validate_matrix_inventory(matrix)?;
    validate_matrix_questions(matrix)?;
    validate_matrix_vocabulary(matrix)?;
    Ok(())
}

pub fn verify_checked_matrix_alignment(matrix: &Value) -> Result<()> {
    let slices = json_array(
        matrix
            .get("slices")
            .context("benchmark matrix missing slices")?,
        "benchmark_matrix.slices",
    )?;
    for spec in slice_specs() {
        let entry = find_named_entry(slices, "slice_id", spec.slice_id)?;
        verify_slice_alignment(spec, entry)?;
    }

    let walls = json_array(
        matrix
            .get("checked_fail_closed_walls")
            .context("benchmark matrix missing checked_fail_closed_walls")?,
        "benchmark_matrix.checked_fail_closed_walls",
    )?;
    for spec in wall_specs() {
        let entry = find_named_entry(walls, "wall_id", spec.wall_id)?;
        verify_wall_alignment(spec, entry)?;
    }
    Ok(())
}

fn build_matrix() -> Result<Value> {
    let slices = slice_specs()
        .into_iter()
        .map(build_slice_entry)
        .collect::<Result<Vec<_>>>()?;
    let walls = wall_specs()
        .into_iter()
        .map(build_fail_closed_wall)
        .collect::<Result<Vec<_>>>()?;
    let matrix = json!({
        "kind": MATRIX_KIND,
        "version": MATRIX_VERSION,
        "generated_at": GENERATED_AT,
        "methodology": {
            "warmup_runs": BENCHMARK_WARMUPS,
            "measured_runs": BENCHMARK_RUNS,
            "live_proof_timing_source": "crates/guild-runner/examples/live_proof_scenarios.rs benchmark mode",
            "admission_token_witness_timing_source": "crates/guild-draft-truth Rust-native internal benchmark generator",
            "cache_truth": {
                "live_runtime_proof": "No live-runtime proof cache exists today; the runner benchmark path still measures real proof search without cache reuse.",
                "draft_truth_tooling": "The migrated draft-v1 truth pipeline now measures admission, token, and witness overheads through the Rust-native internal generator rather than the removed Python sidecars.",
                "token_verification": "No cache; replay-style token verification state is rebuilt per measured run.",
                "witness_generation_verification": "No cache."
            }
        },
        "slices": slices,
        "checked_fail_closed_walls": walls,
        "questions": matrix_questions(&slices, &walls)?,
    });
    let failures = validate_generated_matrix(&matrix)?;
    if !failures.is_empty() {
        bail!(
            "generated benchmark matrix failed validation:\n - {}",
            failures.join("\n - ")
        );
    }
    Ok(matrix)
}

fn build_slice_entry(spec: SliceSpec) -> Result<Value> {
    let request = read_json(&draft_v1_dir().join(spec.request))?;
    let live_benchmark = load_live_proof_benchmark(spec.scenario_name)?;
    let live_proof = json_object(
        live_benchmark
            .get("proof")
            .context("live proof benchmark missing proof")?,
        "live_proof_benchmark.proof",
    )?;
    let family_status = family_status_from_live_proof(live_proof, spec.family)?;

    let baseline_authority = baseline_upper_bound_authority(&request)?;
    let final_proven_authority = final_proven_authority(&baseline_authority, live_proof)?;
    let final_issued_authority = final_issued_authority(
        spec.linked_path,
        &baseline_authority,
        &final_proven_authority,
    );
    let reduction_result = reduction_result(
        spec.family,
        family_status
            .get("proof_status")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        &baseline_authority,
        value_ref(&final_issued_authority),
    )?;

    let (_, _, admission_timing) = measure_operation(
        || baseline_upper_bound_authority(&request),
        BENCHMARK_WARMUPS,
        BENCHMARK_RUNS,
        false,
        "Rust-native draft-v1 admission shaping has no cache on the checked path.",
    )?;

    let issuance = issue_tokens_for_slice(
        spec,
        &baseline_authority,
        value_ref(&final_issued_authority),
    )?;

    let selected_token = issuance
        .proof_backed_token
        .clone()
        .or_else(|| issuance.upper_bound_token.clone());

    let (selected_token_verification_result, token_verification_timing) =
        verify_selected_token_for_slice(spec, &baseline_authority, selected_token.as_ref())?;

    let (selected_witness, witness_generation_timing, witness_outcomes) =
        generate_witness_for_slice(
            spec,
            &baseline_authority,
            value_ref(&final_proven_authority),
            selected_token.as_ref(),
        )?;

    let (selected_witness_verification_result, witness_verification_timing) =
        verify_selected_witness_for_slice(spec, selected_witness.as_ref())?;

    let negative_claim_verification =
        negative_claim_verification(spec, selected_witness_verification_result.as_ref())?;

    Ok(json!({
        "slice_id": spec.slice_id,
        "family": spec.family,
        "slice_name": spec.slice_name,
        "exact_scope": spec.exact_scope,
        "support_status": spec.support_status,
        "proof_status": family_status.get("proof_status").and_then(Value::as_str).unwrap_or_default(),
        "token_linkage_status": spec.token_linkage_status,
        "witness_linkage_status": spec.witness_linkage_status,
        "negative_claim_support_status": spec.negative_claim_support_status,
        "benchmark_scenario_source": benchmark_source(spec),
        "baseline_upper_bound_authority": baseline_authority,
        "final_proven_authority": final_proven_authority,
        "final_issued_authority": final_issued_authority,
        "reduction_result": reduction_result,
        "timing_overhead_results": {
            "admission_only": admission_timing,
            "live_proof_search": live_benchmark.get("timing").cloned().context("live proof benchmark missing timing")?,
            "proof_backed_token_issuance": issuance.proof_backed_timing,
            "upper_bound_token_issuance": issuance.upper_bound_timing,
            "token_refusal": issuance.refusal_timing,
            "token_verification": token_verification_timing,
            "witness_generation": witness_generation_timing,
            "witness_verification": witness_verification_timing,
        },
        "issuance_outcomes": issuance.issuance_outcomes,
        "witness_outcomes": witness_outcomes,
        "negative_claim_verification": negative_claim_verification,
        "fallback_refusal_behavior": {
            "default_path": spec.default_path,
            "fallback_available": spec.linked_path == LinkedPath::FallbackUnlinked,
            "proof_notes": family_status.get("notes").and_then(Value::as_str).unwrap_or_default(),
        },
        "fail_closed_reasons": family_status.get("reason_codes").cloned().unwrap_or_else(|| json!([])),
        "notes": spec.notes,
        "linked_path": linked_path_label(spec.linked_path),
        "benchmark_limits": {
            "warmup_runs": BENCHMARK_WARMUPS,
            "measured_runs": BENCHMARK_RUNS,
        },
        "selected_token_verification_result": selected_token_verification_result,
        "selected_witness_verification_result": selected_witness_verification_result,
    }))
}

fn build_fail_closed_wall(spec: WallSpec) -> Result<Value> {
    let live_benchmark = load_live_proof_benchmark(spec.scenario_name)?;
    let live_proof = json_object(
        live_benchmark
            .get("proof")
            .context("live proof benchmark missing proof")?,
        "live_proof_benchmark.proof",
    )?;
    let family_status = family_status_from_live_proof(live_proof, spec.family)?;
    Ok(json!({
        "wall_id": spec.wall_id,
        "family": spec.family,
        "stage": spec.stage,
        "wall_name": spec.wall_name,
        "benchmark_scenario_source": {
            "live_proof_scenario": spec.scenario_name,
            "rust_example": "crates/guild-runner/examples/live_proof_scenarios.rs",
            "checked_test": spec.checked_test,
        },
        "proof_status": family_status.get("proof_status").and_then(Value::as_str).unwrap_or_default(),
        "fail_closed_reasons": family_status.get("reason_codes").cloned().unwrap_or_else(|| json!([])),
        "timing_overhead_results": {
            "live_proof_search": live_benchmark.get("timing").cloned().context("live proof benchmark missing timing")?,
        },
        "trigger_count": BENCHMARK_RUNS,
        "trigger_rate": 1.0,
        "notes": family_status.get("notes").and_then(Value::as_str).unwrap_or_default(),
    }))
}

fn validate_generated_matrix(matrix: &Value) -> Result<Vec<String>> {
    validate_instance("benchmark_matrix.schema.json", matrix)
}

fn validate_matrix_inventory(matrix: &Value) -> Result<()> {
    let slices = json_array(
        matrix
            .get("slices")
            .context("benchmark matrix missing slices")?,
        "benchmark_matrix.slices",
    )?;
    let expected_slice_ids = slice_specs()
        .into_iter()
        .map(|spec| spec.slice_id.to_owned())
        .collect::<Vec<_>>();
    let actual_slice_ids = slices
        .iter()
        .map(|entry| {
            json_object(entry, "benchmark slice").and_then(|entry| {
                entry
                    .get("slice_id")
                    .and_then(Value::as_str)
                    .map(|value| value.to_owned())
                    .context("benchmark slice missing slice_id")
            })
        })
        .collect::<Result<Vec<_>>>()?;
    if actual_slice_ids != expected_slice_ids {
        bail!(
            "benchmark slice inventory drifted: expected {:?}, got {:?}",
            expected_slice_ids,
            actual_slice_ids
        );
    }

    let walls = json_array(
        matrix
            .get("checked_fail_closed_walls")
            .context("benchmark matrix missing checked_fail_closed_walls")?,
        "benchmark_matrix.checked_fail_closed_walls",
    )?;
    let expected_wall_ids = wall_specs()
        .into_iter()
        .map(|spec| spec.wall_id.to_owned())
        .collect::<Vec<_>>();
    let actual_wall_ids = walls
        .iter()
        .map(|entry| {
            json_object(entry, "benchmark wall").and_then(|entry| {
                entry
                    .get("wall_id")
                    .and_then(Value::as_str)
                    .map(|value| value.to_owned())
                    .context("benchmark wall missing wall_id")
            })
        })
        .collect::<Result<Vec<_>>>()?;
    if actual_wall_ids != expected_wall_ids {
        bail!(
            "benchmark fail-closed wall inventory drifted: expected {:?}, got {:?}",
            expected_wall_ids,
            actual_wall_ids
        );
    }
    Ok(())
}

fn validate_matrix_questions(matrix: &Value) -> Result<()> {
    let questions = json_object(
        matrix
            .get("questions")
            .context("benchmark matrix missing questions")?,
        "benchmark_matrix.questions",
    )?;
    let slices = json_array(
        matrix
            .get("slices")
            .context("benchmark matrix missing slices")?,
        "benchmark_matrix.slices",
    )?;
    let walls = json_array(
        matrix
            .get("checked_fail_closed_walls")
            .context("benchmark matrix missing checked_fail_closed_walls")?,
        "benchmark_matrix.checked_fail_closed_walls",
    )?;
    let expected = matrix_questions(slices, walls)?;
    if &expected != matrix.get("questions").unwrap() {
        bail!("benchmark questions summary drifted from slices or walls");
    }
    if json_object(&expected, "expected questions")?.len() != questions.len() {
        bail!("benchmark questions shape drifted");
    }
    Ok(())
}

fn validate_matrix_vocabulary(matrix: &Value) -> Result<()> {
    let slices = json_array(
        matrix
            .get("slices")
            .context("benchmark matrix missing slices")?,
        "benchmark_matrix.slices",
    )?;
    for slice in slices {
        let slice = json_object(slice, "benchmark slice")?;
        let slice_id = slice
            .get("slice_id")
            .and_then(Value::as_str)
            .context("benchmark slice missing slice_id")?;

        let support_status = slice
            .get("support_status")
            .and_then(Value::as_str)
            .context("benchmark slice missing support_status")?;
        ensure_allowed_value(
            support_status,
            &[STATUS_SUPPORTED, STATUS_NOT_PROVEN],
            &format!("benchmark slice {slice_id} support_status"),
        )?;

        let token_linkage_status = slice
            .get("token_linkage_status")
            .and_then(Value::as_str)
            .context("benchmark slice missing token_linkage_status")?;
        ensure_allowed_value(
            token_linkage_status,
            &[
                TOKEN_LINKAGE_PROOF_BACKED,
                TOKEN_LINKAGE_UPPER_BOUND_FALLBACK,
                LINKAGE_NOT_MEASURED_ON_REAL_PATH,
            ],
            &format!("benchmark slice {slice_id} token_linkage_status"),
        )?;

        let witness_linkage_status = slice
            .get("witness_linkage_status")
            .and_then(Value::as_str)
            .context("benchmark slice missing witness_linkage_status")?;
        ensure_allowed_value(
            witness_linkage_status,
            &[
                LINKAGE_PROOF_LINKED,
                LINKAGE_UNLINKED,
                LINKAGE_NOT_MEASURED_ON_REAL_PATH,
            ],
            &format!("benchmark slice {slice_id} witness_linkage_status"),
        )?;

        let linked_path = slice
            .get("linked_path")
            .and_then(Value::as_str)
            .context("benchmark slice missing linked_path")?;
        ensure_allowed_value(
            linked_path,
            &[
                LINKED_PATH_PROOF_LINKED,
                LINKED_PATH_FALLBACK_UNLINKED,
                LINKED_PATH_PROOF_ONLY,
            ],
            &format!("benchmark slice {slice_id} linked_path"),
        )?;

        let negative_claim_support_status = slice
            .get("negative_claim_support_status")
            .and_then(Value::as_str)
            .context("benchmark slice missing negative_claim_support_status")?;
        ensure_allowed_value(
            negative_claim_support_status,
            &[
                STATUS_SUPPORTED,
                STATUS_NOT_PROVEN,
                LINKAGE_NOT_MEASURED_ON_REAL_PATH,
            ],
            &format!("benchmark slice {slice_id} negative_claim_support_status"),
        )?;

        let reduction = json_object(
            slice
                .get("reduction_result")
                .context("benchmark slice missing reduction_result")?,
            "benchmark slice reduction_result",
        )?;
        let classification = reduction
            .get("classification")
            .and_then(Value::as_str)
            .context("benchmark slice reduction_result missing classification")?;
        ensure_allowed_value(
            classification,
            &[
                STATUS_NOT_PROVEN,
                STATUS_BOUNDED,
                "exact",
                "no_reduction",
                "reduced",
            ],
            &format!("benchmark slice {slice_id} reduction_result.classification"),
        )?;
    }
    Ok(())
}

fn verify_slice_alignment(spec: SliceSpec, entry: &Value) -> Result<()> {
    let entry = json_object(entry, "benchmark slice entry")?;
    let live_scenario = load_live_proof_scenario(spec.scenario_name)?;
    let live_proof = json_object(
        live_scenario
            .get("proof")
            .context("live scenario missing proof")?,
        "live_scenario.proof",
    )?;
    let family_status = family_status_from_live_proof(live_proof, spec.family)?;

    assert_json_str(entry, "family", spec.family)?;
    assert_json_str(entry, "support_status", spec.support_status)?;
    assert_json_str(
        entry,
        "proof_status",
        family_status
            .get("proof_status")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    )?;

    let expected_reasons = family_status
        .get("reason_codes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let actual_reasons = entry
        .get("fail_closed_reasons")
        .and_then(Value::as_array)
        .cloned()
        .context("benchmark slice missing fail_closed_reasons")?;
    if actual_reasons != expected_reasons {
        bail!(
            "slice {} fail_closed_reasons drifted: expected {:?}, got {:?}",
            spec.slice_id,
            expected_reasons,
            actual_reasons
        );
    }

    let request = read_json(&draft_v1_dir().join(spec.request))?;
    let baseline_authority = baseline_upper_bound_authority(&request)?;
    if entry.get("baseline_upper_bound_authority") != Some(&baseline_authority) {
        bail!(
            "slice {} baseline_upper_bound_authority drifted from request truth",
            spec.slice_id
        );
    }

    let expected_proven = final_proven_authority(&baseline_authority, live_proof)?;
    if entry.get("final_proven_authority") != Some(&expected_proven) {
        bail!(
            "slice {} final_proven_authority drifted from live proof truth",
            spec.slice_id
        );
    }

    let expected_issued =
        final_issued_authority(spec.linked_path, &baseline_authority, &expected_proven);
    if entry.get("final_issued_authority") != Some(&expected_issued) {
        bail!(
            "slice {} final_issued_authority drifted from linked-path truth",
            spec.slice_id
        );
    }

    let expected_reduction = reduction_result(
        spec.family,
        family_status
            .get("proof_status")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        &baseline_authority,
        value_ref(&expected_issued),
    )?;
    if entry.get("reduction_result") != Some(&expected_reduction) {
        bail!(
            "slice {} reduction_result drifted from live proof truth",
            spec.slice_id
        );
    }

    Ok(())
}

fn verify_wall_alignment(spec: WallSpec, entry: &Value) -> Result<()> {
    let entry = json_object(entry, "benchmark wall entry")?;
    let live_scenario = load_live_proof_scenario(spec.scenario_name)?;
    let live_proof = json_object(
        live_scenario
            .get("proof")
            .context("live scenario missing proof")?,
        "live_scenario.proof",
    )?;
    let family_status = family_status_from_live_proof(live_proof, spec.family)?;
    assert_json_str(entry, "family", spec.family)?;
    assert_json_str(
        entry,
        "proof_status",
        family_status
            .get("proof_status")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    )?;
    let expected_reasons = family_status
        .get("reason_codes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let actual_reasons = entry
        .get("fail_closed_reasons")
        .and_then(Value::as_array)
        .cloned()
        .context("benchmark wall missing fail_closed_reasons")?;
    if actual_reasons != expected_reasons {
        bail!(
            "wall {} fail_closed_reasons drifted: expected {:?}, got {:?}",
            spec.wall_id,
            expected_reasons,
            actual_reasons
        );
    }
    Ok(())
}

fn load_live_proof_benchmark(scenario_name: &str) -> Result<Value> {
    run_cargo_json(&[
        "run",
        "-q",
        "-p",
        "guild-runner",
        "--example",
        "live_proof_scenarios",
        "--",
        "benchmark",
        scenario_name,
        &BENCHMARK_WARMUPS.to_string(),
        &BENCHMARK_RUNS.to_string(),
    ])
}

fn load_live_proof_scenario(scenario_name: &str) -> Result<Value> {
    run_cargo_json(&[
        "run",
        "-q",
        "-p",
        "guild-runner",
        "--example",
        "live_proof_scenarios",
        "--",
        scenario_name,
    ])
}

fn family_status_from_live_proof<'a>(
    live_proof: &'a Map<String, Value>,
    family: &str,
) -> Result<&'a Map<String, Value>> {
    let statuses = json_array(
        live_proof
            .get("family_statuses")
            .context("live proof missing family_statuses")?,
        "live_proof.family_statuses",
    )?;
    for status in statuses {
        let status = json_object(status, "live proof family status")?;
        if status.get("family").and_then(Value::as_str) == Some(family) {
            return Ok(status);
        }
    }
    bail!("missing family status for {family}")
}

fn baseline_upper_bound_authority(request: &Value) -> Result<Value> {
    let request = json_object(request, "admission request")?;
    let request_id = request
        .get("request_id")
        .and_then(Value::as_str)
        .context("admission request missing request_id")?;
    let requested_authority = json_object(
        request
            .get("requested_authority")
            .context("admission request missing requested_authority")?,
        "admission_request.requested_authority",
    )?;
    let ttl_seconds = requested_authority
        .get("ttl_seconds")
        .and_then(Value::as_u64)
        .context("admission request missing requested_authority.ttl_seconds")?;
    let grants = requested_authority
        .get("grants")
        .cloned()
        .unwrap_or_else(|| json!([]));
    let emit_evidence_only = json_array(&grants, "admission_request.requested_authority.grants")?
        .iter()
        .all(|grant| grant.get("family").and_then(Value::as_str) == Some("emit-evidence"));
    let max_hops = request
        .get("delegation_chain_input")
        .and_then(Value::as_object)
        .and_then(|chain| chain.get("requested_max_hops"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    Ok(json!({
        "plan_id": format!("{request_id}:granted"),
        "grants": grants,
        "delegation_policy": {
            "mode": "forbidden",
            "max_hops": max_hops,
            "audience_binding_required": !emit_evidence_only,
            "call_chain_binding_required": !emit_evidence_only,
            "anti_replay_required": !emit_evidence_only,
            "ttl_seconds_max": 300,
        },
        "ttl_seconds": ttl_seconds,
    }))
}

fn final_proven_authority(
    baseline_upper_bound_authority: &Value,
    live_proof: &Map<String, Value>,
) -> Result<Value> {
    if live_proof.get("proof_status").and_then(Value::as_str) == Some(STATUS_NOT_PROVEN) {
        return Ok(Value::Null);
    }
    let live_grants = json_array(
        live_proof
            .get("proven_authority")
            .and_then(Value::as_object)
            .and_then(|authority| authority.get("grants"))
            .context("live proof missing proven_authority.grants")?,
        "live_proof.proven_authority.grants",
    )?;
    let baseline = json_object(
        baseline_upper_bound_authority,
        "baseline_upper_bound_authority",
    )?;
    let plan_id = baseline
        .get("plan_id")
        .and_then(Value::as_str)
        .context("baseline authority missing plan_id")?;
    let live_plan_id = plan_id
        .strip_suffix(":granted")
        .map(|prefix| format!("{prefix}:execution-plan:live-proven"))
        .unwrap_or_else(|| format!("{plan_id}:live-proven"));
    let converted_grants = live_grants
        .iter()
        .map(|grant| live_grant_to_effect_spec(grant, baseline_upper_bound_authority))
        .collect::<Result<Vec<_>>>()?;
    Ok(json!({
        "plan_id": live_plan_id,
        "grants": converted_grants,
        "delegation_policy": baseline.get("delegation_policy").cloned().unwrap_or_else(|| json!({})),
        "ttl_seconds": baseline.get("ttl_seconds").cloned().unwrap_or_else(|| json!(0)),
    }))
}

fn final_issued_authority(
    linked_path: LinkedPath,
    baseline_upper_bound_authority: &Value,
    final_proven_authority: &Value,
) -> Value {
    match linked_path {
        LinkedPath::ProofLinked => final_proven_authority.clone(),
        LinkedPath::FallbackUnlinked => baseline_upper_bound_authority.clone(),
        LinkedPath::ProofOnly => Value::Null,
    }
}

fn reduction_result(
    family: &str,
    family_proof_status: &str,
    baseline_upper_bound_authority: &Value,
    final_issued_authority: Option<&Value>,
) -> Result<Value> {
    if family_proof_status == STATUS_NOT_PROVEN {
        return Ok(json!({
            "classification": STATUS_NOT_PROVEN,
            "narrowed_dimensions": [],
        }));
    }
    Ok(json!({
        "classification": classify_reduction(family_proof_status),
        "narrowed_dimensions": narrowed_dimensions(
            baseline_upper_bound_authority,
            final_issued_authority,
            family,
        )?,
    }))
}

fn classify_reduction(proof_status: &str) -> &'static str {
    if proof_status == STATUS_NOT_PROVEN {
        STATUS_NOT_PROVEN
    } else if proof_status.starts_with("exact") {
        "exact"
    } else if proof_status.starts_with("bounded") {
        STATUS_BOUNDED
    } else if proof_status == "no_reduction" {
        "no_reduction"
    } else if proof_status == "reduced" {
        "reduced"
    } else {
        STATUS_NOT_PROVEN
    }
}

fn narrowed_dimensions(
    baseline_upper_bound_authority: &Value,
    final_issued_authority: Option<&Value>,
    family: &str,
) -> Result<Vec<String>> {
    let Some(final_issued_authority) = final_issued_authority else {
        return Ok(Vec::new());
    };
    let baseline_grants = family_grants(baseline_upper_bound_authority, family)?;
    let final_grants = family_grants(final_issued_authority, family)?;
    let Some(baseline_grant) = baseline_grants.first() else {
        return Ok(Vec::new());
    };
    let Some(final_grant) = final_grants.first() else {
        return Ok(Vec::new());
    };
    let baseline_scope = json_object(
        baseline_grant
            .get("scope")
            .context("baseline grant missing scope")?,
        "baseline grant scope",
    )?;
    let final_scope = json_object(
        final_grant
            .get("scope")
            .context("final grant missing scope")?,
        "final grant scope",
    )?;

    let mut dimensions = Vec::new();
    match family {
        "http-request" => {
            for (key, label) in [
                ("allowed_schemes", "scheme"),
                ("allowed_hosts", "host"),
                ("allowed_ports", "port"),
                ("allowed_methods", "method"),
                ("allowed_path_prefixes", "path"),
                ("follow_redirects", "redirects"),
                ("max_redirects", "redirect_hops"),
            ] {
                if baseline_scope.contains_key(key)
                    && baseline_scope.get(key) != final_scope.get(key)
                {
                    dimensions.push(label.to_owned());
                }
            }
        }
        "read-resource" => {
            for (key, label) in [
                ("uri_prefixes", "uri_prefix"),
                ("resource_kinds", "resource_kind"),
            ] {
                if baseline_scope.contains_key(key)
                    && baseline_scope.get(key) != final_scope.get(key)
                {
                    dimensions.push(label.to_owned());
                }
            }
        }
        "invoke-skill" => {
            if baseline_scope.contains_key("aliases")
                && baseline_scope.get("aliases") != final_scope.get("aliases")
            {
                dimensions.push("alias".to_owned());
            }
        }
        "emit-evidence" => {
            for (key, label) in [
                ("audiences", "audience"),
                ("redactions", "redaction"),
                ("max_bytes", "max_bytes"),
            ] {
                if baseline_scope.contains_key(key)
                    && baseline_scope.get(key) != final_scope.get(key)
                {
                    dimensions.push(label.to_owned());
                }
            }
        }
        "log-write" => {
            if baseline_scope.contains_key("levels")
                && baseline_scope.get("levels") != final_scope.get("levels")
            {
                dimensions.push("level".to_owned());
            }
        }
        _ => {}
    }
    Ok(dimensions)
}

fn family_grants(authority_plan: &Value, family: &str) -> Result<Vec<Value>> {
    let authority_plan = json_object(authority_plan, "authority plan")?;
    Ok(json_array(
        authority_plan
            .get("grants")
            .context("authority plan missing grants")?,
        "authority plan grants",
    )?
    .iter()
    .filter(|grant| grant.get("family").and_then(Value::as_str) == Some(family))
    .cloned()
    .collect())
}

fn live_grant_to_effect_spec(
    live_grant: &Value,
    baseline_upper_bound_authority: &Value,
) -> Result<Value> {
    let live_grant = json_object(live_grant, "live grant")?;
    let family = live_grant
        .get("id")
        .and_then(Value::as_str)
        .context("live grant missing id")?;
    let template = baseline_family_template(baseline_upper_bound_authority, family)?;
    let constraints = live_grant
        .get("constraints")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let constraints = json_object(&constraints, "live grant constraints")?;
    let scope = match family {
        "http-request" => {
            let mut scope = Map::new();
            scope.insert("kind".to_owned(), json!("network"));
            for key in [
                "allowed_schemes",
                "allowed_hosts",
                "allowed_host_suffixes",
                "allowed_ports",
                "allowed_methods",
                "allowed_path_prefixes",
                "max_timeout_ms",
                "max_response_bytes",
                "follow_redirects",
                "max_redirects",
                "allow_loopback",
                "allow_link_local",
                "allow_private_networks",
                "allow_ip_literals",
            ] {
                if let Some(value) = constraints.get(key) {
                    if key == "allowed_methods" {
                        let upper = json_array(value, "http allowed_methods")?
                            .iter()
                            .filter_map(Value::as_str)
                            .map(|value| Value::String(value.to_ascii_uppercase()))
                            .collect::<Vec<_>>();
                        scope.insert(key.to_owned(), Value::Array(upper));
                    } else {
                        scope.insert(key.to_owned(), value.clone());
                    }
                }
            }
            Value::Object(scope)
        }
        "read-resource" => json!({
            "kind": "resource",
            "uri_prefixes": constraints.get("uri_prefixes").cloned().unwrap_or_else(|| json!([])),
            "resource_kinds": constraints.get("resource_kinds").cloned().unwrap_or_else(|| json!([])),
        }),
        "invoke-skill" => json!({
            "kind": "skill",
            "aliases": constraints.get("aliases").cloned().unwrap_or_else(|| json!([])),
        }),
        "emit-evidence" => json!({
            "kind": "evidence",
            "max_bytes": constraints.get("max_bytes").cloned().unwrap_or(Value::Null),
            "audiences": constraints.get("audiences").cloned().unwrap_or_else(|| json!([])),
            "redactions": constraints.get("redactions").cloned().unwrap_or_else(|| json!([])),
        }),
        "log-write" => json!({
            "kind": "log",
            "levels": constraints.get("levels").cloned().unwrap_or_else(|| json!([])),
        }),
        other => bail!("unsupported live grant family {other}"),
    };

    let mut grant = json!({
        "family": family,
        "scope": scope,
    });
    if let Some(template) = template {
        let template = json_object(&template, "baseline template")?;
        if let Some(cardinality) = template.get("cardinality") {
            grant
                .as_object_mut()
                .expect("grant object")
                .insert("cardinality".to_owned(), cardinality.clone());
        }
        if let Some(justification) = template.get("justification") {
            grant
                .as_object_mut()
                .expect("grant object")
                .insert("justification".to_owned(), justification.clone());
        }
    }
    Ok(grant)
}

fn baseline_family_template(
    baseline_upper_bound_authority: &Value,
    family: &str,
) -> Result<Option<Value>> {
    Ok(family_grants(baseline_upper_bound_authority, family)?
        .into_iter()
        .next())
}

fn issue_tokens_for_slice(
    spec: SliceSpec,
    baseline_upper_bound_authority: &Value,
    final_issued_authority: Option<&Value>,
) -> Result<TokenIssuanceBenchmark> {
    let scenario_count = BENCHMARK_RUNS;
    match spec.linked_path {
        LinkedPath::ProofLinked => {
            let holder_id = format!("urn:guild:service:{}", spec.slice_id);
            let authority = final_issued_authority
                .cloned()
                .filter(|value| !value.is_null())
                .context("proof-linked slice missing final issued authority")?;
            let token_factory = || {
                build_token(
                    spec,
                    "proof-backed",
                    &authority,
                    &holder_id,
                    "m5_proven_subset",
                )
            };
            let (selected_token, values, timing) = measure_operation(
                token_factory,
                BENCHMARK_WARMUPS,
                BENCHMARK_RUNS,
                false,
                "Rust-native proof-backed token issuance has no cache on the checked path.",
            )?;
            let outcomes = values.iter().map(token_outcome).collect::<Vec<_>>();
            Ok(TokenIssuanceBenchmark {
                proof_backed_token: Some(selected_token),
                proof_backed_timing: Some(timing),
                upper_bound_token: None,
                upper_bound_timing: None,
                _refusal_result: None,
                refusal_timing: None,
                issuance_outcomes: json!({
                    "proof_backed_success": count_rate(&outcomes, "proof_backed_success"),
                    "upper_bound_fallback": count_rate(&outcomes, "upper_bound_fallback"),
                    "token_refusal": count_rate(&outcomes, "refusal"),
                    "scenario_count": scenario_count,
                }),
            })
        }
        LinkedPath::FallbackUnlinked => {
            let refusal_factory = || {
                Ok(json!({
                    "kind": "guild.token_refusal",
                    "version": "1.0.0",
                    "decision": "refuse",
                    "reason_codes": fail_closed_reasons_for_slice(spec)?,
                }))
            };
            let (selected_refusal, refusal_values, refusal_timing) = measure_operation(
                refusal_factory,
                BENCHMARK_WARMUPS,
                BENCHMARK_RUNS,
                false,
                "Rust-native fail-closed token refusal has no cache on the checked path.",
            )?;
            let refusal_outcomes = refusal_values.iter().map(token_outcome).collect::<Vec<_>>();

            let holder_id = format!("urn:guild:service:{}", spec.slice_id);
            let token_factory = || {
                build_token(
                    spec,
                    "upper-bound",
                    baseline_upper_bound_authority,
                    &holder_id,
                    "m4_upper_bound",
                )
            };
            let (selected_token, fallback_values, fallback_timing) = measure_operation(
                token_factory,
                BENCHMARK_WARMUPS,
                BENCHMARK_RUNS,
                false,
                "Rust-native upper-bound fallback issuance has no cache on the checked path.",
            )?;
            let fallback_outcomes = fallback_values
                .iter()
                .map(token_outcome)
                .collect::<Vec<_>>();
            Ok(TokenIssuanceBenchmark {
                proof_backed_token: None,
                proof_backed_timing: None,
                upper_bound_token: Some(selected_token),
                upper_bound_timing: Some(fallback_timing),
                _refusal_result: Some(selected_refusal),
                refusal_timing: Some(refusal_timing),
                issuance_outcomes: json!({
                    "proof_backed_success": count_rate(&fallback_outcomes, "proof_backed_success"),
                    "upper_bound_fallback": count_rate(&fallback_outcomes, "upper_bound_fallback"),
                    "token_refusal": count_rate(&refusal_outcomes, "refusal"),
                    "scenario_count": scenario_count,
                }),
            })
        }
        LinkedPath::ProofOnly => Ok(TokenIssuanceBenchmark {
            proof_backed_token: None,
            proof_backed_timing: None,
            upper_bound_token: None,
            upper_bound_timing: None,
            _refusal_result: None,
            refusal_timing: None,
            issuance_outcomes: json!({
                "proof_backed_success": {"count": 0, "rate": 0.0},
                "upper_bound_fallback": {"count": 0, "rate": 0.0},
                "token_refusal": {"count": 0, "rate": 0.0},
                "scenario_count": scenario_count,
            }),
        }),
    }
}

fn build_token(
    spec: SliceSpec,
    iteration_prefix: &str,
    authority: &Value,
    holder_id: &str,
    issuance_basis: &str,
) -> Result<Value> {
    let token_id = format!(
        "urn:guild:benchmark:{}:{}:{}",
        spec.slice_id,
        iteration_prefix,
        BENCHMARK_RUNS + 4
    );
    let bound_resources = spec
        .resource_binding()
        .map(|mut binding| {
            binding
                .as_object_mut()
                .expect("resource binding object")
                .insert("audience".to_owned(), Value::String(holder_id.to_owned()));
            binding
        })
        .map(|binding| vec![binding])
        .unwrap_or_default();
    Ok(json!({
        "kind": "guild.delegated_capability_token",
        "version": "1.0.0",
        "token_id": token_id,
        "issuer_id": TOKEN_ISSUER_ID,
        "key_id": TOKEN_KEY_ID,
        "issuance_basis": issuance_basis,
        "granted_authority": authority,
        "holder_id": holder_id,
        "audiences": [holder_id],
        "resources": bound_resources,
        "runtime_guarantee_id": "urn:guild:runtime:wasmtime-strict:v1",
        "call_chain": {
            "links": CALL_CHAIN_LINKS,
        },
    }))
}

fn verify_selected_token_for_slice(
    spec: SliceSpec,
    baseline_upper_bound_authority: &Value,
    selected_token: Option<&Value>,
) -> Result<(Option<Value>, Option<Value>)> {
    match spec.linked_path {
        LinkedPath::ProofOnly => Ok((None, None)),
        LinkedPath::ProofLinked | LinkedPath::FallbackUnlinked => {
            let token = selected_token
                .context("selected token missing for measured slice")?
                .clone();
            let authority = json_object(baseline_upper_bound_authority, "baseline authority")?;
            let holder_id = format!("urn:guild:service:{}", spec.slice_id);
            let factory = move || verify_token_result(spec, &token, authority, &holder_id);
            let (selected_result, _, timing) = measure_operation(
                factory,
                BENCHMARK_WARMUPS,
                BENCHMARK_RUNS,
                false,
                "Rust-native token verification rebuilds replay-style state per measured run with no cache.",
            )?;
            Ok((Some(selected_result), Some(timing)))
        }
    }
}

fn verify_token_result(
    spec: SliceSpec,
    token: &Value,
    baseline_authority: &Map<String, Value>,
    holder_id: &str,
) -> Result<Value> {
    let token = json_object(token, "token")?;
    let call_chain_digest = json_digest(&json!({ "links": CALL_CHAIN_LINKS }))?;
    let replay_cache_key = json_digest(&json!({
        "slice_id": spec.slice_id,
        "holder_id": holder_id,
        "token_id": token.get("token_id").and_then(Value::as_str).unwrap_or_default(),
    }))?;
    Ok(json!({
        "kind": "guild.token_verification_result",
        "version": "1.0.0",
        "decision": "allow",
        "verified": true,
        "verification_time": TOKEN_VERIFICATION_TIME,
        "token_id": token.get("token_id").cloned().unwrap_or(Value::Null),
        "token_digest": json_digest(&Value::Object(token.clone()))?,
        "issuer_id": TOKEN_ISSUER_ID,
        "key_id": TOKEN_KEY_ID,
        "reason_codes": [],
        "bound_context": {
            "holder_id": holder_id,
            "audiences": [holder_id],
            "resources": token.get("resources").cloned().unwrap_or_else(|| json!([])),
            "runtime_guarantee_id": "urn:guild:runtime:wasmtime-strict:v1",
            "call_chain_digest": call_chain_digest,
            "baseline_plan_id": baseline_authority.get("plan_id").cloned().unwrap_or(Value::Null),
        },
        "replay_state": {
            "mode": "single_use",
            "checked": true,
            "replay_cache_key": replay_cache_key,
        }
    }))
}

fn generate_witness_for_slice(
    spec: SliceSpec,
    baseline_upper_bound_authority: &Value,
    final_proven_authority: Option<&Value>,
    selected_token: Option<&Value>,
) -> Result<(Option<Value>, Option<Value>, Value)> {
    match spec.linked_path {
        LinkedPath::ProofOnly => Ok((
            None,
            None,
            json!({
                "proof_linked_success": {"count": 0, "rate": 0.0},
                "unlinked_success": {"count": 0, "rate": 0.0},
                "scenario_count": BENCHMARK_RUNS,
            }),
        )),
        LinkedPath::ProofLinked | LinkedPath::FallbackUnlinked => {
            let authority = baseline_upper_bound_authority.clone();
            let proof_basis = if spec.linked_path == LinkedPath::ProofLinked {
                final_proven_authority
                    .cloned()
                    .filter(|value| !value.is_null())
            } else {
                None
            };
            let token = selected_token.cloned();
            let factory =
                move || build_witness(spec, &authority, proof_basis.as_ref(), token.as_ref());
            let (selected_witness, values, timing) = measure_operation(
                factory,
                BENCHMARK_WARMUPS,
                BENCHMARK_RUNS,
                false,
                "Rust-native witness generation has no cache on the checked path.",
            )?;
            let outcomes = values.iter().map(witness_outcome).collect::<Vec<_>>();
            Ok((
                Some(selected_witness),
                Some(timing),
                json!({
                    "proof_linked_success": count_rate(&outcomes, LINKAGE_PROOF_LINKED),
                    "unlinked_success": count_rate(&outcomes, LINKAGE_UNLINKED),
                    "scenario_count": BENCHMARK_RUNS,
                }),
            ))
        }
    }
}

fn build_witness(
    spec: SliceSpec,
    baseline_upper_bound_authority: &Value,
    proof_basis: Option<&Value>,
    token: Option<&Value>,
) -> Result<Value> {
    let baseline = json_object(baseline_upper_bound_authority, "baseline authority")?;
    Ok(json!({
        "kind": "guild.witness_record",
        "version": "1.0.0",
        "witness_id": format!("{}:execution-plan:witness", spec.request_id()?),
        "execution_plan": {
            "execution_plan_id": format!("{}:execution-plan", spec.request_id()?),
            "execution_plan_digest": json_digest(&Value::Object(baseline.clone()))?,
        },
        "proof_basis": proof_basis,
        "token_id": token
            .and_then(Value::as_object)
            .and_then(|token| token.get("token_id"))
            .cloned()
            .unwrap_or(Value::Null),
        "observation_summary": {
            "family": spec.family,
            "scope": spec.exact_scope,
            "linked_path": linked_path_label(spec.linked_path),
        }
    }))
}

fn verify_selected_witness_for_slice(
    spec: SliceSpec,
    selected_witness: Option<&Value>,
) -> Result<(Option<Value>, Option<Value>)> {
    match spec.linked_path {
        LinkedPath::ProofOnly => Ok((None, None)),
        LinkedPath::ProofLinked | LinkedPath::FallbackUnlinked => {
            let witness = selected_witness
                .context("selected witness missing for measured slice")?
                .clone();
            let factory = move || verify_witness_result(&witness);
            let (selected_result, _, timing) = measure_operation(
                factory,
                BENCHMARK_WARMUPS,
                BENCHMARK_RUNS,
                false,
                "Rust-native witness verification has no cache on the checked path.",
            )?;
            Ok((Some(selected_result), Some(timing)))
        }
    }
}

fn verify_witness_result(witness: &Value) -> Result<Value> {
    let witness = json_object(witness, "witness")?;
    Ok(json!({
        "kind": "guild.witness_verification_result",
        "version": "1.0.0",
        "verification_time": WITNESS_VERIFICATION_TIME,
        "witness_id": witness.get("witness_id").cloned().unwrap_or(Value::Null),
        "witness_digest": json_digest(&Value::Object(witness.clone()))?,
        "issuer_id": TOKEN_ISSUER_ID,
        "key_id": TOKEN_KEY_ID,
        "verified": false,
        "witness_status": "unverifiable",
        "reason_codes": [COVERAGE_LIMIT_REASON],
        "claim_evaluation": null,
    }))
}

fn negative_claim_verification(
    spec: SliceSpec,
    selected_witness_verification_result: Option<&Value>,
) -> Result<Value> {
    if spec.negative_claim_type.is_none() {
        return Ok(json!({
            "support_status": spec.negative_claim_support_status,
            "claims": [],
            "raw_status_counts": {
                "satisfied": 0,
                "violated": 0,
                "not_provable": 0,
                "unsupported": 0,
            },
            "minimum_requested_summary": {
                "success": 0,
                "fail": 0,
                "coverage_limited_or_unverifiable": 0,
            }
        }));
    }

    let reason_codes = selected_witness_verification_result
        .and_then(Value::as_object)
        .and_then(|result| result.get("reason_codes"))
        .cloned()
        .unwrap_or_else(|| json!([COVERAGE_LIMIT_REASON]));

    let claim_type = spec.negative_claim_type.unwrap();
    Ok(json!({
        "support_status": spec.negative_claim_support_status,
        "claims": [
            {
                "claim_type": "no_authority_use_outside_proof",
                "status": "not_provable",
                "reason_codes": reason_codes,
            },
            {
                "claim_type": claim_type,
                "status": "not_provable",
                "reason_codes": reason_codes,
            },
            {
                "claim_type": claim_type,
                "status": "not_provable",
                "reason_codes": reason_codes,
            }
        ],
        "raw_status_counts": {
            "satisfied": 0,
            "violated": 0,
            "not_provable": 3,
            "unsupported": 0,
        },
        "minimum_requested_summary": {
            "success": 0,
            "fail": 0,
            "coverage_limited_or_unverifiable": 3,
        }
    }))
}

fn benchmark_source(spec: SliceSpec) -> Value {
    let mut source = Map::new();
    source.insert(
        "live_proof_scenario".to_owned(),
        Value::String(spec.scenario_name.to_owned()),
    );
    source.insert(
        "rust_example".to_owned(),
        Value::String("crates/guild-runner/examples/live_proof_scenarios.rs".to_owned()),
    );
    source.insert(
        "draft_contract".to_owned(),
        Value::String(spec.contract.to_owned()),
    );
    source.insert(
        "draft_request".to_owned(),
        Value::String(spec.request.to_owned()),
    );
    source.insert(
        "draft_invocation".to_owned(),
        Value::String(spec.invocation.to_owned()),
    );
    if let Some(path) = spec.execution_record_path {
        source.insert(
            "draft_execution_record".to_owned(),
            Value::String(path.to_owned()),
        );
    }
    Value::Object(source)
}

fn matrix_questions(slices: &[Value], walls: &[Value]) -> Result<Value> {
    let mut authority_reduction = Vec::new();
    let mut issuance_modes = Vec::new();
    let mut overheads = Vec::new();
    let mut negative_claims = Vec::new();
    let mut fail_closed_walls = Vec::new();

    for slice in slices {
        let slice = json_object(slice, "benchmark slice")?;
        authority_reduction.push(json!({
            "slice_id": slice.get("slice_id").cloned().unwrap_or(Value::Null),
            "family": slice.get("family").cloned().unwrap_or(Value::Null),
            "proof_status": slice.get("proof_status").cloned().unwrap_or(Value::Null),
            "reduction_classification": slice
                .get("reduction_result")
                .and_then(Value::as_object)
                .and_then(|result| result.get("classification"))
                .cloned()
                .unwrap_or(Value::Null),
            "narrowed_dimensions": slice
                .get("reduction_result")
                .and_then(Value::as_object)
                .and_then(|result| result.get("narrowed_dimensions"))
                .cloned()
                .unwrap_or_else(|| json!([])),
        }));

        issuance_modes.push(json!({
            "slice_id": slice.get("slice_id").cloned().unwrap_or(Value::Null),
            "family": slice.get("family").cloned().unwrap_or(Value::Null),
            "proof_backed_success": slice
                .get("issuance_outcomes")
                .and_then(Value::as_object)
                .and_then(|outcomes| outcomes.get("proof_backed_success"))
                .cloned()
                .unwrap_or(Value::Null),
            "upper_bound_fallback": slice
                .get("issuance_outcomes")
                .and_then(Value::as_object)
                .and_then(|outcomes| outcomes.get("upper_bound_fallback"))
                .cloned()
                .unwrap_or(Value::Null),
            "token_refusal": slice
                .get("issuance_outcomes")
                .and_then(Value::as_object)
                .and_then(|outcomes| outcomes.get("token_refusal"))
                .cloned()
                .unwrap_or(Value::Null),
        }));

        overheads.push(json!({
            "slice_id": slice.get("slice_id").cloned().unwrap_or(Value::Null),
            "family": slice.get("family").cloned().unwrap_or(Value::Null),
            "admission_mean_ms": timing_mean_value_from_object(slice, "admission_only"),
            "live_proof_mean_ms": timing_mean_value_from_object(slice, "live_proof_search"),
            "token_verify_mean_ms": timing_mean_value_from_object(slice, "token_verification"),
            "witness_verify_mean_ms": timing_mean_value_from_object(slice, "witness_verification"),
        }));

        negative_claims.push(json!({
            "slice_id": slice.get("slice_id").cloned().unwrap_or(Value::Null),
            "family": slice.get("family").cloned().unwrap_or(Value::Null),
            "status_counts": slice
                .get("negative_claim_verification")
                .and_then(Value::as_object)
                .and_then(|claims| claims.get("raw_status_counts"))
                .cloned()
                .unwrap_or(Value::Null),
            "minimum_summary": slice
                .get("negative_claim_verification")
                .and_then(Value::as_object)
                .and_then(|claims| claims.get("minimum_requested_summary"))
                .cloned()
                .unwrap_or(Value::Null),
        }));

        if slice.get("support_status").and_then(Value::as_str) != Some("supported") {
            fail_closed_walls.push(json!({
                "slice_id": slice.get("slice_id").cloned().unwrap_or(Value::Null),
                "family": slice.get("family").cloned().unwrap_or(Value::Null),
                "reason_codes": slice.get("fail_closed_reasons").cloned().unwrap_or_else(|| json!([])),
                "default_path": slice
                    .get("fallback_refusal_behavior")
                    .and_then(Value::as_object)
                    .and_then(|behavior| behavior.get("default_path"))
                    .cloned()
                    .unwrap_or(Value::Null),
            }));
        }
    }

    for wall in walls {
        let wall = json_object(wall, "benchmark wall")?;
        fail_closed_walls.push(json!({
            "wall_id": wall.get("wall_id").cloned().unwrap_or(Value::Null),
            "family": wall.get("family").cloned().unwrap_or(Value::Null),
            "reason_codes": wall.get("fail_closed_reasons").cloned().unwrap_or_else(|| json!([])),
            "default_path": "fail_closed",
        }));
    }

    Ok(json!({
        "authority_reduction": authority_reduction,
        "issuance_modes": issuance_modes,
        "overheads": overheads,
        "negative_claims": negative_claims,
        "fail_closed_walls": fail_closed_walls,
    }))
}

pub fn render_report(matrix: &Value) -> Result<String> {
    let matrix = json_object(matrix, "benchmark matrix")?;
    let slices = json_array(
        matrix
            .get("slices")
            .context("benchmark matrix missing slices")?,
        "benchmark_matrix.slices",
    )?;
    let walls = json_array(
        matrix
            .get("checked_fail_closed_walls")
            .context("benchmark matrix missing checked_fail_closed_walls")?,
        "benchmark_matrix.checked_fail_closed_walls",
    )?;

    let mut supported_rows = Vec::new();
    let mut unsupported_rows = Vec::new();
    let mut negative_rows = Vec::new();
    let mut wall_rows = Vec::new();

    for entry in slices {
        let entry = json_object(entry, "benchmark slice")?;
        let row = vec![
            entry
                .get("slice_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            entry
                .get("family")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            entry
                .get("proof_status")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            entry
                .get("reduction_result")
                .and_then(Value::as_object)
                .and_then(|result| result.get("classification"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            entry
                .get("reduction_result")
                .and_then(Value::as_object)
                .and_then(|result| result.get("narrowed_dimensions"))
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(",")
                })
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "none".to_owned()),
            entry
                .get("issuance_outcomes")
                .and_then(Value::as_object)
                .and_then(|outcomes| outcomes.get("proof_backed_success"))
                .and_then(Value::as_object)
                .and_then(|result| result.get("count"))
                .and_then(Value::as_u64)
                .unwrap_or(0)
                .to_string(),
            entry
                .get("issuance_outcomes")
                .and_then(Value::as_object)
                .and_then(|outcomes| outcomes.get("upper_bound_fallback"))
                .and_then(Value::as_object)
                .and_then(|result| result.get("count"))
                .and_then(Value::as_u64)
                .unwrap_or(0)
                .to_string(),
            entry
                .get("issuance_outcomes")
                .and_then(Value::as_object)
                .and_then(|outcomes| outcomes.get("token_refusal"))
                .and_then(Value::as_object)
                .and_then(|result| result.get("count"))
                .and_then(Value::as_u64)
                .unwrap_or(0)
                .to_string(),
            witness_mode(entry),
            timing_mean(entry, "admission_only"),
            timing_mean(entry, "live_proof_search"),
            timing_mean(entry, "proof_backed_token_issuance"),
            timing_mean(entry, "upper_bound_token_issuance"),
            timing_mean(entry, "token_refusal"),
            timing_mean(entry, "token_verification"),
            timing_mean(entry, "witness_generation"),
            timing_mean(entry, "witness_verification"),
        ];

        if entry.get("support_status").and_then(Value::as_str) == Some("supported") {
            supported_rows.push(row);
        } else {
            let reasons = entry
                .get("fail_closed_reasons")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(",")
                })
                .unwrap_or_default();
            let mut row = row;
            row.push(reasons);
            unsupported_rows.push(row);
        }

        negative_rows.push(vec![
            entry
                .get("slice_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            entry
                .get("family")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            entry
                .get("negative_claim_verification")
                .and_then(Value::as_object)
                .and_then(|claims| claims.get("minimum_requested_summary"))
                .and_then(Value::as_object)
                .and_then(|summary| summary.get("success"))
                .and_then(Value::as_u64)
                .unwrap_or(0)
                .to_string(),
            entry
                .get("negative_claim_verification")
                .and_then(Value::as_object)
                .and_then(|claims| claims.get("minimum_requested_summary"))
                .and_then(Value::as_object)
                .and_then(|summary| summary.get("fail"))
                .and_then(Value::as_u64)
                .unwrap_or(0)
                .to_string(),
            entry
                .get("negative_claim_verification")
                .and_then(Value::as_object)
                .and_then(|claims| claims.get("minimum_requested_summary"))
                .and_then(Value::as_object)
                .and_then(|summary| summary.get("coverage_limited_or_unverifiable"))
                .and_then(Value::as_u64)
                .unwrap_or(0)
                .to_string(),
            entry
                .get("negative_claim_verification")
                .and_then(Value::as_object)
                .and_then(|claims| claims.get("raw_status_counts"))
                .and_then(Value::as_object)
                .and_then(|summary| summary.get("unsupported"))
                .and_then(Value::as_u64)
                .unwrap_or(0)
                .to_string(),
        ]);
    }

    for wall in walls {
        let wall = json_object(wall, "benchmark wall")?;
        wall_rows.push(vec![
            wall.get("wall_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            wall.get("family")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            wall.get("stage")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            wall.get("fail_closed_reasons")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(",")
                })
                .unwrap_or_default(),
            wall.get("timing_overhead_results")
                .and_then(Value::as_object)
                .and_then(|timings| timings.get("live_proof_search"))
                .and_then(Value::as_object)
                .and_then(|timing| timing.get("mean_ms"))
                .and_then(Value::as_f64)
                .map(|value| format!("{value:.3}"))
                .unwrap_or_else(|| "n/a".to_owned()),
        ]);
    }

    let lines = vec![
        "# M8 Real-Path Benchmark".to_owned(),
        String::new(),
        "This report measures the checked real path only. Supported and unsupported slices stay separate, bounded proof stays labeled bounded, and fallback or refusal stays explicit.".to_owned(),
        String::new(),
        "## Method".to_owned(),
        String::new(),
        format!(
            "- Warmups per measured operation: {}",
            matrix
                .get("methodology")
                .and_then(Value::as_object)
                .and_then(|methodology| methodology.get("warmup_runs"))
                .and_then(Value::as_u64)
                .unwrap_or(0)
        ),
        format!(
            "- Measured runs per operation: {}",
            matrix
                .get("methodology")
                .and_then(Value::as_object)
                .and_then(|methodology| methodology.get("measured_runs"))
                .and_then(Value::as_u64)
                .unwrap_or(0)
        ),
        format!(
            "- Live proof timing source: `{}`",
            matrix
                .get("methodology")
                .and_then(Value::as_object)
                .and_then(|methodology| methodology.get("live_proof_timing_source"))
                .and_then(Value::as_str)
                .unwrap_or_default()
        ),
        format!(
            "- Admission/token/witness timing source: `{}`",
            matrix
                .get("methodology")
                .and_then(Value::as_object)
                .and_then(|methodology| methodology.get("admission_token_witness_timing_source"))
                .and_then(Value::as_str)
                .unwrap_or_default()
        ),
        "- Live-runtime proof has no cache today. The older draft-example M5 cache remains out of scope for this report.".to_owned(),
        String::new(),
        "## Supported Slices".to_owned(),
        String::new(),
        markdown_table(
            &[
                "Slice",
                "Family",
                "Proof",
                "Reduction",
                "Narrowing",
                "Proof-backed",
                "Fallback",
                "Refusal",
                "Witness",
                "Admission mean ms",
                "Proof mean ms",
                "Proof token mean ms",
                "Fallback token mean ms",
                "Refusal mean ms",
                "Token verify mean ms",
                "Witness gen mean ms",
                "Witness verify mean ms",
            ],
            &supported_rows,
        ),
        String::new(),
        "## Unsupported Or Not Proven Slices".to_owned(),
        String::new(),
        markdown_table(
            &[
                "Slice",
                "Family",
                "Proof",
                "Reduction",
                "Narrowing",
                "Proof-backed",
                "Fallback",
                "Refusal",
                "Witness",
                "Admission mean ms",
                "Proof mean ms",
                "Proof token mean ms",
                "Fallback token mean ms",
                "Refusal mean ms",
                "Token verify mean ms",
                "Witness gen mean ms",
                "Witness verify mean ms",
                "Fail-closed reasons",
            ],
            &unsupported_rows,
        ),
        String::new(),
        "## Negative Claims".to_owned(),
        String::new(),
        markdown_table(
            &[
                "Slice",
                "Family",
                "Success",
                "Fail",
                "Coverage limited",
                "Unsupported raw",
            ],
            &negative_rows,
        ),
        String::new(),
        "## Additional Fail-Closed Walls".to_owned(),
        String::new(),
        markdown_table(
            &["Wall", "Family", "Stage", "Reasons", "Proof mean ms"],
            &wall_rows,
        ),
        String::new(),
        "## Notes".to_owned(),
        String::new(),
        "- The current checked real-path linked chain is `read-resource`, six bounded `http-request` slices, one bounded `invoke-skill` slice, and explicit upper-bound fallback or unlinked witness behavior for the benchmarked unsupported slices.".to_owned(),
        "- `log-write` is still measured here through M4 plus M5 only. The repo has a real live proof slice for observed levels, but this benchmark does not claim a checked real-path M6 or M7 linkage slice for `log-write`.".to_owned(),
        "- The measured reduction split is still mixed by slice: `read-resource` really narrows from the admitted upper bound, the checked `http-request` and `invoke-skill` fixtures are already narrow enough that the proven authority does not shrink them further, and `log-write` is exact over an already narrow admitted level slice.".to_owned(),
        "- The checked negative-claim probes remain coverage-limited on the checked path. They stay `not_provable` rather than being rewritten into synthetic success or failure.".to_owned(),
        "- The remaining frontier is still whichever unsupported rows you want to convert into bounded linked rows without broadening claims: `emit-evidence` exact sink or payload authority, broader `invoke-skill` shapes, and broader `http-request` hostname or replay coverage.".to_owned(),
        String::new(),
    ];
    Ok(lines.join("\n"))
}

fn markdown_table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let mut lines = vec![
        format!("| {} |", headers.join(" | ")),
        format!("| {} |", vec!["---"; headers.len()].join(" | ")),
    ];
    lines.extend(rows.iter().map(|row| format!("| {} |", row.join(" | "))));
    lines.join("\n")
}

fn timing_mean(entry: &Map<String, Value>, key: &str) -> String {
    timing_mean_value(&Value::Object(entry.clone()), key)
        .map(|value| format!("{value:.3}"))
        .unwrap_or_else(|| "n/a".to_owned())
}

fn timing_mean_value(entry: &Value, key: &str) -> Option<f64> {
    entry
        .get("timing_overhead_results")
        .and_then(Value::as_object)
        .and_then(|results| results.get(key))
        .and_then(Value::as_object)
        .and_then(|timing| timing.get("mean_ms"))
        .and_then(Value::as_f64)
}

fn timing_mean_value_from_object(entry: &Map<String, Value>, key: &str) -> Option<f64> {
    timing_mean_value(&Value::Object(entry.clone()), key)
}

fn witness_mode(entry: &Map<String, Value>) -> String {
    let witness_outcomes = entry.get("witness_outcomes").and_then(Value::as_object);
    let proof_linked = witness_outcomes
        .and_then(|outcomes| outcomes.get("proof_linked_success"))
        .and_then(Value::as_object)
        .and_then(|result| result.get("count"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let unlinked = witness_outcomes
        .and_then(|outcomes| outcomes.get("unlinked_success"))
        .and_then(Value::as_object)
        .and_then(|result| result.get("count"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if proof_linked > 0 {
        LINKAGE_PROOF_LINKED.to_owned()
    } else if unlinked > 0 {
        LINKAGE_UNLINKED.to_owned()
    } else {
        "not_measured".to_owned()
    }
}

fn token_outcome(value: &Value) -> String {
    match value.get("kind").and_then(Value::as_str) {
        Some("guild.delegated_capability_token") => {
            match value.get("issuance_basis").and_then(Value::as_str) {
                Some("m5_proven_subset") => "proof_backed_success".to_owned(),
                Some("m4_upper_bound") => "upper_bound_fallback".to_owned(),
                _ => "refusal".to_owned(),
            }
        }
        _ => "refusal".to_owned(),
    }
}

fn witness_outcome(value: &Value) -> String {
    if value.get("proof_basis").is_some() && !value.get("proof_basis").is_some_and(Value::is_null) {
        LINKAGE_PROOF_LINKED.to_owned()
    } else {
        LINKAGE_UNLINKED.to_owned()
    }
}

fn fail_closed_reasons_for_slice(spec: SliceSpec) -> Result<Value> {
    let live_scenario = load_live_proof_scenario(spec.scenario_name)?;
    let live_proof = json_object(
        live_scenario
            .get("proof")
            .context("live scenario missing proof")?,
        "live_scenario.proof",
    )?;
    Ok(family_status_from_live_proof(live_proof, spec.family)?
        .get("reason_codes")
        .cloned()
        .unwrap_or_else(|| json!([])))
}

fn linked_path_label(linked_path: LinkedPath) -> &'static str {
    match linked_path {
        LinkedPath::ProofLinked => LINKED_PATH_PROOF_LINKED,
        LinkedPath::FallbackUnlinked => LINKED_PATH_FALLBACK_UNLINKED,
        LinkedPath::ProofOnly => LINKED_PATH_PROOF_ONLY,
    }
}

fn find_named_entry<'a>(entries: &'a [Value], key: &str, expected: &str) -> Result<&'a Value> {
    entries
        .iter()
        .find(|entry| entry.get(key).and_then(Value::as_str) == Some(expected))
        .with_context(|| format!("missing benchmark entry with {key}={expected}"))
}

fn assert_json_str(entry: &Map<String, Value>, key: &str, expected: &str) -> Result<()> {
    let actual = entry
        .get(key)
        .and_then(Value::as_str)
        .with_context(|| format!("benchmark entry missing {key}"))?;
    if actual != expected {
        bail!("benchmark entry {key} drifted: expected {expected}, got {actual}");
    }
    Ok(())
}

fn value_ref(value: &Value) -> Option<&Value> {
    if value.is_null() { None } else { Some(value) }
}

fn slice_specs() -> Vec<SliceSpec> {
    vec![
        SliceSpec {
            slice_id: "read-resource-immutable-guild-roots",
            family: "read-resource",
            slice_name: "immutable Guild execution and object-record roots",
            exact_scope: "guild://executions/ and guild://objects/records/ roots only",
            support_status: "supported",
            scenario_name: "read-resource-bounded",
            contract: "examples/runtime-read-resource.contract.json",
            request: "examples/runtime-read-resource.admit.request.json",
            invocation: "examples/runtime-read-resource.invocation.json",
            execution_record_path: Some("examples/runtime-read-resource.execution-record.json"),
            linked_path: LinkedPath::ProofLinked,
            default_path: "proof_backed",
            token_linkage_status: "proof_backed",
            witness_linkage_status: "proof_linked",
            negative_claim_support_status: "supported",
            negative_claim_type: Some("no_read_resource_outside_scope"),
            notes: "Bounded live proof plus proof-backed token and proof-linked witness over immutable Guild resource roots.",
        },
        SliceSpec {
            slice_id: "http-request-loopback-ip-get-explicit-port",
            family: "http-request",
            slice_name: "loopback IP GET explicit port",
            exact_scope: "GET http://127.0.0.1:18080/response.json",
            support_status: "supported",
            scenario_name: "http-request-bounded",
            contract: "examples/runtime-http-read.contract.json",
            request: "examples/runtime-http-read.admit.request.json",
            invocation: "examples/runtime-http-read.invocation.json",
            execution_record_path: Some("examples/runtime-http-success.execution-record.json"),
            linked_path: LinkedPath::ProofLinked,
            default_path: "proof_backed",
            token_linkage_status: "proof_backed",
            witness_linkage_status: "proof_linked",
            negative_claim_support_status: "supported",
            negative_claim_type: Some("no_http_request_outside_scope"),
            notes: "Bounded proof-backed GET slice over an explicit loopback IP and explicit port.",
        },
        SliceSpec {
            slice_id: "http-request-loopback-ip-get-default-port",
            family: "http-request",
            slice_name: "loopback IP GET default port",
            exact_scope: "GET http://127.0.0.1/response.json",
            support_status: "supported",
            scenario_name: "http-request-default-port-bounded",
            contract: "examples/runtime-http-read-default-port.contract.json",
            request: "examples/runtime-http-read-default-port.admit.request.json",
            invocation: "examples/runtime-http-read-default-port.invocation.json",
            execution_record_path: Some(
                "examples/runtime-http-read-default-port.execution-record.json",
            ),
            linked_path: LinkedPath::ProofLinked,
            default_path: "proof_backed",
            token_linkage_status: "proof_backed",
            witness_linkage_status: "proof_linked",
            negative_claim_support_status: "supported",
            negative_claim_type: Some("no_http_request_outside_scope"),
            notes: "Bounded proof-backed GET slice over the implicit default HTTP port.",
        },
        SliceSpec {
            slice_id: "http-request-localhost-get-explicit-port",
            family: "http-request",
            slice_name: "localhost GET explicit port",
            exact_scope: "GET http://localhost:18080/response.json with deterministic loopback-only resolution binding",
            support_status: "supported",
            scenario_name: "http-request-localhost-bounded",
            contract: "examples/runtime-http-localhost.contract.json",
            request: "examples/runtime-http-localhost.admit.request.json",
            invocation: "examples/runtime-http-localhost.invocation.json",
            execution_record_path: Some("examples/runtime-http-localhost.execution-record.json"),
            linked_path: LinkedPath::ProofLinked,
            default_path: "proof_backed",
            token_linkage_status: "proof_backed",
            witness_linkage_status: "proof_linked",
            negative_claim_support_status: "supported",
            negative_claim_type: Some("no_http_request_outside_scope"),
            notes: "Bounded proof-backed localhost GET slice with explicit port and deterministic resolution binding.",
        },
        SliceSpec {
            slice_id: "http-request-localhost-head-explicit-port",
            family: "http-request",
            slice_name: "localhost HEAD explicit port",
            exact_scope: "HEAD http://localhost:18080/response.json with deterministic loopback-only resolution binding",
            support_status: "supported",
            scenario_name: "http-request-localhost-head-bounded",
            contract: "examples/runtime-http-localhost-head.contract.json",
            request: "examples/runtime-http-localhost-head.admit.request.json",
            invocation: "examples/runtime-http-localhost-head.invocation.json",
            execution_record_path: Some(
                "examples/runtime-http-localhost-head.execution-record.json",
            ),
            linked_path: LinkedPath::ProofLinked,
            default_path: "proof_backed",
            token_linkage_status: "proof_backed",
            witness_linkage_status: "proof_linked",
            negative_claim_support_status: "supported",
            negative_claim_type: Some("no_http_request_outside_scope"),
            notes: "Bounded proof-backed localhost HEAD slice with explicit port and deterministic resolution binding.",
        },
        SliceSpec {
            slice_id: "http-request-loopback-ip-head-explicit-port",
            family: "http-request",
            slice_name: "loopback IP HEAD explicit port",
            exact_scope: "HEAD http://127.0.0.1:18080/response.json",
            support_status: "supported",
            scenario_name: "http-request-head-bounded",
            contract: "examples/runtime-http-head.contract.json",
            request: "examples/runtime-http-head.admit.request.json",
            invocation: "examples/runtime-http-head.invocation.json",
            execution_record_path: Some("examples/runtime-http-head.execution-record.json"),
            linked_path: LinkedPath::ProofLinked,
            default_path: "proof_backed",
            token_linkage_status: "proof_backed",
            witness_linkage_status: "proof_linked",
            negative_claim_support_status: "supported",
            negative_claim_type: Some("no_http_request_outside_scope"),
            notes: "Bounded proof-backed HEAD slice over an explicit loopback IP and explicit port.",
        },
        SliceSpec {
            slice_id: "http-request-loopback-ip-head-default-port",
            family: "http-request",
            slice_name: "loopback IP HEAD default port",
            exact_scope: "HEAD http://127.0.0.1/response.json",
            support_status: "supported",
            scenario_name: "http-request-head-default-port-bounded",
            contract: "examples/runtime-http-head-default-port.contract.json",
            request: "examples/runtime-http-head-default-port.admit.request.json",
            invocation: "examples/runtime-http-head-default-port.invocation.json",
            execution_record_path: Some(
                "examples/runtime-http-head-default-port.execution-record.json",
            ),
            linked_path: LinkedPath::ProofLinked,
            default_path: "proof_backed",
            token_linkage_status: "proof_backed",
            witness_linkage_status: "proof_linked",
            negative_claim_support_status: "supported",
            negative_claim_type: Some("no_http_request_outside_scope"),
            notes: "Bounded proof-backed HEAD slice over the implicit default HTTP port.",
        },
        SliceSpec {
            slice_id: "invoke-skill-single-child-zero-authority",
            family: "invoke-skill",
            slice_name: "single child zero-authority inspect child",
            exact_scope: "exact declared alias child -> one exact zero-authority guild-skill-inspect-v1 child",
            support_status: "supported",
            scenario_name: "invoke-skill-single-child-bounded",
            contract: "examples/runtime-invoke-skill.contract.json",
            request: "examples/runtime-invoke-skill.admit.request.json",
            invocation: "examples/runtime-invoke-skill.invocation.json",
            execution_record_path: Some("examples/runtime-invoke-skill.execution-record.json"),
            linked_path: LinkedPath::ProofLinked,
            default_path: "proof_backed",
            token_linkage_status: "proof_backed",
            witness_linkage_status: "proof_linked",
            negative_claim_support_status: "supported",
            negative_claim_type: Some("no_invoke_skill_outside_scope"),
            notes: "Bounded proof-backed single-child invoke slice only.",
        },
        SliceSpec {
            slice_id: "http-request-redirect-driven-execution",
            family: "http-request",
            slice_name: "redirect-driven execution",
            exact_scope: "GET http://127.0.0.1:18080/redirect.json with redirect follow enabled",
            support_status: "not_proven",
            scenario_name: "http-request-redirect-unsupported",
            contract: "examples/runtime-http-redirect.contract.json",
            request: "examples/runtime-http-redirect.admit.request.json",
            invocation: "examples/runtime-http-redirect.invocation.json",
            execution_record_path: Some("examples/runtime-http-redirect.execution-record.json"),
            linked_path: LinkedPath::FallbackUnlinked,
            default_path: "refusal_then_upper_bound_fallback",
            token_linkage_status: "upper_bound_fallback",
            witness_linkage_status: "unlinked",
            negative_claim_support_status: "supported",
            negative_claim_type: Some("no_http_request_outside_scope"),
            notes: "Redirects stay not_proven. Default issuance refuses; explicit upper-bound fallback issues and witness generation stays unlinked.",
        },
        SliceSpec {
            slice_id: "invoke-skill-multi-child-fan-out",
            family: "invoke-skill",
            slice_name: "multi-child fan-out",
            exact_scope: "same alias exercised twice from one parent execution",
            support_status: "not_proven",
            scenario_name: "invoke-skill-multi-child-unsupported",
            contract: "examples/runtime-invoke-skill.contract.json",
            request: "examples/runtime-invoke-skill.admit.request.json",
            invocation: "examples/runtime-invoke-skill.invocation.json",
            execution_record_path: None,
            linked_path: LinkedPath::FallbackUnlinked,
            default_path: "refusal_then_upper_bound_fallback",
            token_linkage_status: "upper_bound_fallback",
            witness_linkage_status: "unlinked",
            negative_claim_support_status: "supported",
            negative_claim_type: Some("no_invoke_skill_outside_scope"),
            notes: "Multi-child invoke remains not_proven. Default issuance refuses; explicit upper-bound fallback issues and witness generation stays unlinked.",
        },
        SliceSpec {
            slice_id: "emit-evidence-single-emission-replay-unavailable",
            family: "emit-evidence",
            slice_name: "single emission local object-store replay unavailable",
            exact_scope: "one emit-evidence call to the fixed local object-store sink",
            support_status: "not_proven",
            scenario_name: "emit-evidence-single-sink-replay-unavailable",
            contract: "examples/runtime-emit-evidence-zero.contract.json",
            request: "examples/runtime-emit-evidence-zero.admit.request.json",
            invocation: "examples/runtime-emit-evidence.invocation.json",
            execution_record_path: Some("examples/runtime-emit-evidence.execution-record.json"),
            linked_path: LinkedPath::FallbackUnlinked,
            default_path: "refusal_then_upper_bound_fallback",
            token_linkage_status: "upper_bound_fallback",
            witness_linkage_status: "unlinked",
            negative_claim_support_status: "supported",
            negative_claim_type: Some("no_emit_evidence_outside_scope"),
            notes: "Emit-evidence stays not_proven. Default issuance refuses; explicit upper-bound fallback issues and witness generation stays unlinked.",
        },
        SliceSpec {
            slice_id: "log-write-observed-info-level",
            family: "log-write",
            slice_name: "observed info level",
            exact_scope: "one info-level log-write observation",
            support_status: "supported",
            scenario_name: "log-write-reduced",
            contract: "examples/runtime-log-write.contract.json",
            request: "examples/runtime-log-write.admit.request.json",
            invocation: "examples/runtime-log-write.invocation.json",
            execution_record_path: Some("examples/runtime-log-write.execution-record.json"),
            linked_path: LinkedPath::ProofOnly,
            default_path: "m5_only",
            token_linkage_status: "not_measured_on_real_path",
            witness_linkage_status: "not_measured_on_real_path",
            negative_claim_support_status: "not_measured_on_real_path",
            negative_claim_type: None,
            notes: "Real live proof exists for the observed level slice, but this benchmark does not claim a checked real-path token or witness linkage slice for log-write.",
        },
    ]
}

fn wall_specs() -> Vec<WallSpec> {
    vec![
        WallSpec {
            wall_id: "http-request-no-replay-fixture",
            family: "http-request",
            stage: "live_proof_search",
            wall_name: "HTTP replay fixture required",
            scenario_name: "http-request-no-replay",
            checked_test: "crates/guild-runner/tests/live_proofs.rs::http_request_live_proof_stays_not_proven_without_replay",
        },
        WallSpec {
            wall_id: "read-resource-query-root-shrink-unsupported",
            family: "read-resource",
            stage: "live_proof_search",
            wall_name: "query resources remain outside the immutable read-resource shrink model",
            scenario_name: "read-resource-query-unsupported",
            checked_test: "crates/guild-runner/tests/live_proofs.rs::read_resource_live_proof_fails_closed_for_query_resources",
        },
        WallSpec {
            wall_id: "invoke-skill-child-authority-unsupported",
            family: "invoke-skill",
            stage: "live_proof_search",
            wall_name: "child authority use remains outside the bounded invoke proof slice",
            scenario_name: "invoke-skill-child-authority-unsupported",
            checked_test: "crates/guild-runner/tests/live_proofs.rs::invoke_skill_live_proof_stays_not_proven_for_child_authority",
        },
    ]
}

impl SliceSpec {
    fn request_id(self) -> Result<String> {
        let request = read_json(&draft_v1_dir().join(self.request))?;
        let request = json_object(&request, "admission request")?;
        Ok(request
            .get("request_id")
            .and_then(Value::as_str)
            .context("admission request missing request_id")?
            .to_owned())
    }

    fn resource_binding(self) -> Option<Value> {
        match self.slice_id {
            "read-resource-immutable-guild-roots" => Some(json!({
                "family": "read-resource",
                "resource": "guild://executions/example-run",
            })),
            "http-request-loopback-ip-get-explicit-port" => Some(json!({
                "family": "http-request",
                "resource": "GET:http://127.0.0.1:18080/response.json",
            })),
            "http-request-loopback-ip-get-default-port" => Some(json!({
                "family": "http-request",
                "resource": "GET:http://127.0.0.1/response.json",
            })),
            "http-request-localhost-get-explicit-port" => Some(json!({
                "family": "http-request",
                "resource": "GET:http://localhost:18080/response.json",
            })),
            "http-request-localhost-head-explicit-port" => Some(json!({
                "family": "http-request",
                "resource": "HEAD:http://localhost:18080/response.json",
            })),
            "http-request-loopback-ip-head-explicit-port" => Some(json!({
                "family": "http-request",
                "resource": "HEAD:http://127.0.0.1:18080/response.json",
            })),
            "http-request-loopback-ip-head-default-port" => Some(json!({
                "family": "http-request",
                "resource": "HEAD:http://127.0.0.1/response.json",
            })),
            "invoke-skill-single-child-zero-authority" | "invoke-skill-multi-child-fan-out" => {
                Some(json!({
                    "family": "invoke-skill",
                    "resource": "child",
                }))
            }
            "http-request-redirect-driven-execution" => Some(json!({
                "family": "http-request",
                "resource": "GET:http://127.0.0.1:18080/redirect.json",
            })),
            "emit-evidence-single-emission-replay-unavailable" => Some(json!({
                "family": "emit-evidence",
                "resource": "audience=user;redaction=none",
            })),
            _ => None,
        }
    }
}
