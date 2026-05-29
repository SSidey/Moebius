use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex};
use anyhow::Result;
use sha2::Digest;

use crate::ports::ToolExecutorPort;
use crate::run_state::SharedRunState;
use super::ToolRegistry;

/// Per-run deduplication cache: path → (sha256_hex, turn first sent).
pub(super) type ContentCache = Mutex<HashMap<String, (String, u32)>>;

pub struct RealToolExecutor {
    pub state: SharedRunState,
    pub registry: ToolRegistry,
    cache: ContentCache,
    read_paths: Arc<Mutex<HashSet<String>>>,
}

impl RealToolExecutor {
    fn from_registry(state: SharedRunState, registry: ToolRegistry, read_paths: Arc<Mutex<HashSet<String>>>) -> Self {
        Self { registry, cache: Mutex::new(HashMap::new()), read_paths, state }
    }

    pub fn new(state: SharedRunState) -> Self {
        let rp = Arc::new(Mutex::new(HashSet::new()));
        let reg = ToolRegistry::standard(Arc::clone(&state), Arc::clone(&rp));
        Self::from_registry(state, reg, rp)
    }

    pub fn new_sub_agent(state: SharedRunState) -> Self {
        let rp = Arc::new(Mutex::new(HashSet::new()));
        let reg = ToolRegistry::sub_agent(Arc::clone(&state));
        Self::from_registry(state, reg, rp)
    }

    pub fn new_mcp(state: SharedRunState) -> Self {
        let rp = Arc::new(Mutex::new(HashSet::new()));
        let reg = ToolRegistry::mcp(Arc::clone(&state), Arc::clone(&rp));
        Self::from_registry(state, reg, rp)
    }

    pub fn new_mcp_with_adapter(
        state: SharedRunState,
        adapter: std::sync::Arc<dyn crate::ports::AiPort>,
    ) -> Self {
        let rp = Arc::new(Mutex::new(HashSet::new()));
        let reg = ToolRegistry::mcp_with_adapter(Arc::clone(&state), adapter, Arc::clone(&rp));
        Self::from_registry(state, reg, rp)
    }

    pub fn new_coordinator(state: SharedRunState, adapter: std::sync::Arc<dyn crate::ports::AiPort>) -> Self {
        let rp = Arc::new(Mutex::new(HashSet::new()));
        let reg = ToolRegistry::with_query_agent(Arc::clone(&state), adapter, Arc::clone(&rp));
        Self::from_registry(state, reg, rp)
    }
}

impl ToolExecutorPort for RealToolExecutor {
    fn execute(&self, name: &str, _call_id: &str, args: &serde_json::Value, working_dir: &Path, current_turn: u32) -> Result<(String, bool)> {
        if name == "write_file" || name == "patch_file" {
            if !self.state.lock().unwrap().task_list_created() {
                eprintln!(
                    "moeb: warning: write_file called without a prior create_task_list \
                     — consider calling create_task_list first to record your plan."
                );
            }
            if let Some(path) = args["path"].as_str() {
                let normalized = path.replace('\\', "/");
                if working_dir.join(path).exists() {
                    let read_paths = self.read_paths.lock().unwrap();
                    if !read_paths.contains(&normalized) {
                        return Ok((
                            format!(
                                "write_file rejected: '{}' exists on disk but has not been read \
                                 during this run. Call read_file on '{}' to obtain the current \
                                 content, then write a complete replacement that carries forward \
                                 all existing code not targeted by the specification.",
                                path, path
                            ),
                            false,
                        ));
                    }
                }
            }
        }

        let tool_result = self.registry.execute(name, args, working_dir);

        if name == "read_file" {
            if let Ok(ref content) = tool_result {
                {
                    let path_key = args["path"].as_str().unwrap_or("").to_string();
                    self.read_paths.lock().unwrap().insert(path_key.replace('\\', "/"));
                }
                let path_key = args["path"].as_str().unwrap_or("").to_string();
                let digest = hex::encode(sha2::Sha256::digest(content.as_bytes()));

                let mut cache = self.cache.lock().unwrap();
                if let Some((cached_hash, first_turn)) = cache.get(&path_key) {
                    if *cached_hash == digest {
                        let msg = format!(
                            "[CACHE HIT: {} — content already sent at turn {} \
                             (sha256: {}). File is unchanged. \
                             Use the content from that turn.]",
                            path_key, first_turn, cached_hash
                        );
                        return Ok((msg, true));
                    }
                }
                cache.insert(path_key, (digest, current_turn));
            }
        }

        if name == "read_files" {
            if let Some(paths) = args["paths"].as_array() {
                let mut rp = self.read_paths.lock().unwrap();
                for pv in paths {
                    if let Some(p) = pv.as_str() {
                        rp.insert(p.replace('\\', "/"));
                    }
                }
            }
        }

        match tool_result {
            Ok(content) => Ok((content, false)),
            Err(e) => {
                let msg = e.to_string();
                self.state.lock().unwrap().record_tool_error(name, &msg);
                Ok((format!("{}: {}", name, msg), false))
            }
        }
    }
}
