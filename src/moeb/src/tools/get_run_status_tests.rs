use tempfile::TempDir;

use crate::run_state::{new_shared_run_state, Task, TaskStatus};
use crate::tools::ToolHandler;
use super::GetRunStatusTool;

#[test]
fn get_run_status_reports_no_task_list_before_create() {
    let state = new_shared_run_state();
    let tool = GetRunStatusTool { state };

    let dir = TempDir::new().unwrap();
    let args = serde_json::json!({});
    let result = tool.execute(&args, dir.path()).unwrap();

    assert!(
        result.contains("task list: not created"),
        "expected 'task list: not created', got: {}",
        result
    );
}

#[test]
fn get_run_status_reports_tasks_after_create() {
    let state = new_shared_run_state();
    {
        let mut s = state.lock().unwrap();
        s.tasks.push(Task {
            id: "1".to_string(),
            description: "implement the feature".to_string(),
            status: TaskStatus::Pending,
        });
    }

    let tool = GetRunStatusTool { state };
    let dir = TempDir::new().unwrap();
    let args = serde_json::json!({});
    let result = tool.execute(&args, dir.path()).unwrap();

    assert!(
        result.contains("implement the feature"),
        "expected task description in output, got: {}",
        result
    );
}
