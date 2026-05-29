use std::path::Path;
use std::time::Duration;
use anyhow::Result;
use serde_json::json;
use crate::adapters::ToolDef;
use super::ToolHandler;

pub struct GithubReleasesTool;

impl ToolHandler for GithubReleasesTool {
    fn name(&self) -> &'static str { "github_releases" }

    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "github_releases",
            description: "Query the GitHub Releases API for a given action repository and return the latest release tag whose action.yml declares a specified runs.using value.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "repo": {
                        "type": "string",
                        "description": "GitHub repository in owner/repo format, e.g. actions/checkout"
                    },
                    "runs_using": {
                        "type": "string",
                        "description": "The runs.using value to match, e.g. node20"
                    }
                },
                "required": ["repo", "runs_using"]
            }),
        }
    }

    fn execute(&self, args: &serde_json::Value, _working_dir: &Path) -> Result<String> {
        let repo = match args["repo"].as_str() {
            Some(s) => s,
            None => return Ok("Missing required field: repo".to_string()),
        };
        let runs_using = match args["runs_using"].as_str() {
            Some(s) => s,
            None => return Ok("Missing required field: runs_using".to_string()),
        };

        let mut parts = repo.splitn(2, '/');
        let owner = parts.next().unwrap_or("");
        let repo_name = parts.next().unwrap_or("");
        if owner.is_empty() || repo_name.is_empty() {
            return Ok("Invalid repo format: expected owner/repo".to_string());
        }

        let client = match reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(30))
            .timeout(Duration::from_secs(30))
            .user_agent("moeb-agent")
            .build()
        {
            Ok(c) => c,
            Err(e) => return Ok(format!("GitHub API request failed: {}", e)),
        };

        let url = format!(
            "https://api.github.com/repos/{}/{}/releases?per_page=30",
            owner, repo_name
        );
        let response = match client.get(&url).header("Accept", "application/vnd.github+json").send() {
            Ok(r) => r,
            Err(e) => return Ok(format!("GitHub API request failed: {}", e)),
        };
        if !response.status().is_success() {
            return Ok(format!("GitHub API error: HTTP {}", response.status()));
        }
        let releases: Vec<serde_json::Value> = match response.json() {
            Ok(v) => v,
            Err(e) => return Ok(format!("Failed to parse GitHub releases response: {}", e)),
        };

        for release in &releases {
            let tag_name = match release["tag_name"].as_str() {
                Some(t) => t,
                None => continue,
            };
            let raw_url = format!(
                "https://raw.githubusercontent.com/{}/{}/{}/action.yml",
                owner, repo_name, tag_name
            );
            let raw_resp = match client.get(&raw_url).send() {
                Ok(r) => r,
                Err(_) => continue,
            };
            if raw_resp.status().as_u16() != 200 {
                continue;
            }
            let body = match raw_resp.text() {
                Ok(t) => t,
                Err(_) => continue,
            };
            let yaml_val: serde_yaml::Value = match serde_yaml::from_str(&body) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if let serde_yaml::Value::String(using) = &yaml_val["runs"]["using"] {
                if using == runs_using {
                    return Ok(json!({"tag": tag_name, "runs_using": runs_using}).to_string());
                }
            }
        }

        Ok(format!("No release found for {} with runs.using = {}", repo, runs_using))
    }
}
