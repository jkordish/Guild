use std::collections::{BTreeMap, BTreeSet};
use std::net::IpAddr;

use serde_json::{Map, Value, json};
use url::Url;

pub fn execution_uri(record: &Value) -> Option<&str> {
    record.pointer("/receipt/uri").and_then(Value::as_str)
}

pub fn execution_id(record: &Value) -> Option<&str> {
    record.pointer("/receipt/execution_id").and_then(Value::as_str)
}

pub fn resolved_skill(record: &Value) -> Value {
    record.get("resolved_skill").cloned().unwrap_or(Value::Null)
}

pub fn status(record: &Value) -> Value {
    record.get("status").cloned().unwrap_or(Value::Null)
}

pub fn termination(record: &Value) -> Value {
    record.get("termination").cloned().unwrap_or(Value::Null)
}

pub fn policy_decision(record: &Value) -> Value {
    record.get("policy_decision").cloned().unwrap_or(Value::Null)
}

pub fn policy_profile(record: &Value) -> Option<&str> {
    record
        .pointer("/policy_decision/profile_name")
        .and_then(Value::as_str)
}

pub fn trust_tier(record: &Value) -> Option<&str> {
    record
        .pointer("/policy_decision/trust_tier")
        .and_then(Value::as_str)
}

pub fn verification_state(record: &Value) -> Option<&str> {
    record
        .pointer("/policy_decision/verification_state")
        .and_then(Value::as_str)
}

pub fn child_execution_count(record: &Value) -> usize {
    record
        .get("child_executions")
        .and_then(Value::as_array)
        .map_or(0, Vec::len)
}

pub fn child_execution_uris(record: &Value) -> Vec<Value> {
    record
        .get("child_executions")
        .and_then(Value::as_array)
        .map(|items| {
            items.iter()
                .filter_map(|item| item.get("uri").cloned())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

pub fn requested_capabilities(record: &Value) -> Vec<Value> {
    capability_grants(record.pointer("/request/requested_capabilities/grants"))
}

pub fn granted_capabilities(record: &Value) -> Vec<Value> {
    capability_grants(record.pointer("/granted_capabilities/grants"))
}

pub fn capability_delta_groups(record: &Value) -> Vec<Value> {
    compare_capability_sets(
        &requested_capabilities(record),
        &granted_capabilities(record),
        "requested",
        "granted",
    )
}

pub fn reduced_or_denied_capability_deltas(record: &Value) -> Vec<Value> {
    capability_delta_groups(record)
        .into_iter()
        .filter(|group| {
            group.get("change").and_then(Value::as_str).is_some_and(|change| {
                matches!(change, "requested-only" | "changed")
            })
        })
        .collect()
}

pub fn compare_execution_requested_capabilities(left: &Value, right: &Value) -> Vec<Value> {
    compare_capability_sets(
        &requested_capabilities(left),
        &requested_capabilities(right),
        "left",
        "right",
    )
}

pub fn compare_execution_granted_capabilities(left: &Value, right: &Value) -> Vec<Value> {
    compare_capability_sets(
        &granted_capabilities(left),
        &granted_capabilities(right),
        "left",
        "right",
    )
}

pub fn required_capability_gaps(record: &Value) -> Vec<Value> {
    policy_reasons(record)
        .iter()
        .find(|reason| {
            reason
                .get("code")
                .and_then(Value::as_str)
                .is_some_and(|code| code == "policy-required-capability-missing")
        })
        .and_then(|reason| reason.pointer("/detail/missing").and_then(Value::as_array))
        .map(|items| items.to_vec())
        .or_else(|| {
            record
                .pointer("/policy_decision/detail/missing")
                .and_then(Value::as_array)
                .map(|items| items.to_vec())
        })
        .unwrap_or_default()
}

pub fn reason_chain(record: &Value) -> Vec<Value> {
    let mut chain = Vec::new();

    if let Some(termination) = record.get("termination").and_then(Value::as_object) {
        chain.push(json!({
            "source": "termination",
            "phase": termination.get("phase").cloned().unwrap_or(Value::Null),
            "code": termination.get("code").cloned().unwrap_or(Value::Null),
            "message": termination.get("message").cloned().unwrap_or(Value::Null),
            "retryable": termination.get("retryable").cloned().unwrap_or(Value::Null),
            "detail": termination.get("detail").cloned().unwrap_or(Value::Null),
        }));
    }

    for reason in policy_reasons(record) {
        chain.push(json!({
            "source": "policy",
            "code": reason.get("code").cloned().unwrap_or(Value::Null),
            "message": reason.get("message").cloned().unwrap_or(Value::Null),
            "detail": reason.get("detail").cloned().unwrap_or(Value::Null),
        }));
    }

    chain
}

pub fn primary_reason(record: &Value) -> Value {
    reason_chain(record).into_iter().next().unwrap_or(Value::Null)
}

pub fn policy_reason_codes(record: &Value) -> Vec<String> {
    policy_reasons(record)
        .iter()
        .filter_map(|reason| reason.get("code").and_then(Value::as_str))
        .map(ToOwned::to_owned)
        .collect()
}

pub fn http_authority_report(
    record: &Value,
    candidate_url: &str,
    candidate_method: &str,
    candidate_timeout_ms: Option<u64>,
) -> Result<Value, HttpAuthorityError> {
    let normalized_method = normalize_http_method(candidate_method)?;
    let parsed = ParsedCandidateRequest::parse(candidate_url)?;
    let http_grants = granted_capabilities(record)
        .into_iter()
        .enumerate()
        .filter(|(_, grant)| {
            grant.get("id").and_then(Value::as_str) == Some("http-request")
                && grant.get("access").and_then(Value::as_str) == Some("read")
        })
        .collect::<Vec<_>>();

    if http_grants.is_empty() {
        return Ok(base_http_report(
            record,
            &parsed,
            &normalized_method,
            candidate_timeout_ms,
            "denied",
            Value::Bool(false),
            false,
            vec![],
            json!({
                "code": "http-request-not-granted",
                "message": "stored execution did not retain any granted http-request capability",
                "detail": {
                    "url": candidate_url,
                    "method": normalized_method,
                },
            }),
        ));
    }

    let per_grant = http_grants
        .iter()
        .map(|(index, grant)| evaluate_http_grant(*index, grant, &parsed, &normalized_method, candidate_timeout_ms))
        .collect::<Vec<_>>();

    if let Some(allowed) = per_grant.iter().find(|evaluation| evaluation.decision == "allowed") {
        return Ok(base_http_report(
            record,
            &parsed,
            &normalized_method,
            candidate_timeout_ms,
            "allowed",
            Value::Bool(true),
            true,
            render_http_grant_evaluations(&per_grant),
            Value::Null,
        )
        .tap_mut(|report| {
            inject_match_summary(report, &allowed.summary);
        }));
    }

    if per_grant.iter().all(|evaluation| evaluation.decision == "denied") {
        let reason = per_grant
            .iter()
            .find_map(|evaluation| evaluation.denial_reason.clone())
            .unwrap_or_else(|| {
                json!({
                    "code": "http-request-not-granted",
                    "message": "candidate request did not match any stored HTTP grant",
                    "detail": {
                        "url": candidate_url,
                        "method": normalized_method,
                    },
                })
            });
        return Ok(base_http_report(
            record,
            &parsed,
            &normalized_method,
            candidate_timeout_ms,
            "denied",
            Value::Bool(false),
            true,
            render_http_grant_evaluations(&per_grant),
            reason,
        )
        .tap_mut(|report| {
            if let Some(summary) = per_grant.iter().find_map(|evaluation| {
                if evaluation.decision == "denied" {
                    Some(&evaluation.summary)
                } else {
                    None
                }
            }) {
                inject_match_summary(report, summary);
            }
        }));
    }

    Ok(base_http_report(
        record,
        &parsed,
        &normalized_method,
        candidate_timeout_ms,
        "indeterminate",
        Value::Null,
        true,
        render_http_grant_evaluations(&per_grant),
        json!({
            "code": "http-request-host-resolution-required",
            "message": "candidate request requires host-side destination resolution that the inspect skill cannot perform",
            "detail": {
                "url": candidate_url,
                "host": parsed.host,
            },
        }),
    ))
}

fn base_http_report(
    record: &Value,
    parsed: &ParsedCandidateRequest,
    method: &str,
    timeout_ms: Option<u64>,
    evaluation_status: &str,
    allowed: Value,
    http_grant_present: bool,
    grant_evaluations: Vec<Value>,
    denial_reason: Value,
) -> Value {
    json!({
        "execution_uri": execution_uri(record).map_or(Value::Null, |uri| Value::String(uri.to_owned())),
        "skill": resolved_skill(record),
        "status": status(record),
        "policy_outcome": record.pointer("/policy_decision/outcome").cloned().unwrap_or(Value::Null),
        "policy_summary": record.pointer("/policy_decision/summary").cloned().unwrap_or(Value::Null),
        "policy_profile": policy_profile(record),
        "trust_tier": trust_tier(record),
        "verification_state": verification_state(record),
        "candidate_url": parsed.url,
        "candidate_method": method,
        "candidate_timeout_ms": timeout_ms,
        "evaluation_status": evaluation_status,
        "allowed": allowed,
        "http_grant_present": http_grant_present,
        "matched_scheme": Value::Null,
        "matched_host_or_suffix": Value::Null,
        "matched_port": Value::Null,
        "matched_method": Value::Null,
        "matched_path_prefix": Value::Null,
        "risky_destination_classification": parsed.destination_classification(),
        "redirect_policy_summary": redirect_policy_summary(record),
        "grant_evaluations": grant_evaluations,
        "denial_reason": denial_reason,
    })
}

fn inject_match_summary(report: &mut Value, summary: &HttpGrantSummary) {
    report["matched_scheme"] = summary.matched_scheme.clone();
    report["matched_host_or_suffix"] = summary.matched_host_or_suffix.clone();
    report["matched_port"] = summary.matched_port.clone();
    report["matched_method"] = summary.matched_method.clone();
    report["matched_path_prefix"] = summary.matched_path_prefix.clone();
}

fn render_http_grant_evaluations(evaluations: &[HttpGrantEvaluation]) -> Vec<Value> {
    evaluations
        .iter()
        .map(|evaluation| {
            json!({
                "grant_index": evaluation.grant_index,
                "decision": evaluation.decision,
                "matched_scheme": evaluation.summary.matched_scheme,
                "matched_host_or_suffix": evaluation.summary.matched_host_or_suffix,
                "matched_port": evaluation.summary.matched_port,
                "matched_method": evaluation.summary.matched_method,
                "matched_path_prefix": evaluation.summary.matched_path_prefix,
                "failed_check": evaluation.failed_check,
                "host_resolution_required": evaluation.host_resolution_required,
                "denial_reason": evaluation.denial_reason,
                "constraints": evaluation.constraints,
            })
        })
        .collect()
}

fn redirect_policy_summary(record: &Value) -> Value {
    let mut redirect_grant_count = 0usize;
    let mut max_redirects = Vec::new();

    for grant in granted_capabilities(record).iter().filter(|grant| {
        grant.get("id").and_then(Value::as_str) == Some("http-request")
            && grant.get("access").and_then(Value::as_str) == Some("read")
    }) {
        if grant
            .pointer("/constraints/follow_redirects")
            .and_then(Value::as_bool)
            == Some(true)
        {
            redirect_grant_count += 1;
            if let Some(value) = grant
                .pointer("/constraints/max_redirects")
                .and_then(Value::as_u64)
            {
                max_redirects.push(value);
            }
        }
    }

    json!({
        "follow_redirects_granted": redirect_grant_count > 0,
        "grant_count_allowing_redirects": redirect_grant_count,
        "max_redirects": max_redirects.into_iter().max(),
        "dry_run_redirects_evaluated": false,
    })
}

fn compare_capability_sets(
    left: &[Value],
    right: &[Value],
    left_label: &str,
    right_label: &str,
) -> Vec<Value> {
    let left_grouped = group_capabilities(left);
    let right_grouped = group_capabilities(right);
    let keys = left_grouped
        .keys()
        .chain(right_grouped.keys())
        .cloned()
        .collect::<BTreeSet<_>>();

    keys.into_iter()
        .map(|key| {
            let left_values = left_grouped.get(&key).cloned().unwrap_or_default();
            let right_values = right_grouped.get(&key).cloned().unwrap_or_default();
            let left_canonical = canonical_value_strings(&left_values);
            let right_canonical = canonical_value_strings(&right_values);
            let change = if left_values.is_empty() && !right_values.is_empty() {
                format!("{right_label}-only")
            } else if !left_values.is_empty() && right_values.is_empty() {
                format!("{left_label}-only")
            } else if left_canonical == right_canonical {
                "same".into()
            } else {
                "changed".into()
            };

            object_with_pairs(&[
                ("id", Value::String(key.0)),
                ("access", Value::String(key.1)),
                (left_label, Value::Array(left_values)),
                (right_label, Value::Array(right_values)),
                ("change", Value::String(change)),
            ])
        })
        .collect()
}

fn group_capabilities(values: &[Value]) -> BTreeMap<(String, String), Vec<Value>> {
    let mut grouped = BTreeMap::<(String, String), Vec<Value>>::new();
    for grant in values {
        let key = (
            grant.get("id")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_owned(),
            grant.get("access")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_owned(),
        );
        grouped.entry(key).or_default().push(grant.clone());
    }

    for grants in grouped.values_mut() {
        grants.sort_by_key(canonical_json_string);
    }

    grouped
}

fn canonical_value_strings(values: &[Value]) -> Vec<String> {
    let mut rendered = values.iter().map(canonical_json_string).collect::<Vec<_>>();
    rendered.sort();
    rendered
}

fn canonical_json_string(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "null".into())
}

fn capability_grants(maybe_grants: Option<&Value>) -> Vec<Value> {
    maybe_grants
        .and_then(Value::as_array)
        .map(|items| items.to_vec())
        .unwrap_or_default()
}

fn policy_reasons(record: &Value) -> Vec<Value> {
    record
        .pointer("/policy_decision/reasons")
        .and_then(Value::as_array)
        .map(|items| items.to_vec())
        .unwrap_or_default()
}

fn normalize_http_method(method: &str) -> Result<String, HttpAuthorityError> {
    match method.to_ascii_lowercase().as_str() {
        "get" => Ok("get".into()),
        "head" => Ok("head".into()),
        _ => Err(HttpAuthorityError {
            code: "invalid-http-method".into(),
            message: "candidate_request.method must be `get` or `head`".into(),
            detail: Some(json!({ "method": method })),
        }),
    }
}

#[derive(Debug)]
pub struct HttpAuthorityError {
    pub code: String,
    pub message: String,
    pub detail: Option<Value>,
}

#[derive(Clone)]
struct ParsedCandidateRequest {
    url: String,
    scheme: String,
    host: String,
    port: u16,
    path: String,
    host_kind: ParsedHostKind,
}

impl ParsedCandidateRequest {
    fn parse(url: &str) -> Result<Self, HttpAuthorityError> {
        let parsed = Url::parse(url).map_err(|error| HttpAuthorityError {
            code: "http-request-url-invalid".into(),
            message: "candidate_request.url must be an absolute HTTP or HTTPS URL".into(),
            detail: Some(json!({
                "url": url,
                "error": error.to_string(),
            })),
        })?;

        if !parsed.username().is_empty() || parsed.password().is_some() {
            return Err(HttpAuthorityError {
                code: "http-request-url-invalid".into(),
                message: "candidate_request.url must not embed credentials".into(),
                detail: Some(json!({ "url": url })),
            });
        }

        let scheme = match parsed.scheme() {
            "http" => "http".to_owned(),
            "https" => "https".to_owned(),
            other => {
                return Err(HttpAuthorityError {
                    code: "http-request-url-invalid".into(),
                    message: "candidate_request.url must use HTTP or HTTPS".into(),
                    detail: Some(json!({ "url": url, "scheme": other })),
                });
            }
        };

        let (host, host_kind) = match parsed.host() {
            Some(url::Host::Domain(domain)) => {
                let normalized = domain.to_ascii_lowercase();
                let loopback_name = normalized == "localhost" || normalized.ends_with(".localhost");
                (normalized, ParsedHostKind::Domain { loopback_name })
            }
            Some(url::Host::Ipv4(address)) => (
                address.to_string(),
                ParsedHostKind::IpLiteral(IpAddr::V4(address)),
            ),
            Some(url::Host::Ipv6(address)) => (
                address.to_string(),
                ParsedHostKind::IpLiteral(IpAddr::V6(address)),
            ),
            None => {
                return Err(HttpAuthorityError {
                    code: "http-request-url-invalid".into(),
                    message: "candidate_request.url must include a host".into(),
                    detail: Some(json!({ "url": url })),
                });
            }
        };

        let port = parsed.port_or_known_default().ok_or_else(|| HttpAuthorityError {
            code: "http-request-url-invalid".into(),
            message: "candidate_request.url must resolve to an explicit or default port".into(),
            detail: Some(json!({ "url": url })),
        })?;

        let path = if parsed.path().is_empty() {
            "/".to_owned()
        } else {
            parsed.path().to_owned()
        };

        Ok(Self {
            url: url.to_owned(),
            scheme,
            host,
            port,
            path,
            host_kind,
        })
    }

    fn is_ip_literal(&self) -> bool {
        matches!(self.host_kind, ParsedHostKind::IpLiteral(_))
    }

    fn loopback_name(&self) -> bool {
        matches!(
            self.host_kind,
            ParsedHostKind::Domain {
                loopback_name: true,
            }
        )
    }

    fn ip_addr(&self) -> Option<IpAddr> {
        match self.host_kind {
            ParsedHostKind::Domain { .. } => None,
            ParsedHostKind::IpLiteral(ip) => Some(ip),
        }
    }

    fn destination_classification(&self) -> Value {
        match self.host_kind {
            ParsedHostKind::IpLiteral(ip) => json!({
                "host_kind": "ip-literal",
                "host": self.host,
                "loopback_name": false,
                "ip_classification": classify_ip_address(ip),
                "host_resolution_required": false,
            }),
            ParsedHostKind::Domain { loopback_name } => json!({
                "host_kind": if loopback_name { "loopback-name" } else { "domain" },
                "host": self.host,
                "loopback_name": loopback_name,
                "ip_classification": Value::Null,
                "host_resolution_required": !loopback_name,
            }),
        }
    }
}

#[derive(Clone, Copy)]
enum ParsedHostKind {
    Domain { loopback_name: bool },
    IpLiteral(IpAddr),
}

#[derive(Default, Clone)]
struct HttpGrantSummary {
    matched_scheme: Value,
    matched_host_or_suffix: Value,
    matched_port: Value,
    matched_method: Value,
    matched_path_prefix: Value,
}

struct HttpGrantEvaluation {
    grant_index: usize,
    decision: &'static str,
    summary: HttpGrantSummary,
    failed_check: Value,
    host_resolution_required: bool,
    denial_reason: Option<Value>,
    constraints: Value,
}

fn evaluate_http_grant(
    grant_index: usize,
    grant: &Value,
    candidate: &ParsedCandidateRequest,
    method: &str,
    timeout_ms: Option<u64>,
) -> HttpGrantEvaluation {
    let constraints = grant
        .get("constraints")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let mut summary = HttpGrantSummary::default();

    if !string_list_matches(constraints.get("allowed_methods"), method) {
        return denied_http_grant(
            grant_index,
            "method",
            "http-request-method-not-granted",
            "candidate method was not granted for this stored execution",
            candidate,
            &constraints,
            summary,
        );
    }
    summary.matched_method = Value::String(method.to_owned());

    if !string_list_matches(constraints.get("allowed_schemes"), &candidate.scheme) {
        return denied_http_grant(
            grant_index,
            "scheme",
            "http-request-scheme-not-granted",
            "candidate scheme was not granted for this stored execution",
            candidate,
            &constraints,
            summary,
        );
    }
    summary.matched_scheme = Value::String(candidate.scheme.clone());

    match match_host_or_suffix(&constraints, &candidate.host, candidate.is_ip_literal()) {
        HostMatch::Denied => {
            return denied_http_grant(
                grant_index,
                "host",
                "http-request-host-not-granted",
                "candidate host was not granted for this stored execution",
                candidate,
                &constraints,
                summary,
            );
        }
        HostMatch::AllowedByExact(host) | HostMatch::AllowedBySuffix(host) => {
            summary.matched_host_or_suffix = Value::String(host);
        }
    }

    if !u64_list_matches(constraints.get("allowed_ports"), u64::from(candidate.port)) {
        return denied_http_grant(
            grant_index,
            "port",
            "http-request-port-not-granted",
            "candidate port was not granted for this stored execution",
            candidate,
            &constraints,
            summary,
        );
    }
    summary.matched_port = Value::Number(u64::from(candidate.port).into());

    match match_path_prefix(&constraints, &candidate.path) {
        None if constraints.get("allowed_path_prefixes").is_some() => {
            return denied_http_grant(
                grant_index,
                "path-prefix",
                "http-request-path-not-granted",
                "candidate path prefix was not granted for this stored execution",
                candidate,
                &constraints,
                summary,
            );
        }
        Some(prefix) => {
            summary.matched_path_prefix = Value::String(prefix);
        }
        None => {
            summary.matched_path_prefix = Value::String("*".into());
        }
    }

    if timeout_ms.is_some_and(|candidate_timeout| {
        constraints
            .get("max_timeout_ms")
            .and_then(Value::as_u64)
            .is_some_and(|max_timeout| candidate_timeout > max_timeout)
    }) {
        return denied_http_grant(
            grant_index,
            "timeout",
            "http-request-timeout-not-granted",
            "candidate timeout exceeded the stored max_timeout_ms grant",
            candidate,
            &constraints,
            summary,
        );
    }

    if candidate.is_ip_literal()
        && constraints
            .get("allow_ip_literals")
            .and_then(Value::as_bool)
            != Some(true)
    {
        return denied_http_grant(
            grant_index,
            "ip-literal",
            "http-request-ip-literal-not-granted",
            "candidate IP-literal destinations were not granted for this stored execution",
            candidate,
            &constraints,
            summary,
        );
    }

    if candidate.loopback_name()
        && constraints
            .get("allow_loopback")
            .and_then(Value::as_bool)
            != Some(true)
    {
        return denied_http_grant(
            grant_index,
            "loopback-name",
            "http-request-loopback-not-granted",
            "candidate loopback hostnames were not granted for this stored execution",
            candidate,
            &constraints,
            summary,
        );
    }

    if let Some(ip) = candidate.ip_addr() {
        match classify_ip_address(ip).as_str() {
            "loopback" if constraints.get("allow_loopback").and_then(Value::as_bool) != Some(true) => {
                return denied_http_grant(
                    grant_index,
                    "loopback",
                    "http-request-loopback-not-granted",
                    "candidate loopback destinations were not granted for this stored execution",
                    candidate,
                    &constraints,
                    summary,
                );
            }
            "link-local" if constraints.get("allow_link_local").and_then(Value::as_bool) != Some(true) => {
                return denied_http_grant(
                    grant_index,
                    "link-local",
                    "http-request-link-local-not-granted",
                    "candidate link-local destinations were not granted for this stored execution",
                    candidate,
                    &constraints,
                    summary,
                );
            }
            "private-network"
                if constraints
                    .get("allow_private_networks")
                    .and_then(Value::as_bool)
                    != Some(true) =>
            {
                return denied_http_grant(
                    grant_index,
                    "private-network",
                    "http-request-private-network-not-granted",
                    "candidate private-network destinations were not granted for this stored execution",
                    candidate,
                    &constraints,
                    summary,
                );
            }
            _ => {}
        }
    } else if !candidate.loopback_name()
        && !all_risky_destination_flags_enabled(&constraints)
    {
        return HttpGrantEvaluation {
            grant_index,
            decision: "indeterminate",
            summary,
            failed_check: Value::String("host-resolution".into()),
            host_resolution_required: true,
            denial_reason: None,
            constraints,
        };
    }

    HttpGrantEvaluation {
        grant_index,
        decision: "allowed",
        summary,
        failed_check: Value::Null,
        host_resolution_required: false,
        denial_reason: None,
        constraints,
    }
}

fn denied_http_grant(
    grant_index: usize,
    failed_check: &str,
    code: &str,
    message: &str,
    candidate: &ParsedCandidateRequest,
    constraints: &Value,
    summary: HttpGrantSummary,
) -> HttpGrantEvaluation {
    HttpGrantEvaluation {
        grant_index,
        decision: "denied",
        summary,
        failed_check: Value::String(failed_check.into()),
        host_resolution_required: false,
        denial_reason: Some(json!({
            "code": code,
            "message": message,
            "detail": {
                "url": candidate.url,
                "host": candidate.host,
                "port": candidate.port,
                "path": candidate.path,
            },
        })),
        constraints: constraints.clone(),
    }
}

fn string_list_matches(maybe_values: Option<&Value>, candidate: &str) -> bool {
    maybe_values.is_none_or(|values| {
        values
            .as_array()
            .is_none_or(|items| items.iter().filter_map(Value::as_str).any(|item| item == candidate))
    })
}

fn u64_list_matches(maybe_values: Option<&Value>, candidate: u64) -> bool {
    maybe_values.is_none_or(|values| {
        values
            .as_array()
            .is_none_or(|items| items.iter().filter_map(Value::as_u64).any(|item| item == candidate))
    })
}

enum HostMatch {
    AllowedByExact(String),
    AllowedBySuffix(String),
    Denied,
}

fn match_host_or_suffix(constraints: &Value, host: &str, is_ip_literal: bool) -> HostMatch {
    let allowed_hosts = constraints
        .get("allowed_hosts")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(canonicalize_http_host)
                .collect::<Vec<_>>()
        });
    let allowed_suffixes = constraints
        .get("allowed_host_suffixes")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(|suffix| suffix.to_ascii_lowercase())
                .collect::<Vec<_>>()
        });

    if allowed_hosts.is_none() && allowed_suffixes.is_none() {
        return HostMatch::AllowedByExact("*".into());
    }

    let canonical_host = canonicalize_http_host(host);
    if allowed_hosts.as_ref().is_some_and(|hosts| hosts.iter().any(|candidate| candidate == &canonical_host)) {
        return HostMatch::AllowedByExact(canonical_host);
    }
    if !is_ip_literal
        && allowed_suffixes.as_ref().is_some_and(|suffixes| {
            suffixes
                .iter()
                .any(|suffix| domain_suffix_matches(&canonical_host, suffix))
        })
    {
        return HostMatch::AllowedBySuffix(canonical_host);
    }

    HostMatch::Denied
}

fn match_path_prefix(constraints: &Value, path: &str) -> Option<String> {
    let prefixes = constraints
        .get("allowed_path_prefixes")
        .and_then(Value::as_array)?;

    prefixes
        .iter()
        .filter_map(Value::as_str)
        .find(|prefix| path.starts_with(prefix))
        .map(ToOwned::to_owned)
}

fn all_risky_destination_flags_enabled(constraints: &Value) -> bool {
    constraints.get("allow_loopback").and_then(Value::as_bool) == Some(true)
        && constraints.get("allow_link_local").and_then(Value::as_bool) == Some(true)
        && constraints
            .get("allow_private_networks")
            .and_then(Value::as_bool)
            == Some(true)
}

fn canonicalize_http_host(host: &str) -> String {
    host.parse::<IpAddr>()
        .map_or_else(|_| host.to_ascii_lowercase(), |address| address.to_string())
}

fn domain_suffix_matches(host: &str, suffix: &str) -> bool {
    host == suffix
        || host
            .strip_suffix(suffix)
            .is_some_and(|prefix| prefix.ends_with('.'))
}

fn classify_ip_address(address: IpAddr) -> String {
    match address {
        IpAddr::V4(address) if address.is_loopback() => "loopback".into(),
        IpAddr::V6(address) if address.is_loopback() => "loopback".into(),
        IpAddr::V4(address) if address.is_link_local() => "link-local".into(),
        IpAddr::V6(address) if address.is_unicast_link_local() => "link-local".into(),
        IpAddr::V4(address) if address.is_private() => "private-network".into(),
        IpAddr::V6(address) if address.is_unique_local() => "private-network".into(),
        _ => "other".into(),
    }
}

trait TapMut {
    fn tap_mut(self, f: impl FnOnce(&mut Self)) -> Self
    where
        Self: Sized;
}

impl TapMut for Value {
    fn tap_mut(mut self, f: impl FnOnce(&mut Self)) -> Self {
        f(&mut self);
        self
    }
}

pub fn diff_summary_object(left: Value, right: Value) -> Value {
    let changed = left != right;
    json!({
        "left": left,
        "right": right,
        "changed": changed,
    })
}

pub fn likely_authority_drivers(left: &Value, right: &Value) -> Vec<Value> {
    let mut drivers = Vec::new();

    if resolved_skill(left) != resolved_skill(right) {
        drivers.push(json!({
            "type": "skill-identity-changed",
            "left": resolved_skill(left),
            "right": resolved_skill(right),
        }));
    }

    if policy_profile(left) != policy_profile(right) {
        drivers.push(json!({
            "type": "policy-profile-changed",
            "left": policy_profile(left),
            "right": policy_profile(right),
        }));
    }

    if trust_tier(left) != trust_tier(right) {
        drivers.push(json!({
            "type": "trust-tier-changed",
            "left": trust_tier(left),
            "right": trust_tier(right),
        }));
    }

    if verification_state(left) != verification_state(right) {
        drivers.push(json!({
            "type": "verification-state-changed",
            "left": verification_state(left),
            "right": verification_state(right),
        }));
    }

    for group in compare_execution_granted_capabilities(left, right) {
        if group.get("change").and_then(Value::as_str) != Some("same") {
            drivers.push(json!({
                "type": "granted-capability-changed",
                "id": group.get("id").cloned().unwrap_or(Value::Null),
                "access": group.get("access").cloned().unwrap_or(Value::Null),
                "change": group.get("change").cloned().unwrap_or(Value::Null),
            }));
        }
    }

    let left_reason_codes = policy_reason_codes(left);
    let right_reason_codes = policy_reason_codes(right);
    if left_reason_codes != right_reason_codes {
        drivers.push(json!({
            "type": "policy-reason-codes-changed",
            "left": left_reason_codes,
            "right": right_reason_codes,
        }));
    }

    if termination(left) != termination(right) {
        drivers.push(json!({
            "type": "termination-changed",
            "left": termination(left),
            "right": termination(right),
        }));
    }

    drivers
}

pub fn object_with_pairs(pairs: &[(&str, Value)]) -> Value {
    let mut object = Map::new();
    for (key, value) in pairs {
        object.insert((*key).into(), value.clone());
    }
    Value::Object(object)
}
