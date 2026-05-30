use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
}

impl TokenUsage {
    pub fn total_tokens(&self) -> u64 {
        self.input_tokens + self.output_tokens + self.cache_read_tokens + self.cache_creation_tokens
    }

    pub fn accumulate(&mut self, other: &TokenUsage) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        self.cache_read_tokens += other.cache_read_tokens;
        self.cache_creation_tokens += other.cache_creation_tokens;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BudgetBreachLevel {
    Tool,
    Phase,
    Run,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BudgetBreach {
    pub level: String,
    pub phase_id: Option<String>,
    pub attempt: u32,
    pub turn: u32,
    pub total_tokens: u64,
    pub threshold: u64,
}

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
    pub thinking_blocks: Vec<String>,
    pub current_tool_usage: TokenUsage,
    pub current_phase_usage: TokenUsage,
    pub run_total_usage: TokenUsage,
    pub over_budget_phase: Option<String>,
    pub over_budget_run: bool,
    pub budget_breaches: Vec<BudgetBreach>,
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

    pub fn push_thinking_blocks(&mut self, texts: Vec<String>) {
        self.thinking_blocks.extend(texts);
    }

    pub fn record_token_usage(
        &mut self,
        usage: &TokenUsage,
        tool_budget: u64,
        phase_budget: u64,
        run_budget: u64,
    ) -> Option<BudgetBreachLevel> {
        self.current_tool_usage.accumulate(usage);
        self.current_phase_usage.accumulate(usage);
        self.run_total_usage.accumulate(usage);

        if tool_budget > 0 && self.current_tool_usage.total_tokens() > tool_budget {
            return Some(BudgetBreachLevel::Tool);
        }
        if phase_budget > 0 && self.current_phase_usage.total_tokens() > phase_budget {
            self.over_budget_phase = self.current_phase.clone();
            return Some(BudgetBreachLevel::Phase);
        }
        if run_budget > 0 && self.run_total_usage.total_tokens() > run_budget {
            self.over_budget_run = true;
            return Some(BudgetBreachLevel::Run);
        }
        None
    }

    pub fn reset_tool_usage(&mut self) {
        self.current_tool_usage = TokenUsage::default();
    }

    pub fn reset_phase_usage(&mut self) {
        self.current_phase_usage = TokenUsage::default();
        self.over_budget_phase = None;
    }

    pub fn record_budget_breach(
        &mut self,
        level: BudgetBreachLevel,
        phase_id: Option<String>,
        attempt: u32,
        turn: u32,
        total_tokens: u64,
        threshold: u64,
    ) {
        let level_str = match level {
            BudgetBreachLevel::Tool => "tool",
            BudgetBreachLevel::Phase => "phase",
            BudgetBreachLevel::Run => "run",
        };
        self.budget_breaches.push(BudgetBreach {
            level: level_str.to_string(),
            phase_id,
            attempt,
            turn,
            total_tokens,
            threshold,
        });
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

#[cfg(test)]
#[path = "run_state_tests.rs"]
mod run_state_tests;
