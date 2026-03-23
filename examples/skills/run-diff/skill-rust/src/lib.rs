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
use crate::guild::skill::inspect_host as host;
use crate::guild::skill::inspect_types::DependencyInvocationRequest;

const RENDER_ALIAS: &str = "renderer";

struct RunDiff;

impl Guest for RunDiff {
    fn run(_ctx: ExecutionContext, input: Json) -> Result<SkillOutput, SkillError> {
        let parsed_input = ops_starter_support::parse_input(&input)?;
        let left_execution_uri =
            ops_starter_support::required_string(&parsed_input, "left_execution_uri")?;
        let right_execution_uri =
            ops_starter_support::required_string(&parsed_input, "right_execution_uri")?;

        let (_, left_record) =
            ops_starter_support::read_json_resource(left_execution_uri, "left execution resource")?;
        let (_, right_record) = ops_starter_support::read_json_resource(
            right_execution_uri,
            "right execution resource",
        )?;

        let left_status = ops_starter_support::status_label(&left_record);
        let right_status = ops_starter_support::status_label(&right_record);
        let left_reason = ops_starter_support::primary_reason_code(&left_record);
        let right_reason = ops_starter_support::primary_reason_code(&right_record);
        let left_childs = ops_starter_support::child_execution_uris(&left_record);
        let right_childs = ops_starter_support::child_execution_uris(&right_record);
        let left_evidence = ops_starter_support::evidence_uris(&left_record);
        let right_evidence = ops_starter_support::evidence_uris(&right_record);
        let changed = left_status != right_status
            || left_reason != right_reason
            || left_childs != right_childs
            || left_evidence != right_evidence
            || ops_starter_support::execution_posture(&left_record)
                != ops_starter_support::execution_posture(&right_record);

        let report = json!({
            "title": "Run Diff",
            "summary_line": format!(
                "{}  {} vs {}",
                if changed { "changed" } else { "unchanged" },
                ops_starter_support::short_execution_ref_from_uri(left_execution_uri),
                ops_starter_support::short_execution_ref_from_uri(right_execution_uri)
            ),
            "facts": [
                { "label": "Left", "value": ops_starter_support::short_execution_ref_from_uri(left_execution_uri) },
                { "label": "Right", "value": ops_starter_support::short_execution_ref_from_uri(right_execution_uri) },
                { "label": "Left status", "value": left_status },
                { "label": "Right status", "value": right_status },
                { "label": "Left posture", "value": ops_starter_support::execution_posture(&left_record) },
                { "label": "Right posture", "value": ops_starter_support::execution_posture(&right_record) }
            ],
            "sections": [
                {
                    "title": "Primary reason diff",
                    "lines": diff_lines("left", &left_reason, "right", &right_reason)
                },
                {
                    "title": "Support diff",
                    "lines": diff_lines(
                        "left",
                        &ops_starter_support::rendered_support(&left_record),
                        "right",
                        &ops_starter_support::rendered_support(&right_record)
                    )
                },
                {
                    "title": "Child execution diff",
                    "lines": collection_diff_lines(&left_childs, &right_childs, ops_starter_support::short_execution_ref_from_uri)
                },
                {
                    "title": "Evidence diff",
                    "lines": collection_diff_lines(&left_evidence, &right_evidence, ops_starter_support::short_evidence_ref_from_uri)
                },
                {
                    "title": "Next refs",
                    "lines": vec![
                        format!("- guild why {}", ops_starter_support::short_execution_ref_from_uri(left_execution_uri)),
                        format!("- guild why {}", ops_starter_support::short_execution_ref_from_uri(right_execution_uri))
                    ]
                }
            ]
        });

        let markdown = render_report(report)?;
        Ok(SkillOutput {
            summary: format!(
                "Prepared bounded run diff for {left_execution_uri} and {right_execution_uri}."
            ),
            structured: Value::String(markdown).to_string(),
            diagnostics: Vec::new(),
            effects: Vec::new(),
            evidence: Vec::new(),
        })
    }
}

fn render_report(report: Value) -> Result<String, SkillError> {
    let child_output = host::invoke_dependency(&DependencyInvocationRequest {
        alias: RENDER_ALIAS.into(),
        input: report.to_string(),
    })?;
    let rendered: Value = serde_json::from_str(&child_output.structured).map_err(|error| {
        SkillError {
            code: "render-report-invalid-output".into(),
            message: "render-report did not return valid JSON".into(),
            retryable: false,
            detail: Some(json!({ "error": error.to_string() }).to_string()),
        }
    })?;
    rendered
        .as_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| SkillError {
            code: "render-report-invalid-output".into(),
            message: "render-report must return a markdown string payload".into(),
            retryable: false,
            detail: Some(rendered.to_string()),
        })
}

fn diff_lines(left_label: &str, left: &str, right_label: &str, right: &str) -> Vec<String> {
    vec![
        format!("- {left_label}: {left}"),
        format!("- {right_label}: {right}"),
        format!("- changed: {}", if left == right { "unchanged" } else { "changed" }),
    ]
}

fn collection_diff_lines<F>(left: &[String], right: &[String], formatter: F) -> Vec<String>
where
    F: Fn(&str) -> String,
{
    let left_rendered = if left.is_empty() {
        "none".into()
    } else {
        left.iter().map(|uri| formatter(uri)).collect::<Vec<_>>().join(", ")
    };
    let right_rendered = if right.is_empty() {
        "none".into()
    } else {
        right.iter().map(|uri| formatter(uri)).collect::<Vec<_>>().join(", ")
    };

    vec![
        format!("- left: {left_rendered}"),
        format!("- right: {right_rendered}"),
        format!("- changed: {}", if left == right { "unchanged" } else { "changed" }),
    ]
}

export!(RunDiff with_types_in self);
