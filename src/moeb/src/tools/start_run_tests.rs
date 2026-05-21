use tempfile::TempDir;

use crate::tools::ToolHandler;
use super::StartRunTool;

#[test]
fn start_run_returns_context_sections() {
    let dir = TempDir::new().unwrap();

    let spec_content = "---\ndomain: test\nslug: test\nstatus: active\n---\n\n# Title\n\n## Raw Requirement\n\ntest\n";
    std::fs::write(dir.path().join("spec.md"), spec_content).unwrap();

    let moeb_dir = dir.path().join(".moeb");
    std::fs::create_dir_all(&moeb_dir).unwrap();
    std::fs::write(moeb_dir.join("README.md"), "# README\n").unwrap();

    let tool = StartRunTool;
    let args = serde_json::json!({ "spec_path": "spec.md" });
    let result = tool.execute(&args, dir.path()).unwrap();

    assert!(result.contains("## Role"), "missing ## Role section");
    assert!(result.contains("## Skill"), "missing ## Skill section");
    assert!(result.contains("## Specification"), "missing ## Specification section");
    assert!(result.contains("## Rubrics"), "missing ## Rubrics section");
}

#[test]
fn start_run_uses_frontmatter_role_and_skill() {
    let dir = TempDir::new().unwrap();

    let spec_content = "---\ndomain: test\nslug: test\nstatus: active\nrole: custom\nskill: custom\n---\n\n# Title\n";
    std::fs::write(dir.path().join("spec.md"), spec_content).unwrap();

    let moeb_dir = dir.path().join(".moeb");
    std::fs::create_dir_all(&moeb_dir).unwrap();

    let tool = StartRunTool;
    let args = serde_json::json!({ "spec_path": "spec.md" });
    let result = tool.execute(&args, dir.path()).unwrap();

    assert!(
        result.contains("custom"),
        "expected 'custom' in output for missing role/skill fallback, got: {}",
        result
    );
}
