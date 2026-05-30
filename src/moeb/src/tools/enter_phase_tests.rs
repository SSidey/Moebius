use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crate::run_state::new_shared_run_state;
use crate::tools::ToolHandler;
use super::EnterPhaseTool;

#[test]
fn enter_phase_annotated_returns_restricted_message() {
    let state = new_shared_run_state();
    {
        let mut s = state.lock().unwrap();
        let mut map = HashMap::new();
        map.insert(
            "phase-5".to_string(),
            vec!["read_file".to_string(), "write_file".to_string()],
        );
        s.set_phase_tool_map(map);
    }

    let tool = EnterPhaseTool { state: Arc::clone(&state) };
    let args = serde_json::json!({ "phase_id": "phase-5" });
    let result = tool.execute(&args, Path::new(".")).unwrap();

    assert!(
        result.contains("phase-5"),
        "result must mention the phase id; got: {}",
        result
    );
    assert!(
        result.contains("read_file"),
        "result must list read_file in allowed tools; got: {}",
        result
    );
    assert!(
        result.contains("write_file"),
        "result must list write_file in allowed tools; got: {}",
        result
    );
    assert!(
        result.contains("Restricted to:"),
        "result must indicate restriction; got: {}",
        result
    );

    let phase = state.lock().unwrap().current_phase.clone();
    assert_eq!(phase, Some("phase-5".to_string()), "current_phase must be set after enter_phase");
}

#[test]
fn enter_phase_unannotated_returns_unrestricted_message() {
    let state = new_shared_run_state();
    let tool = EnterPhaseTool { state: Arc::clone(&state) };
    let args = serde_json::json!({ "phase_id": "phase-1" });
    let result = tool.execute(&args, Path::new(".")).unwrap();

    assert!(
        result.contains("phase-1"),
        "result must mention the phase id; got: {}",
        result
    );
    assert!(
        result.contains("No tool restrictions"),
        "unannotated phase must report no restrictions; got: {}",
        result
    );

    let phase = state.lock().unwrap().current_phase.clone();
    assert_eq!(phase, Some("phase-1".to_string()), "current_phase must be set after enter_phase");
}

#[test]
fn enter_phase_normalises_spaces_to_hyphens() {
    let state = new_shared_run_state();
    let tool = EnterPhaseTool { state: Arc::clone(&state) };
    let args = serde_json::json!({ "phase_id": "Phase 3" });
    tool.execute(&args, Path::new(".")).unwrap();

    let phase = state.lock().unwrap().current_phase.clone();
    assert_eq!(phase, Some("phase 3".to_string()).map(|s| s.replace(' ', "-")));
    assert_eq!(
        state.lock().unwrap().current_phase,
        Some("phase-3".to_string()),
        "spaces must be replaced with hyphens and phase id lowercased"
    );
}

#[test]
fn enter_phase_clears_restriction_for_unannotated_phase() {
    let state = new_shared_run_state();
    {
        let mut s = state.lock().unwrap();
        let mut map = HashMap::new();
        map.insert("phase-5".to_string(), vec!["read_file".to_string()]);
        s.set_phase_tool_map(map);
        s.set_current_phase(Some("phase-5".to_string()));
    }

    let tool = EnterPhaseTool { state: Arc::clone(&state) };
    let args = serde_json::json!({ "phase_id": "phase-6" });
    let result = tool.execute(&args, Path::new(".")).unwrap();

    assert!(
        result.contains("No tool restrictions"),
        "entering unannotated phase must report no restrictions; got: {}",
        result
    );
    let phase = state.lock().unwrap().current_phase.clone();
    assert_eq!(phase, Some("phase-6".to_string()));
}
