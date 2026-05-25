use std::path::Path;
use anyhow::Result;
use chrono::Utc;
use serde_json::json;

use crate::adapters::ToolDef;
use super::ToolHandler;

pub struct TagRunTool;

impl ToolHandler for TagRunTool {
    fn name(&self) -> &'static str {
        "tag_run"
    }

    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "tag_run",
            description: "Create an annotated git tag for this run. Tag: run/<run_id>. \
                Annotation: moeb <domain>/<slug> @ <timestamp>.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "run_id": {
                        "type": "string",
                        "description": "The UUID run identifier."
                    },
                    "domain": {
                        "type": "string",
                        "description": "The spec domain (e.g. moeb, harness, vcs)."
                    },
                    "slug": {
                        "type": "string",
                        "description": "The spec slug (e.g. ai-first-artifact-refinement)."
                    }
                },
                "required": ["run_id", "domain", "slug"]
            }),
        }
    }

    fn execute(&self, args: &serde_json::Value, working_dir: &Path) -> Result<String> {
        let run_id = match args["run_id"].as_str() {
            Some(r) => r,
            None => return Ok("Error: run_id is required".to_string()),
        };
        let domain = match args["domain"].as_str() {
            Some(d) => d,
            None => return Ok("Error: domain is required".to_string()),
        };
        let slug = match args["slug"].as_str() {
            Some(s) => s,
            None => return Ok("Error: slug is required".to_string()),
        };

        let tag_name = format!("run/{}", run_id);
        let annotation = format!("moeb {}/{} @ {}", domain, slug, Utc::now().to_rfc3339());

        let out = std::process::Command::new("git")
            .args(["tag", "-a", &tag_name, "-m", &annotation])
            .current_dir(working_dir)
            .output();

        match out {
            Ok(o) if o.status.success() => Ok(format!(
                "{{ \"tag\": \"{}\", \"annotation\": \"{}\" }}",
                tag_name, annotation
            )),
            Ok(o) => Ok(String::from_utf8_lossy(&o.stderr).into_owned()),
            Err(e) => Ok(format!("Error: git tag failed: {}", e)),
        }
    }
}
