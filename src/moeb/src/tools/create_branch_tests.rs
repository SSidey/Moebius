use crate::tools::ToolHandler;
use super::CreateBranchTool;

#[test]
fn missing_domain_returns_error() {
    let tool = CreateBranchTool;
    let args = serde_json::json!({ "slug": "my-slug" });
    let err = tool.execute(&args, std::path::Path::new(".")).unwrap_err();
    assert!(
        err.to_string().contains("domain"),
        "expected domain error, got: {}",
        err
    );
}

#[test]
fn missing_slug_returns_error() {
    let tool = CreateBranchTool;
    let args = serde_json::json!({ "domain": "moeb" });
    let err = tool.execute(&args, std::path::Path::new(".")).unwrap_err();
    assert!(
        err.to_string().contains("slug"),
        "expected slug error, got: {}",
        err
    );
}
