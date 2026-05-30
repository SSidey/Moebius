use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;
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

#[path = "spec_parser.rs"]
mod spec_parser;
use self::spec_parser::sanitize_slug;
#[cfg(test)]
#[path = "spec_schema.rs"]
mod spec_schema;

const PROMPT_FILE: &str = "spec.prompt";
const INPUT_TOKEN: &str = "{{input}}";
const README_TOKEN: &str = "{{readme_content}}";
const SPEC_SCHEMA_TOKEN: &str = "{{spec_schema_content}}";
const RUBRICS_TOKEN: &str = "{{rubrics_content}}";
const SKILL_CONTENT_TOKEN: &str = "{{skill_content}}";
const ROLE_CONTENT_TOKEN: &str = "{{role_content}}";
const COMMAND_RUBRICS_TOKEN: &str = "{{command_rubrics}}";
const NO_REVIEW_TOKEN: &str = "{{no_review}}";
const METRICS_WINDOW_TOKEN: &str = "{{metrics_window}}";
const METRICS_MARGIN_TOKEN: &str = "{{metrics_degradation_margin}}";
const RUN_ID_TOKEN: &str = "{{run_id}}";
const RUN_FILE_PATH_TOKEN: &str = "{{run_file_path}}";
const RUBRICS_PATH: &str = "rubrics/catalogue.rubrics.md";

pub struct SpecService {
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

impl SpecService {
    pub fn from_config() -> Self {
        Self { factory: Arc::new(crate::adapters::DefaultAdapterFactory) }
    }

    #[cfg(test)]
    pub fn new(ai: Arc<dyn AiPort>) -> Self {
        Self { factory: Arc::new(FixedAdapterFactory(ai)) }
    }

    pub fn run(&self, input: &str, _file_content_mode: FileContentMode, no_review: bool) -> Result<()> {
        let working_dir = Path::new(".moeb");
        if !working_dir.exists() {
            bail!(".moeb/ not found. Run `moeb init` first.");
        }
        let cfg = MoebConfig::load().unwrap_or_default();
        let limit = cfg.effective_spec_retry_limit();
        self.run_in(input, working_dir, limit, _file_content_mode, no_review)
    }

    pub(crate) fn run_in(
        &self,
        input: &str,
        working_dir: &Path,
        retry_limit: u32,
        file_content_mode: FileContentMode,
        no_review: bool,
    ) -> Result<()> {
        let asset = Prompts::get(PROMPT_FILE)
            .context("Embedded prompt template 'spec.prompt' not found in binary")?;
        let template = std::str::from_utf8(asset.data.as_ref())
            .context("spec.prompt is not valid UTF-8")?;

        let readme_content = fs::read_to_string(working_dir.join("README.md"))
            .with_context(|| {
                format!(
                    "Cannot read {}/README.md. Run `moeb init` first.",
                    working_dir.display()
                )
            })?;

        let spec_schema_asset = Internal::get("spec-schema.yaml")
            .context("Embedded asset 'spec-schema.yaml' not found in binary")?;
        let spec_schema_content = std::str::from_utf8(spec_schema_asset.data.as_ref())
            .context("spec-schema.yaml embedded asset is not valid UTF-8")?
            .to_string();

        let rubrics_content =
            fs::read_to_string(working_dir.join(RUBRICS_PATH)).unwrap_or_else(|_| {
                "(rubrics catalogue not found — rubrics/catalogue.rubrics.md is absent)".to_string()
            });

        let skill_content = crate::skills::load_skill(working_dir, "spec")?;
        let skill_review_enabled = crate::skills::load_skill_review_flag(working_dir, "spec");
        let role_content = crate::skills::load_role(working_dir, "spec");

        let command_rubrics = {
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

            let global_project_path = working_dir.join("rubrics/global.rubrics.md");
            let global_project = if global_project_path.exists() {
                std::fs::read_to_string(&global_project_path).unwrap_or_default()
            } else {
                String::new()
            };

            let command_project_path = working_dir.join("rubrics/spec.rubrics.md");
            let command_project = if command_project_path.exists() {
                std::fs::read_to_string(&command_project_path).unwrap_or_default()
            } else {
                String::new()
            };

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

        let adapter_name = cfg.active_adapter.clone().unwrap_or_default();
        let adapter_cfg = cfg.adapter_config(&adapter_name);
        let model = adapter_cfg.effective_model("unknown");

        let run_ts = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
        let run_file_path = format!(".moeb/runs/{}_spec_{}.json", run_ts, sanitize_slug(input));

        let trace_config = TraceConfig {
            command: TraceCommand::Spec,
            spec: format!("spec-{}", sanitize_slug(input)),
            adapter: adapter_name,
            model,
            retention: cfg.effective_run_retention(),
            file_content_mode,
        };
        let trace = Arc::new(TraceContext::new(trace_config));
        let run_id = trace.run_id().to_string();

        let prompt = template
            .replace(ROLE_CONTENT_TOKEN, &role_content)
            .replace(INPUT_TOKEN, input)
            .replace(README_TOKEN, &readme_content)
            .replace(SPEC_SCHEMA_TOKEN, &spec_schema_content)
            .replace(RUBRICS_TOKEN, &rubrics_content)
            .replace(SKILL_CONTENT_TOKEN, &skill_content)
            .replace(COMMAND_RUBRICS_TOKEN, &command_rubrics)
            .replace(NO_REVIEW_TOKEN, no_review_str)
            .replace(METRICS_WINDOW_TOKEN, &metrics_window_str)
            .replace(METRICS_MARGIN_TOKEN, &metrics_margin_str)
            .replace(RUN_ID_TOKEN, &run_id)
            .replace(RUN_FILE_PATH_TOKEN, &run_file_path);

        eprintln!("[moeb] generating specification (up to {} attempt(s))...", retry_limit);

        let ai = self.factory.build(Arc::clone(&trace), None)?;

        let mut last_err: anyhow::Error = anyhow::anyhow!("no attempts made");
        let mut total_attempts = 0u32;

        for attempt in 1..=retry_limit {
            total_attempts = attempt;
            trace.current_attempt.store(attempt, std::sync::atomic::Ordering::SeqCst);

            let state = crate::run_state::new_shared_run_state();
            let executor = crate::tools::RealToolExecutor::new(std::sync::Arc::clone(&state));
            let tools = executor.registry.definitions();
            let initial_messages = vec![crate::adapters::Message::User(prompt.clone())];
            let compaction_config = crate::agent::CompactionConfig {
                enabled: cfg.effective_compaction_enabled(),
                threshold: cfg.effective_compaction_threshold(),
                keep_turns: cfg.effective_compaction_keep_turns(),
            };

            match crate::agent::run_agent_loop_run_mode(
                ai.as_ref(),
                &executor,
                &tools,
                working_dir,
                initial_messages,
                MAX_TURNS,
                &trace,
                attempt,
                compaction_config,
                state,
            ) {
                Ok(_) => {
                    trace.set_total_attempts(total_attempts);
                    if let Err(e) = trace.finalize(TraceOutcome::Success, None) {
                        eprintln!("[moeb] warning: trace could not be saved: {}", e);
                    }
                    check_run_outputs(&run_id, &run_file_path, "spec");
                    return Ok(());
                }
                Err(e) => {
                    eprintln!("[moeb] spec attempt {}/{} failed: {}", attempt, retry_limit, e);
                    last_err = e;
                }
            }
        }

        trace.set_total_attempts(total_attempts);
        if let Err(e) = trace.finalize(TraceOutcome::Failure, Some(last_err.to_string())) {
            eprintln!("[moeb] warning: trace could not be saved: {}", e);
        }
        check_run_outputs(&run_id, &run_file_path, "spec");
        bail!(
            "spec generation failed after {} attempt(s). Last error: {}",
            retry_limit,
            last_err
        );
    }
}

fn make_critical_signal(run_id: &str, title: &str, desc: String) -> serde_json::Value {
    serde_json::json!({"signal_id": uuid::Uuid::new_v4().to_string(), "run_id": run_id,
        "timestamp": chrono::Utc::now().to_rfc3339(), "category": "Error",
        "severity": "Critical", "title": title, "description": desc,
        "proposed_resolution": null, "auto_spec_path": null, "gating_condition": null})
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
        let stub = serde_json::json!({"run_id": run_id, "timestamp": chrono::Utc::now().to_rfc3339(),
            "command": command, "signals_path": format!(".moeb/signals/{}.signals.json", run_id),
            "metrics_path": format!(".moeb/metrics/{}.metrics.json", run_id),
            "rubric_score": 0.0, "end_review_error_count": 0, "kernel_fallback": true});
        let _ = std::fs::write(run_file_path, serde_json::to_string_pretty(&stub)
            .unwrap_or_else(|_| "{}".to_string()));
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "spec_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "spec_integration_tests.rs"]
mod integration_tests;
