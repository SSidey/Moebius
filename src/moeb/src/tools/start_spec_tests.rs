use tempfile::TempDir;

use crate::tools::ToolHandler;
use super::StartSpecTool;

fn setup_moeb_dir(dir: &TempDir) {
    let moeb_dir = dir.path().join(".moeb");
    std::fs::create_dir_all(&moeb_dir).unwrap();
    std::fs::write(moeb_dir.join("README.md"), "# README\n").unwrap();
}

#[test]
fn start_spec_returns_rendered_prompt_with_requirement() {
    let dir = TempDir::new().unwrap();
    setup_moeb_dir(&dir);

    let tool = StartSpecTool { tool_schemas_block: String::new() };
    let args = serde_json::json!({ "requirement": "Add token rotation feature" });
    let result = tool.execute(&args, dir.path()).unwrap();

    assert!(
        result.contains("Add token rotation feature"),
        "rendered prompt must contain the requirement text"
    );
    assert!(
        !result.contains("{{input}}"),
        "{{input}} token must be replaced"
    );
    assert!(
        !result.contains("{{command_rubrics}}"),
        "{{command_rubrics}} token must be replaced"
    );
}

#[test]
fn start_spec_binary_rubrics_appear_in_output() {
    let dir = TempDir::new().unwrap();
    setup_moeb_dir(&dir);

    let tool = StartSpecTool { tool_schemas_block: String::new() };
    let args = serde_json::json!({ "requirement": "Some requirement" });
    let result = tool.execute(&args, dir.path()).unwrap();

    assert!(
        !result.contains("{{command_rubrics}}"),
        "rubrics token must not appear in rendered output"
    );
    assert!(
        !result.contains("{{skill_content}}"),
        "skill token must not appear in rendered output"
    );
}

#[test]
fn start_spec_missing_requirement_returns_error() {
    let dir = TempDir::new().unwrap();
    setup_moeb_dir(&dir);

    let tool = StartSpecTool { tool_schemas_block: String::new() };
    let args = serde_json::json!({});
    let err = tool.execute(&args, dir.path()).unwrap_err();

    assert!(
        err.to_string().contains("requirement"),
        "expected requirement error, got: {}",
        err
    );
}

#[test]
fn test_start_spec_signal_id_optional() {
    let tool = StartSpecTool { tool_schemas_block: String::new() };
    let def = tool.definition();
    let required = def.parameters["required"].as_array()
        .expect("required must be a JSON array");
    assert!(
        !required.iter().any(|v| v.as_str() == Some("requirement")),
        "requirement must not be in required array"
    );
    assert!(
        !required.iter().any(|v| v.as_str() == Some("signal_id")),
        "signal_id must not be in required array"
    );
}

#[test]
fn test_resolve_requirement_uses_requirement_when_present() {
    let dir = TempDir::new().unwrap();
    let result = super::resolve_requirement(dir.path(), "my requirement", "some-signal-id")
        .expect("should return Ok when requirement is non-empty");
    assert_eq!(result, "my requirement");
}

#[test]
fn test_resolve_requirement_errors_on_both_empty() {
    let dir = TempDir::new().unwrap();
    let err = super::resolve_requirement(dir.path(), "", "")
        .expect_err("should return Err when both requirement and signal_id are empty");
    assert!(
        err.contains("requirement"),
        "error message must mention 'requirement', got: {}",
        err
    );
}

#[test]
fn test_resolve_requirement_finds_signal_via_index() {
    let dir = TempDir::new().unwrap();
    let uuid = "a1b2c3d4-e5f6-7890-abcd-ef1234567890";
    let catalogue_dir = dir.path().join(".moeb/signals/catalogue");
    std::fs::create_dir_all(&catalogue_dir).unwrap();
    let signal_json = serde_json::json!({
        "signal_id": uuid,
        "proposed_resolution": "Run moeb spec to add XYZ feature",
        "title": "Missing XYZ feature",
        "description": "The XYZ feature is not implemented"
    }).to_string();
    std::fs::write(catalogue_dir.join(format!("{}.signal.json", uuid)), &signal_json).unwrap();

    let index_content = format!(
        "# Signal Index\n\n\
         | Signal ID | Title | Category | Severity | Status | Occurrences | Last Seen | File |\n\
         |-----------|-------|----------|----------|--------|-------------|-----------|------|\n\
         | {uuid} | Missing XYZ feature | NewCapability | Major | open | 1 | 2026-01-01T00:00:00Z | \
         [catalogue/{uuid}.signal.json](catalogue/{uuid}.signal.json) |\n",
        uuid = uuid
    );
    let signals_dir = dir.path().join(".moeb/signals");
    std::fs::create_dir_all(&signals_dir).unwrap();
    std::fs::write(signals_dir.join("index.md"), &index_content).unwrap();

    let result = super::resolve_requirement(dir.path(), "", uuid)
        .expect("should resolve signal via index");
    assert_eq!(result, "Run moeb spec to add XYZ feature");
}

#[test]
fn test_resolve_requirement_finds_signal_via_direct_filename() {
    let dir = TempDir::new().unwrap();
    let uuid = "b2c3d4e5-f6a7-8901-bcde-f01234567891";
    let catalogue_dir = dir.path().join(".moeb/signals/catalogue");
    std::fs::create_dir_all(&catalogue_dir).unwrap();
    let signal_json = serde_json::json!({
        "signal_id": uuid,
        "proposed_resolution": "Add delete-file capability to moeb",
        "title": "Missing delete tool",
        "description": "No moeb tool for file deletion"
    }).to_string();
    std::fs::write(catalogue_dir.join(format!("{}.signal.json", uuid)), &signal_json).unwrap();

    let result = super::resolve_requirement(dir.path(), "", uuid)
        .expect("should resolve signal via direct filename");
    assert_eq!(result, "Add delete-file capability to moeb");
}

#[test]
fn test_resolve_requirement_title_description_fallback() {
    let dir = TempDir::new().unwrap();
    let uuid = "c3d4e5f6-a7b8-9012-cdef-012345678912";
    let catalogue_dir = dir.path().join(".moeb/signals/catalogue");
    std::fs::create_dir_all(&catalogue_dir).unwrap();
    let signal_json = serde_json::json!({
        "signal_id": uuid,
        "proposed_resolution": "",
        "title": "Missing tool",
        "description": "No moeb equivalent"
    }).to_string();
    std::fs::write(catalogue_dir.join(format!("{}.signal.json", uuid)), &signal_json).unwrap();

    let result = super::resolve_requirement(dir.path(), "", uuid)
        .expect("should fall back to title + description");
    assert_eq!(result, "Missing tool. No moeb equivalent");
}
