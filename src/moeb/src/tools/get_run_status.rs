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
            description: "Return current session state: task list, task statuses, and rubric verifications.",
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

        Ok(output)
    }
}

#[cfg(test)]
#[path = "get_run_status_tests.rs"]
mod tests;
