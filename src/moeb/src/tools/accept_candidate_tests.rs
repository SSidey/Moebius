use crate::tools::ToolHandler;
use super::AcceptCandidateTool;

#[test]
fn test_accept_candidate_name() {
    assert_eq!(AcceptCandidateTool.name(), "accept_candidate");
}

#[test]
fn test_accept_candidate_definition_requires_signal_id() {
    let def = AcceptCandidateTool.definition();
    let required = def.parameters["required"]
        .as_array()
        .expect("required must be an array");
    let has_signal_id = required.iter().any(|v| v.as_str() == Some("signal_id"));
    assert!(has_signal_id, "signal_id must be in required");
}
