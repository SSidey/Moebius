use std::path::Path;

use anyhow::Result;
use serde_json::json;

use crate::adapters::ToolDef;
use crate::run_state::{RubricStatus, SharedRunState, TaskStatus};
use super::ToolHandler;

pub struct GetRunStatusTool {
    pub state: SharedRunState,
}

impl ToolHandler for GetRunStatusTool {
    fn name(&self) -> &'static str {
        "get_run_status"
    }

    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "get_run_status",
            description: "Return current session state: task list, task statuses, rubric verifications, tools used, and token usage.",
            parameters: json!({
                "type": "object",
                "properties": {}
            }),
        }
    }

    fn execute(&self, _args: &serde_json::Value, _working_dir: &Path) -> Result<String> {
        let state = self.state.lock().unwrap();

        let task_list_created = state.task_list_created();
        let total = state.tasks.len();
        let pending = state.tasks.iter().filter(|t| t.status == TaskStatus::Pending).count();
        let done = state.tasks.iter().filter(|t| t.status == TaskStatus::Done).count();
        let skipped = state.tasks.iter().filter(|t| t.status == TaskStatus::Skipped).count();

        let mut output = String::new();

        if !task_list_created {
            output.push_str("task list: not created\n");
        } else {
            output.push_str(&format!(
                "task list: created ({} total, {} pending, {} done, {} skipped)\n",
                total, pending, done, skipped
            ));
            for task in &state.tasks {
                let status_str = match task.status {
                    TaskStatus::Pending => "pending",
                    TaskStatus::Done => "done",
                    TaskStatus::Skipped => "skipped",
                };
                output.push_str(&format!("  [{}] {}: {}\n", status_str, task.id, task.description));
            }
        }

        if state.rubric_verifications.is_empty() {
            output.push_str("rubric verifications: none recorded\n");
        } else {
            output.push_str("rubric verifications:\n");
            for rv in &state.rubric_verifications {
                let status_str = match rv.status {
                    RubricStatus::Pass => "pass",
                    RubricStatus::Fail => "fail",
                    RubricStatus::Na => "na",
                };
                output.push_str(&format!("  [{}] {}", status_str, rv.name));
                if let Some(note) = &rv.note {
                    output.push_str(&format!(": {}", note));
                }
                output.push('\n');
            }
        }

        let tools = state.sorted_tools_used();
        if tools.is_empty() {
            output.push_str("tools used: none\n");
        } else {
            output.push_str(&format!("tools used: {}\n", tools.join(", ")));
        }

        // Token usage block
        let run = &state.run_total_usage;
        let phase = &state.current_phase_usage;
        let tool = &state.current_tool_usage;
        output.push_str(&format!(
            "token usage: input={} output={} cache_read={} cache_creation={} total={}\n",
            run.input_tokens, run.output_tokens, run.cache_read_tokens, run.cache_creation_tokens, run.total_tokens()
        ));
        output.push_str(&format!(
            "  phase: input={} output={} cache_read={} cache_creation={} total={}\n",
            phase.input_tokens, phase.output_tokens, phase.cache_read_tokens, phase.cache_creation_tokens, phase.total_tokens()
        ));
        output.push_str(&format!(
            "  tool:  input={} output={} cache_read={} cache_creation={} total={}\n",
            tool.input_tokens, tool.output_tokens, tool.cache_read_tokens, tool.cache_creation_tokens, tool.total_tokens()
        ));

        // Budget breaches block (omit entirely if empty)
        if !state.budget_breaches.is_empty() {
            output.push_str("budget breaches:\n");
            for b in &state.budget_breaches {
                match b.level.as_str() {
                    "tool" => output.push_str(&format!(
                        "  - level=tool phase={} turn={} total={} threshold={}\n",
                        b.phase_id.as_deref().unwrap_or("none"), b.turn, b.total_tokens, b.threshold
                    )),
                    "phase" => output.push_str(&format!(
                        "  - level=phase phase={} total={} threshold={}\n",
                        b.phase_id.as_deref().unwrap_or("none"), b.total_tokens, b.threshold
                    )),
                    _ => output.push_str(&format!(
                        "  - level=run total={} threshold={}\n",
                        b.total_tokens, b.threshold
                    )),
                }
            }
        }

        Ok(output)
    }
}

#[cfg(test)]
#[path = "get_run_status_tests.rs"]
mod tests;
