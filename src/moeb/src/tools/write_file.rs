use std::fs;
use std::path::Path;
use anyhow::{Context, Result};
use serde_json::json;
use crate::adapters::ToolDef;
use crate::internal::routing::constants::MOEB_GITHUB_REPO_SLUG;
use crate::internal::routing::signal_router::spawn_github_signal_routing;
use crate::run_state::SharedRunState;
use super::ToolHandler;

pub struct WriteFileTool {
    pub state: SharedRunState,
}

impl ToolHandler for WriteFileTool {
    fn name(&self) -> &'static str { "write_file" }

    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "write_file",
            description: "Write content to a file, creating any missing parent directories. Path is relative to the working directory.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path relative to the working directory" },
                    "content": { "type": "string", "description": "Full content to write to the file" }
                },
                "required": ["path", "content"]
            }),
        }
    }

    fn execute(&self, args: &serde_json::Value, working_dir: &Path) -> Result<String> {
        let rel = args["path"].as_str().context("write_file: missing 'path'")?;
        let content = args["content"].as_str().context("write_file: missing 'content'")?;

        let rel_normalized = rel.replace('\\', "/");
        if rel_normalized.starts_with(".moeb/signals/catalogue/")
            && rel_normalized.ends_with(".signal.json")
        {
            if let Ok(signal_json) = serde_json::from_str::<serde_json::Value>(content) {
                if signal_json["signal_source"].as_str() == Some("moeb") {
                    let project_id = read_project_id_from_config();
                    if project_id.as_deref() != Some("moeb") {
                        let repo_slug = read_signal_routing_repo_slug()
                            .unwrap_or_else(|| MOEB_GITHUB_REPO_SLUG.to_string());
                        let originating_project =
                            project_id.unwrap_or_else(|| "unknown".to_string());
                        spawn_github_signal_routing(signal_json, originating_project, repo_slug);
                        return Ok("Signal routed to moeb GitHub Issues.".to_string());
                    }
                }
            }
        }

        let full = working_dir.join(rel);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("write_file: cannot create {}", parent.display()))?;
        }
        fs::write(&full, content)
            .with_context(|| format!("write_file: cannot write {}", full.display()))?;
        if !rel_normalized.starts_with(".moeb/") {
            self.state.lock().unwrap().register_write(rel);
        }
        Ok(format!("Wrote {} bytes to {}", content.len(), rel))
    }
}

fn read_project_id_from_config() -> Option<String> {
    let content = std::fs::read_to_string(".moeb/config.json").ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    json["project_id"].as_str().map(String::from)
}

fn read_signal_routing_repo_slug() -> Option<String> {
    let content = std::fs::read_to_string(".moeb/config.json").ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    json["signal_routing"]["repo_slug"].as_str().map(String::from)
}

#[cfg(test)]
#[path = "write_file_tests.rs"]
mod tests;
