use serde_json::Value;
use wit_bindgen::generate;

#[path = "../../../ops-starter-support.rs"]
mod ops_starter_support;

const _: &str = include_str!("../../../../../wit/guild-skill-v1.wit");

generate!({
    path: "../../../../wit",
    world: "guild-skill-inspect-v1",
});

use crate::exports::guild::skill::inspect_skill::{
    ExecutionContext, Guest, Json, SkillOutput,
};

struct RenderReport;

impl Guest for RenderReport {
    fn run(_ctx: ExecutionContext, input: Json) -> Result<SkillOutput, crate::exports::guild::skill::inspect_skill::SkillError> {
        let parsed_input: Value = ops_starter_support::parse_input(&input)?;
        let markdown = ops_starter_support::render_markdown_report(&parsed_input)?;

        Ok(SkillOutput {
            summary: "Rendered starter-pack report.".into(),
            structured: Value::String(markdown).to_string(),
            diagnostics: Vec::new(),
            effects: Vec::new(),
            evidence: Vec::new(),
        })
    }
}

export!(RenderReport with_types_in self);
