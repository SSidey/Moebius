use std::path::Path;
use anyhow::Result;
use serde_json::json;

use crate::adapters::ToolDef;
use super::ToolHandler;

pub struct CreateBranchTool;

impl ToolHandler for CreateBranchTool {
    fn name(&self) -> &'static str {
        "create_branch"
    }

    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "create_branch",
            description: "Create a Conventional Branch chore/<domain>-<slug> in the repository. \
                Call this after writing the spec file and before linking README.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "domain": {
                        "type": "string",
                        "description": "The spec domain (e.g. moeb, harness, vcs)."
                    },
                    "slug": {
                        "type": "string",
                        "description": "The spec slug (e.g. serve-cli-parity)."
                    }
                },
                "required": ["domain", "slug"]
            }),
        }
    }

    fn execute(&self, args: &serde_json::Value, _working_dir: &Path) -> Result<String> {
        let domain = args["domain"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("create_branch: 'domain' must be a string"))?;
        let slug = args["slug"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("create_branch: 'slug' must be a string"))?;

        let branch = crate::vcs::create_spec_branch(domain, slug)?;
        Ok(format!("Branch created: {}", branch))
    }
}

#[cfg(test)]
#[path = "create_branch_tests.rs"]
mod tests;
