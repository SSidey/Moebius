use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::agent::MAX_TURNS;
use crate::assets::{Internal, Prompts};
use crate::config::MoebConfig;
use crate::ports::AdapterFactoryPort;
#[cfg(test)]
use crate::ports::AiPort;
#[cfg(test)]
use crate::run_state::SharedRunState;
use crate::trace::{
    FileContentMode, TraceCommand, TraceConfig, TraceContext, TraceOutcome,
};

const PROMPT_FILE: &str = "run.prompt";
const SPEC_TOKEN: &str = "{{spec}}";
const README_TOKEN: &str = "{{readme_content}}";
const SPEC_CONTENT_TOKEN: &str = "{{spec_content}}";
const SKILL_CONTENT_TOKEN: &str = "{{skill_content}}";
const ROLE_CONTENT_TOKEN: &str = "{{role_content}}";
const COMMAND_RUBRICS_TOKEN: &str = "{{command_rubrics}}";
const NO_REVIEW_TOKEN: &str = "{{no_review}}";
const METRICS_WINDOW_TOKEN: &str = "{{metrics_window}}";
const METRICS_MARGIN_TOKEN: &str = "{{metrics_degradation_margin}}";
const RUN_ID_TOKEN: &str = "{{run_id}}";
const RUN_FILE_PATH_TOKEN: &str = "{{run_file_path}}";
const SPECS_DIR: &str = ".moeb/specifications";
const README_PATH: &str = ".moeb/README.md";
const MOEB_DIR: &str = ".moeb";

pub struct RunService {
    factory: Arc<dyn AdapterFactoryPort>,
}

#[cfg(test)]
struct FixedAdapterFactory(Arc<dyn AiPort>);

#[cfg(test)]
impl AdapterFactoryPort for FixedAdapterFactory {
    fn build(&self, _trace: Arc<TraceContext>, _run_state: Option<SharedRunState>) -> anyhow::Result<Arc<dyn AiPort>> {
        Ok(Arc::clone(&self.0))
    }
}

impl RunService {
    pub fn from_config() -> Self {
        Self { factory: Arc::new(crate::adapters::DefaultAdapterFactory) }
    }

    #[cfg(test)]
    pub fn new(ai: Arc<dyn AiPort>) -> Self {
        Self { factory: Arc::new(FixedAdapterFactory(ai)) }
    }

    pub fn run(&self, spec: &str, file_content_mode: FileContentMode, no_review: bool) -> Result<()> {
        let harness = Path::new(SPECS_DIR);
        if !harness.exists() {
            anyhow::bail!(".moeb/specifications/ not found. Run `moeb init` first.");
        }

        let matches = find_specs(harness, spec)?;

        let spec_path = match matches.len() {
            0 => anyhow::bail!("No specification found matching '{}' under {}.", spec, SPECS_DIR),
            1 => matches.into_iter().next().unwrap(),
            _ => {
                eprintln!("Multiple specifications match '{}'. Narrow your query:", spec);
                for m in &matches { eprintln!("  {}", m.display()); }
                anyhow::bail!("Ambiguous specification name.");
            }
        };

        let rel_path = spec_path.to_string_lossy().replace('\\', "/");

        let asset = Prompts::get(PROMPT_FILE)
            .context("Embedded prompt template 'run.prompt' not found in binary")?;
        let template = std::str::from_utf8(asset.data.as_ref())
            .context("run.prompt is not valid UTF-8")?;

        let readme_content = fs::read_to_string(README_PATH)
            .with_context(|| format!("Cannot read {}. Run `moeb init` first.", README_PATH))?;

        let spec_content = fs::read_to_string(&spec_path)
            .with_context(|| format!("Cannot read {}", spec_path.display()))?;

        let moeb_dir = Path::new(MOEB_DIR);
        let skill_name = crate::skills::extract_skill_name(&spec_content)
            .unwrap_or_else(|| "run".to_string());
        let skill_content = crate::skills::load_skill(moeb_dir, &skill_name)?;
        let skill_review_enabled = crate::skills::load_skill_review_flag(moeb_dir, &skill_name);
        let role_name = crate::skills::extract_role_name(&spec_content)
            .unwrap_or_else(|| "run".to_string());
        let role_content = crate::skills::load_role(moeb_dir, &role_name);

        let command_rubrics = {
            let binary_layers: Vec<String> = ["rubrics/global.rubrics.md", "rubrics/run.rubrics.md"]
                .iter()
                .filter_map(|asset| {
                    Internal::get(asset)
                        .and_then(|f| std::str::from_utf8(f.data.as_ref()).ok().map(str::to_owned))
                        .filter(|s| !s.trim().is_empty())
                })
                .collect();
            let global_project = if Path::new(".moeb/rubrics/global.rubrics.md").exists() {
                std::fs::read_to_string(".moeb/rubrics/global.rubrics.md").unwrap_or_default()
            } else { String::new() };
            let command_project = if Path::new(".moeb/rubrics/run.rubrics.md").exists() {
                std::fs::read_to_string(".moeb/rubrics/run.rubrics.md").unwrap_or_default()
            } else { String::new() };
            let mut combined: Vec<String> = binary_layers;
            if !global_project.trim().is_empty() { combined.push(global_project); }
            if !command_project.trim().is_empty() { combined.push(command_project); }
            combined.join("\n\n")
        };

        let cfg = MoebConfig::load().unwrap_or_default();
        let effective_no_review = no_review || !skill_review_enabled;
        let no_review_str = if effective_no_review { "true" } else { "false" };
        let metrics_window_str = cfg.effective_metrics_window().to_string();
        let metrics_margin_str = format!("{:.2}", cfg.effective_metrics_degradation_margin());

        let spec_slug = spec_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| spec.to_string());

        let adapter_name = cfg.active_adapter.clone().unwrap_or_default();
        let adapter_cfg = cfg.adapter_config(&adapter_name);
        let model = adapter_cfg.effective_model("unknown");

        let run_ts = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
        let run_file_path = format!(".moeb/runs/{}_run_{}.json", run_ts, spec_slug);

        let trace = Arc::new(TraceContext::new(TraceConfig {
            command: TraceCommand::Run,
            spec: spec_slug,
            adapter: adapter_name,
            model,
            retention: cfg.effective_run_retention(),
            file_content_mode,
        }));
        let run_id = trace.run_id().to_string();

        let prompt = template
            .replace(ROLE_CONTENT_TOKEN, &role_content)
            .replace(SPEC_TOKEN, &rel_path)
            .replace(README_TOKEN, &readme_content)
            .replace(SPEC_CONTENT_TOKEN, &spec_content)
            .replace(SKILL_CONTENT_TOKEN, &skill_content)
            .replace(COMMAND_RUBRICS_TOKEN, &command_rubrics)
            .replace(NO_REVIEW_TOKEN, no_review_str)
            .replace(METRICS_WINDOW_TOKEN, &metrics_window_str)
            .replace(METRICS_MARGIN_TOKEN, &metrics_margin_str)
            .replace(RUN_ID_TOKEN, &run_id)
            .replace(RUN_FILE_PATH_TOKEN, &run_file_path);

        let working_dir = Path::new(".");
        let state = crate::run_state::new_shared_run_state();
        state.lock().unwrap().set_run_id(run_id.clone());
        let ai = self.factory.build(Arc::clone(&trace), Some(Arc::clone(&state)))?;
        let executor = crate::tools::RealToolExecutor::new_coordinator(
            std::sync::Arc::clone(&state),
            std::sync::Arc::clone(&ai),
        );
        let tools = executor.registry.definitions();
        let compaction_config = crate::agent::CompactionConfig {
            enabled: cfg.effective_compaction_enabled(),
            threshold: cfg.effective_compaction_threshold(),
            keep_turns: cfg.effective_compaction_keep_turns(),
        };
        let run_result = crate::agent::run_agent_loop_run_mode(
            ai.as_ref(),
            &executor,
            &tools,
            working_dir,
            vec![crate::adapters::Message::User(prompt)],
            MAX_TURNS,
            &trace,
            1,
            compaction_config,
            state,
        );

        let (outcome, err_msg) = match &run_result {
            Ok(_) => (TraceOutcome::Success, None),
            Err(e) => (TraceOutcome::Failure, Some(e.to_string())),
        };
        if let Err(e) = trace.finalize(outcome, err_msg) {
            eprintln!("[moeb] warning: trace could not be saved: {}", e);
        }
        check_run_outputs(&run_id, &run_file_path, "run");
        {
            let locked = executor.state.lock().unwrap();
            if let (Some(score), Some(err_count)) = (locked.rubric_score, locked.end_review_error_count) {
                drop(locked);
                let metrics_path = format!(".moeb/metrics/{}.metrics.json", run_id);
                if let Err(e) = kernel_write_metrics_fields(&metrics_path, score, err_count, &run_id) {
                    eprintln!("moeb: warn: failed to write kernel metrics fields: {}", e);
                }
            }
        }
        let result = run_result?;
        if !result.is_empty() { println!("{}", result); }
        Ok(())
    }
}

fn make_critical_signal(run_id: &str, title: &str, desc: String) -> serde_json::Value {
    serde_json::json!({"signal_id": uuid::Uuid::new_v4().to_string(), "run_id": run_id,
        "timestamp": chrono::Utc::now().to_rfc3339(), "category": "Error",
        "severity": "Critical", "title": title, "description": desc,
        "proposed_resolution": null, "auto_spec_path": null, "gating_condition": null})
}

fn kernel_write_metrics_fields(path: &str, rubric_score: f32, end_review_error_count: u32, run_id: &str) -> anyhow::Result<()> {
    let p = std::path::Path::new(path);
    let mut obj: serde_json::Value = if p.exists() {
        serde_json::from_str(&std::fs::read_to_string(p)?)
            .unwrap_or_else(|_| serde_json::json!({"run_id": run_id}))
    } else { serde_json::json!({"run_id": run_id}) };
    obj["rubric_score"] = serde_json::json!(rubric_score);
    obj["end_review_error_count"] = serde_json::json!(end_review_error_count);
    std::fs::write(p, serde_json::to_string_pretty(&obj)?)?;
    Ok(())
}

fn check_run_outputs(run_id: &str, run_file_path: &str, command: &str) {
    let signals_path = format!(".moeb/signals/{}.signals.json", run_id);
    let metrics_path = format!(".moeb/metrics/{}.metrics.json", run_id);
    let signals_missing = !std::path::Path::new(&signals_path).exists();
    let metrics_missing = !std::path::Path::new(&metrics_path).exists();
    let mut absent: Vec<serde_json::Value> = Vec::new();
    if signals_missing {
        eprintln!("[moeb] critical: signals file not written by agent — {}", signals_path);
        absent.push(make_critical_signal(run_id, "Signals file not written by agent",
            format!("End-of-Skill Review phase did not complete. Expected: {}", signals_path)));
    }
    if metrics_missing {
        eprintln!("[moeb] critical: metrics file not written by agent — {}", metrics_path);
        absent.push(make_critical_signal(run_id, "Metrics file not written by agent",
            format!("Metrics Recording phase did not complete. Expected: {}", metrics_path)));
    }
    if signals_missing {
        let _ = std::fs::create_dir_all(".moeb/signals");
        let _ = std::fs::write(&signals_path,
            serde_json::to_string_pretty(&absent).unwrap_or_else(|_| "[]".to_string()));
    }
    if metrics_missing {
        let _ = std::fs::create_dir_all(".moeb/metrics");
        let stub = serde_json::json!({"run_id": run_id,
            "timestamp": chrono::Utc::now().to_rfc3339(), "rubric_score": 0.0,
            "step_metrics": [], "end_review_error_count": absent.len() as u32,
            "wall_time_ms": 0, "kernel_fallback": true});
        let _ = std::fs::write(&metrics_path,
            serde_json::to_string_pretty(&stub).unwrap_or_else(|_| "{}".to_string()));
    }
    if !std::path::Path::new(run_file_path).exists() {
        eprintln!("[moeb] warning: run file not written by agent — writing fallback stub");
        let _ = std::fs::create_dir_all(".moeb/runs");
        let stub = serde_json::json!({"run_id": run_id,
            "timestamp": chrono::Utc::now().to_rfc3339(), "command": command,
            "signals_path": format!(".moeb/signals/{}.signals.json", run_id),
            "metrics_path": format!(".moeb/metrics/{}.metrics.json", run_id),
            "rubric_score": 0.0, "end_review_error_count": 0, "kernel_fallback": true});
        let _ = std::fs::write(run_file_path, serde_json::to_string_pretty(&stub)
            .unwrap_or_else(|_| "{}".to_string()));
    }
}

fn find_specs(harness: &Path, query: &str) -> Result<Vec<PathBuf>> {
    let mut matches = Vec::new();
    visit_dir(harness, query, &mut matches)?;
    Ok(matches)
}

fn visit_dir(dir: &Path, query: &str, matches: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir)
        .with_context(|| format!("Cannot read directory {}", dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            visit_dir(&path, query, matches)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name.contains(query) { matches.push(path); }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
