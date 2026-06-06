use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use chrono::Utc;
use serde_json::{json, Value};

use crate::adapters::ToolDef;
use super::ToolHandler;

pub struct ArchiveSignalTool;

impl ToolHandler for ArchiveSignalTool {
    fn name(&self) -> &'static str {
        "archive_signal"
    }

    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "archive_signal",
            description: "Move a resolved or superseded signal from the active catalogue to \
                the signal archive. Performs an integrity check (index.md data-row count must \
                equal catalogue JSON file count) before proceeding. Sets archived_on and \
                optionally resolution_summary on the record.",
            parameters: json!({
                "type": "object",
                "required": ["signal_id"],
                "properties": {
                    "signal_id": {
                        "type": "string",
                        "description": "UUID of the signal to archive. File must exist at \
                            .moeb/signals/catalogue/<signal_id>.signal.json."
                    },
                    "resolution_summary": {
                        "type": "string",
                        "description": "Optional human-readable explanation of how the signal \
                            was resolved. Stored on the archived record."
                    }
                }
            }),
        }
    }

    fn execute(&self, args: &Value, working_dir: &Path) -> Result<String> {
        let signal_id = args["signal_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("archive_signal: 'signal_id' is required"))?;
        let resolution_summary = args["resolution_summary"].as_str();

        let catalogue_dir = working_dir.join(".moeb/signals/catalogue");
        let index_path = working_dir.join(".moeb/signals/index.md");
        let archive_catalogue_dir = working_dir.join(".moeb/signals/archive/catalogue");
        let archive_index_path = working_dir.join(".moeb/signals/archive/index.md");

        let signal_file = catalogue_dir.join(format!("{}.signal.json", signal_id));
        let raw = fs::read_to_string(&signal_file)
            .with_context(|| format!("archive_signal: signal not found: {}", signal_id))?;
        let mut record: Value = serde_json::from_str(&raw)
            .with_context(|| format!("archive_signal: invalid JSON in {}", signal_id))?;

        let status = record["status"].as_str().unwrap_or("").to_string();
        if status != "resolved" && status != "superseded" {
            bail!(
                "archive_signal: cannot archive '{}' — status is '{}' (must be 'resolved' or 'superseded')",
                signal_id,
                status
            );
        }

        let index_content = fs::read_to_string(&index_path)
            .with_context(|| format!("archive_signal: cannot read {}", index_path.display()))?;
        let index_rows = count_data_rows(&index_content);
        let catalogue_files = count_signal_files(&catalogue_dir)?;
        if index_rows != catalogue_files {
            bail!(
                "archive_signal: integrity check failed — index.md has {} data rows but catalogue has {} signal files",
                index_rows,
                catalogue_files
            );
        }

        record["status"] = Value::String("archived".to_string());
        record["archived_on"] = Value::String(Utc::now().to_rfc3339());
        if let Some(s) = resolution_summary {
            if !s.is_empty() {
                record["resolution_summary"] = Value::String(s.to_string());
            }
        }

        fs::create_dir_all(&archive_catalogue_dir)?;
        let archive_file = archive_catalogue_dir.join(format!("{}.signal.json", signal_id));
        fs::write(&archive_file, serde_json::to_string_pretty(&record)?)
            .with_context(|| format!("archive_signal: cannot write archive file for {}", signal_id))?;

        fs::remove_file(&signal_file)
            .with_context(|| format!("archive_signal: cannot delete active file for {}", signal_id))?;

        let updated_index = remove_index_row(&index_content, signal_id);
        fs::write(&index_path, updated_index)
            .with_context(|| "archive_signal: cannot update index.md")?;

        let row = extract_index_row(&index_content, signal_id);
        append_to_archive_index(&archive_index_path, &row)?;

        Ok(format!("Archived: {} (was {})", signal_id, status))
    }
}

fn count_data_rows(content: &str) -> usize {
    content
        .lines()
        .filter(|l| l.starts_with('|') && !l.contains("---"))
        .count()
        .saturating_sub(1)
}

fn count_signal_files(dir: &Path) -> Result<usize> {
    if !dir.exists() {
        return Ok(0);
    }
    let count = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s == "json")
                .unwrap_or(false)
        })
        .count();
    Ok(count)
}

fn remove_index_row(content: &str, signal_id: &str) -> String {
    let mut lines: Vec<&str> = content
        .lines()
        .filter(|l| !l.contains(signal_id))
        .collect();
    while lines.last().map(|l| l.is_empty()).unwrap_or(false) {
        lines.pop();
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

fn extract_index_row(content: &str, signal_id: &str) -> String {
    content
        .lines()
        .find(|l| l.contains(signal_id))
        .unwrap_or("")
        .to_string()
}

fn append_to_archive_index(path: &Path, row: &str) -> Result<()> {
    if row.is_empty() {
        return Ok(());
    }
    if path.exists() {
        let mut existing = fs::read_to_string(path)?;
        if !existing.ends_with('\n') {
            existing.push('\n');
        }
        existing.push_str(row);
        existing.push('\n');
        fs::write(path, existing)?;
    } else {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let header = "| Signal ID | Title | Category | Source | Severity | Status | Count | Last Seen | File |\n\
                      |-----------|-------|----------|--------|----------|--------|-------|-----------|------|\n";
        fs::write(path, format!("{}{}\n", header, row))?;
    }
    Ok(())
}
