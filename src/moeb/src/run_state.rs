use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    Pending,
    Done,
    Skipped,
}

#[derive(Debug, Clone)]
pub struct Task {
    pub id: String,
    pub description: String,
    pub status: TaskStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RubricStatus {
    Pass,
    Fail,
    Na,
}

#[derive(Debug, Clone)]
pub struct RubricVerification {
    pub name: String,
    pub status: RubricStatus,
    pub note: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ToolError {
    pub tool_name: String,
    pub message: String,
}

#[derive(Debug, Default)]
pub struct RunState {
    pub tasks: Vec<Task>,
    pub rubric_verifications: Vec<RubricVerification>,
    pub pending_reviews: HashSet<String>,
    pub tool_errors: Vec<ToolError>,
    pub run_id: String,
    pub rubric_score: Option<f32>,
    pub end_review_error_count: Option<u32>,
    pub tools_used: HashSet<String>,
    pub current_phase: Option<String>,
    pub phase_tool_map: HashMap<String, Vec<String>>,
}

impl RunState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_run_id(&mut self, run_id: String) {
        self.run_id = run_id;
    }

    pub fn pending_tasks(&self) -> Vec<&Task> {
        self.tasks.iter().filter(|t| t.status == TaskStatus::Pending).collect()
    }

    pub fn task_list_created(&self) -> bool {
        !self.tasks.is_empty()
    }

    pub fn register_write(&mut self, path: &str) {
        self.pending_reviews.insert(path.to_string());
    }

    pub fn acknowledge_review(&mut self, path: &str) -> bool {
        self.pending_reviews.remove(path)
    }

    pub fn record_tool_error(&mut self, tool_name: &str, message: &str) {
        self.tool_errors.push(ToolError {
            tool_name: tool_name.to_string(),
            message: message.to_string(),
        });
    }

    pub fn record_tool_used(&mut self, name: &str) {
        self.tools_used.insert(name.to_string());
    }

    pub fn set_current_phase(&mut self, phase_id: Option<String>) {
        self.current_phase = phase_id;
    }

    pub fn sorted_tools_used(&self) -> Vec<String> {
        let mut v: Vec<String> = self.tools_used.iter().cloned().collect();
        v.sort();
        v
    }

    pub fn set_phase_tool_map(&mut self, map: HashMap<String, Vec<String>>) {
        self.phase_tool_map = map;
    }

    pub fn allowed_tools_for_phase(&self, phase_id: &str) -> Option<&Vec<String>> {
        self.phase_tool_map.get(phase_id)
    }
}

pub type SharedRunState = Arc<Mutex<RunState>>;

pub fn new_shared_run_state() -> SharedRunState {
    Arc::new(Mutex::new(RunState::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_tasks_returns_only_pending() {
        let mut state = RunState::new();
        state.tasks.push(Task {
            id: "1".to_string(),
            description: "done task".to_string(),
            status: TaskStatus::Done,
        });
        state.tasks.push(Task {
            id: "2".to_string(),
            description: "pending task".to_string(),
            status: TaskStatus::Pending,
        });
        let pending = state.pending_tasks();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, "2");
    }

    #[test]
    fn task_list_created_false_when_empty() {
        let state = RunState::new();
        assert!(!state.task_list_created());
    }

    #[test]
    fn task_list_created_true_after_push() {
        let mut state = RunState::new();
        state.tasks.push(Task {
            id: "1".to_string(),
            description: "a task".to_string(),
            status: TaskStatus::Pending,
        });
        assert!(state.task_list_created());
    }
}
