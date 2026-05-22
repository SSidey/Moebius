use std::path::Path;
use anyhow::Result;
use serde_json::json;

use crate::adapters::ToolDef;
use super::ToolHandler;

pub struct GitCommitTool;

impl ToolHandler for GitCommitTool {
    fn name(&self) -> &'static str {
        "git_commit"
    }

    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "git_commit",
            description: "Stage the spec file and README then create a Conventional Commits \
                message commit: docs(<domain>): add <slug> specification. \
                Call this as the final step after writing spec, branching, and linking README.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "kind": {
                        "type": "string",
                        "description": "Commit kind: \"spec\" stages spec_path and readme_path with a docs commit; \"run\" stages all working-tree changes with a feat commit. Defaults to \"spec\".",
                        "enum": ["spec", "run"]
                    },
                    "spec_path": {
                        "type": "string",
                        "description": "Path to the spec file relative to the working directory."
                    },
                    "readme_path": {
                        "type": "string",
                        "description": "Path to README.md relative to the working directory."
                    },
                    "domain": {
                        "type": "string",
                        "description": "The spec domain."
                    },
                    "slug": {
                        "type": "string",
                        "description": "The spec slug."
                    }
                },
                "required": ["domain", "slug"]
            }),
        }
    }

    fn execute(&self, args: &serde_json::Value, working_dir: &Path) -> Result<String> {
        let domain = args["domain"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("git_commit: 'domain' must be a string"))?;
        let slug = args["slug"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("git_commit: 'slug' must be a string"))?;

        let kind = args["kind"].as_str().unwrap_or("spec");

        match kind {
            "run" => crate::vcs::commit_run(domain, slug).map_err(|e| anyhow::anyhow!(e)),
            _ => {
                let spec_path = args["spec_path"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("git_commit: 'spec_path' must be a string"))?;
                let readme_path = args["readme_path"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("git_commit: 'readme_path' must be a string"))?;

                let abs_spec = working_dir.join(spec_path);
                let abs_readme = working_dir.join(readme_path);

                crate::vcs::commit_spec(&abs_spec, &abs_readme, domain, slug)
                    .map_err(|e| anyhow::anyhow!(e))
            }
        }
    }
}

#[cfg(test)]
#[path = "git_commit_tests.rs"]
mod tests;
