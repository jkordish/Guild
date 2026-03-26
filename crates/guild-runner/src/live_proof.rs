use std::collections::BTreeSet;

use guild_registry::{InstalledSkill, SkillRegistry};
use guild_types::{
    AbiVersion, AuthorityObservation, AuthorityObservationStatus, CapabilityAccess,
    CapabilityConstraints, CapabilityGrantSet, CapabilityId, EmitEvidenceConstraints,
    EvidenceRecord, EvidenceSinkDescriptor, ExecutionRecord, ExecutionStatus, GrantedCapability,
    GuildResourceScope, GuildResourceUri, HttpMethod, HttpRequestConstraints, HttpScheme,
    InvokeDependencyConstraints, LogConstraints, ReadResourceConstraints,
    ResolvedExecutionEnvelope, ResolvedSkillRef, ResourceKind,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use url::Url;

use crate::{
    ExecutionError, INSPECT_WORLD_ENTRYPOINT, Runner, RuntimeAdapter, exact_requested_skill_ref,
    http_request_covers, invoke_dependency_grants_collectively_cover, is_redirect_status,
    log_grants_collectively_cover, read_resource_grants_collectively_cover,
    reduce_grant_to_cap_set,
};

const LIVE_PROOF_VERSION: &str = "1.0.0";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum LiveProofComparatorProfile {
    ExactOutput,
    NormalizedInspectOutputV1,
    NormalizedInspectSingleChildInvokeV1,
    NormalizedInspectSingleSinkEmitEvidenceV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum LiveProofSupport {
    LiveProofSupported,
    BoundedLiveProof,
    NotProven,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LiveProofComparatorStatus {
    Match,
    Mismatch,
    Error,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LiveProofTrialStatus {
    Succeeded,
    Failed,
    Rejected,
    ValidationError,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LiveProofEnvelope {
    pub granted_capabilities: CapabilityGrantSet,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LiveProofComparatorDescriptor {
    pub comparator_id: String,
    pub version: String,
    pub assumptions: Vec<String>,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LiveProofFamilyStatus {
    pub family: CapabilityId,
    pub support: LiveProofSupport,
    pub proof_status: Option<String>,
    pub reason_codes: Vec<String>,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LiveProofCandidateTrial {
    pub trial_id: String,
    pub family: CapabilityId,
    pub change_kind: String,
    pub candidate_envelope: LiveProofEnvelope,
    pub execution_status: LiveProofTrialStatus,
    pub comparator_status: LiveProofComparatorStatus,
    pub accepted: bool,
    pub reason_codes: Vec<String>,
    pub error_code: Option<String>,
    pub observed_families: Vec<String>,
    pub output_digest: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LiveProofOutcome {
    pub version: String,
    pub proof_status: String,
    pub comparator: LiveProofComparatorDescriptor,
    pub proven_authority: CapabilityGrantSet,
    pub residual_authority: CapabilityGrantSet,
    pub family_statuses: Vec<LiveProofFamilyStatus>,
    pub candidate_trials: Vec<LiveProofCandidateTrial>,
    pub minimization_reason_codes: Vec<String>,
    pub observed_families: Vec<String>,
    pub baseline_output_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replay_input_digest: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LiveProofScenarioResult {
    pub baseline_execution_record: ExecutionRecord,
    pub proof: LiveProofOutcome,
}

#[derive(Debug, Clone)]
struct TrialResult {
    trial: LiveProofCandidateTrial,
    matched: bool,
}

#[derive(Debug, Clone)]
struct ObservedInvokeSkillSlice {
    alias: String,
    expected_child: ResolvedSkillRef,
    child_input_digest: String,
    narrowed_capability: GrantedCapability,
}

#[derive(Debug, Clone)]
struct ObservedEmitEvidenceSlice {
    narrowed_capability: GrantedCapability,
    payload_sha256: String,
    sink: EvidenceSinkDescriptor,
}

#[allow(clippy::too_many_lines)]
pub(crate) fn prove_live_authority<A, R>(
    runner: &Runner<A>,
    registry: &R,
    installed: &InstalledSkill,
    envelope: &ResolvedExecutionEnvelope,
    comparator: LiveProofComparatorProfile,
) -> Result<LiveProofScenarioResult, ExecutionError>
where
    A: RuntimeAdapter + Clone + 'static,
    R: SkillRegistry + Clone + Send + Sync + 'static,
{
    let baseline_execution_record = runner.execute(registry, installed, envelope)?;
    let baseline_projection =
        normalized_execution_projection(registry, &baseline_execution_record, comparator);
    let baseline_output_digest = Some(sha256_json(&baseline_projection));

    let mut proven_authority = CapabilityGrantSet::default();
    let mut residual_authority = CapabilityGrantSet::default();
    let mut family_statuses = Vec::new();
    let mut candidate_trials = Vec::new();
    let mut minimization_reason_codes = Vec::new();

    for family in [
        CapabilityId::HttpRequest,
        CapabilityId::ReadResource,
        CapabilityId::InvokeSkill,
        CapabilityId::EmitEvidence,
        CapabilityId::LogWrite,
    ] {
        let family_grants = family_grants(&envelope.granted_capabilities, &family);
        if family_grants.is_empty() {
            continue;
        }

        let (proven_grants, residual_grants, mut family_trials, family_status, family_reasons) =
            match family {
                CapabilityId::ReadResource => prove_read_resource_family(
                    runner,
                    registry,
                    installed,
                    envelope,
                    &baseline_execution_record,
                    &baseline_projection,
                    comparator,
                ),
                CapabilityId::LogWrite => prove_log_write_family(
                    runner,
                    registry,
                    installed,
                    envelope,
                    &baseline_execution_record,
                    &baseline_projection,
                    comparator,
                ),
                CapabilityId::HttpRequest => prove_http_request_family(
                    runner,
                    registry,
                    installed,
                    envelope,
                    &baseline_execution_record,
                    &baseline_projection,
                    comparator,
                ),
                CapabilityId::InvokeSkill => prove_invoke_skill_family(
                    runner,
                    registry,
                    installed,
                    envelope,
                    &baseline_execution_record,
                    &baseline_projection,
                    comparator,
                ),
                CapabilityId::EmitEvidence => prove_emit_evidence_family(
                    runner,
                    registry,
                    installed,
                    envelope,
                    &baseline_execution_record,
                    &baseline_projection,
                    comparator,
                ),
                CapabilityId::GetSecret
                | CapabilityId::CacheRead
                | CapabilityId::CacheWrite
                | CapabilityId::Filesystem
                | CapabilityId::MonotonicClock
                | CapabilityId::WallClock => unsupported_family_status(
                    &family,
                    &family_grants,
                    "LIVE_PROOF_UNSUPPORTED",
                    "This family is outside the active live proof slice.",
                ),
            };

        for grant in proven_grants.grants {
            push_unique_grant(&mut proven_authority.grants, grant);
        }
        for grant in residual_grants.grants {
            push_unique_grant(&mut residual_authority.grants, grant);
        }
        candidate_trials.append(&mut family_trials);
        family_statuses.push(family_status);
        minimization_reason_codes.extend(family_reasons);
    }

    let proof_status = overall_proof_status(
        &envelope.granted_capabilities,
        &proven_authority,
        &residual_authority,
        &family_statuses,
    );
    let replay_input_digest = build_replay_input_digest(
        runner,
        registry,
        installed,
        &baseline_execution_record,
        comparator,
        &family_statuses,
    );
    if residual_authority.grants.is_empty() {
        minimization_reason_codes.push("TOKEN_PROOF_BASIS_LIVE".into());
    } else {
        minimization_reason_codes.push("PROOF_LINKAGE_UNAVAILABLE".into());
    }

    let baseline_observed_families =
        observed_families(&baseline_execution_record.authority_observations);
    Ok(LiveProofScenarioResult {
        baseline_execution_record,
        proof: LiveProofOutcome {
            version: LIVE_PROOF_VERSION.into(),
            proof_status: proof_status.into(),
            comparator: comparator_descriptor(comparator),
            proven_authority,
            residual_authority,
            family_statuses,
            candidate_trials,
            minimization_reason_codes: stable_sorted_strings(minimization_reason_codes),
            observed_families: baseline_observed_families,
            baseline_output_digest,
            replay_input_digest,
        },
    })
}

fn comparator_descriptor(profile: LiveProofComparatorProfile) -> LiveProofComparatorDescriptor {
    match profile {
        LiveProofComparatorProfile::ExactOutput => LiveProofComparatorDescriptor {
            comparator_id: "guild.runner.live-proof.exact-output.v1".into(),
            version: LIVE_PROOF_VERSION.into(),
            assumptions: vec![
                "Execution status must remain identical.".into(),
                "Structured output, diagnostics, effects, and evidence must remain byte-for-byte equivalent after canonical JSON serialization.".into(),
            ],
            notes: "Exact comparator over the persisted execution output surface.".into(),
        },
        LiveProofComparatorProfile::NormalizedInspectOutputV1 => LiveProofComparatorDescriptor {
            comparator_id: "guild.runner.live-proof.normalized-inspect-output.v1".into(),
            version: LIVE_PROOF_VERSION.into(),
            assumptions: vec![
                "Execution status must remain identical.".into(),
                "The comparator strips host-owned granted_capabilities echoes from structured output.".into(),
                "The comparator strips host-minted evidence URIs while preserving evidence metadata and digests.".into(),
            ],
            notes: "Normalized inspect comparator for deterministic bounded proof over host-owned metadata echoes.".into(),
        },
        LiveProofComparatorProfile::NormalizedInspectSingleChildInvokeV1 => {
            LiveProofComparatorDescriptor {
                comparator_id:
                    "guild.runner.live-proof.normalized-inspect-single-child-invoke.v1".into(),
                version: LIVE_PROOF_VERSION.into(),
                assumptions: vec![
                    "Execution status must remain identical.".into(),
                    "The comparator strips host-owned granted_capabilities echoes from inspect structured output.".into(),
                    "The comparator strips host-minted evidence URIs while preserving evidence metadata and digests.".into(),
                    "The comparator loads the persisted child execution record and compares the exact child digest binding, inspect ABI, canonical child input digest, normalized child output, and child execution count.".into(),
                ],
                notes: "Normalized inspect comparator for the bounded single-child invoke-skill slice. It compares the parent execution plus the persisted child execution record while ignoring host-minted execution and evidence record URIs.".into(),
            }
        }
        LiveProofComparatorProfile::NormalizedInspectSingleSinkEmitEvidenceV1 => {
            LiveProofComparatorDescriptor {
                comparator_id:
                    "guild.runner.live-proof.normalized-inspect-single-sink-emit-evidence.v1"
                        .into(),
                version: LIVE_PROOF_VERSION.into(),
                assumptions: vec![
                    "Execution status must remain identical.".into(),
                    "The comparator strips host-owned granted_capabilities echoes from inspect structured output.".into(),
                    "The comparator strips host-minted evidence record identifiers, record URIs, blob URIs, timestamps, and producing execution identifiers while preserving semantic parent linkage.".into(),
                    "The comparator preserves exact sink identity, emission count, emitted metadata, and payload digest for one fixed local object-store sink emission.".into(),
                ],
                notes: "Normalized inspect comparator for one exact single-sink emit-evidence slice. It compares normalized skill output, normalized emitted evidence records, and normalized emit-evidence observations while ignoring only truly host-minted identifiers.".into(),
            }
        }
    }
}

fn unsupported_family_status(
    family: &CapabilityId,
    family_grants: &[GrantedCapability],
    reason_code: &str,
    notes: &str,
) -> (
    CapabilityGrantSet,
    CapabilityGrantSet,
    Vec<LiveProofCandidateTrial>,
    LiveProofFamilyStatus,
    Vec<String>,
) {
    (
        CapabilityGrantSet::default(),
        CapabilityGrantSet {
            grants: family_grants.to_vec(),
        },
        Vec::new(),
        LiveProofFamilyStatus {
            family: family.clone(),
            support: LiveProofSupport::NotProven,
            proof_status: Some("not_proven".into()),
            reason_codes: vec![reason_code.into()],
            notes: notes.into(),
        },
        vec![reason_code.into()],
    )
}

#[allow(clippy::too_many_lines)]
fn prove_http_request_family<A, R>(
    runner: &Runner<A>,
    registry: &R,
    installed: &InstalledSkill,
    envelope: &ResolvedExecutionEnvelope,
    baseline_record: &ExecutionRecord,
    baseline_projection: &Value,
    comparator: LiveProofComparatorProfile,
) -> (
    CapabilityGrantSet,
    CapabilityGrantSet,
    Vec<LiveProofCandidateTrial>,
    LiveProofFamilyStatus,
    Vec<String>,
)
where
    A: RuntimeAdapter + Clone + 'static,
    R: SkillRegistry + Clone + Send + Sync + 'static,
{
    let family = CapabilityId::HttpRequest;
    let family_grants = family_grants(&envelope.granted_capabilities, &family);
    let mut trials = Vec::new();

    if comparator != LiveProofComparatorProfile::NormalizedInspectOutputV1 {
        return http_request_not_proven(
            family_grants,
            vec!["HTTP_COMPARATOR_UNSUPPORTED_FOR_LIVE_PROOF".into()],
            "Bounded http-request live proof currently supports only the normalized inspect-output comparator.",
            trials,
        );
    }

    if !runner.has_http_replay_fixtures() {
        return http_request_not_proven(
            family_grants,
            vec!["HTTP_REPLAY_FIXTURE_REQUIRED".into()],
            "Bounded http-request live proof currently requires a proof-only deterministic replay fixture catalog on the runner.",
            trials,
        );
    }

    let observed_cap = match observed_http_request_cap(baseline_record) {
        Ok(cap) => cap,
        Err((reason_code, notes)) => {
            return http_request_not_proven(
                family_grants,
                vec![reason_code.into()],
                &notes,
                trials,
            );
        }
    };

    let removal = run_family_trial(
        runner,
        registry,
        installed,
        envelope,
        &family,
        "remove_grant",
        remove_family_grants(&envelope.granted_capabilities, &family),
        baseline_projection,
        comparator,
        "trial-http-request-remove-family",
    );
    let removal_matched = removal.matched;
    trials.push(removal.trial);
    if removal_matched {
        let reasons = vec![
            "LIVE_PROOF_BOUNDED".into(),
            "LIVE_PROOF_SUPPORTED".into(),
            "HTTP_LIVE_PROOF_BOUNDED".into(),
        ];
        return (
            CapabilityGrantSet::default(),
            CapabilityGrantSet::default(),
            trials,
            LiveProofFamilyStatus {
                family,
                support: LiveProofSupport::BoundedLiveProof,
                proof_status: Some("bounded_minimal".into()),
                reason_codes: stable_sorted_strings(reasons.clone()),
                notes: "The family was removable for this invocation under the bounded fixture-backed HTTP replay slice: one loopback IP GET or HEAD request with either an explicit port or the default HTTP port, or one localhost GET or HEAD request with either an explicit port or the implicit default HTTP port plus a deterministic loopback-only resolution binding.".into(),
            },
            reasons,
        );
    }

    let observed_refs = vec![&observed_cap];
    let mut reduced_family = Vec::new();
    for grant in &family_grants {
        for reduced in reduce_grant_to_cap_set(&observed_refs, grant) {
            push_unique_grant(&mut reduced_family, reduced);
        }
    }

    let CapabilityConstraints::HttpRequest(expected) = &observed_cap.constraints else {
        unreachable!("observed http-request cap always carries HTTP constraints");
    };
    if !reduced_family.iter().any(|grant| match &grant.constraints {
        CapabilityConstraints::HttpRequest(value) => http_request_covers(value, expected),
        _ => false,
    }) {
        return http_request_not_proven(
            family_grants,
            vec!["HTTP_REQUEST_SHAPE_UNSUPPORTED".into()],
            "The bounded http-request reduction could not preserve the exercised request envelope under the current typed HTTP scope model.",
            trials,
        );
    }

    let reduction = run_family_trial(
        runner,
        registry,
        installed,
        envelope,
        &family,
        "shrink_scope",
        replace_family_grants(
            &envelope.granted_capabilities,
            &family,
            CapabilityGrantSet {
                grants: reduced_family.clone(),
            },
        ),
        baseline_projection,
        comparator,
        "trial-http-request-shrink-observed-request",
    );
    let reduction_matched = reduction.matched;
    trials.push(reduction.trial);

    if !reduction_matched {
        return http_request_not_proven(
            family_grants,
            vec!["LIVE_PROOF_PREREQUISITE_FAILED".into()],
            "The bounded http-request replay shrink changed execution behavior under the live comparator, so proof failed closed.",
            trials,
        );
    }

    let proof_status = if reduced_family == family_grants {
        "no_reduction"
    } else {
        "bounded_minimal"
    };
    let reasons = vec![
        "LIVE_PROOF_BOUNDED".into(),
        "LIVE_PROOF_SUPPORTED".into(),
        "HTTP_LIVE_PROOF_BOUNDED".into(),
    ];
    (
        CapabilityGrantSet {
            grants: reduced_family,
        },
        CapabilityGrantSet::default(),
        trials,
        LiveProofFamilyStatus {
            family,
            support: LiveProofSupport::BoundedLiveProof,
            proof_status: Some(proof_status.into()),
            reason_codes: stable_sorted_strings(reasons.clone()),
            notes: "Bounded live proof for http-request currently covers only one replay-fixtured request under the normalized inspect-output comparator: a loopback IP-literal GET or HEAD with either an explicit port or the implicit default HTTP port, or a localhost GET or HEAD with either an explicit port or the implicit default HTTP port plus a deterministic loopback-only resolution binding. All slices remain exact-path, query-free, redirect-free, and single-request only.".into(),
        },
        reasons,
    )
}

fn http_request_not_proven(
    family_grants: Vec<GrantedCapability>,
    reasons: Vec<String>,
    notes: &str,
    trials: Vec<LiveProofCandidateTrial>,
) -> (
    CapabilityGrantSet,
    CapabilityGrantSet,
    Vec<LiveProofCandidateTrial>,
    LiveProofFamilyStatus,
    Vec<String>,
) {
    (
        CapabilityGrantSet::default(),
        CapabilityGrantSet {
            grants: family_grants,
        },
        trials,
        LiveProofFamilyStatus {
            family: CapabilityId::HttpRequest,
            support: LiveProofSupport::NotProven,
            proof_status: Some("not_proven".into()),
            reason_codes: stable_sorted_strings(reasons.clone()),
            notes: notes.into(),
        },
        reasons,
    )
}

fn invoke_skill_not_proven(
    family_grants: Vec<GrantedCapability>,
    reasons: Vec<String>,
    notes: &str,
    trials: Vec<LiveProofCandidateTrial>,
) -> (
    CapabilityGrantSet,
    CapabilityGrantSet,
    Vec<LiveProofCandidateTrial>,
    LiveProofFamilyStatus,
    Vec<String>,
) {
    (
        CapabilityGrantSet::default(),
        CapabilityGrantSet {
            grants: family_grants,
        },
        trials,
        LiveProofFamilyStatus {
            family: CapabilityId::InvokeSkill,
            support: LiveProofSupport::NotProven,
            proof_status: Some("not_proven".into()),
            reason_codes: stable_sorted_strings(reasons.clone()),
            notes: notes.into(),
        },
        reasons,
    )
}

fn emit_evidence_not_proven(
    family_grants: Vec<GrantedCapability>,
    reasons: Vec<String>,
    notes: &str,
    trials: Vec<LiveProofCandidateTrial>,
) -> (
    CapabilityGrantSet,
    CapabilityGrantSet,
    Vec<LiveProofCandidateTrial>,
    LiveProofFamilyStatus,
    Vec<String>,
) {
    (
        CapabilityGrantSet::default(),
        CapabilityGrantSet {
            grants: family_grants,
        },
        trials,
        LiveProofFamilyStatus {
            family: CapabilityId::EmitEvidence,
            support: LiveProofSupport::NotProven,
            proof_status: Some("not_proven".into()),
            reason_codes: stable_sorted_strings(reasons.clone()),
            notes: notes.into(),
        },
        reasons,
    )
}

#[allow(clippy::too_many_lines)]
fn prove_emit_evidence_family<A, R>(
    runner: &Runner<A>,
    registry: &R,
    installed: &InstalledSkill,
    envelope: &ResolvedExecutionEnvelope,
    baseline_record: &ExecutionRecord,
    baseline_projection: &Value,
    comparator: LiveProofComparatorProfile,
) -> (
    CapabilityGrantSet,
    CapabilityGrantSet,
    Vec<LiveProofCandidateTrial>,
    LiveProofFamilyStatus,
    Vec<String>,
)
where
    A: RuntimeAdapter + Clone + 'static,
    R: SkillRegistry + Clone + Send + Sync + 'static,
{
    let family = CapabilityId::EmitEvidence;
    let family_grants = family_grants(&envelope.granted_capabilities, &family);
    let mut trials = Vec::new();

    if comparator != LiveProofComparatorProfile::NormalizedInspectSingleSinkEmitEvidenceV1 {
        return emit_evidence_not_proven(
            family_grants,
            vec!["EMIT_EVIDENCE_COMPARATOR_UNAVAILABLE".into()],
            "Bounded emit-evidence feasibility checking currently requires the dedicated single-sink emit comparator.",
            trials,
        );
    }

    let observed = match observed_emit_evidence_slice(baseline_record) {
        Ok(observed) => observed,
        Err((reason_code, notes)) => {
            return emit_evidence_not_proven(
                family_grants,
                vec![reason_code.into()],
                &notes,
                trials,
            );
        }
    };

    let removal = run_family_trial(
        runner,
        registry,
        installed,
        envelope,
        &family,
        "remove_grant",
        remove_family_grants(&envelope.granted_capabilities, &family),
        baseline_projection,
        comparator,
        "trial-emit-evidence-remove-family",
    );
    trials.push(removal.trial);

    let mut reduced_family = Vec::new();
    for grant in &family_grants {
        let CapabilityConstraints::EmitEvidence(grant_constraints) = &grant.constraints else {
            continue;
        };
        let CapabilityConstraints::EmitEvidence(expected) =
            &observed.narrowed_capability.constraints
        else {
            unreachable!("observed emit-evidence cap always carries emit constraints");
        };
        if emit_evidence_constraints_cover(grant_constraints, expected) {
            push_unique_grant(
                &mut reduced_family,
                GrantedCapability {
                    id: grant.id.clone(),
                    access: grant.access.clone(),
                    constraints: observed.narrowed_capability.constraints.clone(),
                },
            );
        }
    }

    if reduced_family.is_empty() {
        return emit_evidence_not_proven(
            family_grants,
            vec!["LIVE_SCOPE_SHRINK_UNSUPPORTED".into()],
            "The bounded emit-evidence reduction could not preserve the exact exercised payload size, audience, and redaction envelope.",
            trials,
        );
    }

    let reduction = run_family_trial(
        runner,
        registry,
        installed,
        envelope,
        &family,
        "shrink_scope",
        replace_family_grants(
            &envelope.granted_capabilities,
            &family,
            CapabilityGrantSet {
                grants: reduced_family,
            },
        ),
        baseline_projection,
        comparator,
        "trial-emit-evidence-shrink-observed-slice",
    );
    let reduction_matched = reduction.matched;
    trials.push(reduction.trial);

    if !reduction_matched {
        return emit_evidence_not_proven(
            family_grants,
            vec!["EMIT_EVIDENCE_REPLAY_UNAVAILABLE".into()],
            "The exact single-sink emit-evidence slice did not re-execute equivalently under the dedicated comparator, so proof failed closed.",
            trials,
        );
    }

    let reasons = vec!["EMIT_EVIDENCE_LINKAGE_MODEL_UNAVAILABLE".into()];
    let notes = format!(
        "Guild can re-execute and compare one exact single-emission local object-store sink slice conservatively, but the current live emit-evidence grant shape and draft-v1 control-plane do not model exact sink identity and payload digest as first-class authority. The family remains not_proven rather than issuing a broader proof envelope. The checked slice bound sink kind `{}`, record namespace `{}`, blob namespace `{}`, routing mode `{:?}`, storage class `{:?}`, and payload digest `{}`.",
        serde_json::to_string(&observed.sink.kind).expect("sink kind serializes"),
        observed.sink.record_uri_prefix,
        observed.sink.blob_uri_prefix,
        observed.sink.routing_mode,
        observed.sink.storage_class,
        observed.payload_sha256,
    );
    emit_evidence_not_proven(family_grants, reasons, &notes, trials)
}

#[allow(clippy::too_many_lines)]
fn prove_invoke_skill_family<A, R>(
    runner: &Runner<A>,
    registry: &R,
    installed: &InstalledSkill,
    envelope: &ResolvedExecutionEnvelope,
    baseline_record: &ExecutionRecord,
    baseline_projection: &Value,
    comparator: LiveProofComparatorProfile,
) -> (
    CapabilityGrantSet,
    CapabilityGrantSet,
    Vec<LiveProofCandidateTrial>,
    LiveProofFamilyStatus,
    Vec<String>,
)
where
    A: RuntimeAdapter + Clone + 'static,
    R: SkillRegistry + Clone + Send + Sync + 'static,
{
    let family = CapabilityId::InvokeSkill;
    let family_grants = family_grants(&envelope.granted_capabilities, &family);
    let mut trials = Vec::new();

    if comparator != LiveProofComparatorProfile::NormalizedInspectSingleChildInvokeV1 {
        return invoke_skill_not_proven(
            family_grants,
            vec!["INVOKE_SKILL_COMPARATOR_UNAVAILABLE".into()],
            "Bounded invoke-skill live proof currently supports only the normalized inspect single-child comparator.",
            trials,
        );
    }

    let observed = match observed_invoke_skill_slice(registry, installed, baseline_record) {
        Ok(observed) => observed,
        Err((reason_code, notes)) => {
            return invoke_skill_not_proven(
                family_grants,
                vec![reason_code.into()],
                &notes,
                trials,
            );
        }
    };

    let removal = run_family_trial(
        runner,
        registry,
        installed,
        envelope,
        &family,
        "remove_grant",
        remove_family_grants(&envelope.granted_capabilities, &family),
        baseline_projection,
        comparator,
        "trial-invoke-skill-remove-family",
    );
    let removal_matched = removal.matched;
    trials.push(removal.trial);
    if removal_matched {
        let reasons = vec![
            "INVOKE_SKILL_LIVE_PROOF_BOUNDED".into(),
            "LIVE_PROOF_BOUNDED".into(),
            "LIVE_PROOF_SUPPORTED".into(),
        ];
        return (
            CapabilityGrantSet::default(),
            CapabilityGrantSet::default(),
            trials,
            LiveProofFamilyStatus {
                family,
                support: LiveProofSupport::BoundedLiveProof,
                proof_status: Some("bounded_minimal".into()),
                reason_codes: stable_sorted_strings(reasons.clone()),
                notes: "The family was removable for this invocation under the bounded single-child invoke comparator, which compares the parent output together with the persisted child execution record.".into(),
            },
            reasons,
        );
    }

    let observed_refs = vec![&observed.narrowed_capability];
    let mut reduced_family = Vec::new();
    for grant in &family_grants {
        for reduced in reduce_grant_to_cap_set(&observed_refs, grant) {
            push_unique_grant(&mut reduced_family, reduced);
        }
    }

    let CapabilityConstraints::InvokeDependency(expected) =
        &observed.narrowed_capability.constraints
    else {
        unreachable!("observed invoke-skill cap always carries invoke constraints");
    };
    if !invoke_dependency_grants_collectively_cover(&reduced_family, expected) {
        return invoke_skill_not_proven(
            family_grants,
            vec!["LIVE_SCOPE_SHRINK_UNSUPPORTED".into()],
            "The bounded invoke-skill reduction could not preserve the exact exercised dependency alias.",
            trials,
        );
    }

    let reduction = run_family_trial(
        runner,
        registry,
        installed,
        envelope,
        &family,
        "shrink_scope",
        replace_family_grants(
            &envelope.granted_capabilities,
            &family,
            CapabilityGrantSet {
                grants: reduced_family.clone(),
            },
        ),
        baseline_projection,
        comparator,
        "trial-invoke-skill-shrink-observed-alias",
    );
    let reduction_matched = reduction.matched;
    trials.push(reduction.trial);

    if !reduction_matched {
        return invoke_skill_not_proven(
            family_grants,
            vec!["LIVE_PROOF_PREREQUISITE_FAILED".into()],
            "The bounded invoke-skill shrink changed parent or child execution behavior under the live comparator, so proof failed closed.",
            trials,
        );
    }

    let proof_status = if reduced_family == family_grants {
        "no_reduction"
    } else {
        "bounded_minimal"
    };
    let reasons = vec![
        "INVOKE_SKILL_LIVE_PROOF_BOUNDED".into(),
        "LIVE_PROOF_BOUNDED".into(),
        "LIVE_PROOF_SUPPORTED".into(),
    ];
    (
        CapabilityGrantSet {
            grants: reduced_family,
        },
        CapabilityGrantSet::default(),
        trials,
        LiveProofFamilyStatus {
            family,
            support: LiveProofSupport::BoundedLiveProof,
            proof_status: Some(proof_status.into()),
            reason_codes: stable_sorted_strings(reasons.clone()),
            notes: "Bounded live proof for invoke-skill currently covers only one exercised declared dependency alias resolved through the installed dependency snapshot to one exact child digest, fixed guild-skill-inspect-v1 ABI, deterministic child input, zero child-side authority use, and zero nested child executions.".into(),
        },
        reasons,
    )
}

#[allow(clippy::too_many_lines)]
fn observed_invoke_skill_slice<R>(
    registry: &R,
    installed: &InstalledSkill,
    record: &ExecutionRecord,
) -> Result<ObservedInvokeSkillSlice, (&'static str, String)>
where
    R: SkillRegistry + ?Sized,
{
    let mut exercised = Vec::new();
    let mut blocked = Vec::new();

    for observation in &record.authority_observations {
        let AuthorityObservation::InvokeSkill { status, detail } = observation else {
            continue;
        };
        match status {
            AuthorityObservationStatus::Exercised => exercised.push(detail),
            AuthorityObservationStatus::Blocked => blocked.push(detail),
        }
    }

    if exercised.len() != 1 || record.child_executions.len() != 1 {
        let reason_code = if exercised.len() > 1 || record.child_executions.len() > 1 {
            "INVOKE_SKILL_MULTI_CHILD_UNSUPPORTED"
        } else {
            "INVOKE_SKILL_REPLAY_UNAVAILABLE"
        };
        let notes = if reason_code == "INVOKE_SKILL_MULTI_CHILD_UNSUPPORTED" {
            "Bounded invoke-skill live proof currently requires exactly one exercised child invocation and exactly one persisted child execution record.".into()
        } else {
            "Bounded invoke-skill live proof requires exactly one exercised child invocation together with one persisted child execution record.".into()
        };
        return Err((reason_code, notes));
    }

    let exercised = exercised[0];
    if exercised.result_error.is_some() {
        return Err((
            "INVOKE_SKILL_REPLAY_UNAVAILABLE",
            "Bounded invoke-skill live proof currently requires a child invocation with a persisted child execution record and no host-side invoke result error.".into(),
        ));
    }

    if !blocked.is_empty() {
        let reason_code = if blocked.iter().any(|detail| detail.alias != exercised.alias) {
            "INVOKE_SKILL_DYNAMIC_RESOLUTION_UNSUPPORTED"
        } else {
            "INVOKE_SKILL_MULTI_CHILD_UNSUPPORTED"
        };
        let notes = if reason_code == "INVOKE_SKILL_DYNAMIC_RESOLUTION_UNSUPPORTED" {
            "Bounded invoke-skill live proof excludes executions that mixed the exercised child with other blocked alias targets or broader invoke dispatch behavior.".into()
        } else {
            "Bounded invoke-skill live proof requires a baseline execution with no blocked invoke attempts.".into()
        };
        return Err((reason_code, notes));
    }

    let child_summary = &record.child_executions[0];
    if child_summary.alias != exercised.alias
        || exercised.child_execution_id.as_deref() != Some(&child_summary.execution_id)
        || exercised.child_status.as_ref() != Some(&child_summary.status)
    {
        return Err((
            "INVOKE_SKILL_CHILD_IDENTITY_UNBOUND",
            "The baseline invoke-skill observation did not bind cleanly to one persisted child execution record.".into(),
        ));
    }

    let Some(dependency) = installed
        .manifest
        .dependencies
        .iter()
        .find(|dependency| dependency.alias == exercised.alias)
    else {
        return Err((
            "INVOKE_SKILL_DYNAMIC_RESOLUTION_UNSUPPORTED",
            "The exercised invoke-skill alias was not bound through the parent's declared installed dependency snapshot.".into(),
        ));
    };

    let expected_child = dependency.skill.clone();
    let child_record = registry
        .load_execution_record(&child_summary.execution_id)
        .map_err(|error| {
            (
                "INVOKE_SKILL_REPLAY_UNAVAILABLE",
                format!(
                    "The exercised invoke-skill child execution record could not be reloaded for comparison: {}",
                    error.message
                ),
            )
        })?;

    if child_record.parent_execution_id.as_deref() != Some(&record.receipt.execution_id)
        || child_record.resolved_skill != expected_child
        || child_record.provenance.resolved_skill != expected_child
        || child_summary.provenance.resolved_skill != expected_child
        || child_summary.parent_execution_id != record.receipt.execution_id
        || child_summary.status != child_record.status
        || child_summary.termination != child_record.termination
    {
        return Err((
            "INVOKE_SKILL_CHILD_IDENTITY_UNBOUND",
            "The exercised invoke-skill child did not stay bound to the exact dependency digest across the parent summary and the persisted child execution record.".into(),
        ));
    }

    if child_record.request.skill != exact_requested_skill_ref(&expected_child) {
        return Err((
            "INVOKE_SKILL_CHILD_IDENTITY_UNBOUND",
            "The persisted child execution request did not stay exact digest-pinned to the declared installed dependency snapshot.".into(),
        ));
    }

    let child_installed = registry.resolve_exact(&expected_child).map_err(|error| {
        (
            "INVOKE_SKILL_CHILD_IDENTITY_UNBOUND",
            format!(
                "The declared installed dependency snapshot could not be reloaded as an exact child executable: {}",
                error.message
            ),
        )
    })?;

    if child_installed.manifest.runtime.entrypoint != INSPECT_WORLD_ENTRYPOINT
        || child_installed.manifest.runtime.guest_abi_version != AbiVersion::GuildSkillInspectV1
        || child_record.provenance.abi != AbiVersion::GuildSkillInspectV1
    {
        return Err((
            "INVOKE_SKILL_EXPORT_WORLD_UNSUPPORTED",
            "Bounded invoke-skill live proof currently supports only child executions that stay on the fixed guild-skill-inspect-v1 world and ABI.".into(),
        ));
    }

    if !child_summary.granted_capabilities.grants.is_empty()
        || !child_record.granted_capabilities.grants.is_empty()
        || !child_record.authority_observations.is_empty()
    {
        return Err((
            "INVOKE_SKILL_CHILD_AUTHORITY_UNSUPPORTED",
            "Bounded invoke-skill live proof currently supports only zero-authority children with no exercised child-side capability families.".into(),
        ));
    }

    if !child_record.child_executions.is_empty() || child_record.metrics.child_executions > 0 {
        return Err((
            "INVOKE_SKILL_RECURSION_UNSUPPORTED",
            "Bounded invoke-skill live proof currently excludes nested child executions and deeper call graphs.".into(),
        ));
    }

    Ok(ObservedInvokeSkillSlice {
        alias: exercised.alias.clone(),
        child_input_digest: sha256_json(&child_record.request.input),
        expected_child,
        narrowed_capability: GrantedCapability {
            id: CapabilityId::InvokeSkill,
            access: CapabilityAccess::Invoke,
            constraints: CapabilityConstraints::InvokeDependency(InvokeDependencyConstraints {
                aliases: Some(vec![exercised.alias.clone()]),
            }),
        },
    })
}

#[allow(clippy::too_many_lines)]
fn observed_http_request_cap(
    record: &ExecutionRecord,
) -> Result<GrantedCapability, (&'static str, String)> {
    let mut exercised = Vec::new();
    let mut blocked = Vec::new();

    for observation in &record.authority_observations {
        let AuthorityObservation::HttpRequest { status, detail } = observation else {
            continue;
        };
        match status {
            AuthorityObservationStatus::Exercised => exercised.push(detail),
            AuthorityObservationStatus::Blocked => blocked.push(detail),
        }
    }

    if exercised.len() != 1 {
        let redirect_like = exercised.iter().any(|detail| {
            detail.redirects_followed.unwrap_or(0) > 0
                || detail.response_status.is_some_and(is_redirect_status)
        });
        if redirect_like {
            return Err((
                "HTTP_REDIRECTS_UNSUPPORTED",
                "Bounded http-request live proof currently excludes redirect-driven executions, including successful redirect follow chains.".into(),
            ));
        }
        return Err((
            "HTTP_MULTI_REQUEST_UNSUPPORTED",
            "Bounded http-request live proof currently requires exactly one exercised HTTP request in the baseline execution.".into(),
        ));
    }

    if !blocked.is_empty() {
        let redirect_blocked = blocked.iter().any(|detail| {
            detail
                .denial
                .as_ref()
                .is_some_and(|denial| denial.code.starts_with("http-request-redirect"))
        });
        let reason_code = if redirect_blocked {
            "HTTP_REDIRECTS_UNSUPPORTED"
        } else {
            "HTTP_REQUEST_SHAPE_UNSUPPORTED"
        };
        let notes = if redirect_blocked {
            "Bounded http-request live proof currently excludes redirect-driven executions and blocked redirect follow-up hops.".into()
        } else {
            "Bounded http-request live proof currently requires a baseline execution with no blocked HTTP attempts.".into()
        };
        return Err((reason_code, notes));
    }

    let detail = exercised[0];
    if detail.result_error.is_some() {
        return Err((
            "HTTP_REQUEST_SHAPE_UNSUPPORTED",
            "Bounded http-request live proof currently requires a successful buffered response with no host result error.".into(),
        ));
    }
    if detail.redirects_followed.unwrap_or(0) > 0
        || detail.response_status.is_some_and(is_redirect_status)
    {
        return Err((
            "HTTP_REDIRECTS_UNSUPPORTED",
            "Bounded http-request live proof currently excludes redirect responses and redirect-following executions.".into(),
        ));
    }

    let request = &detail.request;
    if !matches!(request.method, HttpMethod::Get | HttpMethod::Head) {
        return Err((
            "HTTP_REQUEST_SHAPE_UNSUPPORTED",
            "Bounded http-request live proof currently supports GET and HEAD requests only.".into(),
        ));
    }

    let parsed_url = Url::parse(&request.url).map_err(|error| {
        (
            "HTTP_REQUEST_SHAPE_UNSUPPORTED",
            format!(
                "The exercised http-request URL could not be reparsed safely for live proof: {error}"
            ),
        )
    })?;
    if parsed_url.query().is_some() || parsed_url.fragment().is_some() {
        return Err((
            "HTTP_QUERY_UNSUPPORTED",
            "Bounded http-request live proof currently excludes query and fragment components because the live HTTP scope model does not enforce them directly.".into(),
        ));
    }

    let scheme = match parsed_url.scheme() {
        "http" => HttpScheme::Http,
        "https" => HttpScheme::Https,
        _ => {
            return Err((
                "HTTP_REQUEST_SHAPE_UNSUPPORTED",
                "The exercised http-request URL used an unsupported scheme.".into(),
            ));
        }
    };
    if scheme != HttpScheme::Http {
        return Err((
            "HTTP_SCHEME_UNSUPPORTED_FOR_LIVE_PROOF",
            "Bounded http-request live proof currently supports only HTTP loopback replay fixtures.".into(),
        ));
    }

    let Some(host) = parsed_url.host_str() else {
        return Err((
            "HTTP_REQUEST_SHAPE_UNSUPPORTED",
            "The exercised http-request URL did not expose a host for live proof normalization."
                .into(),
        ));
    };
    let Some(port) = parsed_url.port_or_known_default() else {
        return Err((
            "HTTP_REQUEST_SHAPE_UNSUPPORTED",
            "The exercised http-request URL did not resolve to an explicit or default port.".into(),
        ));
    };
    let host = host.to_ascii_lowercase();
    let allow_ip_literals = if let Ok(host_ip) = host.parse::<std::net::IpAddr>() {
        if !host_ip.is_loopback() {
            return Err((
                "HTTP_HOST_UNSUPPORTED_FOR_LIVE_PROOF",
                "Bounded http-request live proof currently requires either a loopback IP-literal host or exact localhost.".into(),
            ));
        }
        true
    } else {
        if host != "localhost" {
            return Err((
                "HTTP_HOST_UNSUPPORTED_FOR_LIVE_PROOF",
                "Bounded http-request live proof currently supports hostname replay only for exact localhost.".into(),
            ));
        }
        if !matches!(request.method, HttpMethod::Get | HttpMethod::Head) {
            return Err((
                "HTTP_HOST_UNSUPPORTED_FOR_LIVE_PROOF",
                "Bounded http-request hostname live proof currently supports only localhost GET and HEAD requests within the bounded replay-backed slice.".into(),
            ));
        }
        if parsed_url.port().is_none() && port != 80 {
            return Err((
                "HTTP_HOST_UNSUPPORTED_FOR_LIVE_PROOF",
                "Bounded http-request hostname live proof currently supports only an explicit port or the implicit default HTTP port in the localhost URL.".into(),
            ));
        }
        let Some(resolution) = detail.resolution.as_ref() else {
            return Err((
                "HTTP_HOST_RESOLUTION_BINDING_UNAVAILABLE",
                "Bounded localhost http-request live proof requires a deterministic host-owned resolution binding in the exercised observation.".into(),
            ));
        };
        if resolution.requested_host.to_ascii_lowercase() != host || resolution.port != port {
            return Err((
                "HTTP_HOST_RESOLUTION_BINDING_UNSAFE",
                "The exercised localhost http-request resolution binding did not match the observed host and port.".into(),
            ));
        }
        if resolution.addresses.is_empty() {
            return Err((
                "HTTP_HOST_RESOLUTION_BINDING_UNSAFE",
                "The exercised localhost http-request resolution binding did not retain any resolved addresses.".into(),
            ));
        }
        let mut all_loopback = true;
        for address in &resolution.addresses {
            let parsed_address = address.address.parse::<std::net::IpAddr>().map_err(|error| {
                (
                    "HTTP_HOST_RESOLUTION_BINDING_UNSAFE",
                    format!(
                        "The exercised localhost http-request resolution binding contained an invalid IP literal: {error}"
                    ),
                )
            })?;
            let expected_family = match parsed_address {
                std::net::IpAddr::V4(_) => "ipv4",
                std::net::IpAddr::V6(_) => "ipv6",
            };
            let observed_family = match address.family {
                guild_types::HttpAddressFamily::Ipv4 => "ipv4",
                guild_types::HttpAddressFamily::Ipv6 => "ipv6",
            };
            if expected_family != observed_family {
                return Err((
                    "HTTP_HOST_RESOLUTION_BINDING_UNSAFE",
                    "The exercised localhost http-request resolution binding recorded an address family mismatch.".into(),
                ));
            }
            all_loopback &= parsed_address.is_loopback();
        }
        if !all_loopback || !resolution.loopback_only {
            return Err((
                "HTTP_HOST_RESOLUTION_BINDING_UNSAFE",
                "Bounded localhost http-request live proof requires loopback-only resolution semantics.".into(),
            ));
        }
        false
    };
    let path = if parsed_url.path().is_empty() {
        "/".to_owned()
    } else {
        parsed_url.path().to_owned()
    };

    Ok(GrantedCapability {
        id: CapabilityId::HttpRequest,
        access: CapabilityAccess::Read,
        constraints: CapabilityConstraints::HttpRequest(HttpRequestConstraints {
            allowed_schemes: Some(vec![HttpScheme::Http]),
            allowed_hosts: Some(vec![host.clone()]),
            allowed_host_suffixes: None,
            allowed_ports: Some(vec![port]),
            allowed_methods: Some(vec![request.method.clone()]),
            allowed_path_prefixes: Some(vec![path]),
            max_timeout_ms: None,
            max_response_bytes: None,
            follow_redirects: Some(false),
            max_redirects: None,
            allow_loopback: Some(true),
            allow_link_local: Some(false),
            allow_private_networks: Some(false),
            allow_ip_literals: Some(allow_ip_literals),
        }),
    })
}

#[allow(clippy::too_many_lines)]
fn prove_read_resource_family<A, R>(
    runner: &Runner<A>,
    registry: &R,
    installed: &InstalledSkill,
    envelope: &ResolvedExecutionEnvelope,
    baseline_record: &ExecutionRecord,
    baseline_projection: &Value,
    comparator: LiveProofComparatorProfile,
) -> (
    CapabilityGrantSet,
    CapabilityGrantSet,
    Vec<LiveProofCandidateTrial>,
    LiveProofFamilyStatus,
    Vec<String>,
)
where
    A: RuntimeAdapter + Clone + 'static,
    R: SkillRegistry + Clone + Send + Sync + 'static,
{
    let family = CapabilityId::ReadResource;
    let family_grants = family_grants(&envelope.granted_capabilities, &family);
    let mut trials = Vec::new();
    let mut reasons = vec!["LIVE_PROOF_BOUNDED".into()];

    let removal = run_family_trial(
        runner,
        registry,
        installed,
        envelope,
        &family,
        "remove_grant",
        remove_family_grants(&envelope.granted_capabilities, &family),
        baseline_projection,
        comparator,
        "trial-read-resource-remove-family",
    );
    let removal_matched = removal.matched;
    trials.push(removal.trial);
    if removal_matched {
        reasons.push("LIVE_PROOF_SUPPORTED".into());
        return (
            CapabilityGrantSet::default(),
            CapabilityGrantSet::default(),
            trials,
            LiveProofFamilyStatus {
                family,
                support: LiveProofSupport::BoundedLiveProof,
                proof_status: Some("bounded_minimal".into()),
                reason_codes: stable_sorted_strings(reasons.clone()),
                notes: "The family was removable for this invocation under the bounded immutable resource-root search model.".into(),
            },
            reasons,
        );
    }

    let Some(observed_caps) = observed_read_resource_caps(baseline_record) else {
        reasons.push("LIVE_SCOPE_SHRINK_UNSUPPORTED".into());
        return (
            CapabilityGrantSet::default(),
            CapabilityGrantSet {
                grants: family_grants,
            },
            trials,
            LiveProofFamilyStatus {
                family,
                support: LiveProofSupport::NotProven,
                proof_status: Some("not_proven".into()),
                reason_codes: stable_sorted_strings(reasons.clone()),
                notes: "Observed read-resource scopes were not all immutable execution or object-record roots, so the bounded search failed closed.".into(),
            },
            reasons,
        );
    };

    let observed_refs = observed_caps.iter().collect::<Vec<_>>();
    let mut reduced_family = Vec::new();
    for grant in &family_grants {
        for reduced in reduce_grant_to_cap_set(&observed_refs, grant) {
            push_unique_grant(&mut reduced_family, reduced);
        }
    }

    let expected = ReadResourceConstraints {
        uri_prefixes: Some(
            observed_caps
                .iter()
                .flat_map(|grant| match &grant.constraints {
                    CapabilityConstraints::ReadResource(value) => {
                        value.uri_prefixes.clone().unwrap_or_default()
                    }
                    _ => Vec::new(),
                })
                .collect(),
        ),
        resource_kinds: Some(
            observed_caps
                .iter()
                .flat_map(|grant| match &grant.constraints {
                    CapabilityConstraints::ReadResource(value) => {
                        value.resource_kinds.clone().unwrap_or_default()
                    }
                    _ => Vec::new(),
                })
                .collect(),
        ),
    };
    if !read_resource_grants_collectively_cover(&reduced_family, &expected) {
        reasons.push("LIVE_SCOPE_SHRINK_UNSUPPORTED".into());
        return (
            CapabilityGrantSet::default(),
            CapabilityGrantSet {
                grants: family_grants,
            },
            trials,
            LiveProofFamilyStatus {
                family,
                support: LiveProofSupport::NotProven,
                proof_status: Some("not_proven".into()),
                reason_codes: stable_sorted_strings(reasons.clone()),
                notes: "The bounded read-resource reduction could not preserve observed immutable scope coverage.".into(),
            },
            reasons,
        );
    }

    let reduced_envelope = replace_family_grants(
        &envelope.granted_capabilities,
        &family,
        CapabilityGrantSet {
            grants: reduced_family.clone(),
        },
    );
    let reduction = run_family_trial(
        runner,
        registry,
        installed,
        envelope,
        &family,
        "shrink_scope",
        reduced_envelope,
        baseline_projection,
        comparator,
        "trial-read-resource-shrink-observed-roots",
    );
    let reduction_matched = reduction.matched;
    trials.push(reduction.trial);

    if !reduction_matched {
        reasons.push("LIVE_PROOF_PREREQUISITE_FAILED".into());
        return (
            CapabilityGrantSet::default(),
            CapabilityGrantSet {
                grants: family_grants,
            },
            trials,
            LiveProofFamilyStatus {
                family,
                support: LiveProofSupport::NotProven,
                proof_status: Some("not_proven".into()),
                reason_codes: stable_sorted_strings(reasons.clone()),
                notes: "The bounded read-resource shrink changed execution behavior under the live comparator, so proof failed closed.".into(),
            },
            reasons,
        );
    }

    let proof_status = if reduced_family == family_grants {
        "no_reduction"
    } else {
        "bounded_minimal"
    };
    reasons.push("LIVE_PROOF_SUPPORTED".into());
    (
        CapabilityGrantSet {
            grants: reduced_family,
        },
        CapabilityGrantSet::default(),
        trials,
        LiveProofFamilyStatus {
            family,
            support: LiveProofSupport::BoundedLiveProof,
            proof_status: Some(proof_status.into()),
            reason_codes: stable_sorted_strings(reasons.clone()),
            notes: "Bounded live proof for read-resource currently shrinks only across immutable execution and object-record scope roots observed in the baseline run.".into(),
        },
        reasons,
    )
}

#[allow(clippy::too_many_lines)]
fn prove_log_write_family<A, R>(
    runner: &Runner<A>,
    registry: &R,
    installed: &InstalledSkill,
    envelope: &ResolvedExecutionEnvelope,
    baseline_record: &ExecutionRecord,
    baseline_projection: &Value,
    comparator: LiveProofComparatorProfile,
) -> (
    CapabilityGrantSet,
    CapabilityGrantSet,
    Vec<LiveProofCandidateTrial>,
    LiveProofFamilyStatus,
    Vec<String>,
)
where
    A: RuntimeAdapter + Clone + 'static,
    R: SkillRegistry + Clone + Send + Sync + 'static,
{
    let family = CapabilityId::LogWrite;
    let family_grants = family_grants(&envelope.granted_capabilities, &family);
    let mut trials = Vec::new();
    let mut reasons = vec!["LIVE_PROOF_SUPPORTED".into()];

    let removal = run_family_trial(
        runner,
        registry,
        installed,
        envelope,
        &family,
        "remove_grant",
        remove_family_grants(&envelope.granted_capabilities, &family),
        baseline_projection,
        comparator,
        "trial-log-write-remove-family",
    );
    let removal_matched = removal.matched;
    trials.push(removal.trial);
    if removal_matched {
        return (
            CapabilityGrantSet::default(),
            CapabilityGrantSet::default(),
            trials,
            LiveProofFamilyStatus {
                family,
                support: LiveProofSupport::LiveProofSupported,
                proof_status: Some("exact_minimal".into()),
                reason_codes: stable_sorted_strings(reasons.clone()),
                notes: "The family was fully removable for this invocation under the exact discrete level search.".into(),
            },
            reasons,
        );
    }

    let observed_caps = observed_log_caps(baseline_record);
    if observed_caps.is_empty() {
        reasons.push("LIVE_SCOPE_SHRINK_UNSUPPORTED".into());
        return (
            CapabilityGrantSet::default(),
            CapabilityGrantSet {
                grants: family_grants,
            },
            trials,
            LiveProofFamilyStatus {
                family,
                support: LiveProofSupport::NotProven,
                proof_status: Some("not_proven".into()),
                reason_codes: stable_sorted_strings(reasons.clone()),
                notes: "No exercised log-write observations were available for exact discrete level search.".into(),
            },
            reasons,
        );
    }

    let observed_refs = observed_caps.iter().collect::<Vec<_>>();
    let mut reduced_family = Vec::new();
    for grant in &family_grants {
        for reduced in reduce_grant_to_cap_set(&observed_refs, grant) {
            push_unique_grant(&mut reduced_family, reduced);
        }
    }

    let expected = LogConstraints {
        levels: Some(
            observed_caps
                .iter()
                .flat_map(|grant| match &grant.constraints {
                    CapabilityConstraints::Log(value) => value.levels.clone().unwrap_or_default(),
                    _ => Vec::new(),
                })
                .collect(),
        ),
    };
    if !log_grants_collectively_cover(&reduced_family, &expected) {
        reasons.push("LIVE_SCOPE_SHRINK_UNSUPPORTED".into());
        return (
            CapabilityGrantSet::default(),
            CapabilityGrantSet {
                grants: family_grants,
            },
            trials,
            LiveProofFamilyStatus {
                family,
                support: LiveProofSupport::NotProven,
                proof_status: Some("not_proven".into()),
                reason_codes: stable_sorted_strings(reasons.clone()),
                notes: "The reduced log-write grants did not preserve the observed log levels."
                    .into(),
            },
            reasons,
        );
    }

    let reduced_trial = run_family_trial(
        runner,
        registry,
        installed,
        envelope,
        &family,
        "shrink_scope",
        replace_family_grants(
            &envelope.granted_capabilities,
            &family,
            CapabilityGrantSet {
                grants: reduced_family.clone(),
            },
        ),
        baseline_projection,
        comparator,
        "trial-log-write-observed-levels",
    );
    let reduced_matched = reduced_trial.matched;
    trials.push(reduced_trial.trial);
    if !reduced_matched {
        reasons.push("LIVE_PROOF_PREREQUISITE_FAILED".into());
        return (
            CapabilityGrantSet::default(),
            CapabilityGrantSet {
                grants: family_grants,
            },
            trials,
            LiveProofFamilyStatus {
                family,
                support: LiveProofSupport::NotProven,
                proof_status: Some("not_proven".into()),
                reason_codes: stable_sorted_strings(reasons.clone()),
                notes: "The reduced log-write grants changed execution behavior under the live comparator, so proof failed closed.".into(),
            },
            reasons,
        );
    }

    let mut strict_subset_failed = true;
    let proper_subsets = proper_nonempty_subsets(&reduced_family);
    for (index, subset) in proper_subsets.iter().enumerate() {
        let subset_trial = run_family_trial(
            runner,
            registry,
            installed,
            envelope,
            &family,
            "shrink_scope",
            replace_family_grants(
                &envelope.granted_capabilities,
                &family,
                CapabilityGrantSet {
                    grants: subset.clone(),
                },
            ),
            baseline_projection,
            comparator,
            &format!("trial-log-write-subset-{index}"),
        );
        if subset_trial.matched {
            strict_subset_failed = false;
        }
        trials.push(subset_trial.trial);
    }

    let proof_status = if reduced_family == family_grants {
        "no_reduction"
    } else if strict_subset_failed {
        "exact_minimal"
    } else {
        reasons.push("LIVE_PROOF_BOUNDED".into());
        "reduced"
    };

    (
        CapabilityGrantSet {
            grants: reduced_family,
        },
        CapabilityGrantSet::default(),
        trials,
        LiveProofFamilyStatus {
            family,
            support: LiveProofSupport::LiveProofSupported,
            proof_status: Some(proof_status.into()),
            reason_codes: stable_sorted_strings(reasons.clone()),
            notes: "Live log-write proof uses exact discrete search over the finite observed log levels.".into(),
        },
        reasons,
    )
}

#[allow(clippy::too_many_arguments)]
fn run_family_trial<A, R>(
    runner: &Runner<A>,
    registry: &R,
    installed: &InstalledSkill,
    envelope: &ResolvedExecutionEnvelope,
    family: &CapabilityId,
    change_kind: &str,
    candidate_grants: CapabilityGrantSet,
    baseline_projection: &Value,
    comparator: LiveProofComparatorProfile,
    trial_id: &str,
) -> TrialResult
where
    A: RuntimeAdapter + Clone + 'static,
    R: SkillRegistry + Clone + Send + Sync + 'static,
{
    let candidate_envelope = clone_with_grants(envelope, candidate_grants.clone());
    match runner.execute(registry, installed, &candidate_envelope) {
        Ok(record) => {
            let candidate_projection =
                normalized_execution_projection(registry, &record, comparator);
            let output_digest = sha256_json(&candidate_projection);
            let matched = baseline_projection == &candidate_projection;
            let accepted = matched && matches!(record.status, ExecutionStatus::Succeeded);

            TrialResult {
                trial: LiveProofCandidateTrial {
                    trial_id: trial_id.into(),
                    family: family.clone(),
                    change_kind: change_kind.into(),
                    candidate_envelope: LiveProofEnvelope {
                        granted_capabilities: candidate_grants,
                    },
                    execution_status: live_proof_trial_status(&record.status),
                    comparator_status: if matched {
                        LiveProofComparatorStatus::Match
                    } else {
                        LiveProofComparatorStatus::Mismatch
                    },
                    accepted,
                    reason_codes: if matched {
                        vec![]
                    } else {
                        vec!["AUTHORITY_REQUIRED_BY_COMPARATOR".into()]
                    },
                    error_code: record
                        .termination
                        .as_ref()
                        .map(|detail| detail.code.clone()),
                    observed_families: observed_families(&record.authority_observations),
                    output_digest: Some(output_digest),
                },
                matched: accepted,
            }
        }
        Err(error) => TrialResult {
            trial: LiveProofCandidateTrial {
                trial_id: trial_id.into(),
                family: family.clone(),
                change_kind: change_kind.into(),
                candidate_envelope: LiveProofEnvelope {
                    granted_capabilities: candidate_grants,
                },
                execution_status: if error.phase.is_some() {
                    LiveProofTrialStatus::Failed
                } else {
                    LiveProofTrialStatus::ValidationError
                },
                comparator_status: LiveProofComparatorStatus::Error,
                accepted: false,
                reason_codes: vec!["AUTHORITY_REQUIRED_BY_TRACE".into()],
                error_code: Some(error.code),
                observed_families: Vec::new(),
                output_digest: None,
            },
            matched: false,
        },
    }
}

fn normalized_execution_projection(
    registry: &impl SkillRegistry,
    record: &ExecutionRecord,
    profile: LiveProofComparatorProfile,
) -> Value {
    let output = normalized_skill_output(record, profile);

    match profile {
        LiveProofComparatorProfile::ExactOutput => json!({
            "status": record.status,
            "termination": record.termination,
            "output": output,
            "child_execution_statuses": record
                .child_executions
                .iter()
                .map(|child| json!({
                    "uri": child.uri,
                    "status": child.status,
                    "termination": child.termination,
                }))
                .collect::<Vec<_>>(),
        }),
        LiveProofComparatorProfile::NormalizedInspectOutputV1 => json!({
            "status": record.status,
            "termination": record.termination,
            "output": output,
            "child_execution_statuses": record
                .child_executions
                .iter()
                .map(|child| json!({
                    "uri": child.uri,
                    "status": child.status,
                    "termination": child.termination,
                }))
                .collect::<Vec<_>>(),
        }),
        LiveProofComparatorProfile::NormalizedInspectSingleChildInvokeV1 => json!({
            "status": record.status,
            "termination": record.termination,
            "output": output,
            "child_execution_count": record.child_executions.len(),
            "child_execution_statuses": record
                .child_executions
                .iter()
                .map(|child| json!({
                    "alias": child.alias,
                    "status": child.status,
                    "termination": child.termination,
                    "resolved_skill": child.provenance.resolved_skill,
                    "abi": child.provenance.abi,
                    "granted_capabilities": child.granted_capabilities,
                }))
                .collect::<Vec<_>>(),
            "loaded_child_records": record
                .child_executions
                .iter()
                .map(|child| normalized_loaded_child_execution_projection(registry, child.execution_id.as_str()))
                .collect::<Vec<_>>(),
        }),
        LiveProofComparatorProfile::NormalizedInspectSingleSinkEmitEvidenceV1 => json!({
            "status": record.status,
            "termination": record.termination,
            "output": output,
            "emitted_evidence_count": record.emitted_evidence.len(),
            "emitted_evidence": record
                .emitted_evidence
                .iter()
                .map(|evidence| normalized_emitted_evidence_record(
                    evidence,
                    record.receipt.execution_id.as_str(),
                ))
                .collect::<Vec<_>>(),
            "emit_observations": record
                .authority_observations
                .iter()
                .filter_map(|observation| match observation {
                    AuthorityObservation::EmitEvidence { status, detail } => Some(json!({
                        "status": status,
                        "detail": normalized_emit_evidence_observation(
                            detail,
                            record.receipt.execution_id.as_str(),
                        ),
                    })),
                    _ => None,
                })
                .collect::<Vec<_>>(),
        }),
    }
}

fn live_proof_trial_status(status: &ExecutionStatus) -> LiveProofTrialStatus {
    match status {
        ExecutionStatus::Succeeded => LiveProofTrialStatus::Succeeded,
        ExecutionStatus::Failed | ExecutionStatus::Partial => LiveProofTrialStatus::Failed,
        ExecutionStatus::Rejected => LiveProofTrialStatus::Rejected,
    }
}

fn strip_host_owned_projection(value: &mut Value) {
    if let Some(object) = value.as_object_mut() {
        object.remove("granted_capabilities");
    }
}

fn normalized_skill_output(
    record: &ExecutionRecord,
    profile: LiveProofComparatorProfile,
) -> Option<Value> {
    record.output.as_ref().map(|output| {
        let mut structured = output.structured.clone();
        let evidence = match profile {
            LiveProofComparatorProfile::ExactOutput => {
                serde_json::to_value(&output.evidence).expect("evidence refs serialize")
            }
            LiveProofComparatorProfile::NormalizedInspectOutputV1
            | LiveProofComparatorProfile::NormalizedInspectSingleChildInvokeV1
            | LiveProofComparatorProfile::NormalizedInspectSingleSinkEmitEvidenceV1 => {
                normalize_evidence_refs(&output.evidence)
            }
        };

        if matches!(
            profile,
            LiveProofComparatorProfile::NormalizedInspectOutputV1
                | LiveProofComparatorProfile::NormalizedInspectSingleChildInvokeV1
                | LiveProofComparatorProfile::NormalizedInspectSingleSinkEmitEvidenceV1
        ) {
            strip_host_owned_projection(&mut structured);
        }

        json!({
            "summary": output.summary,
            "structured": structured,
            "diagnostics": output.diagnostics,
            "effects": output.effects,
            "evidence": evidence,
        })
    })
}

fn normalized_loaded_child_execution_projection(
    registry: &impl SkillRegistry,
    execution_id: &str,
) -> Value {
    match registry.load_execution_record(execution_id) {
        Ok(record) => json!({
            "resolved_skill": record.resolved_skill,
            "request_skill": record.request.skill,
            "request_input_digest": sha256_json(&record.request.input),
            "status": record.status,
            "termination": record.termination,
            "granted_capabilities": record.granted_capabilities,
            "authority_observations": record.authority_observations,
            "child_execution_count": record.child_executions.len(),
            "abi": record.provenance.abi,
            "output": normalized_skill_output(
                &record,
                LiveProofComparatorProfile::NormalizedInspectSingleChildInvokeV1,
            ),
        }),
        Err(error) => json!({
            "load_error": {
                "code": error.code,
                "message": error.message,
            }
        }),
    }
}

fn normalize_evidence_refs(values: &[guild_types::EvidenceRef]) -> Value {
    json!(
        values
            .iter()
            .map(|item| {
                json!({
                    "title": item.title,
                    "mime_type": item.mime_type,
                    "sha256": item.sha256,
                    "audience": item.audience,
                    "redaction": item.redaction,
                    "freshness": item.freshness,
                })
            })
            .collect::<Vec<_>>()
    )
}

fn normalized_emitted_evidence_record(record: &EvidenceRecord, parent_execution_id: &str) -> Value {
    json!({
        "mime_type": record.mime_type,
        "sha256": record.sha256,
        "size_bytes": record.size_bytes,
        "sink": record.sink,
        "title": record.title,
        "audience": record.audience,
        "redaction": record.redaction,
        "freshness": record.freshness,
        "produced_by_parent_execution": record.produced_by_execution.as_deref() == Some(parent_execution_id),
    })
}

fn normalized_emit_evidence_observation(
    detail: &guild_types::EmitEvidenceAuthorityObservation,
    _parent_execution_id: &str,
) -> Value {
    json!({
        "mime_type": detail.mime_type,
        "audience": detail.audience,
        "redaction": detail.redaction,
        "size_bytes": detail.size_bytes,
        "sink": detail.sink,
        "title": detail.title,
        "sha256": detail.sha256,
        "denial": detail.denial,
        "result_error": detail.result_error,
        "emitted_to_parent_execution": detail.evidence_uri.is_some() && detail.result_error.is_none() && detail.denial.is_none() && detail.sha256.is_some() && detail.sink.is_some(),
    })
}

fn observed_read_resource_caps(record: &ExecutionRecord) -> Option<Vec<GrantedCapability>> {
    let mut scopes = BTreeSet::new();
    let mut kinds = Vec::new();

    for observation in &record.authority_observations {
        let AuthorityObservation::ReadResource { status, detail } = observation else {
            continue;
        };
        if *status != AuthorityObservationStatus::Exercised {
            continue;
        }

        let parsed_uri = GuildResourceUri::parse(&detail.uri).ok()?;
        match parsed_uri.scope() {
            GuildResourceScope::Execution => {
                scopes.insert(GuildResourceScope::Execution.canonical_prefix().to_owned());
                push_unique_kind(&mut kinds, ResourceKind::Execution);
            }
            GuildResourceScope::ObjectRecord => {
                scopes.insert(
                    GuildResourceScope::ObjectRecord
                        .canonical_prefix()
                        .to_owned(),
                );
                push_unique_kind(&mut kinds, ResourceKind::Object);
            }
            GuildResourceScope::ObjectBlob | GuildResourceScope::ExecutionQuery => return None,
        }
    }

    if scopes.is_empty() {
        return Some(Vec::new());
    }

    Some(vec![GrantedCapability {
        id: CapabilityId::ReadResource,
        access: CapabilityAccess::Read,
        constraints: CapabilityConstraints::ReadResource(ReadResourceConstraints {
            uri_prefixes: Some(scopes.into_iter().collect()),
            resource_kinds: Some(kinds),
        }),
    }])
}

fn observed_log_caps(record: &ExecutionRecord) -> Vec<GrantedCapability> {
    let mut levels = Vec::new();
    for observation in &record.authority_observations {
        let AuthorityObservation::LogWrite { status, detail } = observation else {
            continue;
        };
        if *status == AuthorityObservationStatus::Exercised {
            push_unique_level(&mut levels, detail.level.clone());
        }
    }

    levels
        .into_iter()
        .map(|level| GrantedCapability {
            id: CapabilityId::LogWrite,
            access: CapabilityAccess::Write,
            constraints: CapabilityConstraints::Log(LogConstraints {
                levels: Some(vec![level]),
            }),
        })
        .collect()
}

fn observed_emit_evidence_slice(
    record: &ExecutionRecord,
) -> Result<ObservedEmitEvidenceSlice, (&'static str, String)> {
    let mut exercised = Vec::new();
    let mut blocked = Vec::new();

    for observation in &record.authority_observations {
        let AuthorityObservation::EmitEvidence { status, detail } = observation else {
            continue;
        };
        match status {
            AuthorityObservationStatus::Exercised => exercised.push(detail),
            AuthorityObservationStatus::Blocked => blocked.push(detail),
        }
    }

    if exercised.len() != 1 || record.emitted_evidence.len() != 1 {
        let reason_code = if exercised.len() > 1 || record.emitted_evidence.len() > 1 {
            "EMIT_EVIDENCE_MULTI_EMIT_UNSUPPORTED"
        } else {
            "EMIT_EVIDENCE_REPLAY_UNAVAILABLE"
        };
        let notes = if reason_code == "EMIT_EVIDENCE_MULTI_EMIT_UNSUPPORTED" {
            "Bounded emit-evidence feasibility checking currently requires exactly one exercised emission and exactly one persisted evidence record.".into()
        } else {
            "Bounded emit-evidence feasibility checking requires one exercised emission together with one persisted evidence record.".into()
        };
        return Err((reason_code, notes));
    }

    if !blocked.is_empty() {
        return Err((
            "EMIT_EVIDENCE_MULTI_EMIT_UNSUPPORTED",
            "Bounded emit-evidence feasibility checking currently excludes executions with blocked emit-evidence attempts because that would exceed the one-emission slice.".into(),
        ));
    }

    let detail = exercised[0];
    if detail.result_error.is_some() {
        return Err((
            "EMIT_EVIDENCE_RESULT_ERROR_UNSUPPORTED",
            "Bounded emit-evidence feasibility checking currently requires an exercised emission with no host-side result error.".into(),
        ));
    }

    let Some(observed_sink) = detail.sink.as_ref() else {
        return Err((
            "EMIT_EVIDENCE_SINK_MODEL_UNAVAILABLE",
            "The exercised emit-evidence observation did not carry an explicit host-owned sink descriptor.".into(),
        ));
    };
    if !supported_emit_evidence_sink(observed_sink) {
        return Err((
            "EMIT_EVIDENCE_SINK_MODEL_UNAVAILABLE",
            "Bounded emit-evidence feasibility checking currently supports only the fixed local object-store sink model.".into(),
        ));
    }

    let emitted = &record.emitted_evidence[0];
    let Some(persisted_sink) = emitted.sink.as_ref() else {
        return Err((
            "EMIT_EVIDENCE_SINK_MODEL_UNAVAILABLE",
            "The persisted evidence record did not retain the host-owned sink descriptor needed for comparison.".into(),
        ));
    };
    if persisted_sink != observed_sink || !supported_emit_evidence_sink(persisted_sink) {
        return Err((
            "EMIT_EVIDENCE_SINK_MODEL_UNAVAILABLE",
            "The exercised emit-evidence observation and persisted evidence record did not stay bound to the same supported sink model.".into(),
        ));
    }

    if detail.evidence_uri.as_deref() != Some(emitted.uri.as_str())
        || detail.sha256.as_deref() != Some(emitted.sha256.as_str())
        || detail.mime_type != emitted.mime_type
        || detail.audience != emitted.audience
        || detail.redaction != emitted.redaction
        || detail.size_bytes != emitted.size_bytes
        || detail.title != emitted.title
    {
        return Err((
            "EMIT_EVIDENCE_REPLAY_UNAVAILABLE",
            "The exercised emit-evidence observation did not stay aligned with the persisted evidence record metadata.".into(),
        ));
    }

    if emitted.produced_by_execution.as_deref() != Some(record.receipt.execution_id.as_str()) {
        return Err((
            "EMIT_EVIDENCE_REPLAY_UNAVAILABLE",
            "The persisted evidence record did not stay semantically linked to the producing parent execution.".into(),
        ));
    }

    if emitted.sha256.trim().is_empty() {
        return Err((
            "EMIT_EVIDENCE_PAYLOAD_NONDETERMINISTIC",
            "The persisted evidence record did not retain a stable payload digest for deterministic comparison.".into(),
        ));
    }

    Ok(ObservedEmitEvidenceSlice {
        narrowed_capability: GrantedCapability {
            id: CapabilityId::EmitEvidence,
            access: CapabilityAccess::Write,
            constraints: CapabilityConstraints::EmitEvidence(EmitEvidenceConstraints {
                max_bytes: Some(emitted.size_bytes),
                audiences: Some(vec![emitted.audience.clone()]),
                redactions: Some(vec![emitted.redaction.clone()]),
            }),
        },
        payload_sha256: emitted.sha256.clone(),
        sink: observed_sink.clone(),
    })
}

fn supported_emit_evidence_sink(sink: &EvidenceSinkDescriptor) -> bool {
    sink.kind == guild_types::EvidenceSinkKind::LocalObjectStore
        && sink.record_uri_prefix == guild_types::GUILD_OBJECT_RECORD_URI_PREFIX
        && sink.blob_uri_prefix == guild_types::GUILD_OBJECT_BLOB_URI_PREFIX
        && sink.routing_mode == guild_types::EvidenceRoutingMode::Direct
        && sink.storage_class == guild_types::EvidenceStorageClass::LocalPersistentContentAddressed
}

fn emit_evidence_constraints_cover(
    grant: &EmitEvidenceConstraints,
    required: &EmitEvidenceConstraints,
) -> bool {
    grant
        .max_bytes
        .zip(required.max_bytes)
        .is_none_or(|(grant_value, required_value)| grant_value >= required_value)
        && required
            .audiences
            .as_ref()
            .is_none_or(|required_audiences| {
                required_audiences.iter().all(|required_audience| {
                    grant
                        .audiences
                        .as_ref()
                        .is_some_and(|grant_audiences| grant_audiences.contains(required_audience))
                })
            })
        && required
            .redactions
            .as_ref()
            .is_none_or(|required_redactions| {
                required_redactions.iter().all(|required_redaction| {
                    grant.redactions.as_ref().is_some_and(|grant_redactions| {
                        grant_redactions.contains(required_redaction)
                    })
                })
            })
}

fn family_grants(grants: &CapabilityGrantSet, family: &CapabilityId) -> Vec<GrantedCapability> {
    grants
        .grants
        .iter()
        .filter(|grant| grant.id == *family)
        .cloned()
        .collect()
}

fn remove_family_grants(grants: &CapabilityGrantSet, family: &CapabilityId) -> CapabilityGrantSet {
    CapabilityGrantSet {
        grants: grants
            .grants
            .iter()
            .filter(|grant| grant.id != *family)
            .cloned()
            .collect(),
    }
}

fn replace_family_grants(
    grants: &CapabilityGrantSet,
    family: &CapabilityId,
    replacement: CapabilityGrantSet,
) -> CapabilityGrantSet {
    let mut updated = remove_family_grants(grants, family).grants;
    for grant in replacement.grants {
        push_unique_grant(&mut updated, grant);
    }
    CapabilityGrantSet { grants: updated }
}

fn clone_with_grants(
    envelope: &ResolvedExecutionEnvelope,
    grants: CapabilityGrantSet,
) -> ResolvedExecutionEnvelope {
    let mut cloned = envelope.clone();
    cloned.granted_capabilities = grants;
    cloned
}

fn overall_proof_status(
    baseline: &CapabilityGrantSet,
    proven: &CapabilityGrantSet,
    residual: &CapabilityGrantSet,
    family_statuses: &[LiveProofFamilyStatus],
) -> &'static str {
    let any_bounded = family_statuses
        .iter()
        .any(|status| matches!(status.support, LiveProofSupport::BoundedLiveProof));
    let any_reduced = proven.grants != baseline.grants || !residual.grants.is_empty();
    let all_exact = !family_statuses.is_empty()
        && family_statuses.iter().all(|status| {
            matches!(status.support, LiveProofSupport::LiveProofSupported)
                && matches!(
                    status.proof_status.as_deref(),
                    Some("exact_minimal" | "no_reduction")
                )
        })
        && residual.grants.is_empty();

    if proven.grants.is_empty() && !residual.grants.is_empty() {
        return "not_proven";
    }
    if all_exact && any_reduced {
        return "exact_minimal";
    }
    if all_exact {
        return "no_reduction";
    }
    if any_bounded {
        return "bounded_minimal";
    }
    if any_reduced {
        return "reduced";
    }
    "not_proven"
}

fn observed_families(observations: &[AuthorityObservation]) -> Vec<String> {
    let mut families = BTreeSet::new();
    for observation in observations {
        let family = match observation {
            AuthorityObservation::HttpRequest { .. } => "http-request",
            AuthorityObservation::ReadResource { .. } => "read-resource",
            AuthorityObservation::InvokeSkill { .. } => "invoke-skill",
            AuthorityObservation::EmitEvidence { .. } => "emit-evidence",
            AuthorityObservation::LogWrite { .. } => "log-write",
        };
        families.insert(family.to_owned());
    }
    families.into_iter().collect()
}

fn proper_nonempty_subsets(values: &[GrantedCapability]) -> Vec<Vec<GrantedCapability>> {
    if values.len() <= 1 || values.len() >= usize::BITS as usize {
        return Vec::new();
    }

    let mut subsets = Vec::new();
    let max_mask = 1_usize << values.len();
    for mask in 1..(max_mask - 1) {
        let mut subset = Vec::new();
        for (index, value) in values.iter().enumerate() {
            if mask & (1 << index) != 0 {
                subset.push(value.clone());
            }
        }
        subsets.push(subset);
    }
    subsets
}

fn push_unique_grant(grants: &mut Vec<GrantedCapability>, grant: GrantedCapability) {
    if !grants.contains(&grant) {
        grants.push(grant);
    }
}

fn push_unique_kind(kinds: &mut Vec<ResourceKind>, kind: ResourceKind) {
    if !kinds.contains(&kind) {
        kinds.push(kind);
    }
}

fn push_unique_level(levels: &mut Vec<guild_types::Severity>, level: guild_types::Severity) {
    if !levels.contains(&level) {
        levels.push(level);
    }
}

fn stable_sorted_strings(values: Vec<String>) -> Vec<String> {
    let mut unique = BTreeSet::new();
    unique.extend(values);
    unique.into_iter().collect()
}

fn sha256_json(value: &Value) -> String {
    let bytes = serde_json::to_vec(value).expect("JSON projection serializes");
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn build_replay_input_digest<A, R>(
    runner: &Runner<A>,
    registry: &R,
    installed: &InstalledSkill,
    baseline_record: &ExecutionRecord,
    comparator: LiveProofComparatorProfile,
    family_statuses: &[LiveProofFamilyStatus],
) -> Option<String>
where
    A: RuntimeAdapter + Clone + 'static,
    R: SkillRegistry + ?Sized,
{
    let mut entries = Vec::new();

    if let Some(digest) = runner.http_replay_input_digest() {
        entries.push(json!({
            "family": "http-request",
            "replay_input_digest": digest,
        }));
    }

    let invoke_supported = family_statuses.iter().any(|status| {
        status.family == CapabilityId::InvokeSkill
            && !matches!(
                status.support,
                LiveProofSupport::NotProven | LiveProofSupport::Unsupported
            )
            && status.proof_status.as_deref() != Some("not_proven")
    });
    if invoke_supported
        && let Some(digest) =
            invoke_replay_input_digest(registry, installed, baseline_record, comparator)
    {
        entries.push(json!({
            "family": "invoke-skill",
            "replay_input_digest": digest,
        }));
    }

    if comparator == LiveProofComparatorProfile::NormalizedInspectSingleSinkEmitEvidenceV1
        && let Some(digest) = emit_evidence_replay_input_digest(baseline_record, comparator)
    {
        entries.push(json!({
            "family": "emit-evidence",
            "replay_input_digest": digest,
        }));
    }

    match entries.as_slice() {
        [] => None,
        [entry] => entry
            .get("replay_input_digest")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        _ => Some(sha256_json(&json!(entries))),
    }
}

fn invoke_replay_input_digest<R>(
    registry: &R,
    installed: &InstalledSkill,
    baseline_record: &ExecutionRecord,
    comparator: LiveProofComparatorProfile,
) -> Option<String>
where
    R: SkillRegistry + ?Sized,
{
    let observed = observed_invoke_skill_slice(registry, installed, baseline_record).ok()?;
    let comparator_descriptor = comparator_descriptor(comparator);
    Some(sha256_json(&json!({
        "family": "invoke-skill",
        "parent_resolved_digest": installed.resolved_ref.digest,
        "alias": observed.alias,
        "expected_child_resolved_digest": observed.expected_child.digest,
        "fixed_world": INSPECT_WORLD_ENTRYPOINT,
        "child_input_digest": observed.child_input_digest,
        "comparator_id": comparator_descriptor.comparator_id,
        "comparator_version": comparator_descriptor.version,
        "slice": {
            "single_child": true,
            "child_authority": "none",
            "nested_child_executions": 0,
        },
    })))
}

fn emit_evidence_replay_input_digest(
    baseline_record: &ExecutionRecord,
    comparator: LiveProofComparatorProfile,
) -> Option<String> {
    let observed = observed_emit_evidence_slice(baseline_record).ok()?;
    let comparator_descriptor = comparator_descriptor(comparator);
    Some(sha256_json(&json!({
        "family": "emit-evidence",
        "sink": observed.sink,
        "payload_sha256": observed.payload_sha256,
        "comparator_id": comparator_descriptor.comparator_id,
        "comparator_version": comparator_descriptor.version,
        "slice": {
            "single_emission": true,
            "produced_by_parent_execution": true,
            "multi_emit": false,
            "fan_out": false,
            "fallback_routing": false,
        },
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    use guild_registry::RegistryError;
    use guild_types::{
        Budget, CallerRequest, CapabilityGrantSet, EvidenceAudience, EvidenceEmissionRequest,
        ExecutionMetrics, ExecutionMode, ExecutionReceipt, InstalledVerificationState,
        LocalPolicyConfig, LocalTrustTier, PolicyDecision, PolicyDecisionOutcome, Provenance,
        RedactionClass, RequestedSkillRef, ResolvedSkillRef, SkillKey, SkillOutput, SkillVersion,
        VersionRequirement, local_object_store_evidence_sink_descriptor,
    };

    #[derive(Clone)]
    struct UnusedRegistry;

    impl SkillRegistry for UnusedRegistry {
        fn resolve(&self, _skill: &RequestedSkillRef) -> Result<InstalledSkill, RegistryError> {
            unreachable!("resolve should not be called in comparator unit tests")
        }

        fn resolve_exact(
            &self,
            _skill: &ResolvedSkillRef,
        ) -> Result<InstalledSkill, RegistryError> {
            unreachable!("resolve_exact should not be called in comparator unit tests")
        }

        fn search(
            &self,
            _query: &guild_registry::SearchQuery,
        ) -> Vec<guild_registry::SearchResult> {
            unreachable!("search should not be called in comparator unit tests")
        }

        fn persist_execution_record(&self, _record: &ExecutionRecord) -> Result<(), RegistryError> {
            unreachable!("persist_execution_record should not be called in comparator unit tests")
        }

        fn load_execution_record(
            &self,
            _execution_id: &str,
        ) -> Result<ExecutionRecord, RegistryError> {
            unreachable!("load_execution_record should not be called in comparator unit tests")
        }

        fn store_evidence(
            &self,
            _produced_by_execution: &str,
            _request: &EvidenceEmissionRequest,
        ) -> Result<guild_types::EvidenceRef, RegistryError> {
            unreachable!("store_evidence should not be called in comparator unit tests")
        }

        fn load_evidence_record(&self, _uri: &str) -> Result<EvidenceRecord, RegistryError> {
            unreachable!("load_evidence_record should not be called in comparator unit tests")
        }

        fn read_resource(
            &self,
            _uri: &str,
        ) -> Result<guild_types::ResourceReadResult, RegistryError> {
            unreachable!("read_resource should not be called in comparator unit tests")
        }

        fn load_policy_config(&self) -> Result<LocalPolicyConfig, RegistryError> {
            unreachable!("load_policy_config should not be called in comparator unit tests")
        }
    }

    #[allow(clippy::too_many_lines)]
    fn sample_emit_record() -> ExecutionRecord {
        let execution_id = "exec-1".to_owned();
        let evidence_uri = "guild://objects/records/record-1".to_owned();
        let blob_uri = "guild://objects/sha256/abc123".to_owned();
        let sink = local_object_store_evidence_sink_descriptor();
        let output_evidence = guild_types::EvidenceRef {
            uri: evidence_uri.clone(),
            title: Some("hello-inspect snapshot".into()),
            mime_type: Some("application/json".into()),
            sha256: Some("sha256:abc123".into()),
            audience: EvidenceAudience::User,
            redaction: RedactionClass::None,
            freshness: Some("deterministic".into()),
        };
        ExecutionRecord {
            receipt: ExecutionReceipt {
                execution_id: execution_id.clone(),
                uri: format!("guild://executions/{execution_id}"),
                trace_id: "trace-1".into(),
                status: ExecutionStatus::Succeeded,
            },
            request: CallerRequest {
                request_id: "request-1".into(),
                skill: RequestedSkillRef {
                    key: SkillKey {
                        namespace: "example".into(),
                        name: "hello-inspect".into(),
                    },
                    version_req: VersionRequirement::parse("^0.1").unwrap(),
                },
                tenant_id: "tenant-1".into(),
                actor_id: "actor-1".into(),
                mode: ExecutionMode::Inspect,
                input: json!({"name": "Ada"}),
                budget: Budget::default(),
                requested_capabilities: CapabilityGrantSet::default(),
                idempotency_key: None,
                trace_id: "trace-1".into(),
            },
            policy_decision: PolicyDecision {
                outcome: PolicyDecisionOutcome::Allowed,
                summary: "allowed".into(),
                profile_name: "default".into(),
                trust_tier: LocalTrustTier::LocalDev,
                verification_state: InstalledVerificationState::LocalSource,
                reasons: Vec::new(),
                detail: None,
            },
            resolved_skill: ResolvedSkillRef {
                key: SkillKey {
                    namespace: "example".into(),
                    name: "hello-inspect".into(),
                },
                version: SkillVersion::parse("0.1.0").unwrap(),
                digest: "sha256:digest".into(),
            },
            parent_execution_id: None,
            status: ExecutionStatus::Succeeded,
            output: Some(SkillOutput {
                summary: "Hello, Ada. Guild inspect is working.".into(),
                structured: json!({
                    "echoed_input": {"name": "Ada"},
                    "mode": "inspect",
                    "message": "Hello, Ada",
                    "granted_capabilities": {"grants": [{"id": "emit-evidence"}]},
                }),
                diagnostics: Vec::new(),
                effects: Vec::new(),
                evidence: vec![output_evidence],
            }),
            termination: None,
            granted_capabilities: CapabilityGrantSet::default(),
            emitted_evidence: vec![EvidenceRecord {
                uri: evidence_uri.clone(),
                blob_uri,
                mime_type: "application/json".into(),
                sha256: "sha256:abc123".into(),
                size_bytes: 32,
                sink: Some(sink.clone()),
                title: Some("hello-inspect snapshot".into()),
                audience: EvidenceAudience::User,
                redaction: RedactionClass::None,
                freshness: Some("deterministic".into()),
                produced_by_execution: Some(execution_id.clone()),
            }],
            authority_observations: vec![AuthorityObservation::EmitEvidence {
                status: AuthorityObservationStatus::Exercised,
                detail: guild_types::EmitEvidenceAuthorityObservation {
                    mime_type: "application/json".into(),
                    audience: EvidenceAudience::User,
                    redaction: RedactionClass::None,
                    size_bytes: 32,
                    sink: Some(sink),
                    title: Some("hello-inspect snapshot".into()),
                    evidence_uri: Some(evidence_uri),
                    sha256: Some("sha256:abc123".into()),
                    denial: None,
                    result_error: None,
                },
            }],
            authority_observations_recorded: true,
            metrics: ExecutionMetrics::default(),
            provenance: Provenance {
                resolved_skill: ResolvedSkillRef {
                    key: SkillKey {
                        namespace: "example".into(),
                        name: "hello-inspect".into(),
                    },
                    version: SkillVersion::parse("0.1.0").unwrap(),
                    digest: "sha256:digest".into(),
                },
                abi: AbiVersion::GuildSkillInspectV1,
                dependency_digests: Vec::new(),
                started_at_utc: Some("2026-03-21T00:00:00Z".into()),
                finished_at_utc: Some("2026-03-21T00:00:01Z".into()),
            },
            child_executions: Vec::new(),
        }
    }

    #[test]
    fn single_sink_emit_evidence_projection_ignores_host_minted_record_handles() {
        let registry = UnusedRegistry;
        let baseline = sample_emit_record();
        let mut changed = baseline.clone();
        changed.receipt.execution_id = "exec-2".into();
        changed.receipt.uri = "guild://executions/exec-2".into();
        changed.emitted_evidence[0].uri = "guild://objects/records/record-2".into();
        changed.emitted_evidence[0].blob_uri = "guild://objects/sha256/def456".into();
        changed.emitted_evidence[0].produced_by_execution = Some("exec-2".into());
        let AuthorityObservation::EmitEvidence { detail, .. } =
            &mut changed.authority_observations[0]
        else {
            panic!("expected emit-evidence observation");
        };
        detail.evidence_uri = Some("guild://objects/records/record-2".into());

        let baseline_projection = normalized_execution_projection(
            &registry,
            &baseline,
            LiveProofComparatorProfile::NormalizedInspectSingleSinkEmitEvidenceV1,
        );
        let changed_projection = normalized_execution_projection(
            &registry,
            &changed,
            LiveProofComparatorProfile::NormalizedInspectSingleSinkEmitEvidenceV1,
        );

        assert_eq!(baseline.output, changed.output);
        assert_eq!(baseline_projection, changed_projection);
    }

    #[test]
    fn single_sink_emit_evidence_projection_rejects_changed_sink_identity_with_same_output() {
        let registry = UnusedRegistry;
        let baseline = sample_emit_record();
        let mut changed = baseline.clone();
        changed.emitted_evidence[0].sink = Some(EvidenceSinkDescriptor {
            kind: guild_types::EvidenceSinkKind::LocalObjectStore,
            record_uri_prefix: "guild://objects/records-alt/".into(),
            blob_uri_prefix: guild_types::GUILD_OBJECT_BLOB_URI_PREFIX.into(),
            routing_mode: guild_types::EvidenceRoutingMode::Direct,
            storage_class: guild_types::EvidenceStorageClass::LocalPersistentContentAddressed,
        });

        let baseline_projection = normalized_execution_projection(
            &registry,
            &baseline,
            LiveProofComparatorProfile::NormalizedInspectSingleSinkEmitEvidenceV1,
        );
        let changed_projection = normalized_execution_projection(
            &registry,
            &changed,
            LiveProofComparatorProfile::NormalizedInspectSingleSinkEmitEvidenceV1,
        );

        assert_eq!(baseline.output, changed.output);
        assert_ne!(baseline_projection, changed_projection);
    }
}
