use std::path::Path;
use anyhow::Result;
use serde_json::json;

use crate::adapters::ToolDef;
use crate::assets::{Internal, Prompts};
use super::ToolHandler;

const RUN_ID_TOKEN: &str = "{{run_id}}";
const RUN_FILE_PATH_TOKEN: &str = "{{run_file_path}}";

pub struct StartSpecTool;

impl ToolHandler for StartSpecTool {
    fn name(&self) -> &'static str {
        "start_spec"
    }

    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "start_spec",
            description: "Render the spec.prompt template with all context layers (role, schema, \
                rubrics, skill) and return it as the initial prompt for a spec session. \
                Call this at the start of a spec session before using file or task tools.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "requirement": {
                        "type": "string",
                        "description": "The raw requirement to author a specification for."
                    }
                },
                "required": ["requirement"]
            }),
        }
    }

    fn execute(&self, args: &serde_json::Value, working_dir: &Path) -> Result<String> {
        let requirement = args["requirement"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("start_spec: 'requirement' must be a string"))?;

        let moeb_dir = working_dir.join(".moeb");

        let asset = Prompts::get("spec.prompt")
            .ok_or_else(|| anyhow::anyhow!("start_spec: spec.prompt not found in binary"))?;
        let template = std::str::from_utf8(asset.data.as_ref())
            .map_err(|e| anyhow::anyhow!("start_spec: spec.prompt not valid UTF-8: {}", e))?;

        let readme_content = std::fs::read_to_string(moeb_dir.join("README.md"))
            .unwrap_or_else(|_| "(not found)".to_string());

        let spec_schema_asset = Internal::get("spec-schema.yaml")
            .ok_or_else(|| anyhow::anyhow!("start_spec: spec-schema.yaml not found in binary"))?;
        let spec_schema_content = std::str::from_utf8(spec_schema_asset.data.as_ref())
            .map_err(|e| anyhow::anyhow!("start_spec: spec-schema.yaml not valid UTF-8: {}", e))?;

        let rubrics_content =
            std::fs::read_to_string(moeb_dir.join("rubrics/catalogue.rubrics.md"))
                .unwrap_or_else(|_| {
                    "(rubrics catalogue not found — rubrics/catalogue.rubrics.md is absent)"
                        .to_string()
                });

        let skill_content = crate::skills::load_skill(&moeb_dir, "spec");
        let role_content = crate::skills::load_role(&moeb_dir, "spec");
        let reviewer_role = crate::skills::load_role(&moeb_dir, "reviewer");
        let moderator_role = crate::skills::load_role(&moeb_dir, "moderator");
        let qa_architect_role = crate::skills::load_role(&moeb_dir, "qa-architect");

        let command_rubrics = build_spec_rubrics(&moeb_dir);

        let run_id = uuid::Uuid::new_v4().to_string();

        let input_slug: String = requirement
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '-' })
            .collect::<String>()
            .trim_matches('-')
            .to_string()
            .chars()
            .take(40)
            .collect();
        let run_ts = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
        let run_file_path = format!(".moeb/runs/{}_spec_{}.json", run_ts, input_slug);

        let prompt = template
            .replace("{{role_content}}", &role_content)
            .replace("{{readme_content}}", &readme_content)
            .replace("{{spec_schema_content}}", &spec_schema_content)
            .replace("{{rubrics_content}}", &rubrics_content)
            .replace("{{skill_content}}", &skill_content)
            .replace("{{command_rubrics}}", &command_rubrics)
            .replace("{{input}}", requirement)
            .replace(RUN_ID_TOKEN, &run_id)
            .replace(RUN_FILE_PATH_TOKEN, &run_file_path)
            .replace("{{reviewer_role_content}}", &reviewer_role)
            .replace("{{moderator_role_content}}", &moderator_role)
            .replace("{{qa_architect_role_content}}", &qa_architect_role);

        Ok(prompt)
    }
}

fn build_spec_rubrics(moeb_dir: &Path) -> String {
    let binary_layers: Vec<String> = [
        "rubrics/global.rubrics.md",
        "rubrics/spec.rubrics.md",
    ]
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

    let command_project_path = moeb_dir.join("rubrics/spec.rubrics.md");
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
#[path = "start_spec_tests.rs"]
mod tests;
