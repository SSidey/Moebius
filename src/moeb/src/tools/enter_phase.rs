use std::path::Path;
use anyhow::Result;
use serde_json::json;

use crate::adapters::ToolDef;
use crate::run_state::SharedRunState;
use super::ToolHandler;

pub struct EnterPhaseTool {
    pub state: SharedRunState,
}

impl ToolHandler for EnterPhaseTool {
    fn name(&self) -> &'static str {
        "enter_phase"
    }

    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "enter_phase",
            description: "Declare the start of a skill phase. If the skill annotates this \
                phase with a tool restriction, all subsequent tool calls not in the allowed \
                set will return a rejection error until enter_phase is called for another \
                phase. Call this as the first action at the start of every phase in the skill.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "phase_id": {
                        "type": "string",
                        "description": "The identifier of the phase being entered, e.g. \
                            'phase-5'. Normalised to lowercase with spaces replaced by hyphens."
                    }
                },
                "required": ["phase_id"]
            }),
        }
    }

    fn execute(&self, args: &serde_json::Value, _working_dir: &Path) -> Result<String> {
        let raw_phase_id = args["phase_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("enter_phase: 'phase_id' must be a string"))?;

        let phase_id = raw_phase_id.to_lowercase().replace(' ', "-");

        let mut state = self.state.lock().unwrap();
        state.set_current_phase(Some(phase_id.clone()));
        let allowed = state.allowed_tools_for_phase(&phase_id).cloned();
        drop(state);

        match allowed {
            Some(tools) => Ok(format!(
                "enter_phase: now in '{}'. Restricted to: [{}].",
                phase_id,
                tools.join(", ")
            )),
            None => Ok(format!(
                "enter_phase: now in '{}'. No tool restrictions.",
                phase_id
            )),
        }
    }
}

#[cfg(test)]
#[path = "enter_phase_tests.rs"]
mod tests;
