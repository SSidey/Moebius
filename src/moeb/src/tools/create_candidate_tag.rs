use std::path::Path;
use anyhow::Result;
use regex::Regex;
use serde_json::json;

use crate::adapters::ToolDef;
use super::ToolHandler;

pub struct CreateCandidateTagTool;

impl ToolHandler for CreateCandidateTagTool {
    fn name(&self) -> &'static str {
        "create_candidate_tag"
    }

    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "create_candidate_tag",
            description: "Reads the current version from src/moeb/Cargo.toml and creates an \
                annotated git tag v<version>-candidate.<domain>-<slug> on HEAD. The \
                <domain>-<slug> suffix mirrors the feat/<domain>-<slug> branch name, ensuring \
                no clash between concurrent inflight branches. Returns the created tag name.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "domain": {
                        "type": "string",
                        "description": "The spec domain (e.g. moeb, harness, vcs)."
                    },
                    "slug": {
                        "type": "string",
                        "description": "The spec slug (e.g. run-semver-bump-and-candidate-tag)."
                    },
                    "force": {
                        "type": "boolean",
                        "description": "When true, force-update an existing tag with git tag -f. Defaults to false."
                    }
                },
                "required": ["domain", "slug"]
            }),
        }
    }

    fn execute(&self, args: &serde_json::Value, working_dir: &Path) -> Result<String> {
        let domain = match args["domain"].as_str() {
            Some(d) => d,
            None => return Ok("Error: domain is required".to_string()),
        };
        let slug = match args["slug"].as_str() {
            Some(s) => s,
            None => return Ok("Error: slug is required".to_string()),
        };
        let force = args["force"].as_bool().unwrap_or(false);

        let cargo_path = working_dir.join("src/moeb/Cargo.toml");
        let content = match std::fs::read_to_string(&cargo_path) {
            Ok(c) => c,
            Err(e) => return Ok(format!("Error: could not read src/moeb/Cargo.toml: {}", e)),
        };

        let re = Regex::new(r#"(?m)^version\s*=\s*"(\d+)\.(\d+)\.(\d+)""#).unwrap();
        let caps = match re.captures(&content) {
            Some(c) => c,
            None => return Ok("Error: version field not found in src/moeb/Cargo.toml".to_string()),
        };

        let (major, minor, patch) = (&caps[1], &caps[2], &caps[3]);
        let tag_name = format!("v{}.{}.{}-candidate.{}-{}", major, minor, patch, domain, slug);
        let tag_message = format!(
            "Release candidate: {}/{} at v{}.{}.{}",
            domain, slug, major, minor, patch
        );

        let mut git_args: Vec<&str> = vec!["tag", "-a"];
        if force {
            git_args.push("-f");
        }
        git_args.push(&tag_name);
        git_args.push("-m");
        git_args.push(&tag_message);

        let out = std::process::Command::new("git")
            .args(&git_args)
            .current_dir(working_dir)
            .output();
        match out {
            Ok(o) if !o.status.success() => {
                return Ok(format!(
                    "Error: git tag failed: {}",
                    String::from_utf8_lossy(&o.stderr)
                ))
            }
            Err(e) => return Ok(format!("Error: git tag failed: {}", e)),
            _ => {}
        }

        Ok(tag_name)
    }
}

#[cfg(test)]
#[path = "create_candidate_tag_tests.rs"]
mod tests;
