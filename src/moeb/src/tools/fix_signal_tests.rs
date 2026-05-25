use crate::tools::ToolHandler;
use super::FixSignalTool;

#[test]
fn test_fix_signal_name() {
    assert_eq!(FixSignalTool.name(), "fix_signal");
}

#[test]
fn test_fix_signal_definition_no_required_params() {
    let def = FixSignalTool.definition();
    let required = def.parameters.get("required");
    let is_empty = match required {
        None => true,
        Some(v) => v.as_array().map(|a| a.is_empty()).unwrap_or(true),
    };
    assert!(is_empty, "fix_signal definition must have no required parameters");
}
