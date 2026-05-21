use std::path::Path;

use anyhow::Result;
use serde_json::json;

use crate::adapters::ToolDef;
use super::ToolHandler;

pub struct StartRunTool;

impl ToolHandler for StartRunTool {
    fn name(&self) -> &'static str {
        "start_run"
    }

    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "start_run",
            description: "Load a moeb specification and return the full run-context prompt \
                (spec, role, skill, rubrics). Call this at the start of a session before \
                using file or task tools.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "spec_path": {
                        "type": "string",
                        "description": "Relative path to the spec file from the working directory, \
                            e.g. .moeb/specifications/moeb/moeb.kernel.md"
                    }
                },
                "required": ["spec_path"]
            }),
        }
    }

    fn execute(&self, args: &serde_json::Value, working_dir: &Path) -> Result<String> {
        let spec_path = args["spec_path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("start_run: 'spec_path' must be a string"))?;

        let spec_full_path = working_dir.join(spec_path);
        let spec_content = std::fs::read_to_string(&spec_full_path)
            .map_err(|e| anyhow::anyhow!("start_run: cannot read spec '{}': {}", spec_path, e))?;

        let readme_path = working_dir.join(".moeb").join("README.md");
        let readme_content = std::fs::read_to_string(&readme_path)
            .unwrap_or_else(|_| "(not found)".to_string());

        let (role, skill) = extract_frontmatter_role_skill(&spec_content);

        let role_path = working_dir.join(".moeb").join("roles").join(format!("{}.role.md", role));
        let role_content = std::fs::read_to_string(&role_path)
            .unwrap_or_else(|_| format!("(no role file found for '{}')", role));

        let skill_path = working_dir.join(".moeb").join("skills").join(format!("{}.skill.md", skill));
        let skill_content = std::fs::read_to_string(&skill_path)
            .unwrap_or_else(|_| format!("(no skill file found for '{}')", skill));

        let global_rubrics_path = working_dir.join(".moeb").join("rubrics").join("global.rubrics.md");
        let run_rubrics_path = working_dir.join(".moeb").join("rubrics").join("run.rubrics.md");

        let mut rubrics_parts = Vec::new();
        if let Ok(content) = std::fs::read_to_string(&global_rubrics_path) {
            rubrics_parts.push(content);
        }
        if let Ok(content) = std::fs::read_to_string(&run_rubrics_path) {
            rubrics_parts.push(content);
        }
        let rubrics_content = rubrics_parts.join("\n\n");

        Ok(format!(
            "## Role\n\n{}\n\n## Skill\n\n{}\n\n## Specification Index (README)\n\n{}\n\n## Specification\n\n{}\n\n## Rubrics\n\n{}",
            role_content, skill_content, readme_content, spec_content, rubrics_content
        ))
    }
}

fn extract_frontmatter_role_skill(content: &str) -> (String, String) {
    let mut in_frontmatter = false;
    let mut found_open = false;
    let mut role = "run".to_string();
    let mut skill = "run".to_string();

    for line in content.lines() {
        if line.trim() == "---" {
            if !found_open {
                found_open = true;
                in_frontmatter = true;
            } else if in_frontmatter {
                break;
            }
        } else if in_frontmatter {
            if let Some(val) = line.strip_prefix("role:") {
                role = val.trim().to_string();
            } else if let Some(val) = line.strip_prefix("skill:") {
                skill = val.trim().to_string();
            }
        }
    }

    (role, skill)
}

#[cfg(test)]
#[path = "start_run_tests.rs"]
mod tests;
