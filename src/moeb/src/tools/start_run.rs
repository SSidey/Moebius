use std::collections::HashMap;
use std::path::{Path, PathBuf};
use anyhow::Result;
use serde_json::json;

use crate::adapters::ToolDef;
use crate::assets::{Internal, Prompts};
use crate::run_state::SharedRunState;
use super::ToolHandler;

const RUN_ID_TOKEN: &str = "{{run_id}}";
const RUN_FILE_PATH_TOKEN: &str = "{{run_file_path}}";
const SIGNAL_ID_TOKEN: &str = "{{signal_id}}";

pub struct StartRunTool {
    pub state: SharedRunState,
    pub tool_schemas_block: String,
}

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
                        "description": "Relative path to the spec file. Optional when signal_id resolves a unique spec via frontmatter scan."
                    },
                    "signal_id": {
                        "type": "string",
                        "description": "Optional UUID of the signal in .moeb/signals/catalogue/ that triggered this run. When provided without spec_path, the tool scans spec frontmatter to resolve spec_path automatically."
                    }
                },
                "required": []
            }),
        }
    }

    fn execute(&self, args: &serde_json::Value, working_dir: &Path) -> Result<String> {
        let spec_path_opt = args["spec_path"].as_str().map(str::to_owned);
        let signal_id = args["signal_id"].as_str().unwrap_or("").to_string();

        let resolved_spec_path: String = match spec_path_opt {
            Some(p) => p,
            None => {
                if signal_id.is_empty() {
                    anyhow::bail!("start_run: 'spec_path' is required when no signal_id is provided");
                }
                resolve_spec_by_signal_id(&signal_id, working_dir)?
            }
        };

        let (abs_spec_path, rel_spec_path) = resolve_spec_path(&resolved_spec_path, working_dir)?;

        let spec_content = std::fs::read_to_string(&abs_spec_path)
            .map_err(|e| anyhow::anyhow!("start_run: cannot read '{}': {}", rel_spec_path, e))?;

        let moeb_dir = working_dir.join(".moeb");

        let readme_content = std::fs::read_to_string(moeb_dir.join("README.md"))
            .unwrap_or_else(|_| "(not found)".to_string());

        let skill_name = crate::skills::extract_skill_name(&spec_content)
            .unwrap_or_else(|| "run".to_string());
        let skill_content = crate::skills::load_skill(&moeb_dir, &skill_name)?;

        let phase_tool_map = parse_phase_tool_map(&skill_content);
        self.state.lock().unwrap().set_phase_tool_map(phase_tool_map);

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
        let reasoning_reviewer_role = crate::skills::load_role(&moeb_dir, "reasoning-reviewer");

        let command_rubrics = build_run_rubrics(&moeb_dir);

        let cfg = crate::config::MoebConfig::load().unwrap_or_default();
        let metrics_window_str = cfg.effective_metrics_window().to_string();
        let metrics_margin_str = format!("{:.2}", cfg.effective_metrics_degradation_margin());

        let run_id = uuid::Uuid::new_v4().to_string();

        let run_ts = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
        let spec_stem = abs_spec_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| resolved_spec_path.to_string());
        let run_file_path = format!(".moeb/runs/{}_run_{}.json", run_ts, spec_stem);

        let asset = Prompts::get("run.prompt")
            .ok_or_else(|| anyhow::anyhow!("start_run: run.prompt not found in binary"))?;
        let template = std::str::from_utf8(asset.data.as_ref())
            .map_err(|e| anyhow::anyhow!("start_run: run.prompt not valid UTF-8: {}", e))?;

        let prompt = template
            .replace("{{tool_schemas}}", &self.tool_schemas_block)
            .replace("{{role_content}}", &role_content)
            .replace("{{spec}}", &rel_spec_path)
            .replace("{{readme_content}}", &readme_content)
            .replace("{{spec_content}}", &spec_content)
            .replace("{{skill_content}}", &skill_content)
            .replace("{{command_rubrics}}", &command_rubrics)
            .replace("{{no_review}}", no_review_value)
            .replace("{{metrics_window}}", &metrics_window_str)
            .replace("{{metrics_degradation_margin}}", &metrics_margin_str)
            .replace(RUN_ID_TOKEN, &run_id)
            .replace(RUN_FILE_PATH_TOKEN, &run_file_path)
            .replace(SIGNAL_ID_TOKEN, &signal_id)
            .replace("{{reviewer_role_content}}", &reviewer_role)
            .replace("{{moderator_role_content}}", &moderator_role)
            .replace("{{qa_architect_role_content}}", &qa_architect_role)
            .replace("{{reasoning_reviewer_persona}}", &reasoning_reviewer_role);

        Ok(prompt)
    }
}

fn parse_phase_tool_map(skill_content: &str) -> HashMap<String, Vec<String>> {
    let mut map = HashMap::new();
    for line in skill_content.lines() {
        if line.contains("<!-- tools:") && line.contains("## Phase ") {
            if let Some(phase_pos) = line.find("## Phase ") {
                let after_phase = &line[phase_pos + "## Phase ".len()..];
                let label = after_phase.split_whitespace().next().unwrap_or("").to_lowercase();
                if label.is_empty() {
                    continue;
                }
                if let Some(tools_start) = line.find("<!-- tools:") {
                    let after_tools = &line[tools_start + "<!-- tools:".len()..];
                    if let Some(end) = after_tools.find("-->") {
                        let tools: Vec<String> = after_tools[..end]
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                        if !tools.is_empty() {
                            map.insert(format!("phase-{}", label), tools);
                        }
                    }
                }
            }
        }
    }
    map
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

fn walk_md_files<F: Fn(&Path) -> bool>(dir: &Path, pred: &F) -> Result<Vec<PathBuf>> {
    let mut matches = Vec::new();
    for entry in std::fs::read_dir(dir)
        .map_err(|e| anyhow::anyhow!("start_run: cannot read {}: {}", dir.display(), e))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            matches.extend(walk_md_files(&path, pred)?);
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") && pred(&path) {
            matches.push(path);
        }
    }
    Ok(matches)
}

fn find_spec_matches(dir: &Path, query: &str) -> Result<Vec<PathBuf>> {
    walk_md_files(dir, &|p| p.file_name().unwrap_or_default().to_string_lossy().contains(query))
}

fn collect_specs_with_signal_id(dir: &Path, signal_id: &str) -> Result<Vec<PathBuf>> {
    walk_md_files(dir, &|p| spec_has_signal_id(p, signal_id))
}

fn spec_has_signal_id(path: &Path, signal_id: &str) -> bool {
    let Ok(content) = std::fs::read_to_string(path) else { return false };
    if !content.starts_with("---") { return false; }
    let Some(nl) = content.find('\n') else { return false };
    let rest = &content[nl + 1..];
    let end = rest.find("\n---").unwrap_or(rest.len());
    let target = format!("signal_id: {}", signal_id.trim());
    rest[..end].lines().any(|l| l.trim() == target)
}

pub fn resolve_spec_by_signal_id(signal_id: &str, working_dir: &Path) -> Result<String> {
    let specs_dir = working_dir.join(".moeb").join("specifications");
    if !specs_dir.exists() {
        anyhow::bail!("no spec found for signal_id {}; provide spec_path explicitly", signal_id);
    }
    let mut candidates = collect_specs_with_signal_id(&specs_dir, signal_id)?;
    if candidates.is_empty() {
        anyhow::bail!("no spec found for signal_id {}; provide spec_path explicitly", signal_id);
    }
    if candidates.len() > 1 {
        candidates.sort_by(|a, b| {
            let mtime = |p: &PathBuf| std::fs::metadata(p).and_then(|m| m.modified()).ok();
            mtime(b).partial_cmp(&mtime(a)).unwrap_or(std::cmp::Ordering::Equal)
        });
        eprintln!("warning: multiple specs found for signal_id {}; using most recently modified: {}",
            signal_id, candidates[0].display());
    }
    let path = candidates.into_iter().next().unwrap();
    Ok(path.strip_prefix(working_dir)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| path.to_string_lossy().replace('\\', "/")))
}

#[cfg(test)]
#[path = "start_run_tests.rs"]
mod tests;
