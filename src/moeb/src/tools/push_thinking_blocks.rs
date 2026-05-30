use std::path::Path;
use anyhow::Result;
use serde_json::json;

use crate::adapters::ToolDef;
use crate::run_state::SharedRunState;
use super::ToolHandler;

pub struct PushThinkingBlocksTool {
    pub state: SharedRunState,
}

impl ToolHandler for PushThinkingBlocksTool {
    fn name(&self) -> &'static str {
        "push_thinking_blocks"
    }

    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "push_thinking_blocks",
            description: "Push reasoning text into RunState.thinking_blocks so the Reasoning \
                Reviewer can evaluate it in the Reasoning Review phase. Call this in the Push \
                Thinking Blocks phase with key decision rationale and deliberation from the \
                current run.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "texts": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Array of reasoning block strings to append to RunState.thinking_blocks"
                    }
                },
                "required": ["texts"]
            }),
        }
    }

    fn execute(&self, args: &serde_json::Value, _working_dir: &Path) -> Result<String> {
        let texts: Vec<String> = args["texts"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect();
        let count = texts.len();
        self.state.lock().unwrap().push_thinking_blocks(texts);
        Ok(format!("pushed {} thinking block(s) into RunState", count))
    }
}

#[cfg(test)]
#[path = "push_thinking_blocks_tests.rs"]
mod tests;
