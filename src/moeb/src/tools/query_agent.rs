use std::collections::HashSet;
use std::path::Path;
use std::sync::{Arc, Mutex};
use anyhow::{Context, Result};
use serde_json::json;

use crate::adapters::{AgentResponse, Message, ToolDef};
use crate::ports::{AdapterFactoryPort, AiPort};
use super::ToolHandler;

pub struct QueryAgentTool {
    pub adapter: Option<Arc<dyn AiPort>>,
    pub read_paths: Arc<Mutex<HashSet<String>>>,
}

impl ToolHandler for QueryAgentTool {
    fn name(&self) -> &'static str { "query_agent" }

    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "query_agent",
            description: "Run an isolated sub-agent with a role, injected context, and a prompt. \
                The sub-agent has no tools and returns its response as a plain string. \
                Use for review, analysis, or structured text-completion tasks. \
                Set expected_response_type to \"diff\" for unified diffs, \"json\" for JSON, \
                or \"text\" for plain text. Auto-includes .moeb/** files from the current \
                run's read-set as read-only context.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "role": {
                        "type": "string",
                        "description": "Role name resolved to <role>.role.md via .moeb/roles/ then assets/roles/. \
                            Returns an error if neither location contains the file."
                    },
                    "prompt": {
                        "type": "string",
                        "description": "The task prompt for the sub-agent."
                    },
                    "expected_response_type": {
                        "type": "string",
                        "enum": ["text", "diff", "json"],
                        "description": "Output format constraint appended to the system prompt as a hard instruction. \
                            \"diff\": unified diff or empty string. \"json\": raw JSON only. \"text\": plain text."
                    },
                    "context_files": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Optional additional file paths (relative to repo root) included as read-only context. \
                            No scope enforcement applies — query_agent is read-only."
                    }
                },
                "required": ["role", "prompt", "expected_response_type"]
            }),
        }
    }

    fn execute(&self, args: &serde_json::Value, working_dir: &Path) -> Result<String> {
        if self.resolve_adapter().is_err() {
            return Ok("query_agent is not available in MCP serve mode. Use inline reasoning from your training knowledge instead of calling this tool.".to_string());
        }
        let role = args["role"].as_str().context("query_agent: missing 'role'")?;
        let prompt = args["prompt"].as_str().context("query_agent: missing 'prompt'")?;
        let response_type = args["expected_response_type"].as_str().unwrap_or("text");

        let role_content = resolve_role(role, working_dir)?;

        let mut context_pairs: Vec<(String, String)> = Vec::new();

        {
            let read_paths = self.read_paths.lock().unwrap();
            for path in read_paths.iter() {
                if path.starts_with(".moeb/") {
                    if let Ok(content) = std::fs::read_to_string(working_dir.join(path)) {
                        context_pairs.push((path.clone(), content));
                    }
                }
            }
        }

        if let Some(files) = args["context_files"].as_array() {
            for v in files {
                if let Some(path) = v.as_str() {
                    if let Ok(content) = std::fs::read_to_string(working_dir.join(path)) {
                        context_pairs.push((path.to_string(), content));
                    }
                }
            }
        }

        let response_instruction = match response_type {
            "diff" => "Return a unified diff (--- / +++ / @@ hunk headers) or an empty string \
                if no changes are proposed. Output the diff only — no preamble, no explanation.",
            "json" => "Return valid JSON only. No preamble, no markdown fencing, \
                no explanation — just the raw JSON object or array.",
            _ => "Respond in plain text.",
        };

        let mut full_prompt = role_content;
        if !context_pairs.is_empty() {
            full_prompt.push_str(
                "\n\n## Context files\n\nThe following files are provided as read-only context:\n",
            );
            for (path, content) in &context_pairs {
                full_prompt.push_str(&format!("\n### {}\n{}\n", path, content));
            }
        }
        full_prompt.push_str(&format!(
            "\n\n## Output constraint\n\n{}\n\n## Task\n\n{}",
            response_instruction, prompt
        ));

        let adapter = self.resolve_adapter()?;
        let messages = vec![Message::User(full_prompt)];
        let response = adapter.send(&messages, &[])?;

        // Unwrap WithThinking: discard thinking blocks and process inner response.
        let response = if let AgentResponse::WithThinking { inner, .. } = response {
            *inner
        } else {
            response
        };

        let text = match response {
            AgentResponse::Text(t) => t,
            AgentResponse::ToolCalls(_) => String::new(),
            AgentResponse::WithThinking { .. } => unreachable!("WithThinking is always unwrapped before dispatch"),
        };

        if response_type == "json" && serde_json::from_str::<serde_json::Value>(&text).is_err() {
            return Ok(format!("[PARSE_WARNING] {}", text));
        }

        Ok(text)
    }
}

impl QueryAgentTool {
    fn resolve_adapter(&self) -> Result<Arc<dyn AiPort>> {
        if let Some(a) = &self.adapter {
            return Ok(Arc::clone(a));
        }
        let noop_trace = Arc::new(crate::trace::TraceContext::new(crate::trace::TraceConfig {
            command: crate::trace::TraceCommand::Run,
            spec: String::new(),
            adapter: String::new(),
            model: String::new(),
            retention: 0,
            file_content_mode: crate::trace::FileContentMode::Embed,
        }));
        crate::adapters::DefaultAdapterFactory
            .build(noop_trace)
            .context("query_agent: failed to build adapter from config")
    }
}

fn resolve_role(role: &str, working_dir: &Path) -> Result<String> {
    let project_path = working_dir
        .join(".moeb/roles")
        .join(format!("{}.role.md", role));
    if project_path.exists() {
        return std::fs::read_to_string(&project_path)
            .with_context(|| format!("query_agent: cannot read '{}'", project_path.display()));
    }
    crate::assets::Internal::get(&format!("roles/{}.role.md", role))
        .and_then(|f| std::str::from_utf8(f.data.as_ref()).ok().map(str::to_owned))
        .ok_or_else(|| anyhow::anyhow!("query_agent error: role file not found for role '{}'", role))
}
