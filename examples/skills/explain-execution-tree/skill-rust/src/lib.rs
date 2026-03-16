use std::cmp;
use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Map, Value};
use wit_bindgen::generate;

generate!({
    path: "../../../../wit",
    world: "guild-skill",
});

use crate::exports::guild::skill::skill::{ExecutionContext, Guest, Json, SkillError, SkillOutput};
use crate::guild::skill::host;
use crate::guild::skill::types::{
    CapabilityAccess, CapabilityConstraints, CapabilityId, ExecutionMode, GrantedCapability,
    HttpMethod, HttpScheme, ResolvedSkillRef, ResourceKind, ResourceReadResult, Severity,
};

const DEFAULT_MAX_DEPTH: u64 = 4;
const HARD_MAX_DEPTH: u64 = 8;
const DEFAULT_MAX_NODES: u64 = 32;
const HARD_MAX_NODES: u64 = 128;
const NOTABLE_EVIDENCE_LIMIT: usize = 8;

struct ExplainExecutionTree;

#[derive(Debug)]
struct ExplainTreeInput {
    execution_uri: String,
    max_depth: u64,
    max_nodes: u64,
    include_evidence_resources: bool,
}

struct TraversalState {
    root_execution_id: Option<String>,
    root_resolved_skill: Option<Value>,
    root_status: Option<String>,
    root_output_summary: Option<String>,
    root_termination: Value,
    root_child_execution_count: usize,
    nodes_visited: u64,
    max_depth_walked: u64,
    traversal_truncated: bool,
    executions: Vec<Value>,
    lineage_warnings: Vec<Value>,
    status_counts: BTreeMap<String, u64>,
    termination_summaries: BTreeMap<String, AggregateSummary>,
    denial_summaries: BTreeMap<String, AggregateSummary>,
    evidence_count_total: u64,
    evidence_by_mime_type: BTreeMap<String, u64>,
    evidence_by_audience: BTreeMap<String, u64>,
    evidence_by_redaction: BTreeMap<String, u64>,
    notable_evidence_uris: Vec<String>,
    evidence_resource_descriptors: Vec<Value>,
    seen_execution_uris: BTreeSet<String>,
    seen_evidence_uris: BTreeSet<String>,
}

#[derive(Clone)]
struct AggregateSummary {
    phase: Option<String>,
    code: String,
    message: String,
    retryable: bool,
    count: u64,
    sample_execution_uris: Vec<String>,
}

impl AggregateSummary {
    fn new(
        phase: Option<String>,
        code: String,
        message: String,
        retryable: bool,
        execution_uri: &str,
    ) -> Self {
        Self {
            phase,
            code,
            message,
            retryable,
            count: 1,
            sample_execution_uris: vec![execution_uri.to_owned()],
        }
    }

    fn record_execution(&mut self, execution_uri: &str) {
        self.count += 1;
        if self.sample_execution_uris.len() < 4
            && !self.sample_execution_uris.iter().any(|uri| uri == execution_uri)
        {
            self.sample_execution_uris.push(execution_uri.to_owned());
        }
    }

    fn to_value(&self) -> Value {
        json!({
            "phase": self.phase,
            "code": self.code,
            "message": self.message,
            "retryable": self.retryable,
            "count": self.count,
            "sample_execution_uris": self.sample_execution_uris,
        })
    }
}

impl TraversalState {
    fn new() -> Self {
        let mut status_counts = BTreeMap::new();
        status_counts.insert("succeeded".into(), 0);
        status_counts.insert("failed".into(), 0);
        status_counts.insert("partial".into(), 0);
        status_counts.insert("rejected".into(), 0);

        Self {
            root_execution_id: None,
            root_resolved_skill: None,
            root_status: None,
            root_output_summary: None,
            root_termination: Value::Null,
            root_child_execution_count: 0,
            nodes_visited: 0,
            max_depth_walked: 0,
            traversal_truncated: false,
            executions: Vec::new(),
            lineage_warnings: Vec::new(),
            status_counts,
            termination_summaries: BTreeMap::new(),
            denial_summaries: BTreeMap::new(),
            evidence_count_total: 0,
            evidence_by_mime_type: BTreeMap::new(),
            evidence_by_audience: BTreeMap::new(),
            evidence_by_redaction: BTreeMap::new(),
            notable_evidence_uris: Vec::new(),
            evidence_resource_descriptors: Vec::new(),
            seen_execution_uris: BTreeSet::new(),
            seen_evidence_uris: BTreeSet::new(),
        }
    }

    fn push_warning(&mut self, value: Value) {
        self.lineage_warnings.push(value);
    }

    fn push_error_warning(
        &mut self,
        kind: &str,
        execution_uri: &str,
        depth: u64,
        error: &SkillError,
    ) {
        self.push_warning(json!({
            "kind": kind,
            "code": error.code,
            "execution_uri": execution_uri,
            "depth": depth,
            "message": error.message,
            "detail": parse_error_detail(error.detail.as_deref()),
        }));
    }

    fn mark_truncated(&mut self) {
        self.traversal_truncated = true;
    }

    fn record_status(&mut self, status: &str) {
        let entry = self.status_counts.entry(status.to_owned()).or_insert(0);
        *entry += 1;
    }

    fn record_aggregate(
        map: &mut BTreeMap<String, AggregateSummary>,
        phase: Option<String>,
        code: String,
        message: String,
        retryable: bool,
        execution_uri: &str,
    ) {
        let key = format!(
            "{}|{}|{}|{}",
            phase.as_deref().unwrap_or(""),
            code,
            message,
            retryable
        );
        if let Some(summary) = map.get_mut(&key) {
            summary.record_execution(execution_uri);
        } else {
            map.insert(
                key,
                AggregateSummary::new(phase, code, message, retryable, execution_uri),
            );
        }
    }

    fn record_termination_summary(&mut self, execution_uri: &str, termination: &Value) {
        let Some(phase) = termination
            .get("phase")
            .and_then(Value::as_str)
            .map(str::to_owned)
        else {
            return;
        };
        let Some(code) = termination
            .get("code")
            .and_then(Value::as_str)
            .map(str::to_owned)
        else {
            return;
        };
        let message = termination
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("execution terminated")
            .to_owned();
        let retryable = termination
            .get("retryable")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        Self::record_aggregate(
            &mut self.termination_summaries,
            Some(phase),
            code,
            message,
            retryable,
            execution_uri,
        );
    }

    fn record_denial_summary(&mut self, execution_uri: &str, record: &Value) {
        if let Some(termination) = record.get("termination") {
            let Some(phase) = termination
                .get("phase")
                .and_then(Value::as_str)
                .map(str::to_owned)
            else {
                return;
            };
            let Some(code) = termination
                .get("code")
                .and_then(Value::as_str)
                .map(str::to_owned)
            else {
                return;
            };
            let message = termination
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("execution was rejected")
                .to_owned();
            let retryable = termination
                .get("retryable")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            Self::record_aggregate(
                &mut self.denial_summaries,
                Some(phase),
                code,
                message,
                retryable,
                execution_uri,
            );
            return;
        }

        if record
            .pointer("/policy_decision/outcome")
            .and_then(Value::as_str)
            == Some("rejected")
        {
            let message = record
                .pointer("/policy_decision/summary")
                .and_then(Value::as_str)
                .unwrap_or("policy rejected execution")
                .to_owned();
            Self::record_aggregate(
                &mut self.denial_summaries,
                None,
                "policy-rejected".into(),
                message,
                false,
                execution_uri,
            );
        }
    }

    fn record_evidence(
        &mut self,
        evidence_items: &[Value],
        include_evidence_resources: bool,
        can_read_evidence_resources: bool,
    ) {
        for evidence in evidence_items {
            self.evidence_count_total += 1;

            let mime_type = evidence
                .get("mime_type")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let audience = evidence
                .get("audience")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let redaction = evidence
                .get("redaction")
                .and_then(Value::as_str)
                .unwrap_or("unknown");

            increment_count(&mut self.evidence_by_mime_type, mime_type);
            increment_count(&mut self.evidence_by_audience, audience);
            increment_count(&mut self.evidence_by_redaction, redaction);

            let Some(uri) = evidence.get("uri").and_then(Value::as_str) else {
                continue;
            };

            if self.seen_evidence_uris.insert(uri.to_owned())
                && self.notable_evidence_uris.len() < NOTABLE_EVIDENCE_LIMIT
            {
                self.notable_evidence_uris.push(uri.to_owned());
            }

            if !include_evidence_resources
                || !can_read_evidence_resources
                || self.evidence_resource_descriptors.len() >= NOTABLE_EVIDENCE_LIMIT
                || !uri.starts_with("guild://objects/records/")
            {
                continue;
            }

            match read_resource(uri) {
                Ok(resource) => self.evidence_resource_descriptors.push(json!({
                    "uri": resource.uri,
                    "mime_type": resource.mime_type,
                    "sha256": resource.sha256,
                    "readable": true,
                })),
                Err(error) => self.evidence_resource_descriptors.push(json!({
                    "uri": uri,
                    "readable": false,
                    "error": {
                        "code": error.code,
                        "message": error.message,
                        "detail": parse_error_detail(error.detail.as_deref()),
                    }
                })),
            }
        }
    }
}

impl Guest for ExplainExecutionTree {
    fn run(ctx: ExecutionContext, input: Json) -> Result<SkillOutput, SkillError> {
        let parsed_input: Value = serde_json::from_str(&input).map_err(|error| SkillError {
            code: "invalid-input".into(),
            message: "input JSON could not be parsed".into(),
            retryable: false,
            detail: Some(json!({ "error": error.to_string() }).to_string()),
        })?;
        let config = parse_input(&parsed_input)?;
        let can_read_evidence_resources =
            can_read_object_record_resources(&ctx.granted_capabilities);
        let grants = granted_capabilities_payload(&ctx.granted_capabilities);
        let mut state = TraversalState::new();

        let root_record = read_execution_record(&config.execution_uri)?;
        visit_execution_record(
            &config.execution_uri,
            &root_record,
            0,
            None,
            &config,
            can_read_evidence_resources,
            &mut state,
        )?;

        if config.include_evidence_resources
            && state.evidence_count_total > 0
            && !can_read_evidence_resources
        {
            state.push_warning(json!({
                "kind": "evidence-resource-scope-not-granted",
                "code": "object-scope-not-granted",
                "depth": 0,
                "message": "optional evidence resource reads were skipped because object-record read scope was not granted",
                "detail": {
                    "required_scope": "guild://objects/records/",
                    "resource_kind": "object",
                }
            }));
        }

        let succeeded = *state.status_counts.get("succeeded").unwrap_or(&0);
        let failed = *state.status_counts.get("failed").unwrap_or(&0);
        let rejected = *state.status_counts.get("rejected").unwrap_or(&0);
        let summary = format!(
            "Execution tree rooted at {} visited {} node(s): {} succeeded, {} failed, {} rejected, {} evidence record(s).{}",
            config.execution_uri,
            state.nodes_visited,
            succeeded,
            failed,
            rejected,
            state.evidence_count_total,
            if state.traversal_truncated {
                " Traversal was truncated by configured bounds."
            } else {
                ""
            }
        );

        Ok(SkillOutput {
            summary,
            structured: json!({
                "target_execution_uri": config.execution_uri,
                "root_execution_id": state.root_execution_id.unwrap_or_default(),
                "root_execution_uri": state.executions.first()
                    .and_then(|record| record.get("execution_uri"))
                    .cloned()
                    .unwrap_or(Value::Null),
                "root_resolved_skill": state.root_resolved_skill.unwrap_or(Value::Null),
                "root_status": state.root_status.unwrap_or_else(|| "unknown".into()),
                "root_output_summary": state.root_output_summary,
                "root_termination": state.root_termination,
                "child_execution_count": state.root_child_execution_count,
                "descendant_execution_count": state.nodes_visited.saturating_sub(1),
                "nodes_visited": state.nodes_visited,
                "max_depth_walked": state.max_depth_walked,
                "traversal_truncated": state.traversal_truncated,
                "status_counts": counts_to_value(&state.status_counts),
                "termination_summaries": aggregate_summaries_to_value(&state.termination_summaries),
                "denial_summaries": aggregate_summaries_to_value(&state.denial_summaries),
                "lineage_warnings": state.lineage_warnings,
                "executions": state.executions,
                "evidence_summary": {
                    "total": state.evidence_count_total,
                    "by_mime_type": counts_to_value(&state.evidence_by_mime_type),
                    "by_audience": counts_to_value(&state.evidence_by_audience),
                    "by_redaction": counts_to_value(&state.evidence_by_redaction),
                    "notable_evidence_uris": state.notable_evidence_uris,
                    "resource_descriptors": state.evidence_resource_descriptors,
                },
                "granted_capabilities": grants,
            })
            .to_string(),
            diagnostics: Vec::new(),
            effects: Vec::new(),
            evidence: Vec::new(),
        })
    }
}

fn parse_input(parsed_input: &Value) -> Result<ExplainTreeInput, SkillError> {
    let execution_uri = parsed_input
        .get("execution_uri")
        .and_then(Value::as_str)
        .filter(|uri| !uri.is_empty())
        .ok_or_else(|| SkillError {
            code: "missing-execution-uri".into(),
            message: "execution_uri must be a non-empty string".into(),
            retryable: false,
            detail: None,
        })?
        .to_owned();
    let max_depth = parse_bounded_u64(
        parsed_input,
        "max_depth",
        DEFAULT_MAX_DEPTH,
        0,
        HARD_MAX_DEPTH,
    )?;
    let max_nodes = parse_bounded_u64(
        parsed_input,
        "max_nodes",
        DEFAULT_MAX_NODES,
        1,
        HARD_MAX_NODES,
    )?;
    let include_evidence_resources = parsed_input
        .get("include_evidence_resources")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    Ok(ExplainTreeInput {
        execution_uri,
        max_depth,
        max_nodes,
        include_evidence_resources,
    })
}

fn parse_bounded_u64(
    parsed_input: &Value,
    field: &str,
    default: u64,
    min: u64,
    max: u64,
) -> Result<u64, SkillError> {
    let Some(value) = parsed_input.get(field) else {
        return Ok(default);
    };
    let Some(parsed) = value.as_u64() else {
        return Err(SkillError {
            code: "invalid-bound".into(),
            message: format!("{field} must be an integer"),
            retryable: false,
            detail: Some(json!({ "field": field, "value": value }).to_string()),
        });
    };
    if parsed < min || parsed > max {
        return Err(SkillError {
            code: "invalid-bound".into(),
            message: format!("{field} must be between {min} and {max}"),
            retryable: false,
            detail: Some(json!({ "field": field, "value": parsed }).to_string()),
        });
    }
    Ok(parsed)
}

fn visit_execution_record(
    execution_uri: &str,
    record: &Value,
    depth: u64,
    alias_from_parent: Option<&str>,
    config: &ExplainTreeInput,
    can_read_evidence_resources: bool,
    state: &mut TraversalState,
) -> Result<(), SkillError> {
    if state.nodes_visited >= config.max_nodes {
        state.mark_truncated();
        state.push_warning(json!({
            "kind": "max-nodes-reached",
            "code": "max-nodes-reached",
            "execution_uri": execution_uri,
            "depth": depth,
            "message": "execution tree traversal stopped after reaching the configured node limit",
            "detail": {
                "max_nodes": config.max_nodes,
            }
        }));
        return Ok(());
    }

    if !state.seen_execution_uris.insert(execution_uri.to_owned()) {
        state.mark_truncated();
        state.push_warning(json!({
            "kind": "execution-uri-revisited",
            "code": "execution-uri-revisited",
            "execution_uri": execution_uri,
            "depth": depth,
            "message": "execution lineage referenced a previously visited execution URI; traversal stopped on that branch",
            "detail": null
        }));
        return Ok(());
    }

    state.nodes_visited += 1;
    state.max_depth_walked = cmp::max(state.max_depth_walked, depth);

    let status = record
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let child_executions = record
        .get("child_executions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let emitted_evidence = record
        .get("emitted_evidence")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let termination = record.get("termination").cloned().unwrap_or(Value::Null);
    let output_summary = record
        .pointer("/output/summary")
        .and_then(Value::as_str)
        .map(str::to_owned);

    if depth == 0 {
        state.root_execution_id = record
            .pointer("/receipt/execution_id")
            .and_then(Value::as_str)
            .map(str::to_owned);
        state.root_resolved_skill = record.get("resolved_skill").cloned();
        state.root_status = Some(status.to_owned());
        state.root_output_summary = output_summary.clone();
        state.root_termination = termination.clone();
        state.root_child_execution_count = child_executions.len();
    }

    state.record_status(status);
    if !termination.is_null() {
        state.record_termination_summary(execution_uri, &termination);
    }
    if status == "rejected"
        || record
            .pointer("/policy_decision/outcome")
            .and_then(Value::as_str)
            == Some("rejected")
    {
        state.record_denial_summary(execution_uri, record);
    }

    state.record_evidence(
        &emitted_evidence,
        config.include_evidence_resources,
        can_read_evidence_resources,
    );

    state.executions.push(json!({
        "depth": depth,
        "alias_from_parent": alias_from_parent,
        "execution_uri": execution_uri,
        "execution_id": record.pointer("/receipt/execution_id").cloned().unwrap_or(Value::Null),
        "parent_execution_id": record.get("parent_execution_id").cloned().unwrap_or(Value::Null),
        "resolved_skill": record.get("resolved_skill").cloned().unwrap_or(Value::Null),
        "status": status,
        "output_summary": output_summary,
        "child_execution_count": child_executions.len(),
        "evidence_count": emitted_evidence.len(),
        "termination": termination,
    }));

    if depth >= config.max_depth {
        if !child_executions.is_empty() {
            state.mark_truncated();
            state.push_warning(json!({
                "kind": "max-depth-reached",
                "code": "max-depth-reached",
                "execution_uri": execution_uri,
                "depth": depth,
                "message": "execution tree traversal stopped after reaching the configured depth limit",
                "detail": {
                    "max_depth": config.max_depth,
                    "remaining_children": child_executions.len(),
                }
            }));
        }
        return Ok(());
    }

    for child in child_executions {
        if state.nodes_visited >= config.max_nodes {
            state.mark_truncated();
            state.push_warning(json!({
                "kind": "max-nodes-reached",
                "code": "max-nodes-reached",
                "execution_uri": execution_uri,
                "depth": depth + 1,
                "message": "execution tree traversal stopped after reaching the configured node limit",
                "detail": {
                    "max_nodes": config.max_nodes,
                }
            }));
            break;
        }

        let Some(child_uri) = child.get("uri").and_then(Value::as_str) else {
            state.push_warning(json!({
                "kind": "child-link-missing-uri",
                "code": "child-link-missing-uri",
                "execution_uri": execution_uri,
                "depth": depth + 1,
                "message": "child execution linkage did not contain a valid execution URI",
                "detail": {
                    "child_link": child,
                }
            }));
            continue;
        };

        if state.seen_execution_uris.contains(child_uri) {
            state.mark_truncated();
            state.push_warning(json!({
                "kind": "execution-uri-revisited",
                "code": "execution-uri-revisited",
                "execution_uri": child_uri,
                "depth": depth + 1,
                "message": "execution lineage referenced a previously visited execution URI; traversal stopped on that branch",
                "detail": {
                    "alias": child.get("alias").cloned().unwrap_or(Value::Null),
                }
            }));
            continue;
        }

        let child_record = match read_execution_record(child_uri) {
            Ok(record) => record,
            Err(error) => {
                state.push_error_warning("child-read-failed", child_uri, depth + 1, &error);
                continue;
            }
        };

        visit_execution_record(
            child_uri,
            &child_record,
            depth + 1,
            child.get("alias").and_then(Value::as_str),
            config,
            can_read_evidence_resources,
            state,
        )?;
    }

    Ok(())
}

fn read_execution_record(uri: &str) -> Result<Value, SkillError> {
    let resource = read_resource(uri)?;
    parse_json_bytes(&resource, "execution resource")
}

fn read_resource(uri: &str) -> Result<ResourceReadResult, SkillError> {
    host::read_resource(uri).map_err(|message| SkillError {
        code: "read-resource-failed".into(),
        message: "host failed to read the requested Guild resource".into(),
        retryable: false,
        detail: Some(json!({ "uri": uri, "error": message }).to_string()),
    })
}

fn parse_json_bytes(resource: &ResourceReadResult, label: &str) -> Result<Value, SkillError> {
    serde_json::from_slice(&resource.bytes).map_err(|error| SkillError {
        code: "invalid-resource-json".into(),
        message: format!("{label} did not contain valid JSON"),
        retryable: false,
        detail: Some(
            json!({
                "uri": resource.uri,
                "mime_type": resource.mime_type,
                "error": error.to_string(),
            })
            .to_string(),
        ),
    })
}

fn parse_error_detail(detail: Option<&str>) -> Value {
    detail
        .and_then(|value| serde_json::from_str::<Value>(value).ok())
        .unwrap_or_else(|| {
            detail
                .map(|value| Value::String(value.to_owned()))
                .unwrap_or(Value::Null)
        })
}

fn counts_to_value(counts: &BTreeMap<String, u64>) -> Value {
    let object = counts.iter().fold(Map::new(), |mut object, (key, value)| {
        object.insert(key.clone(), Value::Number((*value).into()));
        object
    });
    Value::Object(object)
}

fn aggregate_summaries_to_value(summaries: &BTreeMap<String, AggregateSummary>) -> Value {
    Value::Array(
        summaries
            .values()
            .map(AggregateSummary::to_value)
            .collect::<Vec<_>>(),
    )
}

fn increment_count(counts: &mut BTreeMap<String, u64>, key: &str) {
    let entry = counts.entry(key.to_owned()).or_insert(0);
    *entry += 1;
}

fn can_read_object_record_resources(grants: &[GrantedCapability]) -> bool {
    grants.iter().any(|grant| {
        grant.id == CapabilityId::ReadResource
            && grant.access == CapabilityAccess::Read
            && constraints_allow_object_record_reads(&grant.constraints)
    })
}

fn constraints_allow_object_record_reads(constraints: &CapabilityConstraints) -> bool {
    match constraints {
        CapabilityConstraints::None => true,
        CapabilityConstraints::HttpRequest(_) => false,
        CapabilityConstraints::ReadResource(value) => {
            let kind_allowed = value
                .resource_kinds
                .as_ref()
                .map_or(true, |kinds| kinds.contains(&ResourceKind::Object));
            let scope_allowed = value.uri_prefixes.as_ref().map_or(true, |prefixes| {
                prefixes
                    .iter()
                    .any(|prefix| prefix == "guild://objects/records/")
            });
            kind_allowed && scope_allowed
        }
        CapabilityConstraints::InvokeDependency(_) => false,
        CapabilityConstraints::EmitEvidence(_) => false,
        CapabilityConstraints::Log(_) => false,
    }
}

fn granted_capabilities_payload(grants: &[GrantedCapability]) -> Value {
    json!({
        "grants": grants.iter().map(|grant| {
            json!({
                "id": capability_id_label(&grant.id),
                "access": capability_access_label(&grant.access),
                "constraints": capability_constraints_payload(&grant.constraints),
            })
        }).collect::<Vec<_>>()
    })
}

fn capability_id_label(id: &CapabilityId) -> &'static str {
    match id {
        CapabilityId::HttpRequest => "http-request",
        CapabilityId::ReadResource => "read-resource",
        CapabilityId::InvokeSkill => "invoke-skill",
        CapabilityId::EmitEvidence => "emit-evidence",
        CapabilityId::GetSecret => "get-secret",
        CapabilityId::CacheRead => "cache-read",
        CapabilityId::CacheWrite => "cache-write",
        CapabilityId::LogWrite => "log-write",
        CapabilityId::MonotonicClock => "monotonic-clock",
        CapabilityId::WallClock => "wall-clock",
    }
}

fn capability_access_label(access: &CapabilityAccess) -> &'static str {
    match access {
        CapabilityAccess::Read => "read",
        CapabilityAccess::Write => "write",
        CapabilityAccess::Invoke => "invoke",
    }
}

fn capability_constraints_payload(constraints: &CapabilityConstraints) -> Value {
    match constraints {
        CapabilityConstraints::None => json!({}),
        CapabilityConstraints::HttpRequest(value) => json!({
            "allowed_schemes": value.allowed_schemes.as_ref().map(|schemes| {
                schemes.iter().map(http_scheme_label).collect::<Vec<_>>()
            }),
            "allowed_hosts": value.allowed_hosts,
            "allowed_ports": value.allowed_ports,
            "allowed_methods": value.allowed_methods.as_ref().map(|methods| {
                methods.iter().map(http_method_label).collect::<Vec<_>>()
            }),
            "allowed_path_prefixes": value.allowed_path_prefixes,
            "max_timeout_ms": value.max_timeout_ms,
            "max_response_bytes": value.max_response_bytes,
        }),
        CapabilityConstraints::ReadResource(value) => json!({
            "uri_prefixes": value.uri_prefixes,
            "resource_kinds": value.resource_kinds.as_ref().map(|kinds| {
                kinds.iter().map(resource_kind_label).collect::<Vec<_>>()
            }),
        }),
        CapabilityConstraints::InvokeDependency(value) => json!({
            "aliases": value.aliases,
        }),
        CapabilityConstraints::EmitEvidence(value) => json!({
            "max_bytes": value.max_bytes,
            "audiences": value.audiences.as_ref().map(|audiences| {
                audiences.iter().map(evidence_audience_label).collect::<Vec<_>>()
            }),
            "redactions": value.redactions.as_ref().map(|redactions| {
                redactions.iter().map(redaction_label).collect::<Vec<_>>()
            }),
        }),
        CapabilityConstraints::Log(value) => json!({
            "levels": value.levels.as_ref().map(|levels| {
                levels.iter().map(severity_label).collect::<Vec<_>>()
            }),
        }),
    }
}

fn resource_kind_label(kind: &ResourceKind) -> &'static str {
    match kind {
        ResourceKind::Execution => "execution",
        ResourceKind::Object => "object",
    }
}

fn evidence_audience_label(audience: &crate::guild::skill::types::EvidenceAudience) -> &'static str {
    match audience {
        crate::guild::skill::types::EvidenceAudience::User => "user",
        crate::guild::skill::types::EvidenceAudience::Assistant => "assistant",
        crate::guild::skill::types::EvidenceAudience::Internal => "internal",
    }
}

fn redaction_label(redaction: &crate::guild::skill::types::RedactionClass) -> &'static str {
    match redaction {
        crate::guild::skill::types::RedactionClass::None => "none",
        crate::guild::skill::types::RedactionClass::SecretsRemoved => "secrets-removed",
        crate::guild::skill::types::RedactionClass::PiiRemoved => "pii-removed",
        crate::guild::skill::types::RedactionClass::TenantSensitive => "tenant-sensitive",
    }
}

fn severity_label(severity: &Severity) -> &'static str {
    match severity {
        Severity::Info => "info",
        Severity::Warn => "warn",
        Severity::Error => "error",
    }
}

fn http_method_label(method: &HttpMethod) -> &'static str {
    match method {
        HttpMethod::Get => "get",
        HttpMethod::Head => "head",
    }
}

fn http_scheme_label(scheme: &HttpScheme) -> &'static str {
    match scheme {
        HttpScheme::Http => "http",
        HttpScheme::Https => "https",
    }
}

#[allow(dead_code)]
fn execution_mode_label(mode: &ExecutionMode) -> &'static str {
    match mode {
        ExecutionMode::Inspect => "inspect",
        ExecutionMode::Plan => "plan",
        ExecutionMode::Apply => "apply",
    }
}

#[allow(dead_code)]
fn resolved_skill_identity(skill: &ResolvedSkillRef) -> Value {
    json!({
        "key": {
            "namespace": skill.key.namespace,
            "name": skill.key.name,
        },
        "version": skill.version,
        "digest": skill.digest,
    })
}

export!(ExplainExecutionTree with_types_in self);
