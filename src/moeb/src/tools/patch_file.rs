use std::path::Path;
use anyhow::{Context, Result};
use serde_json::json;
use crate::adapters::ToolDef;
use crate::tools::ToolHandler;

/// Rewrite each `@@ -orig_start[,orig_count] +new_start[,new_count] @@[ text]` header
/// so that the declared counts match the actual hunk body lines.  Lines prefixed with
/// `-` count only toward the original count; `+` only toward the new count; a space (or
/// an otherwise empty line that represents a blank context line) counts toward both.
/// Lines starting with `\` (the "no newline at end of file" marker) are skipped in the
/// count.  Headers that cannot be parsed are passed through unchanged so diffy can emit
/// its own error.
fn normalize_hunk_counts(diff: &str) -> String {
    // Use lines() rather than split('\n') to avoid a spurious trailing empty element
    // when the diff string ends with '\n' — that empty element would otherwise be
    // miscounted as a context line and corrupt every hunk header.
    let lines: Vec<&str> = diff.lines().collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        if !line.starts_with("@@") {
            out.push(line.to_string());
            i += 1;
            continue;
        }

        // Parse: @@ -orig_start[,orig_count] +new_start[,new_count] @@ [suffix]
        // We need only the start line numbers; we will recompute the counts.
        let inner = line.trim_start_matches('@').trim_start().trim_end_matches('@').trim();
        let (ranges, suffix) = if let Some(idx) = inner.find("@@") {
            (inner[..idx].trim(), inner[idx + 2..].trim())
        } else {
            (inner, "")
        };

        let mut parts = ranges.split_whitespace();
        let orig_range = parts.next().unwrap_or("");
        let new_range = parts.next().unwrap_or("");

        let orig_start = orig_range
            .trim_start_matches('-')
            .split(',')
            .next()
            .and_then(|s| s.parse::<usize>().ok());
        let new_start = new_range
            .trim_start_matches('+')
            .split(',')
            .next()
            .and_then(|s| s.parse::<usize>().ok());

        let (Some(orig_start), Some(new_start)) = (orig_start, new_start) else {
            // Cannot parse; pass through and let diffy report the error.
            out.push(line.to_string());
            i += 1;
            continue;
        };

        // Collect the hunk body (until next @@ or end of input).
        i += 1;
        let body_start = i;
        while i < lines.len() && !lines[i].starts_with("@@") {
            i += 1;
        }
        let body = &lines[body_start..i];

        // Count actual lines.
        let mut orig_count: usize = 0;
        let mut new_count: usize = 0;
        for body_line in body {
            if body_line.starts_with('-') {
                orig_count += 1;
            } else if body_line.starts_with('+') {
                new_count += 1;
            } else if body_line.starts_with('\\') {
                // "No newline at end of file" — does not count.
            } else {
                // Context line (space-prefixed or empty blank-context line).
                orig_count += 1;
                new_count += 1;
            }
        }

        // Rebuild the header.
        let new_header = if suffix.is_empty() {
            format!("@@ -{},{} +{},{} @@", orig_start, orig_count, new_start, new_count)
        } else {
            format!("@@ -{},{} +{},{} @@ {}", orig_start, orig_count, new_start, new_count, suffix)
        };
        out.push(new_header);
        for body_line in body {
            out.push(body_line.to_string());
        }
    }

    // Restore the trailing newline that lines() stripped, so diffy receives the
    // same line-terminator convention as the original diff.
    let mut result = out.join("\n");
    if diff.ends_with('\n') {
        result.push('\n');
    }
    result
}

pub struct PatchFileTool;

impl ToolHandler for PatchFileTool {
    fn name(&self) -> &'static str {
        "patch_file"
    }

    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "patch_file",
            description: "Apply a unified diff to an existing file. Use this instead of \
                write_file when making targeted changes to a small number of lines in a large \
                file — only the changed lines are transmitted rather than the complete file \
                content. The diff must be in unified format with @@ hunk headers (as produced \
                by `git diff` or `diff -u`). The file must have been read via read_file or \
                read_files earlier in this run.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to patch, relative to the working directory."
                    },
                    "diff": {
                        "type": "string",
                        "description": "The unified diff to apply. Must include @@ hunk headers and context lines."
                    }
                },
                "required": ["path", "diff"]
            }),
        }
    }

    fn execute(&self, args: &serde_json::Value, working_dir: &Path) -> Result<String> {
        let path = args["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("patch_file: missing required argument 'path'"))?;
        let diff = args["diff"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("patch_file: missing required argument 'diff'"))?;

        let abs_path = working_dir.join(path);
        let original = std::fs::read_to_string(&abs_path)
            .with_context(|| format!("patch_file: could not read '{}'", path))?;

        let normalized = normalize_hunk_counts(diff);
        let patch = diffy::Patch::from_str(&normalized)
            .map_err(|e| anyhow::anyhow!(
                "patch_file: failed to parse diff for '{}': {}. \
                 Ensure the diff uses @@ hunk headers with context lines.",
                path, e
            ))?;

        let patched = diffy::apply(&original, &patch)
            .map_err(|e| anyhow::anyhow!(
                "patch_file: failed to apply diff to '{}': {}. \
                 The diff context lines may not match the current file content — \
                 re-read the file and regenerate the diff.",
                path, e
            ))?;

        let lines_before = original.lines().count();
        let lines_after = patched.lines().count();
        std::fs::write(&abs_path, patched)
            .with_context(|| format!("patch_file: could not write '{}'", path))?;

        Ok(format!(
            "patch_file: applied to '{}' ({} → {} lines).",
            path, lines_before, lines_after
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn temp_dir() -> TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn patch_file_applies_single_hunk() {
        let dir = temp_dir();
        let file = dir.path().join("target.rs");
        std::fs::write(&file, "fn foo() {\n    let x = 1;\n    x\n}\n").unwrap();

        let tool = PatchFileTool;
        let diff = "@@ -2,2 +2,2 @@\n-    let x = 1;\n+    let x = 42;\n     x\n";
        let args = serde_json::json!({"path": "target.rs", "diff": diff});
        let result = tool.execute(&args, dir.path()).unwrap();

        assert!(result.contains("applied"), "expected success: {}", result);
        let content = std::fs::read_to_string(&file).unwrap();
        assert!(content.contains("let x = 42;"), "patch must be applied");
        assert!(!content.contains("let x = 1;"), "old line must be removed");
    }

    #[test]
    fn patch_file_returns_error_on_context_mismatch() {
        let dir = temp_dir();
        let file = dir.path().join("target.rs");
        std::fs::write(&file, "fn foo() {}\n").unwrap();

        let tool = PatchFileTool;
        // Syntactically valid diff (correct counts) but context line doesn't match the file
        let diff = "@@ -1,1 +1,1 @@\n-fn bar() {}\n+fn new() {}\n";
        let args = serde_json::json!({"path": "target.rs", "diff": diff});
        let err = tool.execute(&args, dir.path()).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("failed to apply"), "expected apply error: {}", msg);
        // File must be unchanged
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "fn foo() {}\n");
    }

    #[test]
    fn patch_file_normalizes_wrong_counts_context_mismatch() {
        // Header says -1,3 +1,3 but the hunk has only 2 original and 2 result lines.
        // After normalization the header becomes -1,2 +1,2; diffy then rejects it
        // because the context line "fn bar() {}" does not match the file "fn foo() {}".
        let dir = temp_dir();
        let file = dir.path().join("target.rs");
        std::fs::write(&file, "fn foo() {}\n").unwrap();

        let tool = PatchFileTool;
        let diff = "@@ -1,3 +1,3 @@\n fn bar() {}\n-fn old() {}\n+fn new() {}\n";
        let args = serde_json::json!({"path": "target.rs", "diff": diff});
        let err = tool.execute(&args, dir.path()).unwrap_err();
        assert!(err.to_string().contains("failed to apply"), "expected apply error: {}", err);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "fn foo() {}\n");
    }

    #[test]
    fn patch_file_normalizes_wrong_counts_and_applies() {
        // Header declares wildly wrong counts (-1,999 +1,999) but the hunk content
        // is correct.  The normalizer fixes the header to -1,2 +1,2 and the patch
        // applies successfully.
        let dir = temp_dir();
        let file = dir.path().join("target.rs");
        std::fs::write(&file, "fn foo() {}\nfn old() {}\n").unwrap();

        let tool = PatchFileTool;
        let diff = "@@ -1,999 +1,999 @@\n fn foo() {}\n-fn old() {}\n+fn new() {}\n";
        let args = serde_json::json!({"path": "target.rs", "diff": diff});
        let result = tool.execute(&args, dir.path()).unwrap();

        assert!(result.contains("applied"), "expected success: {}", result);
        let content = std::fs::read_to_string(&file).unwrap();
        assert!(content.contains("fn new() {}"), "added line must be present");
        assert!(!content.contains("fn old() {}"), "removed line must be absent");
    }

    #[test]
    fn patch_file_returns_error_on_missing_file() {
        let dir = temp_dir();
        let tool = PatchFileTool;
        let diff = "@@ -1,1 +1,1 @@\n-old\n+new\n";
        let args = serde_json::json!({"path": "nonexistent.rs", "diff": diff});
        let err = tool.execute(&args, dir.path()).unwrap_err();
        assert!(err.to_string().contains("could not read"), "expected read error: {}", err);
    }
}
