    use super::*;
    use crate::adapters::{AgentResponse, Message, ToolCall, ToolDef};
    use crate::ports::AiPort;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    fn spec_doc(domain: &str, slug: &str) -> String {
        format!(
            "---\ndomain: {}\nslug: {}\nstatus: active\n---\n\n# Spec\n\n## Raw Requirement\ntest\n",
            domain, slug
        )
    }

    struct MockAi {
        responses: Mutex<VecDeque<AgentResponse>>,
    }

    impl MockAi {
        fn new(responses: Vec<AgentResponse>) -> Arc<Self> {
            Arc::new(Self {
                responses: Mutex::new(responses.into_iter().collect()),
            })
        }
    }

    impl AiPort for MockAi {
        fn send(&self, _: &[Message], _: &[ToolDef]) -> anyhow::Result<AgentResponse> {
            self.responses
                .lock()
                .unwrap()
                .pop_front()
                .ok_or_else(|| anyhow::anyhow!("MockAi: no more queued responses"))
        }
    }

    fn setup_harness(tmp: &tempfile::TempDir) {
        let dir = tmp.path();
        fs::create_dir_all(dir.join("specifications")).unwrap();
        fs::write(dir.join("README.md"), "# Harness\n").unwrap();
    }

    fn write_file_call(path: &str, content: &str) -> AgentResponse {
        AgentResponse::ToolCalls(vec![ToolCall {
            id: "w1".to_string(),
            name: "write_file".to_string(),
            arguments: serde_json::json!({"path": path, "content": content}).to_string(),
        }])
    }

    #[test]
    fn run_in_creates_spec_file_at_correct_path() {
        let tmp = tempfile::tempdir().unwrap();
        setup_harness(&tmp);

        let ai = MockAi::new(vec![
            write_file_call(
                "specifications/auth/auth.token-rotation.md",
                &spec_doc("auth", "token-rotation"),
            ),
            AgentResponse::Text("Done.".to_string()),
        ]);
        let result = SpecService::new(ai)
            .run_in("rotate tokens", tmp.path(), 1, FileContentMode::Embed, false);
        assert!(result.is_ok(), "run_in must succeed: {:?}", result);
    }

    #[test]
    fn run_in_readme_updated_when_agent_writes_it() {
        let tmp = tempfile::tempdir().unwrap();
        setup_harness(&tmp);

        let ai = MockAi::new(vec![
            write_file_call(
                "specifications/auth/auth.token-rotation.md",
                &spec_doc("auth", "token-rotation"),
            ),
            AgentResponse::Text("Done.".to_string()),
        ]);
        let result = SpecService::new(ai)
            .run_in("rotate tokens", tmp.path(), 1, FileContentMode::Embed, false);
        assert!(result.is_ok(), "run_in must succeed: {:?}", result);
    }

    #[test]
    fn run_in_retries_on_validation_failure() {
        let tmp = tempfile::tempdir().unwrap();
        setup_harness(&tmp);

        let ai = MockAi::new(vec![
            write_file_call(
                "specifications/auth/auth.token-rotation.md",
                &spec_doc("auth", "token-rotation"),
            ),
            AgentResponse::Text("Done.".to_string()),
        ]);
        let result = SpecService::new(ai)
            .run_in("rotate tokens", tmp.path(), 2, FileContentMode::Embed, false);
        assert!(result.is_ok(), "run_in must succeed: {:?}", result);
    }

    #[test]
    fn run_in_retries_on_empty_response() {
        let tmp = tempfile::tempdir().unwrap();
        setup_harness(&tmp);

        let ai = MockAi::new(vec![
            write_file_call(
                "specifications/auth/auth.token-rotation.md",
                &spec_doc("auth", "token-rotation"),
            ),
            AgentResponse::Text("Done.".to_string()),
        ]);
        let result = SpecService::new(ai)
            .run_in("rotate tokens", tmp.path(), 2, FileContentMode::Embed, false);
        assert!(result.is_ok(), "run_in must succeed: {:?}", result);
    }

    #[test]
    fn run_in_fails_after_exhausting_retries() {
        let tmp = tempfile::tempdir().unwrap();
        setup_harness(&tmp);

        // Two text responses exhaust the queue; the adapter then returns Err on
        // subsequent calls, causing run_agent_loop_run_mode to return Err on each attempt.
        let ai = MockAi::new(vec![
            AgentResponse::Text("No frontmatter — attempt 1.".to_string()),
            AgentResponse::Text("No frontmatter — attempt 2.".to_string()),
        ]);
        let err = SpecService::new(ai)
            .run_in("rotate tokens", tmp.path(), 2, FileContentMode::Embed, false)
            .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("failed after 2 attempt(s)"),
            "expected exhaustion message, got: {}",
            msg
        );
    }
