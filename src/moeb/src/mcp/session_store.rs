use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use uuid::Uuid;

use crate::mcp::session::McpSession;
use crate::run_state::SharedRunState;
use crate::tools::RealToolExecutor;

pub struct SessionEntry {
    pub session: McpSession,
    pub executor: RealToolExecutor,
}

pub struct SessionStore(Arc<Mutex<HashMap<String, SessionEntry>>>);

impl SessionStore {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(HashMap::new())))
    }

    pub fn get_or_create(&self, session_id: Option<&str>, working_dir: &Path) -> (String, bool) {
        let mut map = self.0.lock().unwrap();
        if let Some(id) = session_id {
            if map.contains_key(id) {
                return (id.to_string(), false);
            }
        }
        let new_id = Uuid::new_v4().to_string();
        let session = McpSession::new(working_dir.to_path_buf());
        let state: SharedRunState = Arc::clone(&session.state);
        let executor = RealToolExecutor::new_mcp(state);
        map.insert(new_id.clone(), SessionEntry { session, executor });
        (new_id, true)
    }

    pub fn with_session<F, R>(&self, id: &str, f: F) -> Option<R>
    where
        F: FnOnce(&mut SessionEntry) -> R,
    {
        let mut map = self.0.lock().unwrap();
        map.get_mut(id).map(f)
    }
}

#[cfg(test)]
#[path = "session_store_tests.rs"]
mod tests;
