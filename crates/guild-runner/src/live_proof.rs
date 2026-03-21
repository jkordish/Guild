use std::collections::BTreeSet;

use guild_registry::{InstalledSkill, SkillRegistry};
use guild_types::{
    AuthorityObservation, AuthorityObservationStatus, CapabilityAccess, CapabilityConstraints,
    CapabilityGrantSet, CapabilityId, ExecutionRecord, ExecutionStatus, GrantedCapability,
    GuildResourceScope, GuildResourceUri, HttpMethod, HttpRequestConstraints, HttpScheme,
    LogConstraints, ReadResourceConstraints, ResolvedExecutionEnvelope, ResourceKind,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use url::Url;

use crate::{
    ExecutionError, Runner, RuntimeAdapter, http_request_covers, is_redirect_status,
    log_grants_collectively_cover, read_resource_grants_collectively_cover,
    reduce_grant_to_cap_set,
};

const LIVE_PROOF_VERSION: &str = "1.0.0";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
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
        normalized_execution_projection(&baseline_execution_record, comparator);
    let baseline_output_digest = Some(sha256_json(&baseline_projection));
    let replay_input_digest = runner.http_replay_input_digest();

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
                notes: "The family was removable for this invocation under the bounded fixture-backed HTTP replay slice: one loopback IP GET or HEAD request with either an explicit port or the default HTTP port, or one explicit-port localhost GET request with a deterministic loopback-only resolution binding.".into(),
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
        "trial-http-request-shrink-observed-loopback-get",
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
            notes: "Bounded live proof for http-request currently covers only one replay-fixtured request under the normalized inspect-output comparator: a loopback IP-literal GET or HEAD with either an explicit port or the implicit default HTTP port, or an explicit-port localhost GET with a deterministic loopback-only resolution binding. All slices remain exact-path, query-free, redirect-free, and single-request only.".into(),
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
        if request.method != HttpMethod::Get {
            return Err((
                "HTTP_HOST_UNSUPPORTED_FOR_LIVE_PROOF",
                "Bounded http-request hostname live proof currently supports only explicit-port localhost GET requests.".into(),
            ));
        }
        if parsed_url.port().is_none() {
            return Err((
                "HTTP_HOST_UNSUPPORTED_FOR_LIVE_PROOF",
                "Bounded http-request hostname live proof currently requires an explicit port in the localhost URL.".into(),
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
            allowed_hosts: Some(vec![host.to_owned()]),
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
            let candidate_projection = normalized_execution_projection(&record, comparator);
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
    record: &ExecutionRecord,
    profile: LiveProofComparatorProfile,
) -> Value {
    let output = record.output.as_ref().map(|output| {
        let mut structured = output.structured.clone();
        let evidence = match profile {
            LiveProofComparatorProfile::ExactOutput => {
                serde_json::to_value(&output.evidence).expect("evidence refs serialize")
            }
            LiveProofComparatorProfile::NormalizedInspectOutputV1 => {
                normalize_evidence_refs(&output.evidence)
            }
        };

        if matches!(
            profile,
            LiveProofComparatorProfile::NormalizedInspectOutputV1
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
    });

    json!({
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
    })
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
