use std::collections::BTreeMap;

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

const TOP_EXECUTION_LIMIT: usize = 5;
const TOP_GROUP_LIMIT: usize = 5;

struct RecentFailures;

impl Guest for RecentFailures {
    fn run(_ctx: ExecutionContext, input: Json) -> Result<SkillOutput, SkillError> {
        let parsed_input = ops_starter_support::parse_input(&input)?;
        let query_uri = ops_starter_support::required_string(&parsed_input, "query_uri")?;
        let (_resource, query_result) =
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

        let mut grouped_reasons = BTreeMap::<String, usize>::new();
        let mut grouped_postures = BTreeMap::<String, usize>::new();
        let mut execution_lines = Vec::new();
        let mut next_ref_lines = Vec::new();

        for (index, result) in results.iter().enumerate() {
            let execution_uri = result
                .pointer("/receipt/uri")
                .and_then(Value::as_str)
                .ok_or_else(|| SkillError {
                    code: "invalid-query-resource".into(),
                    message: "execution query match did not include a receipt URI".into(),
                    retryable: false,
                    detail: Some(result.to_string()),
                })?;
            let (_record_resource, record) =
                ops_starter_support::read_json_resource(execution_uri, "execution resource")?;
            let reason = ops_starter_support::primary_reason_code(&record);
            let posture = ops_starter_support::execution_posture(&record).to_owned();

            *grouped_reasons.entry(reason.clone()).or_default() += 1;
            *grouped_postures.entry(posture.clone()).or_default() += 1;

            if execution_lines.len() < TOP_EXECUTION_LIMIT {
                execution_lines.push(format!(
                    "- {}  {}  {}  {}",
                    ops_starter_support::short_execution_ref_from_uri(execution_uri),
                    ops_starter_support::status_label(&record),
                    posture,
                    reason
                ));
            }

            if index < TOP_EXECUTION_LIMIT {
                next_ref_lines.push(format!(
                    "- guild why {}",
                    ops_starter_support::short_execution_ref_from_uri(execution_uri)
                ));
            }
        }

        let markdown = ops_starter_support::render_markdown_report(&json!({
            "title": "Recent Failures",
            "summary_line": format!(
                "{} stored failures/refusals from {}",
                query_result
                    .get("returned_matches")
                    .and_then(Value::as_u64)
                    .unwrap_or(0),
                query_uri
            ),
            "facts": [
                {
                    "label": "Total matches",
                    "value": query_result
                        .get("total_matches")
                        .and_then(Value::as_u64)
                        .unwrap_or(0)
                        .to_string()
                },
                {
                    "label": "Returned matches",
                    "value": query_result
                        .get("returned_matches")
                        .and_then(Value::as_u64)
                        .unwrap_or(0)
                        .to_string()
                },
                {
                    "label": "Truncated",
                    "value": query_result
                        .get("truncated")
                        .and_then(Value::as_bool)
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "false".into())
                }
            ],
            "sections": [
                {
                    "title": "Grouped reasons",
                    "lines": grouped_lines(&grouped_reasons)
                },
                {
                    "title": "Posture groups",
                    "lines": grouped_lines(&grouped_postures)
                },
                {
                    "title": "Top executions",
                    "lines": if execution_lines.is_empty() {
                        vec!["no executions matched the bounded query".into()]
                    } else {
                        execution_lines
                    }
                },
                {
                    "title": "Next refs",
                    "lines": if next_ref_lines.is_empty() {
                        vec!["no follow-up refs available".into()]
                    } else {
                        next_ref_lines
                    }
                }
            ]
        }))?;

        Ok(SkillOutput {
            summary: format!("Summarized recent failures from {query_uri}."),
            structured: Value::String(markdown).to_string(),
            diagnostics: Vec::new(),
            effects: Vec::new(),
            evidence: Vec::new(),
        })
    }
}

fn grouped_lines(groups: &BTreeMap<String, usize>) -> Vec<String> {
    if groups.is_empty() {
        return vec!["none".into()];
    }

    let mut ordered = groups
        .iter()
        .map(|(key, count)| (key.clone(), *count))
        .collect::<Vec<_>>();
    ordered.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    ordered
        .into_iter()
        .take(TOP_GROUP_LIMIT)
        .map(|(key, count)| format!("- {key}: {count}"))
        .collect()
}

export!(RecentFailures with_types_in self);
