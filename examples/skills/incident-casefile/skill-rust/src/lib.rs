use serde_json::{Value, json};
use wit_bindgen::generate;

#[path = "../../../ops-starter-support.rs"]
mod ops_starter_support;

const _: &str = include_str!("../../../../../wit/guild-skill-v1.wit");

generate!({
    path: "../../../../wit",
    world: "guild-skill-inspect-v1",
});

use crate::exports::guild::skill::inspect_skill::{
    ExecutionContext, Guest, Json, SkillError, SkillOutput,
};

const SUBJECT_REF_LIMIT: usize = 3;
const PAYLOAD_DETAIL_MAX_BYTES: u64 = 32 * 1024;

struct IncidentCasefile;

struct QueryContext {
    lines: Vec<String>,
    expanded_execution_uris: Vec<String>,
}

impl Guest for IncidentCasefile {
    fn run(_ctx: ExecutionContext, input: Json) -> Result<SkillOutput, SkillError> {
        let parsed_input = ops_starter_support::parse_input(&input)?;
        let subject_execution_uri =
            ops_starter_support::required_string(&parsed_input, "subject_execution_uri")?;
        let comparison_execution_uri = ops_starter_support::optional_string(
            &parsed_input,
            "comparison_execution_uri",
        )
        .map(ToOwned::to_owned);
        let query_uri =
            ops_starter_support::optional_string(&parsed_input, "query_uri").map(ToOwned::to_owned);
        let evidence_uri = ops_starter_support::optional_string(&parsed_input, "evidence_uri")
            .map(ToOwned::to_owned);

        let (_, subject_record) =
            ops_starter_support::read_json_resource(subject_execution_uri, "subject execution")?;

        let subject_ref = ops_starter_support::short_execution_ref_from_uri(subject_execution_uri);
        let subject_status = ops_starter_support::status_label(&subject_record);
        let subject_posture = ops_starter_support::execution_posture(&subject_record);
        let subject_skill = ops_starter_support::resolved_skill_label(&subject_record);
        let subject_support = ops_starter_support::rendered_support(&subject_record);
        let subject_primary_reason = ops_starter_support::primary_reason_code(&subject_record);
        let subject_primary_message = ops_starter_support::primary_reason_message(&subject_record);
        let subject_policy_outcome = subject_record
            .pointer("/policy_decision/outcome")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let subject_child_uris = ops_starter_support::child_execution_uris(&subject_record);
        let subject_evidence_uris = ops_starter_support::evidence_uris(&subject_record);

        let comparison_lines = match comparison_execution_uri.as_deref() {
            Some(uri) => comparison_snapshot_lines(uri, &subject_record)?,
            None => vec!["no comparison execution ref supplied".into()],
        };
        let query_context = match query_uri.as_deref() {
            Some(uri) => build_query_context(
                uri,
                subject_execution_uri,
                comparison_execution_uri.as_deref(),
            )?,
            None => QueryContext {
                lines: vec!["no bounded execution-query ref supplied".into()],
                expanded_execution_uris: Vec::new(),
            },
        };
        let evidence_lines = match evidence_uri.as_deref() {
            Some(uri) => evidence_context_lines(uri)?,
            None => missing_evidence_lines(&subject_evidence_uris),
        };
        let exact_refs = exact_ref_lines(
            subject_execution_uri,
            comparison_execution_uri.as_deref(),
            query_uri.as_deref(),
            &query_context.expanded_execution_uris,
            evidence_uri.as_deref(),
        );

        let markdown = ops_starter_support::render_markdown_report(&json!({
            "title": "Incident Casefile",
            "summary_line": format!(
                "{subject_status}  {subject_posture}  {subject_ref}  {subject_skill}"
            ),
            "facts": [
                { "label": "Subject execution", "value": subject_ref },
                { "label": "Subject skill", "value": subject_skill },
                { "label": "Subject status", "value": subject_status },
                { "label": "Subject posture", "value": subject_posture },
                { "label": "Subject support", "value": subject_support },
                {
                    "label": "Comparison execution",
                    "value": comparison_execution_uri
                        .as_deref()
                        .map(ops_starter_support::short_execution_ref_from_uri)
                        .unwrap_or_else(|| "not supplied".into())
                },
                {
                    "label": "Query ref",
                    "value": query_uri.clone().unwrap_or_else(|| "not supplied".into())
                },
                {
                    "label": "Evidence ref",
                    "value": evidence_uri
                        .as_deref()
                        .map(ops_starter_support::short_evidence_ref_from_uri)
                        .unwrap_or_else(|| "not supplied".into())
                }
            ],
            "sections": [
                {
                    "title": "Primary reason",
                    "lines": vec![
                        subject_primary_reason,
                        subject_primary_message,
                        format!("- policy outcome: {subject_policy_outcome}")
                    ]
                },
                {
                    "title": "Nearby subject refs",
                    "lines": subject_ref_lines(&subject_child_uris, &subject_evidence_uris)
                },
                {
                    "title": "Comparison snapshot",
                    "lines": comparison_lines
                },
                {
                    "title": "Query context",
                    "lines": query_context.lines
                },
                {
                    "title": "Evidence context",
                    "lines": evidence_lines
                },
                {
                    "title": "Exact refs used",
                    "lines": exact_refs
                },
                {
                    "title": "Next refs",
                    "lines": next_ref_lines(
                        subject_execution_uri,
                        comparison_execution_uri.as_deref(),
                        query_uri.as_deref(),
                        evidence_uri
                            .as_deref()
                            .or_else(|| subject_evidence_uris.first().map(String::as_str))
                    )
                }
            ]
        }))?;

        Ok(SkillOutput {
            summary: format!("Prepared incident casefile for {subject_execution_uri}."),
            structured: Value::String(markdown).to_string(),
            diagnostics: Vec::new(),
            effects: Vec::new(),
            evidence: Vec::new(),
        })
    }
}

fn comparison_snapshot_lines(
    comparison_execution_uri: &str,
    subject_record: &Value,
) -> Result<Vec<String>, SkillError> {
    let (_, comparison_record) = ops_starter_support::read_json_resource(
        comparison_execution_uri,
        "comparison execution",
    )?;

    Ok(vec![
        format!(
            "- execution: {}",
            ops_starter_support::short_execution_ref_from_uri(comparison_execution_uri)
        ),
        format!(
            "- status: {}",
            ops_starter_support::status_label(&comparison_record)
        ),
        format!(
            "- posture: {}",
            ops_starter_support::execution_posture(&comparison_record)
        ),
        format!(
            "- skill: {}",
            ops_starter_support::resolved_skill_label(&comparison_record)
        ),
        format!(
            "- primary reason: {}",
            ops_starter_support::primary_reason_code(&comparison_record)
        ),
        format!(
            "- changed vs subject: {}",
            diff_summary(subject_record, &comparison_record)
        ),
    ])
}

fn diff_summary(subject_record: &Value, comparison_record: &Value) -> String {
    let mut changes = Vec::new();

    if ops_starter_support::status_label(subject_record)
        != ops_starter_support::status_label(comparison_record)
    {
        changes.push("status");
    }
    if ops_starter_support::execution_posture(subject_record)
        != ops_starter_support::execution_posture(comparison_record)
    {
        changes.push("posture");
    }
    if ops_starter_support::resolved_skill_label(subject_record)
        != ops_starter_support::resolved_skill_label(comparison_record)
    {
        changes.push("skill");
    }
    if ops_starter_support::primary_reason_code(subject_record)
        != ops_starter_support::primary_reason_code(comparison_record)
    {
        changes.push("primary reason");
    }
    if ops_starter_support::rendered_support(subject_record)
        != ops_starter_support::rendered_support(comparison_record)
    {
        changes.push("support");
    }

    if changes.is_empty() {
        "unchanged on status, posture, skill, primary reason, and support".into()
    } else {
        changes.join(", ")
    }
}

fn build_query_context(
    query_uri: &str,
    subject_execution_uri: &str,
    comparison_execution_uri: Option<&str>,
) -> Result<QueryContext, SkillError> {
    let (_, query_result) =
        ops_starter_support::read_json_resource(query_uri, "execution query resource")?;
    let results = query_result
        .get("results")
        .and_then(Value::as_array)
        .ok_or_else(|| SkillError {
            code: "invalid-query-resource".into(),
            message: "execution query resource did not contain a results array".into(),
            retryable: false,
            detail: Some(query_result.to_string()),
        })?;

    let matched_execution_uris = results
        .iter()
        .filter_map(|result| result.pointer("/receipt/uri").and_then(Value::as_str))
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    let mut lines = vec![
        format!(
            "- total matches: {}",
            query_result
                .get("total_matches")
                .and_then(Value::as_u64)
                .unwrap_or(0)
        ),
        format!(
            "- returned matches: {}",
            query_result
                .get("returned_matches")
                .and_then(Value::as_u64)
                .unwrap_or(0)
        ),
        format!(
            "- truncated: {}",
            query_result
                .get("truncated")
                .and_then(Value::as_bool)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "false".into())
        ),
        format!(
            "- subject listed: {}",
            yes_no(
                matched_execution_uris
                    .iter()
                    .any(|uri| uri == subject_execution_uri)
            )
        ),
    ];

    if let Some(comparison_execution_uri) = comparison_execution_uri {
        lines.push(format!(
            "- comparison listed: {}",
            yes_no(
                matched_execution_uris
                    .iter()
                    .any(|uri| uri == comparison_execution_uri)
            )
        ));
    }

    let mut expanded_execution_uris = Vec::new();
    let mut rendered_matches = Vec::new();
    for execution_uri in [Some(subject_execution_uri), comparison_execution_uri]
        .into_iter()
        .flatten()
    {
        if expanded_execution_uris
            .iter()
            .any(|existing| existing == execution_uri)
        {
            continue;
        }
        if !matched_execution_uris.iter().any(|uri| uri == execution_uri) {
            continue;
        }
        let (_, record) =
            ops_starter_support::read_json_resource(execution_uri, "query execution resource")?;
        expanded_execution_uris.push(execution_uri.to_owned());
        rendered_matches.push(format!(
            "- {}  {}  {}  {}",
            ops_starter_support::short_execution_ref_from_uri(execution_uri),
            ops_starter_support::status_label(&record),
            ops_starter_support::execution_posture(&record),
            ops_starter_support::primary_reason_code(&record)
        ));
    }

    if rendered_matches.is_empty() {
        if matched_execution_uris.is_empty() {
            lines.push("no stored execution matches were returned".into());
        } else {
            lines.push("no supplied execution refs were expanded from this query".into());
        }
    } else {
        lines.extend(rendered_matches);
    }

    let additional_match_count = matched_execution_uris
        .iter()
        .filter(|execution_uri| {
            !expanded_execution_uris
                .iter()
                .any(|expanded| expanded == *execution_uri)
        })
        .count();
    if additional_match_count > 0 {
        lines.push(format!(
            "- additional query matches not expanded: {additional_match_count}"
        ));
    }

    Ok(QueryContext {
        lines,
        expanded_execution_uris,
    })
}

fn evidence_context_lines(evidence_uri: &str) -> Result<Vec<String>, SkillError> {
    let metadata_uri = ops_starter_support::evidence_metadata_uri(evidence_uri);
    let (_, metadata) =
        ops_starter_support::read_json_resource(&metadata_uri, "evidence metadata resource")?;

    let kind = metadata
        .get("title")
        .and_then(Value::as_str)
        .or_else(|| metadata.get("mime_type").and_then(Value::as_str))
        .unwrap_or("unknown");
    let produced_by_execution = metadata
        .get("produced_by_execution")
        .and_then(Value::as_str)
        .map(ops_starter_support::short_execution_ref_from_uri)
        .unwrap_or_else(|| "none".into());
    let sink_line = metadata
        .get("sink")
        .map(ops_starter_support::render_sink_summary)
        .unwrap_or_else(|| "sink metadata unavailable".into());

    let mut lines = vec![
        format!(
            "- evidence: {}",
            ops_starter_support::short_evidence_ref_from_uri(evidence_uri)
        ),
        format!("- kind: {kind}"),
        format!("- metadata: {metadata_uri}"),
        format!(
            "- blob: {}",
            metadata
                .get("blob_uri")
                .and_then(Value::as_str)
                .map(ops_starter_support::short_object_ref_from_uri)
                .unwrap_or_else(|| "none".into())
        ),
        format!(
            "- audience: {}",
            metadata
                .get("audience")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
        ),
        format!(
            "- redaction: {}",
            metadata
                .get("redaction")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
        ),
        format!(
            "- size: {}",
            metadata
                .get("size_bytes")
                .and_then(Value::as_u64)
                .unwrap_or(0)
        ),
        format!("- produced by: {produced_by_execution}"),
        format!("- sink: {sink_line}"),
    ];

    if metadata.get("mime_type").and_then(Value::as_str) == Some("application/json")
        && metadata
            .get("size_bytes")
            .and_then(Value::as_u64)
            .unwrap_or(u64::MAX)
            <= PAYLOAD_DETAIL_MAX_BYTES
    {
        let (_, payload) =
            ops_starter_support::read_json_resource(evidence_uri, "evidence payload resource")?;
        let details = ops_starter_support::summarize_json_payload(&payload, 3);
        lines.extend(
            details
                .into_iter()
                .map(|(label, value)| format!("- {label}: {value}")),
        );
    }

    Ok(lines)
}

fn missing_evidence_lines(subject_evidence_uris: &[String]) -> Vec<String> {
    if subject_evidence_uris.is_empty() {
        return vec!["no explicit evidence ref supplied".into()];
    }

    vec![
        "no explicit evidence ref supplied".into(),
        format!(
            "- subject nearby evidence: {}",
            render_short_refs(
                subject_evidence_uris,
                SUBJECT_REF_LIMIT,
                ops_starter_support::short_evidence_ref_from_uri
            )
        ),
    ]
}

fn subject_ref_lines(subject_child_uris: &[String], subject_evidence_uris: &[String]) -> Vec<String> {
    vec![
        format!(
            "- child refs: {}",
            render_short_refs(
                subject_child_uris,
                SUBJECT_REF_LIMIT,
                ops_starter_support::short_execution_ref_from_uri
            )
        ),
        format!(
            "- nearby evidence refs: {}",
            render_short_refs(
                subject_evidence_uris,
                SUBJECT_REF_LIMIT,
                ops_starter_support::short_evidence_ref_from_uri
            )
        ),
    ]
}

fn exact_ref_lines(
    subject_execution_uri: &str,
    comparison_execution_uri: Option<&str>,
    query_uri: Option<&str>,
    query_expanded_execution_uris: &[String],
    evidence_uri: Option<&str>,
) -> Vec<String> {
    let mut lines = vec![format!("- subject execution: {subject_execution_uri}")];

    if let Some(comparison_execution_uri) = comparison_execution_uri {
        lines.push(format!("- comparison execution: {comparison_execution_uri}"));
    }
    if let Some(query_uri) = query_uri {
        lines.push(format!("- bounded query: {query_uri}"));
        lines.extend(
            query_expanded_execution_uris
                .iter()
                .map(|execution_uri| format!("- query-expanded execution: {execution_uri}")),
        );
    }
    if let Some(evidence_uri) = evidence_uri {
        lines.push(format!("- evidence: {evidence_uri}"));
        lines.push(format!(
            "- evidence metadata: {}",
            ops_starter_support::evidence_metadata_uri(evidence_uri)
        ));
    }

    lines
}

fn next_ref_lines(
    subject_execution_uri: &str,
    comparison_execution_uri: Option<&str>,
    query_uri: Option<&str>,
    evidence_uri: Option<&str>,
) -> Vec<String> {
    let mut lines = vec![
        format!(
            "- guild why {}",
            ops_starter_support::short_execution_ref_from_uri(subject_execution_uri)
        ),
        format!(
            "- guild why --lineage {}",
            ops_starter_support::short_execution_ref_from_uri(subject_execution_uri)
        ),
    ];

    if let Some(comparison_execution_uri) = comparison_execution_uri {
        lines.push(format!(
            "- guild why {}",
            ops_starter_support::short_execution_ref_from_uri(comparison_execution_uri)
        ));
    }
    if let Some(query_uri) = query_uri {
        lines.push(format!("- guild get {query_uri}"));
    }
    if let Some(evidence_uri) = evidence_uri {
        lines.push(format!(
            "- guild show {}",
            ops_starter_support::short_evidence_ref_from_uri(evidence_uri)
        ));
    }

    lines
}

fn render_short_refs<F>(uris: &[String], limit: usize, formatter: F) -> String
where
    F: Fn(&str) -> String,
{
    if uris.is_empty() {
        return "none".into();
    }

    uris.iter()
        .take(limit)
        .map(|uri| formatter(uri))
        .collect::<Vec<_>>()
        .join(", ")
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

export!(IncidentCasefile with_types_in self);
