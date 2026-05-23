pub mod bump_version;
pub mod create_candidate_tag;
pub mod create_branch;
pub mod create_task_list;
pub mod get_run_status;
pub mod get_version;
pub mod git_commit;
pub mod grep_files;
pub mod list_directory;
pub mod patch_file;
pub mod query_agent;
pub mod read_file;
pub mod read_file_range;
pub mod read_files;
pub mod search_files;
pub mod start_run;
pub mod start_spec;
pub mod update_task;
pub mod verify_rubrics;
pub mod write_file;

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex};
use anyhow::Result;
use sha2::Digest;

use crate::adapters::ToolDef;
use crate::ports::ToolExecutorPort;
use crate::run_state::SharedRunState;

pub const MAX_READ_BYTES: usize = 102_400;
pub const MAX_RANGE_LINES: usize = 300;

pub fn truncate_to_byte_limit(content: String, limit: usize) -> String {
    if content.len() <= limit {
        return content;
    }
    let mut boundary = limit;
    while boundary > 0 && !content.is_char_boundary(boundary) {
        boundary -= 1;
    }
    let total = content.len();
    format!(
        "{}\n[... truncated: {} of {} chars shown ...]",
        &content[..boundary],
        boundary,
        total
    )
}

pub trait ToolHandler: Send + Sync {
    fn name(&self) -> &'static str;
    fn definition(&self) -> ToolDef;
    fn execute(&self, args: &serde_json::Value, working_dir: &Path) -> Result<String>;
}

pub struct ToolRegistry {
    handlers: HashMap<&'static str, Box<dyn ToolHandler>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { handlers: HashMap::new() }
    }

    /// Register the standard tools (file tools + task-list tools + VCS tools + query_agent).
    pub fn standard(state: SharedRunState, read_paths: Arc<Mutex<HashSet<String>>>) -> Self {
        let mut r = Self::new();
        r.register(Box::new(read_file::ReadFileTool));
        r.register(Box::new(write_file::WriteFileTool));
        r.register(Box::new(patch_file::PatchFileTool));
        r.register(Box::new(list_directory::ListDirectoryTool));
        r.register(Box::new(search_files::SearchFilesTool));
        r.register(Box::new(grep_files::GrepFilesTool));
        r.register(Box::new(read_files::ReadFilesTool));
        r.register(Box::new(read_file_range::ReadFileRangeTool));
        r.register(Box::new(create_task_list::CreateTaskListTool { state: std::sync::Arc::clone(&state) }));
        r.register(Box::new(update_task::UpdateTaskTool { state: std::sync::Arc::clone(&state) }));
        r.register(Box::new(verify_rubrics::VerifyRubricsTool { state: std::sync::Arc::clone(&state) }));
        r.register(Box::new(create_branch::CreateBranchTool));
        r.register(Box::new(git_commit::GitCommitTool));
        r.register(Box::new(bump_version::BumpVersionTool));
        r.register(Box::new(create_candidate_tag::CreateCandidateTagTool));
        r.register(Box::new(get_version::GetVersionTool));
        r.register(Box::new(query_agent::QueryAgentTool { adapter: None, read_paths }));
        r
    }

    /// Register the sub-agent tools (read tools + task-list, no write/patch/query).
    pub fn sub_agent(state: SharedRunState) -> Self {
        let mut r = Self::new();
        r.register(Box::new(read_file::ReadFileTool));
        r.register(Box::new(read_files::ReadFilesTool));
        r.register(Box::new(read_file_range::ReadFileRangeTool));
        r.register(Box::new(list_directory::ListDirectoryTool));
        r.register(Box::new(search_files::SearchFilesTool));
        r.register(Box::new(grep_files::GrepFilesTool));
        r.register(Box::new(create_task_list::CreateTaskListTool { state: std::sync::Arc::clone(&state) }));
        r.register(Box::new(update_task::UpdateTaskTool { state: std::sync::Arc::clone(&state) }));
        r
    }

    /// Register the MCP tools (standard tools + start_run + start_spec + get_run_status).
    pub fn mcp(state: SharedRunState, read_paths: Arc<Mutex<HashSet<String>>>) -> Self {
        let mut r = Self::standard(std::sync::Arc::clone(&state), Arc::clone(&read_paths));
        r.register(Box::new(start_run::StartRunTool));
        r.register(Box::new(start_spec::StartSpecTool));
        r.register(Box::new(get_run_status::GetRunStatusTool { state: std::sync::Arc::clone(&state) }));
        r
    }

    /// Register the MCP tools (standard tools + start_run + start_spec + get_run_status)
    /// with an eagerly wired query_agent adapter.
    pub fn mcp_with_adapter(
        state: SharedRunState,
        adapter: std::sync::Arc<dyn crate::ports::AiPort>,
        read_paths: Arc<Mutex<HashSet<String>>>,
    ) -> Self {
        let mut r = Self::with_query_agent(
            std::sync::Arc::clone(&state),
            std::sync::Arc::clone(&adapter),
            Arc::clone(&read_paths),
        );
        r.register(Box::new(start_run::StartRunTool));
        r.register(Box::new(start_spec::StartSpecTool));
        r.register(Box::new(get_run_status::GetRunStatusTool {
            state: std::sync::Arc::clone(&state),
        }));
        r
    }

    /// Register the coordinator tools (standard tools + query_agent with adapter).
    pub fn with_query_agent(state: SharedRunState, adapter: std::sync::Arc<dyn crate::ports::AiPort>, read_paths: Arc<Mutex<HashSet<String>>>) -> Self {
        let mut r = Self::standard(std::sync::Arc::clone(&state), Arc::clone(&read_paths));
        r.register(Box::new(query_agent::QueryAgentTool { adapter: Some(adapter), read_paths }));
        r
    }

    pub fn register(&mut self, handler: Box<dyn ToolHandler>) {
        self.handlers.insert(handler.name(), handler);
    }

    pub fn execute(&self, name: &str, args: &serde_json::Value, working_dir: &Path) -> Result<String> {
        match self.handlers.get(name) {
            Some(h) => h.execute(args, working_dir),
            None => anyhow::bail!(
                "Unknown tool '{}'. Available: {}",
                name,
                self.handlers.keys().cloned().collect::<Vec<_>>().join(", ")
            ),
        }
    }

    pub fn definitions(&self) -> Vec<ToolDef> {
        let order = [
            "read_file", "write_file", "patch_file", "list_directory",
            "search_files", "grep_files", "read_files", "read_file_range",
            "create_task_list", "update_task", "verify_rubrics",
            "create_branch", "git_commit",
            "bump_version", "create_candidate_tag",
            "get_version", "query_agent", "start_run", "start_spec", "get_run_status",
        ];
        order.iter()
            .filter_map(|name| self.handlers.get(name).map(|h| h.definition()))
            .collect()
    }
}

// ── RealToolExecutor ──────────────────────────────────────────────────────────

/// Per-run deduplication cache: path → (sha256_hex, turn first sent).
type ContentCache = Mutex<HashMap<String, (String, u32)>>;

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

        Ok((tool_result?, false))
    }
}

#[cfg(test)]
#[path = "tool_executor_tests.rs"]
mod tests;
