use guild_mcp::protocol::CallToolResult;
use guild_types::ExecutionRecord;
use serde_json::{Value, json};

pub fn example_inspect_request(skill_name: &str, input: &Value, grants: &[Value]) -> Value {
    json!({
        "name": "guild.inspect",
        "arguments": {
            "skill": {
                "key": {
                    "namespace": "example",
                    "name": skill_name,
                },
                "version_req": "^0.1",
            },
            "input": input,
            "grants": {
                "grants": grants,
            }
        }
    })
}

pub fn parse_execution_record(result: &CallToolResult) -> ExecutionRecord {
    serde_json::from_value(
        result
            .structured_content
            .clone()
            .expect("inspect returns structured content"),
    )
    .expect("structured content is an execution record")
}
