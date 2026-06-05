use std::path::Path;
use anyhow::{Context, Result};
use serde_json::json;
use crate::adapters::ToolDef;
use crate::tools::ToolHandler;

pub struct PatchFileTool;

impl ToolHandler for PatchFileTool {
    fn name(&self) -> &'static str {
        "patch_file"
    }

    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "patch_file",
            description: "Apply a targeted replacement to an existing file. Locates \
                `old_string` as an exact substring match and replaces it with `new_string`. \
                Fails immediately if `old_string` is not found, or if it appears more than \
                once (provide more surrounding context to make the match unique). Use for \
                changes of up to ~20 lines when the exact content to replace is known. On \
                failure, fall back to `write_file` with the complete new file content — do \
                not retry `patch_file`.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to patch, relative to the working directory. The file should have been read via read_file or read_files earlier in this run."
                    },
                    "old_string": {
                        "type": "string",
                        "description": "The exact substring to replace. Must appear exactly once in the file."
                    },
                    "new_string": {
                        "type": "string",
                        "description": "The replacement content."
                    }
                },
                "required": ["path", "old_string", "new_string"]
            }),
        }
    }

    fn execute(&self, args: &serde_json::Value, working_dir: &Path) -> Result<String> {
        let path = args["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("patch_file: missing required argument 'path'"))?;
        let old_string = args["old_string"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("patch_file: missing required argument 'old_string'"))?;
        let new_string = args["new_string"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("patch_file: missing required argument 'new_string'"))?;

        let abs_path = working_dir.join(path);
        let original = std::fs::read_to_string(&abs_path)
            .with_context(|| format!("patch_file: could not read '{}'", path))?;
        let original = original.replace('\r', "");
        let old_string = old_string.replace('\r', "");

        let count = original.matches(old_string.as_str()).count();

        if count == 0 {
            return Err(anyhow::anyhow!(
                "patch_file: old_string not found in '{}'. Re-read the file and provide the exact substring to replace.",
                path
            ));
        }
        if count > 1 {
            return Err(anyhow::anyhow!(
                "patch_file: old_string matches {} locations in '{}'. Add more surrounding context to make the match unique.",
                count, path
            ));
        }

        let patched = original.replacen(old_string.as_str(), new_string, 1);
        let lines_before = original.lines().count();
        let lines_after = patched.lines().count();

        std::fs::write(&abs_path, &patched)
            .with_context(|| format!("patch_file: could not write '{}'", path))?;

        Ok(format!(
            "patch_file: applied to '{}' ({} → {} lines).",
            path, lines_before, lines_after
        ))
    }
}

#[cfg(test)]
#[path = "patch_file_tests.rs"]
mod tests;
