use std::path::PathBuf;
use std::sync::Arc;

use tempfile::TempDir;

use super::McpServer;
use crate::mcp::protocol::JsonRpcOutcome;
use crate::mcp::session::McpSession;
use crate::tools::RealToolExecutor;

fn make_server(dir: &TempDir) -> McpServer {
    let working_dir = PathBuf::from(dir.path());
    let session = McpSession::new(working_dir);
    let state = Arc::clone(&session.state);
    let executor = RealToolExecutor::new_mcp(state);
    McpServer::new(session, executor)
}

#[test]
fn initialize_returns_correct_protocol_version() {
    let dir = TempDir::new().unwrap();
    let server = make_server(&dir);
    let response = server.handle_initialize(serde_json::json!(1));
    match &response.outcome {
        JsonRpcOutcome::Result { result } => {
            assert_eq!(result["protocolVersion"], "2024-11-05");
            assert!(result.get("capabilities").is_some());
            assert!(result["capabilities"].get("tools").is_some());
        }
        JsonRpcOutcome::Error { .. } => panic!("expected result, got error"),
    }
}

#[test]
fn list_tools_includes_start_run_and_get_run_status() {
    let dir = TempDir::new().unwrap();
    let server = make_server(&dir);
    let response = server.handle_list_tools(serde_json::json!(1));
    match &response.outcome {
        JsonRpcOutcome::Result { result } => {
            let tools = result["tools"].as_array().expect("tools should be array");
            let names: Vec<&str> = tools
                .iter()
                .filter_map(|t| t["name"].as_str())
                .collect();
            assert!(names.contains(&"start_run"), "missing start_run, got: {:?}", names);
            assert!(
                names.contains(&"get_run_status"),
                "missing get_run_status, got: {:?}",
                names
            );
        }
        JsonRpcOutcome::Error { .. } => panic!("expected result, got error"),
    }
}

#[test]
fn call_tool_on_unread_existing_file_returns_rejection() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("existing.txt"), "some content").unwrap();

    let mut server = make_server(&dir);
    let params = serde_json::json!({
        "name": "write_file",
        "arguments": {
            "path": "existing.txt",
            "content": "new content"
        }
    });

    let response = server.handle_call_tool(serde_json::json!(1), Some(params));
    match &response.outcome {
        JsonRpcOutcome::Result { result } => {
            let content = result["content"].as_array().expect("content should be array");
            let text = content[0]["text"].as_str().expect("text should be string");
            assert!(
                text.contains("write_file rejected"),
                "expected rejection message, got: {}",
                text
            );
        }
        JsonRpcOutcome::Error { .. } => panic!("expected content result, got JSON-RPC error"),
    }
}
