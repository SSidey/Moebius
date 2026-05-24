use super::*;
use crate::adapters::{AgentResponse, Message, ToolDef};
use crate::config::tests::CWD_LOCK;
use crate::ports::AiPort;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

struct CapturingStub { captured: Mutex<Option<String>> }

impl AiPort for CapturingStub {
    fn send(&self, messages: &[Message], _tools: &[ToolDef]) -> Result<AgentResponse> {
        let mut captured = self.captured.lock().unwrap();
        if captured.is_none() {
            if let Some(Message::User(text)) = messages.first() {
                *captured = Some(text.clone());
            }
        }
        Ok(AgentResponse::Text(String::new()))
    }
}

fn setup() -> (TempDir, std::sync::MutexGuard<'static, ()>) {
    let guard = CWD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().expect("tempdir");
    std::env::set_current_dir(dir.path()).expect("set_current_dir");
    (dir, guard)
}

#[test]
fn run_substitutes_readme_and_spec_content() {
    let (_dir, _guard) = setup();
    fs::create_dir_all(".moeb/specifications/moeb").expect("create spec dir");
    fs::write(".moeb/README.md", "readme-body").expect("write README");
    fs::write(".moeb/specifications/moeb/test.spec.md", "spec-body").expect("write spec");

    let stub = Arc::new(CapturingStub { captured: Mutex::new(None) });
    let service = RunService::new(stub.clone() as Arc<dyn AiPort>);
    service.run("test.spec", crate::trace::FileContentMode::Embed, false).expect("run should succeed");

    let captured = stub.captured.lock().unwrap();
    let prompt = captured.as_ref().expect("prompt should have been captured");
    assert!(prompt.contains("readme-body"), "prompt must contain README content");
    assert!(prompt.contains("spec-body"), "prompt must contain spec content");
    assert!(!prompt.contains("{{readme_content}}"), "{{readme_content}} token must be replaced");
    assert!(!prompt.contains("{{spec_content}}"), "{{spec_content}} token must be replaced");
}
