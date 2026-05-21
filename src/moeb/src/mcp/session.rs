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
}
