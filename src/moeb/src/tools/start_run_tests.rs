use tempfile::TempDir;

use crate::run_state::new_shared_run_state;
use crate::tools::ToolHandler;
use super::StartRunTool;

fn setup_moeb_dir(dir: &TempDir) {
    let moeb_dir = dir.path().join(".moeb");
    std::fs::create_dir_all(&moeb_dir).unwrap();
    std::fs::write(moeb_dir.join("README.md"), "# README\n").unwrap();
}

fn make_tool() -> StartRunTool {
    StartRunTool { state: new_shared_run_state(), tool_schemas_block: String::new() }
}

#[test]
fn start_run_renders_run_prompt_template() {
    let dir = TempDir::new().unwrap();
    setup_moeb_dir(&dir);

    let spec_content = "---\ndomain: test\nslug: test\nstatus: active\n---\n\n# Title\n\n## Raw Requirement\n\ntest\n";
    std::fs::write(dir.path().join("spec.md"), spec_content).unwrap();

    let tool = make_tool();
    let args = serde_json::json!({ "spec_path": "spec.md" });
    let result = tool.execute(&args, dir.path()).unwrap();

    assert!(!result.contains("{{role_content}}"), "{{role_content}} token must be replaced");
    assert!(!result.contains("{{spec_content}}"), "{{spec_content}} token must be replaced");
    assert!(!result.contains("{{skill_content}}"), "{{skill_content}} token must be replaced");
    assert!(!result.contains("{{command_rubrics}}"), "{{command_rubrics}} token must be replaced");
    assert!(result.contains("# Title"), "rendered prompt must include spec content");
}

#[test]
fn start_run_binary_rubrics_appear_in_output() {
    let dir = TempDir::new().unwrap();
    setup_moeb_dir(&dir);

    let spec_content = "---\ndomain: test\nslug: my-spec\nstatus: active\n---\n\n# Title\n";
    std::fs::write(dir.path().join("spec.md"), spec_content).unwrap();

    let tool = make_tool();
    let args = serde_json::json!({ "spec_path": "spec.md" });
    let result = tool.execute(&args, dir.path()).unwrap();

    // Binary rubrics layers are always present; the rendered output must not show
    // the raw token even if the layers are empty.
    assert!(!result.contains("{{command_rubrics}}"), "rubrics token must not appear in rendered output");
}

#[test]
fn start_run_name_based_discovery_finds_spec() {
    let dir = TempDir::new().unwrap();
    setup_moeb_dir(&dir);

    let specs_dir = dir.path().join(".moeb").join("specifications").join("moeb");
    std::fs::create_dir_all(&specs_dir).unwrap();
    let spec_content = "---\ndomain: moeb\nslug: kernel\nstatus: active\n---\n\n# Kernel\n";
    std::fs::write(specs_dir.join("moeb.kernel.md"), spec_content).unwrap();

    let tool = make_tool();
    let args = serde_json::json!({ "spec_path": "moeb.kernel" });
    let result = tool.execute(&args, dir.path()).unwrap();

    assert!(result.contains("# Kernel"), "discovered spec content must appear in output");
}

#[test]
fn start_run_name_based_discovery_errors_on_no_match() {
    let dir = TempDir::new().unwrap();
    setup_moeb_dir(&dir);

    let specs_dir = dir.path().join(".moeb").join("specifications");
    std::fs::create_dir_all(&specs_dir).unwrap();

    let tool = make_tool();
    let args = serde_json::json!({ "spec_path": "nonexistent.spec" });
    let err = tool.execute(&args, dir.path()).unwrap_err();

    assert!(
        err.to_string().contains("no specification found"),
        "expected no-match error, got: {}",
        err
    );
}

#[test]
fn start_run_errors_on_ambiguous_match() {
    let dir = TempDir::new().unwrap();
    setup_moeb_dir(&dir);

    let specs_dir = dir.path().join(".moeb").join("specifications").join("moeb");
    std::fs::create_dir_all(&specs_dir).unwrap();
    std::fs::write(specs_dir.join("moeb.foo-one.md"), "---\ndomain: moeb\nslug: foo-one\nstatus: active\n---\n# Foo One\n").unwrap();
    std::fs::write(specs_dir.join("moeb.foo-two.md"), "---\ndomain: moeb\nslug: foo-two\nstatus: active\n---\n# Foo Two\n").unwrap();

    let tool = make_tool();
    let args = serde_json::json!({ "spec_path": "moeb.foo" });
    let err = tool.execute(&args, dir.path()).unwrap_err();

    assert!(
        err.to_string().contains("multiple specifications"),
        "expected ambiguous error, got: {}",
        err
    );
}

#[test]
fn start_run_parses_phase_tool_map_from_skill() {
    let dir = TempDir::new().unwrap();
    setup_moeb_dir(&dir);

    // Write a custom (non-protected) skill with a phase annotation
    let skills_dir = dir.path().join(".moeb").join("skills");
    std::fs::create_dir_all(&skills_dir).unwrap();
    std::fs::write(
        skills_dir.join("custom-test.skill.md"),
        "## Phase 5 — Link README <!-- tools: read_file, write_file -->\n\nDo stuff.\n",
    ).unwrap();

    // Spec declares skill: custom-test so the custom skill file is loaded
    let spec_content = "---\ndomain: test\nslug: test\nstatus: active\nskill: custom-test\n---\n\n# T\n";
    std::fs::write(dir.path().join("spec.md"), spec_content).unwrap();

    let state = new_shared_run_state();
    let tool = StartRunTool { state: std::sync::Arc::clone(&state), tool_schemas_block: String::new() };
    let args = serde_json::json!({ "spec_path": "spec.md" });
    tool.execute(&args, dir.path()).unwrap();

    let locked = state.lock().unwrap();
    let allowed = locked.allowed_tools_for_phase("phase-5");
    assert!(allowed.is_some(), "phase-5 must be in the tool map");
    let tools = allowed.unwrap();
    assert!(tools.contains(&"read_file".to_string()));
    assert!(tools.contains(&"write_file".to_string()));
}
