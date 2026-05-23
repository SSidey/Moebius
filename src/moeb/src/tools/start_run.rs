use std::path::{Path, PathBuf};
use anyhow::Result;
use serde_json::json;

use crate::adapters::ToolDef;
use crate::assets::{Internal, Prompts};
use super::ToolHandler;

pub struct StartRunTool;

impl ToolHandler for StartRunTool {
    fn name(&self) -> &'static str {
        "start_run"
    }

    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "start_run",
            description: "Load a moeb specification and return the fully rendered run-context \
                prompt (role, spec, skill, rubrics). Call this at the start of a run session \
                before using file or task tools. Accepts either a full relative path or a \
                partial name that uniquely identifies a spec under .moeb/specifications/.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "spec_path": {
                        "type": "string",
                        "description": "Relative path or partial name of the spec file, \
                            e.g. 'moeb.kernel' or '.moeb/specifications/moeb/moeb.kernel.md'."
                    }
                },
                "required": ["spec_path"]
            }),
        }
    }

    fn execute(&self, args: &serde_json::Value, working_dir: &Path) -> Result<String> {
        let spec_path_arg = args["spec_path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("start_run: 'spec_path' must be a string"))?;

        let (abs_spec_path, rel_spec_path) = resolve_spec_path(spec_path_arg, working_dir)?;

        let spec_content = std::fs::read_to_string(&abs_spec_path)
            .map_err(|e| anyhow::anyhow!("start_run: cannot read '{}': {}", rel_spec_path, e))?;

        let moeb_dir = working_dir.join(".moeb");

        let readme_content = std::fs::read_to_string(moeb_dir.join("README.md"))
            .unwrap_or_else(|_| "(not found)".to_string());

        let skill_name = crate::skills::extract_skill_name(&spec_content)
            .unwrap_or_else(|| "run".to_string());
        let skill_content = crate::skills::load_skill(&moeb_dir, &skill_name);

        let skill_review_enabled =
            crate::skills::load_skill_review_flag(&moeb_dir, &skill_name);
        let effective_no_review = !skill_review_enabled;
        let no_review_value = if effective_no_review { "true" } else { "false" };

        let role_name = crate::skills::extract_role_name(&spec_content)
            .unwrap_or_else(|| "run".to_string());
        let role_content = crate::skills::load_role(&moeb_dir, &role_name);
        let reviewer_role = crate::skills::load_role(&moeb_dir, "reviewer");
        let moderator_role = crate::skills::load_role(&moeb_dir, "moderator");
        let qa_architect_role = crate::skills::load_role(&moeb_dir, "qa-architect");

        let command_rubrics = build_run_rubrics(&moeb_dir);

        let cfg = crate::config::MoebConfig::load().unwrap_or_default();
        let metrics_window_str = cfg.effective_metrics_window().to_string();
        let metrics_margin_str = format!("{:.2}", cfg.effective_metrics_degradation_margin());

        let asset = Prompts::get("run.prompt")
            .ok_or_else(|| anyhow::anyhow!("start_run: run.prompt not found in binary"))?;
        let template = std::str::from_utf8(asset.data.as_ref())
            .map_err(|e| anyhow::anyhow!("start_run: run.prompt not valid UTF-8: {}", e))?;

        let prompt = template
            .replace("{{role_content}}", &role_content)
            .replace("{{spec}}", &rel_spec_path)
            .replace("{{readme_content}}", &readme_content)
            .replace("{{spec_content}}", &spec_content)
            .replace("{{skill_content}}", &skill_content)
            .replace("{{command_rubrics}}", &command_rubrics)
            .replace("{{no_review}}", no_review_value)
            .replace("{{metrics_window}}", &metrics_window_str)
            .replace("{{metrics_degradation_margin}}", &metrics_margin_str)
            .replace("{{reviewer_role_content}}", &reviewer_role)
            .replace("{{moderator_role_content}}", &moderator_role)
            .replace("{{qa_architect_role_content}}", &qa_architect_role);

        Ok(prompt)
    }
}

pub(crate) fn build_run_rubrics(moeb_dir: &Path) -> String {
    let binary_layers: Vec<String> = [
        "rubrics/global.rubrics.md",
        "rubrics/run.rubrics.md",
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

    let command_project_path = moeb_dir.join("rubrics/run.rubrics.md");
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

fn resolve_spec_path(spec_path: &str, working_dir: &Path) -> Result<(PathBuf, String)> {
    let candidate = working_dir.join(spec_path);
    if candidate.exists() {
        let rel = spec_path.replace('\\', "/");
        return Ok((candidate, rel));
    }

    let specs_dir = working_dir.join(".moeb").join("specifications");
    if !specs_dir.exists() {
        anyhow::bail!(
            "start_run: '{}' not found and .moeb/specifications/ does not exist",
            spec_path
        );
    }

    let matches = find_spec_matches(&specs_dir, spec_path)?;
    match matches.len() {
        0 => anyhow::bail!("start_run: no specification found matching '{}'", spec_path),
        1 => {
            let abs = matches.into_iter().next().unwrap();
            let rel = abs
                .strip_prefix(working_dir)
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|_| abs.to_string_lossy().replace('\\', "/"));
            Ok((abs, rel))
        }
        _ => anyhow::bail!(
            "start_run: multiple specifications match '{}', be more specific",
            spec_path
        ),
    }
}

fn find_spec_matches(dir: &Path, query: &str) -> Result<Vec<PathBuf>> {
    let mut matches = Vec::new();
    for entry in std::fs::read_dir(dir)
        .map_err(|e| anyhow::anyhow!("start_run: cannot read {}: {}", dir.display(), e))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            matches.extend(find_spec_matches(&path, query)?);
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name.contains(query) {
                matches.push(path);
            }
        }
    }
    Ok(matches)
}

#[cfg(test)]
#[path = "start_run_tests.rs"]
mod tests;
