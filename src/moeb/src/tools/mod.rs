pub mod bump_version;
pub mod complete_review;
pub mod create_candidate_tag;
pub mod create_branch;
pub mod create_task_list;
pub mod enter_phase;
pub mod get_run_status;
pub mod get_version;
pub mod git_commit;
pub mod github_releases;
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
pub mod fix_signal;
pub mod tag_run;
pub mod tag_signal;
pub mod update_task;
pub mod verify_rubrics;
pub mod write_file;

mod real_tool_executor;
pub use real_tool_executor::RealToolExecutor;
pub use crate::ports::ToolExecutorPort;

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex};
use anyhow::Result;

use crate::adapters::ToolDef;
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
        r.register(Box::new(write_file::WriteFileTool { state: Arc::clone(&state) }));
        r.register(Box::new(patch_file::PatchFileTool));
        r.register(Box::new(list_directory::ListDirectoryTool));
        r.register(Box::new(search_files::SearchFilesTool));
        r.register(Box::new(grep_files::GrepFilesTool));
        r.register(Box::new(read_files::ReadFilesTool));
        r.register(Box::new(read_file_range::ReadFileRangeTool));
        r.register(Box::new(create_task_list::CreateTaskListTool { state: std::sync::Arc::clone(&state) }));
        r.register(Box::new(update_task::UpdateTaskTool { state: std::sync::Arc::clone(&state) }));
        r.register(Box::new(verify_rubrics::VerifyRubricsTool { state: std::sync::Arc::clone(&state) }));
        r.register(Box::new(complete_review::CompleteReviewTool { state: Arc::clone(&state) }));
        r.register(Box::new(enter_phase::EnterPhaseTool { state: Arc::clone(&state) }));
        r.register(Box::new(create_branch::CreateBranchTool));
        r.register(Box::new(git_commit::GitCommitTool));
        r.register(Box::new(bump_version::BumpVersionTool));
        r.register(Box::new(create_candidate_tag::CreateCandidateTagTool));
        r.register(Box::new(get_version::GetVersionTool));
        r.register(Box::new(tag_run::TagRunTool));
        r.register(Box::new(tag_signal::TagSignalTool));
        r.register(Box::new(query_agent::QueryAgentTool { adapter: None, read_paths }));
        r.register(Box::new(github_releases::GithubReleasesTool));
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
        r.register(Box::new(start_run::StartRunTool { state: Arc::clone(&state) }));
        r.register(Box::new(start_spec::StartSpecTool));
        // fix_signal is MCP-only per moeb.signal-fix-command Decision 1; not in standard()
        r.register(Box::new(fix_signal::FixSignalTool));
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
        r.register(Box::new(start_run::StartRunTool { state: Arc::clone(&state) }));
        r.register(Box::new(start_spec::StartSpecTool));
        // fix_signal is MCP-only per moeb.signal-fix-command Decision 1; not in standard()
        r.register(Box::new(fix_signal::FixSignalTool));
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
            "create_task_list", "update_task", "verify_rubrics", "complete_review",
            "enter_phase",
            "create_branch", "git_commit",
            "bump_version", "create_candidate_tag",
            "get_version", "tag_run", "tag_signal", "query_agent", "github_releases",
            "start_run", "start_spec", "fix_signal", "get_run_status",
        ];
        order.iter()
            .filter_map(|name| self.handlers.get(name).map(|h| h.definition()))
            .collect()
    }
}

#[cfg(test)]
#[path = "tool_executor_tests.rs"]
mod tests;
