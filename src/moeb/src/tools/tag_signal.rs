use std::path::Path;
use anyhow::Result;
use serde_json::json;

use crate::adapters::ToolDef;
use super::ToolHandler;

pub struct TagSignalTool;

impl ToolHandler for TagSignalTool {
    fn name(&self) -> &'static str {
        "tag_signal"
    }

    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "tag_signal",
            description: "Create an annotated git tag for a signal selection. \
                Tag: signal/<signal_id>. \
                Annotation: fix_signal selected <signal_id> event:<event_id>.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "signal_id": {
                        "type": "string",
                        "description": "The UUID signal identifier."
                    },
                    "event_id": {
                        "type": "string",
                        "description": "The UUID event identifier from the emitted event artifact."
                    }
                },
                "required": ["signal_id", "event_id"]
            }),
        }
    }

    fn execute(&self, args: &serde_json::Value, working_dir: &Path) -> Result<String> {
        let signal_id = match args["signal_id"].as_str() {
            Some(s) => s,
            None => return Ok("Error: signal_id is required".to_string()),
        };
        let event_id = match args["event_id"].as_str() {
            Some(e) => e,
            None => return Ok("Error: event_id is required".to_string()),
        };

        let tag_name = format!("signal/{}", signal_id);
        let annotation = format!("fix_signal selected {} event:{}", signal_id, event_id);

        let out = std::process::Command::new("git")
            .args(["tag", "-a", &tag_name, "-m", &annotation])
            .current_dir(working_dir)
            .output();

        match out {
            Ok(o) if o.status.success() => Ok(format!(
                "Tagged {} with annotation event:{}",
                tag_name, event_id
            )),
            Ok(o) => Ok(String::from_utf8_lossy(&o.stderr).into_owned()),
            Err(e) => Ok(format!("Error: git tag failed: {}", e)),
        }
    }
}
