use std::path::Path;
use anyhow::Result;
use serde_json::json;

use crate::adapters::ToolDef;
use crate::assets::{Internal, Prompts};
use super::ToolHandler;

const RUN_ID_TOKEN: &str = "{{run_id}}";
const RUN_FILE_PATH_TOKEN: &str = "{{run_file_path}}";
const SIGNAL_ID_TOKEN: &str = "{{signal_id}}";

pub struct FixSignalTool;

impl ToolHandler for FixSignalTool {
    fn name(&self) -> &'static str {
        "fix_signal"
    }

    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "fix_signal",
            description: "Scan .moeb/signals/ for the most-critical unresolved signal, mark it \
                as picked up, create an isolation branch, call start_spec to author a \
                specification for the fix, then call start_run to implement it. Returns a \
                rendered fix_signal.prompt containing the full fix_signal.skill.md workflow.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "signal_id": {
                        "type": "string",
                        "description": "Optional signal ID for direct invocation, bypassing signal-selection phases."
                    },
                    "signal_source": {
                        "type": "string",
                        "enum": ["moeb", "project"],
                        "description": "Optional. Restrict signal selection to signals of this origin. When absent, all unresolved signals are eligible."
                    }
                },
                "required": []
            }),
        }
    }

    fn execute(&self, args: &serde_json::Value, working_dir: &Path) -> Result<String> {
        let moeb_dir = working_dir.join(".moeb");

        let signal_id = args["signal_id"].as_str().unwrap_or("").to_string();
        let signal_source = args["signal_source"].as_str().unwrap_or("").to_string();

        let asset = Prompts::get("fix_signal.prompt")
            .ok_or_else(|| anyhow::anyhow!("fix_signal: fix_signal.prompt not found in binary"))?;
        let template = std::str::from_utf8(asset.data.as_ref())
            .map_err(|e| anyhow::anyhow!("fix_signal: fix_signal.prompt not valid UTF-8: {}", e))?;

        let readme_content = std::fs::read_to_string(moeb_dir.join("README.md"))
            .unwrap_or_else(|_| "(not found)".to_string());

        let role_content = crate::skills::load_role(&moeb_dir, "run");
        let reasoning_reviewer_role = crate::skills::load_role(&moeb_dir, "reasoning-reviewer");
        let skill_content = crate::skills::load_skill(&moeb_dir, "fix_signal")?;

        let command_rubrics = build_fix_signal_rubrics(&moeb_dir);

        let run_id = uuid::Uuid::new_v4().to_string();
        let run_ts = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
        let run_file_path = format!(".moeb/runs/{}_fix_signal.json", run_ts);

        let prompt = template
            .replace("{{role_content}}", &role_content)
            .replace("{{readme_content}}", &readme_content)
            .replace("{{skill_content}}", &skill_content)
            .replace("{{command_rubrics}}", &command_rubrics)
            .replace(RUN_ID_TOKEN, &run_id)
            .replace(RUN_FILE_PATH_TOKEN, &run_file_path)
            .replace(SIGNAL_ID_TOKEN, &signal_id)
            .replace("{{fix_signal_source}}", &signal_source)
            .replace("{{reasoning_reviewer_persona}}", &reasoning_reviewer_role);

        Ok(prompt)
    }
}

fn build_fix_signal_rubrics(moeb_dir: &Path) -> String {
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

    let command_project_path = moeb_dir.join("rubrics/fix_signal.rubrics.md");
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
#[path = "fix_signal_tests.rs"]
mod tests;
