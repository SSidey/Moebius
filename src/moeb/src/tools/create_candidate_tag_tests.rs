use crate::tools::ToolHandler;
use super::CreateCandidateTagTool;

#[test]
fn create_candidate_tag_missing_domain_returns_error() {
    let tool = CreateCandidateTagTool;
    let args = serde_json::json!({ "slug": "some-slug" });
    let result = tool.execute(&args, std::path::Path::new(".")).unwrap();
    assert!(result.contains("Error"), "expected Error, got: {}", result);
    assert!(result.contains("domain"), "expected domain in message, got: {}", result);
}

#[test]
fn create_candidate_tag_missing_slug_returns_error() {
    let tool = CreateCandidateTagTool;
    let args = serde_json::json!({ "domain": "moeb" });
    let result = tool.execute(&args, std::path::Path::new(".")).unwrap();
    assert!(result.contains("Error"), "expected Error, got: {}", result);
    assert!(result.contains("slug"), "expected slug in message, got: {}", result);
}
