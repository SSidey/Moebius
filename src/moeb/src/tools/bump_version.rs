use std::path::Path;
use anyhow::Result;
use regex::Regex;
use serde_json::json;

use crate::adapters::ToolDef;
use super::ToolHandler;

pub struct BumpVersionTool;

impl ToolHandler for BumpVersionTool {
    fn name(&self) -> &'static str {
        "bump_version"
    }

    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "bump_version",
            description: "Reads the current version from src/moeb/Cargo.toml, increments it \
                according to bump_type per SemVer 2.0.0 (major resets minor and patch to 0; \
                minor resets patch to 0; patch increments only patch), writes the updated file, \
                stages it, and commits with 'chore(release): bump version to v<new-version>'. \
                Returns the new version string.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "bump_type": {
                        "type": "string",
                        "enum": ["major", "minor", "patch"],
                        "description": "SemVer 2.0.0 increment class."
                    }
                },
                "required": ["bump_type"]
            }),
        }
    }

    fn execute(&self, args: &serde_json::Value, working_dir: &Path) -> Result<String> {
        let bump_type = match args["bump_type"].as_str() {
            Some(v) if matches!(v, "major" | "minor" | "patch") => v,
            _ => {
                return Ok(
                    "Error: bump_type is required and must be \"major\", \"minor\", or \"patch\""
                        .to_string(),
                )
            }
        };

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

        let parse_part = |s: &str| s.parse::<u32>();
        let (old_major, old_minor, old_patch) = match (
            parse_part(&caps[1]),
            parse_part(&caps[2]),
            parse_part(&caps[3]),
        ) {
            (Ok(a), Ok(b), Ok(c)) => (a, b, c),
            _ => return Ok("Error: version components are not valid integers".to_string()),
        };

        let (new_major, new_minor, new_patch) = match bump_type {
            "major" => (old_major + 1, 0, 0),
            "minor" => (old_major, old_minor + 1, 0),
            _ => (old_major, old_minor, old_patch + 1),
        };

        let old_ver = format!("{}.{}.{}", old_major, old_minor, old_patch);
        let new_ver = format!("{}.{}.{}", new_major, new_minor, new_patch);
        let new_content = content.replacen(
            &format!("version = \"{}\"", old_ver),
            &format!("version = \"{}\"", new_ver),
            1,
        );

        if let Err(e) = std::fs::write(&cargo_path, &new_content) {
            return Ok(format!("Error: could not write src/moeb/Cargo.toml: {}", e));
        }

        let add_out = std::process::Command::new("git")
            .args(["add", "src/moeb/Cargo.toml"])
            .current_dir(working_dir)
            .output();
        match add_out {
            Ok(o) if !o.status.success() => {
                return Ok(format!(
                    "Error: git add failed: {}",
                    String::from_utf8_lossy(&o.stderr)
                ))
            }
            Err(e) => return Ok(format!("Error: git add failed: {}", e)),
            _ => {}
        }

        let commit_msg = format!("chore(release): bump version to v{}", new_ver);
        let commit_out = std::process::Command::new("git")
            .args(["commit", "-m", &commit_msg])
            .current_dir(working_dir)
            .output();
        match commit_out {
            Ok(o) if !o.status.success() => {
                return Ok(format!(
                    "Error: git commit failed: {}",
                    String::from_utf8_lossy(&o.stderr)
                ))
            }
            Err(e) => return Ok(format!("Error: git commit failed: {}", e)),
            _ => {}
        }

        Ok(new_ver)
    }
}

#[cfg(test)]
#[path = "bump_version_tests.rs"]
mod tests;
