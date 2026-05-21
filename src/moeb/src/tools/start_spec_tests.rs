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

    let tool = StartSpecTool;
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

    let tool = StartSpecTool;
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

    let tool = StartSpecTool;
    let args = serde_json::json!({});
    let err = tool.execute(&args, dir.path()).unwrap_err();

    assert!(
        err.to_string().contains("requirement"),
        "expected requirement error, got: {}",
        err
    );
}
