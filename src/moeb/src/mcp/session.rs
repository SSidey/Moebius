use std::path::PathBuf;

use crate::run_state::{new_shared_run_state, SharedRunState};

pub struct McpSession {
    pub state: SharedRunState,
    pub working_dir: PathBuf,
    pub turn_count: u32,
}

impl McpSession {
    pub fn new(working_dir: PathBuf) -> Self {
        Self {
            state: new_shared_run_state(),
            working_dir,
            turn_count: 0,
        }
    }

    pub fn increment_turn(&mut self) -> u32 {
        self.turn_count += 1;
        self.turn_count
    }

    pub fn current_phase(&self) -> Option<String> {
        self.state.lock().unwrap().current_phase.clone()
    }

    pub fn set_phase_tool_map(&mut self, map: std::collections::HashMap<String, Vec<String>>) {
        self.state.lock().unwrap().set_phase_tool_map(map);
    }

    pub fn allowed_tools_for_phase(&self, phase_id: &str) -> Option<Vec<String>> {
        self.state.lock().unwrap().allowed_tools_for_phase(phase_id).cloned()
    }
}
