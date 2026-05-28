use std::path::Path;
use anyhow::{Context, Result};
use serde_json::json;
use crate::adapters::ToolDef;
use crate::tools::ToolHandler;

#[path = "patch_file_impl.rs"]
mod patch_file_impl;
use patch_file_impl::{normalize_hunk_counts, relocate_hunk_positions};

/// Search `orig_lines` for the first consecutive run of `context` lines closest to
/// `near` (1-indexed).  Returns the 1-indexed start line of that run, or `None` if
/// `context` does not appear consecutively anywhere in `orig_lines`.
fn find_closest_context(orig_lines: &[&str], context: &[&str], near: usize) -> Option<usize> {
    if context.is_empty() || orig_lines.len() < context.len() {
        return None;
    }
    let mut best: Option<(usize, usize)> = None; // (distance, 1-indexed line)
    let max_start = orig_lines.len() - context.len();
    'outer: for i in 0..=max_start {
        for (j, &c) in context.iter().enumerate() {
            if orig_lines[i + j].trim_end() != c.trim_end() {
                continue 'outer;
            }
        }
        let line = i + 1; // 1-indexed
        let dist = if line >= near { line - near } else { near - line };
        if best.map_or(true, |(d, _)| dist < d) {
            best = Some((dist, line));
        }
    }
    best.map(|(_, line)| line)
}

fn normalize_line_endings(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

fn normalize_trailing_whitespace(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for line in s.lines() {
        result.push_str(line.trim_end());
        result.push('\n');
    }
    if !s.ends_with('\n') && result.ends_with('\n') {
        result.pop();
    }
    result
}

/// Returns the context lines from `relocated_diff` that are absent from `original`
/// (using exact equality after trimming trailing whitespace on both sides — consistent
/// with the comparator in `find_closest_context`).  Called only on the apply-failure
/// path to produce an actionable diagnostic.
fn diagnose_missing_context<'a>(relocated_diff: &'a str, original: &str) -> Vec<&'a str> {
    let orig_lines: Vec<&str> = original.lines().collect();
    let mut absent = Vec::new();
    for line in relocated_diff.lines() {
        if line.starts_with('@')
            || line.starts_with('-')
            || line.starts_with('+')
            || line.starts_with('\\')
        {
            continue;
        }
        let ctx = if line.starts_with(' ') { &line[1..] } else { line };
        let ctx_t = ctx.trim_end();
        if ctx_t.is_empty() {
            continue;
        }
        if !orig_lines.iter().any(|l| l.trim_end() == ctx_t) {
            absent.push(ctx);
        }
    }
    absent
}

/// Returns the removed lines (`-` prefix) from `relocated_diff` that do not appear
/// anywhere in `original` (using `.trim_end()` comparison, consistent with
/// `diagnose_missing_context`).  Called only when context lines all match but
/// `diffy::apply` still fails, to name the specific removed lines that differ from
/// the file content.
fn diagnose_non_matching_removed_lines<'a>(relocated_diff: &'a str, original: &str) -> Vec<&'a str> {
    let orig_lines: Vec<&str> = original.lines().collect();
    let mut non_matching = Vec::new();
    for line in relocated_diff.lines() {
        if !line.starts_with('-') {
            continue;
        }
        let content = &line[1..];
        let content_t = content.trim_end();
        if content_t.is_empty() {
            continue;
        }
        if !orig_lines.iter().any(|l| l.trim_end() == content_t) {
            non_matching.push(content);
        }
    }
    non_matching
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

        let diff = normalize_line_endings(diff);
        let original = normalize_line_endings(&original);
        let original = normalize_trailing_whitespace(&original);
        let normalized = normalize_hunk_counts(&diff);
        let relocated = relocate_hunk_positions(&normalized, &original);
        let patch = diffy::Patch::from_str(&relocated)
            .map_err(|e| anyhow::anyhow!(
                "patch_file: failed to parse diff for '{}': {}. \
                 Ensure the diff uses @@ hunk headers with context lines.",
                path, e
            ))?;

        let patched = diffy::apply(&original, &patch)
            .map_err(|e| {
                let absent = diagnose_missing_context(&relocated, &original);
                if absent.is_empty() {
                    let non_matching = diagnose_non_matching_removed_lines(&relocated, &original);
                    if non_matching.is_empty() {
                        anyhow::anyhow!(
                            "patch_file: failed to apply diff to '{}': {}. \
                             All context lines appear in the file — check that the \
                             removed lines (-) exactly match the file content, and that \
                             no duplicate context block is selecting the wrong hunk. \
                             Re-read the file and regenerate the diff.",
                            path, e
                        )
                    } else {
                        anyhow::anyhow!(
                            "patch_file: failed to apply diff to '{}': {}. \
                             Context lines found; removed lines not in file: [{}]. \
                             Re-read the file and ensure the diff's removed lines (-) \
                             match the file content exactly.",
                            path, e,
                            non_matching.join("; ")
                        )
                    }
                } else {
                    anyhow::anyhow!(
                        "patch_file: failed to apply diff to '{}': {}. \
                         Context lines not found in file: [{}]. \
                         Re-read the file and use only lines that appear verbatim.",
                        path, e,
                        absent.join("; ")
                    )
                }
            })?;

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
#[path = "patch_file_tests.rs"]
mod tests;
