use crate::tools::ToolHandler;
use super::GitCommitTool;

#[test]
fn missing_spec_path_returns_error() {
    let tool = GitCommitTool;
    let args = serde_json::json!({
        "readme_path": "README.md",
        "domain": "moeb",
        "slug": "my-spec"
    });
    let err = tool.execute(&args, std::path::Path::new(".")).unwrap_err();
    assert!(
        err.to_string().contains("spec_path"),
        "expected spec_path error, got: {}",
        err
    );
}

#[test]
fn missing_domain_returns_error() {
    let tool = GitCommitTool;
    let args = serde_json::json!({
        "spec_path": "specifications/moeb/moeb.my-spec.md",
        "readme_path": "README.md",
        "slug": "my-spec"
    });
    let err = tool.execute(&args, std::path::Path::new(".")).unwrap_err();
    assert!(
        err.to_string().contains("domain"),
        "expected domain error, got: {}",
        err
    );
}
