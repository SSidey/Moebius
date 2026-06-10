use std::fs;
use std::path::Path;
use anyhow::{Context, Result};
use serde_json::json;
use crate::adapters::ToolDef;
use super::{ToolHandler, MAX_READ_BYTES, truncate_to_byte_limit, resolve_absolute_path};

pub struct ReadFileTool;

impl ToolHandler for ReadFileTool {
    fn name(&self) -> &'static str { "read_file" }

    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "read_file",
            description: "Read the full contents of a file. Path is relative to the working directory. \
                          Pass absolute: true to read a file by its full filesystem path (must be under \
                          the project root or the OS temp directory; use this for overflow result files \
                          written to temp paths by moeb). Content is capped at 100 KiB; use \
                          read_file_range with absolute: true for larger files.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path relative to the working directory" },
                    "absolute": {
                        "type": "boolean",
                        "default": false,
                        "description": "When true, treat path as a literal filesystem path rather than \
                                        relative to the working directory. The path must be absolute and \
                                        must fall under the project root or the OS temp directory."
                    }
                },
                "required": ["path"]
            }),
        }
    }

    fn execute(&self, args: &serde_json::Value, working_dir: &Path) -> Result<String> {
        let rel = args["path"].as_str().context("read_file: missing 'path'")?;
        let absolute = args["absolute"].as_bool().unwrap_or(false);
        let full = if absolute {
            resolve_absolute_path(rel, working_dir)?
        } else {
            working_dir.join(rel)
        };
        let content = fs::read_to_string(&full)
            .with_context(|| format!("read_file: cannot read {}", full.display()))?;
        Ok(truncate_to_byte_limit(content, MAX_READ_BYTES))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn read_file_truncates_large_file() {
        let tmp = tempfile::tempdir().unwrap();
        let content = "x".repeat(MAX_READ_BYTES + 1000);
        fs::write(tmp.path().join("big.txt"), &content).unwrap();
        let args = serde_json::json!({"path": "big.txt"});
        let result = ReadFileTool.execute(&args, tmp.path()).unwrap();
        assert!(result.contains("[... truncated:"));
        assert!(result.len() < MAX_READ_BYTES + 200);
    }

    #[test]
    fn read_file_does_not_truncate_exact_limit() {
        let tmp = tempfile::tempdir().unwrap();
        let content = "x".repeat(MAX_READ_BYTES);
        fs::write(tmp.path().join("exact.txt"), &content).unwrap();
        let args = serde_json::json!({"path": "exact.txt"});
        let result = ReadFileTool.execute(&args, tmp.path()).unwrap();
        assert!(!result.contains("[... truncated:"));
    }

    #[test]
    fn read_file_absolute_path_in_temp_dir_succeeds() {
        let file_dir = tempfile::tempdir().unwrap();
        fs::write(file_dir.path().join("test_content.txt"), "hello absolute").unwrap();
        let abs_path = file_dir.path().join("test_content.txt");
        // project_root is a separate tempdir; file is under temp_dir, not project_root
        let project_root_dir = tempfile::tempdir().unwrap();
        let args = serde_json::json!({
            "path": abs_path.to_str().unwrap(),
            "absolute": true
        });
        let result = ReadFileTool.execute(&args, project_root_dir.path()).unwrap();
        assert!(result.contains("hello absolute"), "expected file content in result");
    }

    #[test]
    fn read_file_absolute_path_outside_allowed_roots_is_rejected() {
        let project_root_dir = tempfile::tempdir().unwrap();
        // Construct a path at the drive root level — outside both temp_dir and project_root
        let temp_dir = std::env::temp_dir();
        let mut root = temp_dir.as_path();
        while let Some(p) = root.parent() { root = p; }
        let outside_path = root.join("not_a_real_moeb_path_xyzzy_test");
        let args = serde_json::json!({
            "path": outside_path.to_str().unwrap(),
            "absolute": true
        });
        let result = ReadFileTool.execute(&args, project_root_dir.path());
        assert!(result.is_err(), "expected error for outside-roots path");
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("outside allowed roots"), "expected 'outside allowed roots' in: {}", msg);
    }
}
