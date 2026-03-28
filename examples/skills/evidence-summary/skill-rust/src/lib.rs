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

const PAYLOAD_DETAIL_MAX_BYTES: u64 = 32 * 1024;

struct EvidenceSummary;

impl Guest for EvidenceSummary {
    fn run(_ctx: ExecutionContext, input: Json) -> Result<SkillOutput, SkillError> {
        let parsed_input = ops_starter_support::parse_input(&input)?;
        let evidence_uri = ops_starter_support::required_string(&parsed_input, "evidence_uri")?;
        let metadata_uri = ops_starter_support::evidence_metadata_uri(evidence_uri);
        let (_metadata_resource, metadata) =
            ops_starter_support::read_json_resource(&metadata_uri, "evidence metadata resource")?;

        let payload_lines = if metadata.get("mime_type").and_then(Value::as_str)
            == Some("application/json")
            && metadata
                .get("size_bytes")
                .and_then(Value::as_u64)
                .unwrap_or(u64::MAX)
                <= PAYLOAD_DETAIL_MAX_BYTES
        {
            let (_payload_resource, payload) =
                ops_starter_support::read_json_resource(evidence_uri, "evidence payload resource")?;
            let details = ops_starter_support::summarize_json_payload(&payload, 5);
            if details.is_empty() {
                vec!["payload was readable JSON but did not expose normalized summary fields".into()]
            } else {
                details
                    .into_iter()
                    .map(|(label, value)| format!("- {label}: {value}"))
                    .collect::<Vec<_>>()
            }
        } else {
            vec!["payload normalization skipped because the stored payload is not small JSON".into()]
        };

        let sink_line = metadata
            .get("sink")
            .map(ops_starter_support::render_sink_summary)
            .unwrap_or_else(|| "sink metadata unavailable".into());
        let produced_by_execution = metadata
            .get("produced_by_execution")
            .and_then(Value::as_str)
            .map(ops_starter_support::short_execution_ref_from_uri)
            .unwrap_or_else(|| "none".into());
        let evidence_ref = ops_starter_support::short_evidence_ref_from_uri(evidence_uri);
        let kind = metadata
            .get("title")
            .and_then(Value::as_str)
            .or_else(|| metadata.get("mime_type").and_then(Value::as_str))
            .unwrap_or("unknown");

        let markdown = ops_starter_support::render_markdown_report(&json!({
            "title": "Evidence Summary",
            "summary_line": format!("{kind}  not_proven  {evidence_ref}"),
            "facts": [
                { "label": "Evidence", "value": evidence_ref },
                { "label": "Mime type", "value": metadata.get("mime_type").and_then(Value::as_str).unwrap_or("unknown") },
                { "label": "Audience", "value": metadata.get("audience").and_then(Value::as_str).unwrap_or("unknown") },
                { "label": "Redaction", "value": metadata.get("redaction").and_then(Value::as_str).unwrap_or("unknown") },
                { "label": "Size", "value": metadata.get("size_bytes").and_then(Value::as_u64).unwrap_or(0).to_string() },
                { "label": "Produced by", "value": produced_by_execution }
            ],
            "sections": [
                {
                    "title": "Linkage",
                    "lines": vec![
                        format!("- metadata: {metadata_uri}"),
                        format!("- blob: {}", metadata.get("blob_uri").and_then(Value::as_str).map(ops_starter_support::short_object_ref_from_uri).unwrap_or_else(|| "none".into())),
                        format!("- sink: {sink_line}")
                    ]
                },
                {
                    "title": "Normalized details",
                    "lines": payload_lines
                },
                {
                    "title": "Next refs",
                    "lines": next_ref_lines(evidence_uri, &metadata)
                }
            ]
        }))?;

        Ok(SkillOutput {
            summary: format!("Summarized stored evidence {evidence_uri}."),
            structured: Value::String(markdown).to_string(),
            diagnostics: Vec::new(),
            effects: Vec::new(),
            evidence: Vec::new(),
        })
    }
}

fn next_ref_lines(evidence_uri: &str, metadata: &Value) -> Vec<String> {
    let mut lines = vec![format!(
        "- guild show {}",
        ops_starter_support::short_evidence_ref_from_uri(evidence_uri)
    )];
    if let Some(execution_uri) = metadata.get("produced_by_execution").and_then(Value::as_str) {
        lines.push(format!(
            "- guild why {}",
            ops_starter_support::short_execution_ref_from_uri(execution_uri)
        ));
    }
    lines
}

export!(EvidenceSummary with_types_in self);
