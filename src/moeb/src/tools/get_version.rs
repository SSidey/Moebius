use std::path::Path;
use anyhow::Result;
use serde_json::json;
use crate::adapters::ToolDef;
use super::ToolHandler;

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct GetVersionTool;

impl ToolHandler for GetVersionTool {
    fn name(&self) -> &'static str { "get_version" }

    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "get_version",
            description: "Return the running binary's semantic version string. No input required.",
            parameters: json!({
                "type": "object",
                "properties": {}
            }),
        }
    }

    fn execute(&self, _args: &serde_json::Value, _working_dir: &Path) -> Result<String> {
        Ok(format!(r#"{{"version":"{}"}}"#, VERSION))
    }
}
