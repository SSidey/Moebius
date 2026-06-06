use std::path::Path;
use anyhow::Result;
use serde_json::json;

use crate::adapters::ToolDef;
use crate::assets::{Internal, Prompts};
use super::ToolHandler;

const RUN_ID_TOKEN: &str = "{{run_id}}";
const RUN_FILE_PATH_TOKEN: &str = "{{run_file_path}}";
const RESOLUTION_SUMMARY_TOKEN: &str = "{{resolution_summary}}";
const SIGNAL_ID_TOKEN: &str = "{{signal_id}}";

pub struct AcceptCandidateTool;

impl ToolHandler for AcceptCandidateTool {
    fn name(&self) -> &'static str {
        "accept_candidate"
    }

    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "accept_candidate",
            description: "Accept a candidate signal: reads the signal catalogue entry, sets \
                status to 'resolved' with a resolved_at timestamp, updates the signal index, \
                and emits a signal.resolved event to .moeb/events/<signal_id>.resolved.event.json.",
            parameters: json!({
                "type": "object",
                "required": ["signal_id"],
                "properties": {
                    "signal_id": {
                        "type": "string",
                        "description": "UUID of the signal catalogue entry to mark as resolved."
                    },
                    "resolution_summary": {
                        "type": "string",
                        "description": "Optional human-readable explanation of how the signal was resolved. Stored on the signal record alongside resolved_at."
                    }
                }
            }),
        }
    }

    fn execute(&self, args: &serde_json::Value, working_dir: &Path) -> Result<String> {
        let signal_id = args["signal_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("accept_candidate: 'signal_id' must be a string"))?
            .to_string();
        let resolution_summary = args["resolution_summary"].as_str().unwrap_or("").to_string();

        let moeb_dir = working_dir.join(".moeb");

        let asset = Prompts::get("accept_candidate.prompt")
            .ok_or_else(|| anyhow::anyhow!("accept_candidate: accept_candidate.prompt not found in binary"))?;
        let template = std::str::from_utf8(asset.data.as_ref())
            .map_err(|e| anyhow::anyhow!("accept_candidate: accept_candidate.prompt not valid UTF-8: {}", e))?;

        let readme_content = std::fs::read_to_string(moeb_dir.join("README.md"))
            .unwrap_or_else(|_| String::new());

        let role_content = crate::skills::load_role(&moeb_dir, "run");
        let reasoning_reviewer_role = crate::skills::load_role(&moeb_dir, "reasoning-reviewer");
        let skill_content = crate::skills::load_skill(&moeb_dir, "accept_candidate")?;

        let command_rubrics = build_accept_candidate_rubrics(&moeb_dir);

        let run_id = uuid::Uuid::new_v4().to_string();
        let run_ts = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
        let run_file_path = format!(".moeb/runs/{}_accept_candidate.json", run_ts);

        let prompt = template
            .replace("{{role_content}}", &role_content)
            .replace("{{readme_content}}", &readme_content)
            .replace("{{skill_content}}", &skill_content)
            .replace("{{command_rubrics}}", &command_rubrics)
            .replace("{{reasoning_reviewer_persona}}", &reasoning_reviewer_role)
            .replace(RUN_ID_TOKEN, &run_id)
            .replace(RUN_FILE_PATH_TOKEN, &run_file_path)
            .replace(RESOLUTION_SUMMARY_TOKEN, &resolution_summary)
            .replace(SIGNAL_ID_TOKEN, &signal_id);

        Ok(prompt)
    }
}

fn build_accept_candidate_rubrics(moeb_dir: &Path) -> String {
    let binary_layers: Vec<String> = ["rubrics/global.rubrics.md"]
        .iter()
        .filter_map(|key| {
            Internal::get(key)
                .and_then(|f| std::str::from_utf8(f.data.as_ref()).ok().map(str::to_owned))
                .filter(|s| !s.trim().is_empty())
        })
        .collect();

    let global_project_path = moeb_dir.join("rubrics/global.rubrics.md");
    let global_project = if global_project_path.exists() {
        std::fs::read_to_string(&global_project_path).unwrap_or_default()
    } else {
        String::new()
    };

    let command_project_path = moeb_dir.join("rubrics/accept_candidate.rubrics.md");
    let command_project = if command_project_path.exists() {
        std::fs::read_to_string(&command_project_path).unwrap_or_default()
    } else {
        String::new()
    };

    let mut combined: Vec<String> = binary_layers;
    if !global_project.trim().is_empty() { combined.push(global_project); }
    if !command_project.trim().is_empty() { combined.push(command_project); }
    combined.join("\n\n")
}

#[cfg(test)]
#[path = "accept_candidate_tests.rs"]
mod tests;
