use std::path::Path;
use std::sync::Arc;
use anyhow::{Context, Result};
use serde_json::json;

use crate::adapters::ToolDef;
use crate::ports::{AdapterFactoryPort, AiPort};
use super::ToolHandler;

const MAX_REVIEW_TURNS: usize = 5;

pub struct ReviewArtifactTool {
    pub adapter: Option<Arc<dyn AiPort>>,
}

impl ToolHandler for ReviewArtifactTool {
    fn name(&self) -> &'static str { "review_artifact" }

    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "review_artifact",
            description: "Coordinate a Reviewer→Moderator review sub-loop for an artifact just \
                written. Returns a verdict with proposed_changes (unified diff or null), \
                delta_score, accepted, and rationale. Call after each write_file or patch_file \
                within a skill phase when review is enabled.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "artifact_path": {
                        "type": "string",
                        "description": "Path to the artifact just written, relative to repo root."
                    },
                    "step_id": {
                        "type": "string",
                        "description": "Short identifier for the current step, used in StepMetric."
                    },
                    "step_intent": {
                        "type": "string",
                        "description": "Step title and description; used to auto-generate rubric when rubric_criteria is empty."
                    },
                    "rubric_criteria": {
                        "type": "string",
                        "description": "Markdown table rows from the spec rubric applicable to this artifact; empty string if none apply."
                    },
                    "iteration_history": {
                        "type": "array",
                        "description": "Prior review decisions for this step: [{delta_score, accepted}].",
                        "items": { "type": "object" }
                    }
                },
                "required": ["artifact_path", "step_id", "step_intent", "rubric_criteria", "iteration_history"]
            }),
        }
    }

    fn execute(&self, args: &serde_json::Value, working_dir: &Path) -> Result<String> {
        let artifact_path = args["artifact_path"].as_str()
            .context("review_artifact: missing 'artifact_path'")?;
        let step_intent = args["step_intent"].as_str()
            .context("review_artifact: missing 'step_intent'")?;
        let rubric_criteria = args["rubric_criteria"].as_str().unwrap_or("");
        let iteration_history = args["iteration_history"].as_array().cloned().unwrap_or_default();

        let content = std::fs::read_to_string(working_dir.join(artifact_path))
            .with_context(|| format!("review_artifact: cannot read '{}'", artifact_path))?;

        let rubric_for_review = if rubric_criteria.trim().is_empty() {
            format!(
                "Criterion: The artifact fully and correctly accomplishes the following intent: {}",
                step_intent
            )
        } else {
            rubric_criteria.to_string()
        };

        let adapter = self.get_adapter()?;
        let history_json = serde_json::to_string(&iteration_history).unwrap_or_default();

        let reviewer_prompt = build_reviewer_prompt(
            artifact_path, &content, &rubric_for_review, &history_json,
        );
        let reviewer_result = run_sub_agent(&adapter, working_dir, &reviewer_prompt)?;

        if reviewer_result.trim().is_empty() {
            return Ok(serde_json::to_string(&json!({
                "proposed_changes": null,
                "delta_score": 0.0,
                "accepted": false,
                "rationale": "Reviewer found no improvements needed"
            }))?);
        }

        let moderator_prompt = build_moderator_prompt(
            &content, &reviewer_result, &rubric_for_review, &history_json,
        );
        let moderator_result = run_sub_agent(&adapter, working_dir, &moderator_prompt)?;

        let (accepted, delta_score, rationale) = parse_moderator_response(&moderator_result);

        let proposed_changes = if accepted { json!(reviewer_result) } else { json!(null) };

        Ok(serde_json::to_string(&json!({
            "proposed_changes": proposed_changes,
            "delta_score": delta_score,
            "accepted": accepted,
            "rationale": rationale
        }))?)
    }
}

impl ReviewArtifactTool {
    fn get_adapter(&self) -> Result<Arc<dyn AiPort>> {
        if let Some(adapter) = &self.adapter {
            return Ok(Arc::clone(adapter));
        }
        let noop_trace = Arc::new(crate::trace::TraceContext::new(crate::trace::TraceConfig {
            command: crate::trace::TraceCommand::Run,
            spec: String::new(),
            adapter: String::new(),
            model: String::new(),
            retention: 0,
            file_content_mode: crate::trace::FileContentMode::Embed,
        }));
        crate::adapters::DefaultAdapterFactory.build(noop_trace)
    }
}

fn build_reviewer_prompt(path: &str, content: &str, rubric: &str, history: &str) -> String {
    format!(
        "You are a Reviewer. Review the following artifact against the rubric criteria.\n\
         Return a unified diff of proposed improvements if any are needed, or an empty\n\
         string if the artifact fully satisfies all criteria.\n\n\
         Artifact path: {}\n\
         Artifact content:\n{}\n\n\
         Rubric criteria:\n{}\n\n\
         Iteration history (prior proposals for this step): {}\n\n\
         Output format: unified diff only (--- / +++ / @@ headers), or empty string.",
        path, content, rubric, history
    )
}

fn build_moderator_prompt(content: &str, diff: &str, rubric: &str, history: &str) -> String {
    format!(
        "You are a Moderator evaluating a proposed change to an artifact.\n\
         Accept the change only if it produces a genuine, measurable quality improvement\n\
         (not a stylistic preference or speculative enhancement).\n\
         Score the expected improvement delta on a scale 0.0 (no improvement) to 1.0\n\
         (fully resolves all rubric gaps).\n\n\
         Original artifact content:\n{}\n\n\
         Proposed diff:\n{}\n\n\
         Rubric criteria:\n{}\n\n\
         Prior iteration history: {}\n\n\
         Return JSON only:\n\
         {{ \"accepted\": true|false, \"delta_score\": 0.0, \"rationale\": \"string\" }}",
        content, diff, rubric, history
    )
}

fn run_sub_agent(adapter: &Arc<dyn AiPort>, working_dir: &Path, prompt: &str) -> Result<String> {
    let state = crate::run_state::new_shared_run_state();
    let registry = super::ToolRegistry::sub_agent(Arc::clone(&state));
    let tool_defs = registry.definitions();
    let executor = super::RealToolExecutor::new_sub_agent(Arc::clone(&state));
    let noop_trace = Arc::new(crate::trace::TraceContext::new(crate::trace::TraceConfig {
        command: crate::trace::TraceCommand::Run,
        spec: String::new(),
        adapter: String::new(),
        model: String::new(),
        retention: 0,
        file_content_mode: crate::trace::FileContentMode::Embed,
    }));
    let messages = vec![crate::adapters::Message::User(prompt.to_string())];
    crate::agent::run_agent_loop_traced(
        adapter.as_ref(),
        &executor,
        &tool_defs,
        working_dir,
        messages,
        MAX_REVIEW_TURNS,
        &noop_trace,
        1,
        crate::agent::CompactionConfig::default(),
        state,
    )
}

fn parse_moderator_response(response: &str) -> (bool, f64, String) {
    let text = response.trim();
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            let json_str = &text[start..=end];
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
                let accepted = val["accepted"].as_bool().unwrap_or(false);
                let delta = val["delta_score"].as_f64().unwrap_or(0.0);
                let rationale = val["rationale"].as_str().unwrap_or("").to_string();
                return (accepted, delta, rationale);
            }
        }
    }
    (false, 0.0, "Moderator response parse failure — defaulting to reject".to_string())
}
