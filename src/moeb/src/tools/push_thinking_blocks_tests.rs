use std::path::Path;
use std::sync::Arc;

use crate::run_state::new_shared_run_state;
use crate::tools::ToolHandler;
use super::PushThinkingBlocksTool;

#[test]
fn push_two_texts_appends_both_and_returns_correct_count() {
    let state = new_shared_run_state();
    let tool = PushThinkingBlocksTool { state: Arc::clone(&state) };
    let args = serde_json::json!({ "texts": ["decision A rationale", "decision B rationale"] });
    let result = tool.execute(&args, Path::new(".")).unwrap();

    assert_eq!(result, "pushed 2 thinking block(s) into RunState");
    let blocks = state.lock().unwrap().thinking_blocks.clone();
    assert_eq!(blocks, vec!["decision A rationale", "decision B rationale"]);
}

#[test]
fn push_empty_array_leaves_thinking_blocks_unchanged_and_returns_zero_count() {
    let state = new_shared_run_state();
    let tool = PushThinkingBlocksTool { state: Arc::clone(&state) };
    let args = serde_json::json!({ "texts": [] });
    let result = tool.execute(&args, Path::new(".")).unwrap();

    assert_eq!(result, "pushed 0 thinking block(s) into RunState");
    assert!(state.lock().unwrap().thinking_blocks.is_empty());
}

#[test]
fn two_sequential_calls_accumulate_all_texts() {
    let state = new_shared_run_state();
    let tool = PushThinkingBlocksTool { state: Arc::clone(&state) };

    let args1 = serde_json::json!({ "texts": ["first block"] });
    tool.execute(&args1, Path::new(".")).unwrap();

    let args2 = serde_json::json!({ "texts": ["second block", "third block"] });
    tool.execute(&args2, Path::new(".")).unwrap();

    let blocks = state.lock().unwrap().thinking_blocks.clone();
    assert_eq!(blocks, vec!["first block", "second block", "third block"]);
}
