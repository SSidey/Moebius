use anyhow::{Context, Result};
use serde_json::Value;

use super::{AgentResponse, ToolCall};

pub(super) fn parse_response(value: &Value) -> Result<(AgentResponse, Vec<String>)> {
    let stop_reason = value["stop_reason"].as_str().unwrap_or("");
    let content = value["content"]
        .as_array()
        .context("Missing content array in Anthropic response")?;

    let thinking_texts: Vec<String> = content
        .iter()
        .filter(|block| block["type"].as_str() == Some("thinking"))
        .filter_map(|block| block["thinking"].as_str())
        .map(|s| s.to_string())
        .collect();

    if stop_reason == "tool_use" {
        let calls: Result<Vec<ToolCall>> = content
            .iter()
            .filter(|block| block["type"].as_str() == Some("tool_use"))
            .map(parse_tool_call)
            .collect();
        return Ok((AgentResponse::ToolCalls(calls?), thinking_texts));
    }

    let text = content
        .iter()
        .find(|block| block["type"].as_str() == Some("text"))
        .and_then(|block| block["text"].as_str())
        .unwrap_or("")
        .to_string();

    Ok((AgentResponse::Text(text), thinking_texts))
}

fn parse_tool_call(block: &Value) -> Result<ToolCall> {
    let id = block["id"]
        .as_str()
        .context("Tool use block missing id")?
        .to_string();
    let name = block["name"]
        .as_str()
        .context("Tool use block missing name")?
        .to_string();
    let arguments = serde_json::to_string(&block["input"])
        .context("Failed to serialise tool call input")?;
    Ok(ToolCall { id, name, arguments })
}
