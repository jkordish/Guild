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
const CHILD_REF_LIMIT: usize = 3;
const EVIDENCE_REF_LIMIT: usize = 3;

struct IncidentBrief;

impl Guest for IncidentBrief {
    fn run(_ctx: ExecutionContext, input: Json) -> Result<SkillOutput, SkillError> {
        let parsed_input = ops_starter_support::parse_input(&input)?;
        let execution_uri = ops_starter_support::required_string(&parsed_input, "execution_uri")?;
        let (_resource, record) =
            ops_starter_support::read_json_resource(execution_uri, "execution resource")?;

        let child_uris = ops_starter_support::child_execution_uris(&record);
        let evidence_uris = ops_starter_support::evidence_uris(&record);
        let execution_ref = ops_starter_support::short_execution_ref_from_uri(execution_uri);
        let status = ops_starter_support::status_label(&record);
        let posture = ops_starter_support::execution_posture(&record);
        let skill = ops_starter_support::resolved_skill_label(&record);
        let support = ops_starter_support::rendered_support(&record);
        let policy_outcome = record
            .pointer("/policy_decision/outcome")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let primary_code = ops_starter_support::primary_reason_code(&record);
        let primary_message = ops_starter_support::primary_reason_message(&record);

        let report = json!({
            "title": "Incident Brief",
            "summary_line": format!("{status}  {posture}  {execution_ref}  {skill}"),
            "facts": [
                { "label": "Execution", "value": execution_ref },
                { "label": "Skill", "value": skill },
                { "label": "Status", "value": status },
                { "label": "Posture", "value": posture },
                { "label": "Support", "value": support },
                { "label": "Policy outcome", "value": policy_outcome },
                { "label": "Child executions", "value": child_uris.len().to_string() },
                { "label": "Evidence records", "value": evidence_uris.len().to_string() }
            ],
            "sections": [
                {
                    "title": "Primary reason",
                    "lines": [primary_code, primary_message]
                },
                {
                    "title": "Nearby child refs",
                    "lines": section_lines(
                        &child_uris,
                        CHILD_REF_LIMIT,
                        ops_starter_support::short_execution_ref_from_uri,
                        "no child execution refs recorded"
                    )
                },
                {
                    "title": "Nearby evidence refs",
                    "lines": section_lines(
                        &evidence_uris,
                        EVIDENCE_REF_LIMIT,
                        ops_starter_support::short_evidence_ref_from_uri,
                        "no evidence refs recorded"
                    )
                },
                {
                    "title": "Next refs",
                    "lines": next_ref_lines(execution_uri, &child_uris, &evidence_uris)
                }
            ]
        });

        let markdown = render_report(report)?;
        Ok(SkillOutput {
            summary: format!("Prepared incident brief for stored execution {execution_uri}."),
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

fn section_lines<F>(uris: &[String], limit: usize, formatter: F, empty: &str) -> Vec<String>
where
    F: Fn(&str) -> String,
{
    if uris.is_empty() {
        return vec![empty.into()];
    }

    uris.iter()
        .take(limit)
        .map(|uri| format!("- {}", formatter(uri)))
        .collect()
}

fn next_ref_lines(execution_uri: &str, child_uris: &[String], evidence_uris: &[String]) -> Vec<String> {
    let mut lines = vec![format!(
        "- guild why {}",
        ops_starter_support::short_execution_ref_from_uri(execution_uri)
    )];

    if let Some(uri) = child_uris.first() {
        lines.push(format!(
            "- guild why {}",
            ops_starter_support::short_execution_ref_from_uri(uri)
        ));
    }
    if let Some(uri) = evidence_uris.first() {
        lines.push(format!(
            "- guild show {}",
            ops_starter_support::short_evidence_ref_from_uri(uri)
        ));
    }

    lines
}

export!(IncidentBrief with_types_in self);
