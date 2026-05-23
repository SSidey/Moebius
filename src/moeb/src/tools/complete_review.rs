use std::path::Path;
use anyhow::Result;
use serde_json::json;
use crate::adapters::ToolDef;
use crate::run_state::SharedRunState;
use super::ToolHandler;

pub struct CompleteReviewTool {
    pub state: SharedRunState,
}

impl ToolHandler for CompleteReviewTool {
    fn name(&self) -> &'static str { "complete_review" }

    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "complete_review",
            description: "Acknowledge that the Reviewer\u{2192}Moderator review cycle has completed for a written file. Call once per write_file call, after the review sub-loop terminates, with the same path passed to write_file. Clears the review obligation for that path; omitting this call causes verify_rubrics to fail write-review-compliance.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The file path passed to the most recent write_file call for which the review cycle is now complete."
                    }
                },
                "required": ["path"]
            }),
        }
    }

    fn execute(&self, args: &serde_json::Value, _working_dir: &Path) -> Result<String> {
        let path = args["path"].as_str()
            .ok_or_else(|| anyhow::anyhow!("complete_review: missing 'path'"))?;
        let removed = self.state.lock().unwrap().acknowledge_review(path);
        if removed {
            Ok(format!("Review acknowledged for {}", path))
        } else {
            Ok(format!("Path '{}' not in pending_reviews (no-op)", path))
        }
    }
}
