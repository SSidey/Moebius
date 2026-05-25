use std::fs;
use std::path::Path;
use anyhow::Result;
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use crate::adapters::ToolDef;
use crate::domain::review::{ReviewCategory, ReviewSeverity, ReviewSignal};
use crate::run_state::{RubricStatus, RubricVerification, SharedRunState};
use super::ToolHandler;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KernelOverride {
    pub name: String,
    pub agent_status: String,
    pub forced_status: String,
    pub reason: String,
}

#[derive(serde::Deserialize)]
struct AgentCriterion {
    name: String,
    status: String,
    note: Option<String>,
    #[serde(default)]
    acknowledged_failures: Vec<String>,
}

pub struct VerifyRubricsTool {
    pub state: SharedRunState,
}

impl ToolHandler for VerifyRubricsTool {
    fn name(&self) -> &'static str {
        "verify_rubrics"
    }

    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "verify_rubrics",
            description: "Record the pass/fail/na verdict for each structured rubric criterion in the specification's ## Rubric section. Call this as your final tool invocation before your completion summary.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "criteria": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "name": { "type": "string", "description": "Criterion name from the rubric table." },
                                "status": { "type": "string", "enum": ["pass", "fail", "na"] },
                                "note": { "type": "string", "description": "Optional explanation, required when status is fail." },
                                "acknowledged_failures": {
                                    "type": "array",
                                    "items": { "type": "string" },
                                    "description": "Failures observed but judged acceptable for a Pass verdict. Populate this when issuing Pass for a criterion where failures were observed — the kernel uses it to inject the rubric-qualification verdict."
                                }
                            },
                            "required": ["name", "status"]
                        }
                    }
                },
                "required": ["criteria"]
            }),
        }
    }

    fn execute(&self, args: &serde_json::Value, _working_dir: &Path) -> Result<String> {
        let criteria_json = args["criteria"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("verify_rubrics: 'criteria' must be an array"))?;

        let mut agent_criteria: Vec<AgentCriterion> = criteria_json
            .iter()
            .filter_map(|c| serde_json::from_value(c.clone()).ok())
            .collect();

        // Step 1: Force acknowledged_failures + pass → fail at kernel level
        let mut kernel_overrides: Vec<KernelOverride> = Vec::new();
        for criterion in agent_criteria.iter_mut() {
            if !criterion.acknowledged_failures.is_empty() && criterion.status == "pass" {
                kernel_overrides.push(KernelOverride {
                    name: criterion.name.clone(),
                    agent_status: "pass".to_string(),
                    forced_status: "fail".to_string(),
                    reason: format!(
                        "acknowledged_failures is non-empty; a criterion with documented \
                         failures cannot be recorded as pass. Failures: {:?}",
                        criterion.acknowledged_failures
                    ),
                });
                criterion.status = "fail".to_string();
            }
        }

        let (unreviewed, tool_errors, run_id) = {
            let state = self.state.lock().unwrap();
            (
                state.pending_reviews.iter().cloned().collect::<Vec<_>>(),
                state.tool_errors.clone(),
                state.run_id.clone(),
            )
        };

        let agent_supplied_snapshot: Vec<serde_json::Value> = agent_criteria.iter()
            .map(|c| json!({ "name": c.name, "status": c.status, "acknowledged_failures": c.acknowledged_failures }))
            .collect();

        let mut verifications: Vec<RubricVerification> = agent_criteria
            .iter()
            .filter(|c| !matches!(c.name.as_str(), "write-review-compliance" | "tool-errors" | "rubric-qualification"))
            .map(|c| RubricVerification {
                name: c.name.clone(),
                status: match c.status.as_str() {
                    "pass" => RubricStatus::Pass,
                    "fail" => RubricStatus::Fail,
                    _ => RubricStatus::Na,
                },
                note: c.note.clone(),
            })
            .collect();

        verifications.push(if unreviewed.is_empty() {
            RubricVerification { name: "write-review-compliance".to_string(), status: RubricStatus::Pass, note: Some("All writes acknowledged".to_string()) }
        } else {
            RubricVerification { name: "write-review-compliance".to_string(), status: RubricStatus::Fail, note: Some(format!("Unreviewed writes: {}", unreviewed.join(", "))) }
        });

        verifications.push(if tool_errors.is_empty() {
            RubricVerification { name: "tool-errors".to_string(), status: RubricStatus::Pass, note: Some("No tool errors recorded".to_string()) }
        } else {
            RubricVerification { name: "tool-errors".to_string(), status: RubricStatus::Fail, note: Some(format!("Tool errors recorded: {}", serde_json::to_string(&tool_errors).unwrap_or_default())) }
        });

        let qualified_passes: Vec<(String, Vec<String>)> = agent_criteria
            .iter()
            .filter(|c| c.status == "pass" && !c.acknowledged_failures.is_empty())
            .map(|c| (c.name.clone(), c.acknowledged_failures.clone()))
            .collect();

        verifications.push(if qualified_passes.is_empty() {
            RubricVerification { name: "rubric-qualification".to_string(), status: RubricStatus::Pass, note: Some("No qualified Pass verdicts recorded".to_string()) }
        } else {
            RubricVerification { name: "rubric-qualification".to_string(), status: RubricStatus::Fail, note: Some(format!("Pass verdicts with acknowledged failures: {}", serde_json::to_string(&qualified_passes).unwrap_or_default())) }
        });

        // Step 2: Kernel-write signals for every fail verdict before returning to agent
        let overridden_names: Vec<&str> = kernel_overrides.iter().map(|o| o.name.as_str()).collect();
        let mut signal_ids: Vec<String> = Vec::new();
        if !run_id.is_empty() {
            let signals_path = format!(".moeb/signals/{}.signals.json", run_id);
            for v in verifications.iter().filter(|v| v.status == RubricStatus::Fail) {
                let severity = if overridden_names.contains(&v.name.as_str()) {
                    ReviewSeverity::Critical
                } else {
                    ReviewSeverity::Major
                };
                let description = if let Some(ko) = kernel_overrides.iter().find(|o| o.name == v.name) {
                    format!("Criterion '{}' verdict: fail (kernel override). {}", v.name, ko.reason)
                } else {
                    format!("Criterion '{}' verdict: fail. {}", v.name, v.note.as_deref().unwrap_or("Agent reported fail directly."))
                };
                let sid = Uuid::new_v4().to_string();
                let signal = ReviewSignal {
                    signal_id: sid.clone(),
                    run_id: run_id.clone(),
                    timestamp: Utc::now().to_rfc3339(),
                    category: ReviewCategory::Error,
                    severity,
                    title: format!("Rubric criterion failed: {}", v.name),
                    description,
                    proposed_resolution: Some(format!(
                        "Investigate and resolve the failure in criterion '{}' in a follow-on invocation.", v.name
                    )),
                    auto_spec_path: None,
                    gating_condition: None,
                };
                match append_signal_to_file(&signals_path, &signal) {
                    Ok(()) => signal_ids.push(sid),
                    Err(e) => eprintln!("moeb: warn: failed to write signal: {}", e),
                }
            }
        }

        // Step 3: Compute kernel-authoritative rubric_score and end_review_error_count
        let total = verifications.len();
        let passing = verifications.iter().filter(|v| v.status == RubricStatus::Pass).count();
        let fail_count = verifications.iter().filter(|v| v.status == RubricStatus::Fail).count();
        let na_count = total - passing - fail_count;
        let rubric_score: f32 = if total == 0 { 1.0 } else { passing as f32 / total as f32 };
        let end_review_error_count = fail_count as u32;

        {
            let mut state = self.state.lock().unwrap();
            state.rubric_verifications = verifications.clone();
            state.rubric_score = Some(rubric_score);
            state.end_review_error_count = Some(end_review_error_count);
        }

        // Step 4: Write tamper-evident audit file
        if !run_id.is_empty() {
            let audit_path = format!(".moeb/rubric-audit/{}.audit.json", run_id);
            let final_verdicts: Vec<serde_json::Value> = verifications.iter()
                .map(|v| json!({ "name": v.name, "status": match v.status {
                    RubricStatus::Pass => "pass", RubricStatus::Fail => "fail", RubricStatus::Na => "na"
                }}))
                .collect();
            let audit = json!({
                "run_id": run_id,
                "timestamp": Utc::now().to_rfc3339(),
                "agent_supplied": agent_supplied_snapshot,
                "kernel_overrides": serde_json::to_value(&kernel_overrides).unwrap_or_default(),
                "final_verdicts": final_verdicts,
                "rubric_score": rubric_score,
                "end_review_error_count": end_review_error_count,
                "signals_written": signal_ids,
            });
            if let Err(e) = write_audit_file(&audit_path, &audit) {
                eprintln!("moeb: warn: failed to write rubric audit: {}", e);
            }
        }

        let mut result = format!("Rubric verified: {} pass, {} fail, {} na.", passing, fail_count, na_count);
        if fail_count > 0 {
            result.push_str(&format!(" WARNING: {} criterion/criteria failed.", fail_count));
        }
        if !kernel_overrides.is_empty() {
            result.push_str(&format!(
                " KERNEL OVERRIDES: {} criterion/criteria forced from pass to fail.",
                kernel_overrides.len()
            ));
        }
        Ok(result)
    }
}

fn append_signal_to_file(path: &str, signal: &ReviewSignal) -> anyhow::Result<()> {
    let signal_val = serde_json::to_value(signal)?;
    let mut signals: Vec<serde_json::Value> = if Path::new(path).exists() {
        fs::read_to_string(path).ok()
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or_else(|| {
                eprintln!("moeb: warn: signals file malformed, overwriting");
                Vec::new()
            })
    } else {
        if let Some(parent) = Path::new(path).parent() { let _ = fs::create_dir_all(parent); }
        Vec::new()
    };
    signals.push(signal_val);
    fs::write(path, serde_json::to_string_pretty(&signals)?)?;
    Ok(())
}

fn write_audit_file(path: &str, audit: &serde_json::Value) -> anyhow::Result<()> {
    let p = Path::new(path);
    if let Some(parent) = p.parent() { let _ = fs::create_dir_all(parent); }
    if p.exists() {
        if let Ok(content) = fs::read_to_string(p) {
            if let Ok(mut existing) = serde_json::from_str::<serde_json::Value>(&content) {
                for field in &["rubric_score", "end_review_error_count", "final_verdicts", "timestamp"] {
                    existing[*field] = audit[*field].clone();
                }
                if let (Some(arr), Some(new)) = (existing["kernel_overrides"].as_array_mut(), audit["kernel_overrides"].as_array()) {
                    arr.extend(new.iter().cloned());
                }
                if let (Some(arr), Some(new)) = (existing["signals_written"].as_array_mut(), audit["signals_written"].as_array()) {
                    arr.extend(new.iter().cloned());
                }
                fs::write(p, serde_json::to_string_pretty(&existing)?)?;
                return Ok(());
            }
        }
    }
    fs::write(p, serde_json::to_string_pretty(audit)?)?;
    Ok(())
}
