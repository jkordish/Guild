use std::collections::BTreeSet;

use guild_registry::{InstalledSkill, SkillRegistry};
use guild_types::{
    AuthorityObservation, AuthorityObservationStatus, CapabilityAccess, CapabilityConstraints,
    CapabilityGrantSet, CapabilityId, ExecutionRecord, ExecutionStatus, GrantedCapability,
    GuildResourceScope, GuildResourceUri, LogConstraints, ReadResourceConstraints, ResourceKind,
    ResolvedExecutionEnvelope,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{
    ExecutionError, Runner, RuntimeAdapter, log_grants_collectively_cover,
    read_resource_grants_collectively_cover, reduce_grant_to_cap_set,
};

const LIVE_PROOF_VERSION: &str = "1.0.0";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum LiveProofComparatorProfile {
    ExactOutput,
    NormalizedInspectOutputV1,
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
        normalized_execution_projection(&baseline_execution_record, comparator.clone()).map_err(
            |message| {
                ExecutionError::new(
                    "live-proof-comparator-unavailable",
                    "live proof comparator could not normalize the baseline execution",
                )
                .with_detail(json!({ "message": message }))
            },
        )?;
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
                    comparator.clone(),
                ),
                CapabilityId::LogWrite => prove_log_write_family(
                    runner,
                    registry,
                    installed,
                    envelope,
                    &baseline_execution_record,
                    &baseline_projection,
                    comparator.clone(),
                ),
                CapabilityId::HttpRequest => unsupported_family_status(
                    &family,
                    &family_grants,
                    "LIVE_REPLAY_UNAVAILABLE",
                    "HTTP requests still lack a real runtime replay transport, so live proof stays not_proven.",
                ),
                CapabilityId::InvokeSkill => unsupported_family_status(
                    &family,
                    &family_grants,
                    "LIVE_REPLAY_UNAVAILABLE",
                    "Invoke-skill still lacks an honest child-execution replay proof path, so live proof stays not_proven.",
                ),
                CapabilityId::EmitEvidence => unsupported_family_status(
                    &family,
                    &family_grants,
                    "LIVE_COMPARATOR_UNAVAILABLE",
                    "Emit-evidence still lacks a stable sink comparator, so live proof stays not_proven.",
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
    if residual_authority.grants.is_empty() {
        minimization_reason_codes.push("TOKEN_PROOF_BASIS_LIVE".into());
    } else {
        minimization_reason_codes.push("PROOF_LINKAGE_UNAVAILABLE".into());
    }

    let baseline_observed_families = observed_families(&baseline_execution_record.authority_observations);
    Ok(LiveProofScenarioResult {
        baseline_execution_record,
        proof: LiveProofOutcome {
            version: LIVE_PROOF_VERSION.into(),
            proof_status: proof_status.into(),
            comparator: comparator_descriptor(&comparator),
            proven_authority,
            residual_authority,
            family_statuses,
            candidate_trials,
            minimization_reason_codes: stable_sorted_strings(minimization_reason_codes),
            observed_families: baseline_observed_families,
            baseline_output_digest,
        },
    })
}

fn comparator_descriptor(profile: &LiveProofComparatorProfile) -> LiveProofComparatorDescriptor {
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
        comparator.clone(),
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
        comparator.clone(),
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
                notes: "The reduced log-write grants did not preserve the observed log levels.".into(),
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
        comparator.clone(),
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
            comparator.clone(),
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
            let comparator_result =
                normalized_execution_projection(&record, comparator).map(|candidate_projection| {
                    let output_digest = sha256_json(&candidate_projection);
                    (
                        baseline_projection == &candidate_projection,
                        output_digest,
                        LiveProofComparatorStatus::Match,
                    )
                });

            match comparator_result {
                Ok((matched, output_digest, comparator_status)) => TrialResult {
                    trial: LiveProofCandidateTrial {
                        trial_id: trial_id.into(),
                        family: family.clone(),
                        change_kind: change_kind.into(),
                        candidate_envelope: LiveProofEnvelope {
                            granted_capabilities: candidate_grants,
                        },
                        execution_status: match record.status {
                            ExecutionStatus::Succeeded => LiveProofTrialStatus::Succeeded,
                            ExecutionStatus::Failed => LiveProofTrialStatus::Failed,
                            ExecutionStatus::Rejected => LiveProofTrialStatus::Rejected,
                            ExecutionStatus::Partial => LiveProofTrialStatus::Failed,
                        },
                        comparator_status: if matched {
                            comparator_status
                        } else {
                            LiveProofComparatorStatus::Mismatch
                        },
                        accepted: matched && matches!(record.status, ExecutionStatus::Succeeded),
                        reason_codes: if matched {
                            vec![]
                        } else {
                            vec!["AUTHORITY_REQUIRED_BY_COMPARATOR".into()]
                        },
                        error_code: record.termination.as_ref().map(|detail| detail.code.clone()),
                        observed_families: observed_families(&record.authority_observations),
                        output_digest: Some(output_digest),
                    },
                    matched: matched && matches!(record.status, ExecutionStatus::Succeeded),
                },
                Err(message) => TrialResult {
                    trial: LiveProofCandidateTrial {
                        trial_id: trial_id.into(),
                        family: family.clone(),
                        change_kind: change_kind.into(),
                        candidate_envelope: LiveProofEnvelope {
                            granted_capabilities: candidate_grants,
                        },
                        execution_status: match record.status {
                            ExecutionStatus::Succeeded => LiveProofTrialStatus::Succeeded,
                            ExecutionStatus::Failed => LiveProofTrialStatus::Failed,
                            ExecutionStatus::Rejected => LiveProofTrialStatus::Rejected,
                            ExecutionStatus::Partial => LiveProofTrialStatus::Failed,
                        },
                        comparator_status: LiveProofComparatorStatus::Unavailable,
                        accepted: false,
                        reason_codes: vec!["LIVE_COMPARATOR_UNAVAILABLE".into()],
                        error_code: Some(message),
                        observed_families: observed_families(&record.authority_observations),
                        output_digest: None,
                    },
                    matched: false,
                },
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
    record: &ExecutionRecord,
    profile: LiveProofComparatorProfile,
) -> Result<Value, String> {
    let output = record.output.as_ref().map(|output| {
        let mut structured = output.structured.clone();
        let evidence = match profile {
            LiveProofComparatorProfile::ExactOutput => serde_json::to_value(&output.evidence)
                .expect("evidence refs serialize"),
            LiveProofComparatorProfile::NormalizedInspectOutputV1 => {
                normalize_evidence_refs(&output.evidence)
            }
        };

        if matches!(profile, LiveProofComparatorProfile::NormalizedInspectOutputV1) {
            strip_host_owned_projection(&mut structured);
        }

        json!({
            "summary": output.summary,
            "structured": structured,
            "diagnostics": output.diagnostics,
            "effects": output.effects,
            "evidence": evidence,
        })
    });

    Ok(json!({
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
    }))
}

fn strip_host_owned_projection(value: &mut Value) {
    if let Some(object) = value.as_object_mut() {
        object.remove("granted_capabilities");
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
                scopes.insert(GuildResourceScope::ObjectRecord.canonical_prefix().to_owned());
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
                    Some("exact_minimal") | Some("no_reduction")
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
    if any_bounded && any_reduced {
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
