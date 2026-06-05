use crate::tools::ToolHandler;
use super::PatchFileTool;
use tempfile::TempDir;

fn temp_dir() -> TempDir {
    tempfile::tempdir().unwrap()
}

#[test]
fn patch_file_basic_replacement() {
    let dir = temp_dir();
    let file = dir.path().join("target.rs");
    std::fs::write(&file, "fn foo() {\n    let x = 1;\n    x\n}\n").unwrap();

    let tool = PatchFileTool;
    let args = serde_json::json!({
        "path": "target.rs",
        "old_string": "    let x = 1;",
        "new_string": "    let x = 42;"
    });
    let result = tool.execute(&args, dir.path()).unwrap();

    assert!(result.contains("applied"), "expected success: {}", result);
    let content = std::fs::read_to_string(&file).unwrap();
    assert!(content.contains("let x = 42;"), "new content must be present");
    assert!(!content.contains("let x = 1;"), "old content must be removed");
}

#[test]
fn patch_file_multi_line_replacement() {
    let dir = temp_dir();
    let file = dir.path().join("target.rs");
    std::fs::write(&file, "fn old() {\n    x\n}\n\nfn bar() {}\n").unwrap();

    let tool = PatchFileTool;
    let args = serde_json::json!({
        "path": "target.rs",
        "old_string": "fn old() {\n    x\n}",
        "new_string": "fn new() {\n    y\n}"
    });
    let result = tool.execute(&args, dir.path()).unwrap();

    assert!(result.contains("applied"), "expected success: {}", result);
    let content = std::fs::read_to_string(&file).unwrap();
    assert!(content.contains("fn new() {"), "replacement must be present");
    assert!(content.contains("    y"), "replacement body must be present");
    assert!(!content.contains("fn old()"), "old function must be removed");
    assert!(content.contains("fn bar()"), "unrelated function must be preserved");
}

#[test]
fn patch_file_not_found_returns_error() {
    let dir = temp_dir();
    let file = dir.path().join("target.rs");
    std::fs::write(&file, "fn foo() {}\n").unwrap();

    let tool = PatchFileTool;
    let args = serde_json::json!({
        "path": "target.rs",
        "old_string": "fn missing() {}",
        "new_string": "fn new() {}"
    });
    let err = tool.execute(&args, dir.path()).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("not found"), "error must say not found: {}", msg);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "fn foo() {}\n", "file must be unchanged");
}

#[test]
fn patch_file_ambiguous_match_returns_error_with_count() {
    let dir = temp_dir();
    let file = dir.path().join("target.rs");
    std::fs::write(&file, "let x = 1;\nlet x = 1;\n").unwrap();

    let tool = PatchFileTool;
    let args = serde_json::json!({
        "path": "target.rs",
        "old_string": "let x = 1;",
        "new_string": "let x = 42;"
    });
    let err = tool.execute(&args, dir.path()).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains('2'), "error must contain the occurrence count: {}", msg);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "let x = 1;\nlet x = 1;\n", "file must be unchanged");
}

#[test]
fn patch_file_returns_error_on_missing_file() {
    let dir = temp_dir();
    let tool = PatchFileTool;
    let args = serde_json::json!({
        "path": "nonexistent.rs",
        "old_string": "old",
        "new_string": "new"
    });
    let err = tool.execute(&args, dir.path()).unwrap_err();
    assert!(err.to_string().contains("could not read"), "expected read error: {}", err);
}

#[test]
fn patch_file_reports_line_counts_in_success_message() {
    let dir = temp_dir();
    let file = dir.path().join("target.rs");
    std::fs::write(&file, "line1\nfn old() {\n    body\n}\nline5\n").unwrap();

    let tool = PatchFileTool;
    let args = serde_json::json!({
        "path": "target.rs",
        "old_string": "fn old() {\n    body\n}",
        "new_string": "fn new() {}"
    });
    let result = tool.execute(&args, dir.path()).unwrap();

    // Original: 5 lines, patched: 3 lines
    assert!(result.contains("5"), "result must mention original line count: {}", result);
    assert!(result.contains("3"), "result must mention patched line count: {}", result);
}

#[test]
fn test_patch_file_crlf_normalization() {
    let dir = temp_dir();
    let file = dir.path().join("target.txt");
    std::fs::write(&file, "line1\r\nline2\r\nline3\r\n").unwrap();

    let tool = PatchFileTool;
    let args = serde_json::json!({
        "path": "target.txt",
        "old_string": "line2\n",
        "new_string": "replaced\n"
    });
    let result = tool.execute(&args, dir.path()).unwrap();

    assert!(result.contains("applied"), "expected success: {}", result);
    let content = std::fs::read_to_string(&file).unwrap();
    assert_eq!(content, "line1\nreplaced\nline3\n");
}
