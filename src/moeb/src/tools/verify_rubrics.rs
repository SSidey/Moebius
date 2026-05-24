use std::path::Path;
use anyhow::Result;
use serde_json::json;

use crate::adapters::ToolDef;
use crate::run_state::{RubricStatus, RubricVerification, SharedRunState};
use super::ToolHandler;

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

        let agent_criteria: Vec<AgentCriterion> = criteria_json
            .iter()
            .filter_map(|c| serde_json::from_value(c.clone()).ok())
            .collect();

        let unreviewed: Vec<String> = {
            let state = self.state.lock().unwrap();
            state.pending_reviews.iter().cloned().collect()
        };

        let tool_errors: Vec<crate::run_state::ToolError> = {
            let state = self.state.lock().unwrap();
            state.tool_errors.clone()
        };

        let mut passes = 0usize;
        let mut fails = 0usize;
        let mut nas = 0usize;

        let mut verifications: Vec<RubricVerification> = agent_criteria
            .iter()
            .filter(|c| !matches!(
                c.name.as_str(),
                "write-review-compliance" | "tool-errors" | "rubric-qualification"
            ))
            .map(|c| {
                let status = match c.status.as_str() {
                    "pass" => { passes += 1; RubricStatus::Pass }
                    "fail" => { fails += 1; RubricStatus::Fail }
                    _ => { nas += 1; RubricStatus::Na }
                };
                RubricVerification { name: c.name.clone(), status, note: c.note.clone() }
            })
            .collect();

        if unreviewed.is_empty() {
            passes += 1;
            verifications.push(RubricVerification {
                name: "write-review-compliance".to_string(),
                status: RubricStatus::Pass,
                note: Some("All writes acknowledged".to_string()),
            });
        } else {
            fails += 1;
            verifications.push(RubricVerification {
                name: "write-review-compliance".to_string(),
                status: RubricStatus::Fail,
                note: Some(format!("Unreviewed writes: {}", unreviewed.join(", "))),
            });
        }

        if tool_errors.is_empty() {
            passes += 1;
            verifications.push(RubricVerification {
                name: "tool-errors".to_string(),
                status: RubricStatus::Pass,
                note: Some("No tool errors recorded".to_string()),
            });
        } else {
            fails += 1;
            verifications.push(RubricVerification {
                name: "tool-errors".to_string(),
                status: RubricStatus::Fail,
                note: Some(format!(
                    "Tool errors recorded: {}",
                    serde_json::to_string(&tool_errors).unwrap_or_default()
                )),
            });
        }

        let qualified_passes: Vec<(String, Vec<String>)> = agent_criteria
            .iter()
            .filter(|c| c.status == "pass" && !c.acknowledged_failures.is_empty())
            .map(|c| (c.name.clone(), c.acknowledged_failures.clone()))
            .collect();

        if qualified_passes.is_empty() {
            passes += 1;
            verifications.push(RubricVerification {
                name: "rubric-qualification".to_string(),
                status: RubricStatus::Pass,
                note: Some("No qualified Pass verdicts recorded".to_string()),
            });
        } else {
            fails += 1;
            verifications.push(RubricVerification {
                name: "rubric-qualification".to_string(),
                status: RubricStatus::Fail,
                note: Some(format!(
                    "Pass verdicts with acknowledged failures: {}",
                    serde_json::to_string(&qualified_passes).unwrap_or_default()
                )),
            });
        }

        self.state.lock().unwrap().rubric_verifications = verifications;

        let mut result = format!("Rubric verified: {} pass, {} fail, {} na.", passes, fails, nas);
        if fails > 0 {
            result.push_str(&format!(" WARNING: {} criterion/criteria failed.", fails));
        }
        Ok(result)
    }
}
