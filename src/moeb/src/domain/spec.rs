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
use crate::trace::{
    FileContentMode, TraceCommand, TraceConfig, TraceContext, TraceOutcome,
};

#[path = "spec_parser.rs"]
mod spec_parser;
use self::spec_parser::sanitize_slug;
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
const RUBRICS_PATH: &str = "rubrics/catalogue.rubrics.md";

pub struct SpecService {
    factory: Arc<dyn AdapterFactoryPort>,
}

#[cfg(test)]
struct FixedAdapterFactory(Arc<dyn AiPort>);

#[cfg(test)]
impl AdapterFactoryPort for FixedAdapterFactory {
    fn build(&self, _trace: Arc<TraceContext>) -> anyhow::Result<Arc<dyn AiPort>> {
        Ok(Arc::clone(&self.0))
    }
}

impl SpecService {
    pub fn from_config() -> Self {
        Self {
            factory: Arc::new(crate::adapters::DefaultAdapterFactory),
        }
    }

    #[cfg(test)]
    pub fn new(ai: Arc<dyn AiPort>) -> Self {
        Self {
            factory: Arc::new(FixedAdapterFactory(ai)),
        }
    }

    pub fn run(&self, input: &str, _file_content_mode: FileContentMode) -> Result<()> {
        let working_dir = Path::new(".moeb");
        if !working_dir.exists() {
            bail!(".moeb/ not found. Run `moeb init` first.");
        }
        let cfg = MoebConfig::load().unwrap_or_default();
        let limit = cfg.effective_spec_retry_limit();
        self.run_in(input, working_dir, limit, _file_content_mode)
    }

    pub(crate) fn run_in(
        &self,
        input: &str,
        working_dir: &Path,
        retry_limit: u32,
        file_content_mode: FileContentMode,
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

        let skill_content = crate::skills::load_skill(working_dir, "spec");
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

        let prompt = template
            .replace(ROLE_CONTENT_TOKEN, &role_content)
            .replace(INPUT_TOKEN, input)
            .replace(README_TOKEN, &readme_content)
            .replace(SPEC_SCHEMA_TOKEN, &spec_schema_content)
            .replace(RUBRICS_TOKEN, &rubrics_content)
            .replace(SKILL_CONTENT_TOKEN, &skill_content)
            .replace(COMMAND_RUBRICS_TOKEN, &command_rubrics);

        eprintln!("[moeb] generating specification (up to {} attempt(s))...", retry_limit);

        let cfg = MoebConfig::load().unwrap_or_default();
        let adapter_name = cfg.active_adapter.clone().unwrap_or_default();
        let adapter_cfg = cfg.adapter_config(&adapter_name);
        let model = adapter_cfg.effective_model("unknown");

        let trace_config = TraceConfig {
            command: TraceCommand::Spec,
            spec: format!("spec-{}", sanitize_slug(input)),
            adapter: adapter_name,
            model,
            retention: cfg.effective_run_retention(),
            file_content_mode,
        };
        let trace = Arc::new(TraceContext::new(trace_config));
        let ai = self.factory.build(Arc::clone(&trace))?;

        let mut last_err: anyhow::Error = anyhow::anyhow!("no attempts made");
        let mut total_attempts = 0u32;

        for attempt in 1..=retry_limit {
            total_attempts = attempt;
            trace.current_attempt.store(attempt, std::sync::atomic::Ordering::SeqCst);

            let state = crate::run_state::new_shared_run_state();
            let tools = crate::tools::ToolRegistry::standard(
                std::sync::Arc::clone(&state),
            ).definitions();
            let executor = crate::tools::RealToolExecutor::new(std::sync::Arc::clone(&state));
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
        bail!(
            "spec generation failed after {} attempt(s). Last error: {}",
            retry_limit,
            last_err
        );
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "spec_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "spec_integration_tests.rs"]
mod integration_tests;
