use crate::tools::ToolHandler;
use super::BumpVersionTool;

fn setup_test_repo(version: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let working_dir = dir.path().to_path_buf();

    std::process::Command::new("git")
        .args(["init"])
        .current_dir(&working_dir)
        .output()
        .expect("git init");
    std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(&working_dir)
        .output()
        .expect("git config email");
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(&working_dir)
        .output()
        .expect("git config name");

    let cargo_dir = working_dir.join("src/moeb");
    std::fs::create_dir_all(&cargo_dir).expect("create dir");
    let cargo_content = format!(
        "[package]\nname = \"moeb\"\nversion = \"{}\"\n",
        version
    );
    std::fs::write(cargo_dir.join("Cargo.toml"), &cargo_content).expect("write Cargo.toml");

    std::process::Command::new("git")
        .args(["add", "-A"])
        .current_dir(&working_dir)
        .output()
        .expect("git add");
    std::process::Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(&working_dir)
        .output()
        .expect("git commit");

    (dir, working_dir)
}

#[test]
fn bump_patch_increments_patch_only() {
    let (_dir, working_dir) = setup_test_repo("1.2.3");
    let tool = BumpVersionTool;
    let args = serde_json::json!({ "bump_type": "patch" });
    let result = tool.execute(&args, &working_dir).unwrap();
    assert_eq!(result, "1.2.4");
    let content = std::fs::read_to_string(working_dir.join("src/moeb/Cargo.toml")).unwrap();
    assert!(content.contains("version = \"1.2.4\""));
}

#[test]
fn bump_minor_resets_patch() {
    let (_dir, working_dir) = setup_test_repo("1.2.3");
    let tool = BumpVersionTool;
    let args = serde_json::json!({ "bump_type": "minor" });
    let result = tool.execute(&args, &working_dir).unwrap();
    assert_eq!(result, "1.3.0");
    let content = std::fs::read_to_string(working_dir.join("src/moeb/Cargo.toml")).unwrap();
    assert!(content.contains("version = \"1.3.0\""));
}

#[test]
fn bump_major_resets_minor_and_patch() {
    let (_dir, working_dir) = setup_test_repo("1.2.3");
    let tool = BumpVersionTool;
    let args = serde_json::json!({ "bump_type": "major" });
    let result = tool.execute(&args, &working_dir).unwrap();
    assert_eq!(result, "2.0.0");
    let content = std::fs::read_to_string(working_dir.join("src/moeb/Cargo.toml")).unwrap();
    assert!(content.contains("version = \"2.0.0\""));
}

#[test]
fn bump_version_missing_bump_type_returns_error() {
    let (_dir, working_dir) = setup_test_repo("1.0.0");
    let tool = BumpVersionTool;
    let args = serde_json::json!({});
    let result = tool.execute(&args, &working_dir).unwrap();
    assert!(result.contains("Error"), "expected Error, got: {}", result);
    assert!(result.contains("bump_type"), "expected bump_type in message, got: {}", result);
}
