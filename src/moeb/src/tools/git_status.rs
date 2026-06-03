use std::path::Path;
use anyhow::Result;
use serde_json::json;

use crate::adapters::ToolDef;
use super::ToolHandler;

pub struct GitStatusTool;

impl ToolHandler for GitStatusTool {
    fn name(&self) -> &'static str {
        "git_status"
    }

    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "git_status",
            description: "Return the current working-tree state as a --porcelain git status string. An empty string means the tree is clean.",
            parameters: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }

    fn execute(&self, _args: &serde_json::Value, working_dir: &Path) -> Result<String> {
        let output = std::process::Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(working_dir)
            .output()
            .map_err(|e| anyhow::anyhow!("git_status: failed to spawn git: {}", e))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(anyhow::anyhow!("git_status: git exited non-zero: {}", stderr));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}
