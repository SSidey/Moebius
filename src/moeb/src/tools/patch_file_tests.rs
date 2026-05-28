use crate::tools::ToolHandler;
use super::PatchFileTool;
use tempfile::TempDir;

fn temp_dir() -> TempDir {
    tempfile::tempdir().unwrap()
}

#[test]
fn patch_file_applies_single_hunk() {
    let dir = temp_dir();
    let file = dir.path().join("target.rs");
    std::fs::write(&file, "fn foo() {\n    let x = 1;\n    x\n}\n").unwrap();

    let tool = PatchFileTool;
    let diff = "@@ -2,2 +2,2 @@\n-    let x = 1;\n+    let x = 42;\n     x\n";
    let args = serde_json::json!({"path": "target.rs", "diff": diff});
    let result = tool.execute(&args, dir.path()).unwrap();

    assert!(result.contains("applied"), "expected success: {}", result);
    let content = std::fs::read_to_string(&file).unwrap();
    assert!(content.contains("let x = 42;"), "patch must be applied");
    assert!(!content.contains("let x = 1;"), "old line must be removed");
}

#[test]
fn patch_file_returns_error_on_context_mismatch() {
    let dir = temp_dir();
    let file = dir.path().join("target.rs");
    std::fs::write(&file, "fn foo() {}\n").unwrap();

    let tool = PatchFileTool;
    // Syntactically valid diff (correct counts) but context line doesn't match the file
    let diff = "@@ -1,1 +1,1 @@\n-fn bar() {}\n+fn new() {}\n";
    let args = serde_json::json!({"path": "target.rs", "diff": diff});
    let err = tool.execute(&args, dir.path()).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("failed to apply"), "expected apply error: {}", msg);
    // File must be unchanged
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "fn foo() {}\n");
}

#[test]
fn patch_file_normalizes_wrong_counts_context_mismatch() {
    // Header says -1,3 +1,3 but the hunk has only 2 original and 2 result lines.
    // After normalization the header becomes -1,2 +1,2; diffy then rejects it
    // because the context line "fn bar() {}" does not match the file "fn foo() {}".
    let dir = temp_dir();
    let file = dir.path().join("target.rs");
    std::fs::write(&file, "fn foo() {}\n").unwrap();

    let tool = PatchFileTool;
    let diff = "@@ -1,3 +1,3 @@\n fn bar() {}\n-fn old() {}\n+fn new() {}\n";
    let args = serde_json::json!({"path": "target.rs", "diff": diff});
    let err = tool.execute(&args, dir.path()).unwrap_err();
    assert!(err.to_string().contains("failed to apply"), "expected apply error: {}", err);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "fn foo() {}\n");
}

#[test]
fn patch_file_normalizes_wrong_counts_and_applies() {
    // Header declares wildly wrong counts (-1,999 +1,999) but the hunk content
    // is correct.  The normalizer fixes the header to -1,2 +1,2 and the patch
    // applies successfully.
    let dir = temp_dir();
    let file = dir.path().join("target.rs");
    std::fs::write(&file, "fn foo() {}\nfn old() {}\n").unwrap();

    let tool = PatchFileTool;
    let diff = "@@ -1,999 +1,999 @@\n fn foo() {}\n-fn old() {}\n+fn new() {}\n";
    let args = serde_json::json!({"path": "target.rs", "diff": diff});
    let result = tool.execute(&args, dir.path()).unwrap();

    assert!(result.contains("applied"), "expected success: {}", result);
    let content = std::fs::read_to_string(&file).unwrap();
    assert!(content.contains("fn new() {}"), "added line must be present");
    assert!(!content.contains("fn old() {}"), "removed line must be absent");
}

#[test]
fn patch_file_returns_error_on_missing_file() {
    let dir = temp_dir();
    let tool = PatchFileTool;
    let diff = "@@ -1,1 +1,1 @@\n-old\n+new\n";
    let args = serde_json::json!({"path": "nonexistent.rs", "diff": diff});
    let err = tool.execute(&args, dir.path()).unwrap_err();
    assert!(err.to_string().contains("could not read"), "expected read error: {}", err);
}

#[test]
fn patch_file_relocates_wrong_start_and_applies() {
    // Content is at line 6 but the diff declares line 1.
    // relocate_hunk_positions finds the context at line 6 and adjusts the header;
    // diffy::apply then succeeds.
    let dir = temp_dir();
    let file = dir.path().join("target.rs");
    std::fs::write(
        &file,
        "fn a() {}\nfn b() {}\nfn c() {}\nfn d() {}\nfn e() {}\nfn foo() {}\nfn old() {}\n",
    )
    .unwrap();

    let tool = PatchFileTool;
    // Declares start at line 1, but fn foo() is at line 6.
    let diff = "@@ -1,2 +1,2 @@\n fn foo() {}\n-fn old() {}\n+fn new() {}\n";
    let args = serde_json::json!({"path": "target.rs", "diff": diff});
    let result = tool.execute(&args, dir.path()).unwrap();

    assert!(result.contains("applied"), "expected success: {}", result);
    let content = std::fs::read_to_string(&file).unwrap();
    assert!(content.contains("fn new() {}"), "added line must be present");
    assert!(!content.contains("fn old() {}"), "removed line must be absent");
}

#[test]
fn patch_file_does_not_relocate_absent_context() {
    // Context line does not exist anywhere in the file; relocation cannot help.
    // The hunk is passed through unchanged and diffy returns an apply error.
    let dir = temp_dir();
    let file = dir.path().join("target.rs");
    std::fs::write(&file, "fn foo() {}\n").unwrap();

    let tool = PatchFileTool;
    let diff = "@@ -99,2 +99,2 @@\n fn nonexistent() {}\n-fn old() {}\n+fn new() {}\n";
    let args = serde_json::json!({"path": "target.rs", "diff": diff});
    let err = tool.execute(&args, dir.path()).unwrap_err();
    assert!(
        err.to_string().contains("failed to apply"),
        "expected apply error: {}",
        err
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "fn foo() {}\n");
}

#[test]
fn test_patch_crlf_file_with_lf_diff() {
    let dir = temp_dir();
    let file = dir.path().join("target.rs");
    std::fs::write(&file, "fn foo() {\r\n    bar();\r\n}\r\n").unwrap();

    let tool = PatchFileTool;
    let diff = "@@ -1,3 +1,3 @@\n fn foo() {\n-    bar();\n+    baz();\n }\n";
    let args = serde_json::json!({"path": "target.rs", "diff": diff});
    let result = tool.execute(&args, dir.path()).unwrap();

    assert!(result.contains("applied"), "expected success: {}", result);
    let content = std::fs::read_to_string(&file).unwrap();
    assert!(content.contains("baz()"), "patched content must contain baz()");
    assert!(!content.contains("bar()"), "patched content must not contain bar()");
}

#[test]
fn patch_file_tolerates_trailing_whitespace_in_file() {
    // File lines have trailing spaces (common in markdown tables);
    // diff context lines do not — patch must still apply successfully.
    let dir = temp_dir();
    let file = dir.path().join("target.md");
    std::fs::write(&file, "| col1 | col2 |   \n| old  | val  |   \n").unwrap();

    let tool = PatchFileTool;
    let diff = "@@ -1,2 +1,2 @@\n | col1 | col2 |\n-| old  | val  |\n+| new  | val  |\n";
    let args = serde_json::json!({"path": "target.md", "diff": diff});
    let result = tool.execute(&args, dir.path()).unwrap();

    assert!(result.contains("applied"), "expected success: {}", result);
    let content = std::fs::read_to_string(&file).unwrap();
    assert!(content.contains("new"), "patched content must contain new row");
    assert!(!content.contains("| old"), "old row must be removed");
}

#[test]
fn patch_file_diagnostic_names_absent_context_line() {
    // Context line does not exist in the file; error message must name it.
    let dir = temp_dir();
    let file = dir.path().join("target.md");
    std::fs::write(&file, "| real | row |\n").unwrap();

    let tool = PatchFileTool;
    let diff = "@@ -1,2 +1,2 @@\n | phantom | row |\n-| old | val |\n+| new | val |\n";
    let args = serde_json::json!({"path": "target.md", "diff": diff});
    let err = tool.execute(&args, dir.path()).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("phantom"),
        "error must name the absent context line: {}",
        msg
    );
}

#[test]
fn patch_file_diagnostic_all_present_when_context_found() {
    // Context line exists but the removal line (-) does not match the file,
    // so diffy::apply fails. The diagnostic must report all context lines present.
    let dir = temp_dir();
    let file = dir.path().join("target.rs");
    std::fs::write(&file, "fn foo() {}\nfn bar() {}\n").unwrap();

    let tool = PatchFileTool;
    let diff = "@@ -1,2 +1,2 @@\n fn foo() {}\n-fn baz() {}\n+fn qux() {}\n";
    let args = serde_json::json!({"path": "target.rs", "diff": diff});
    let err = tool.execute(&args, dir.path()).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("All context lines appear"),
        "expected all-present diagnostic: {}",
        msg
    );
}
