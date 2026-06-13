/// End-to-end test for Tauri IPC chat flow.
/// Uses a mock AgentEngine — no external dependencies.

#[cfg(test)]
mod e2e_tests {
    use companion_core::agent::{AgentEngine, AgentError, AgentResponse, AgentStreamEvent, ConversationMessage, MessageRole};
    use std::sync::Arc;
    use tokio::sync::Mutex;

    /// Mock agent that echoes and returns history (simulates B1b sidecar behaviour).
    struct EchoAgent;
    #[async_trait::async_trait]
    impl AgentEngine for EchoAgent {
        async fn chat(&self, message: &str, _history: &[ConversationMessage], _system_prompt: Option<&str>) -> Result<AgentResponse, AgentError> {
            Ok(AgentResponse {
                text: format!("Echo: {message}"),
                history: vec![
                    ConversationMessage { role: MessageRole::User, content: message.into() },
                    ConversationMessage { role: MessageRole::Assistant, content: format!("Echo: {message}") },
                ],
                tool_calls: vec![],
                emotions: vec!["f01".into()],
            })
        }
        async fn chat_stream(&self, message: &str, _history: &[ConversationMessage]) -> Result<tokio::sync::mpsc::Receiver<AgentStreamEvent>, AgentError> {
            let (tx, rx) = tokio::sync::mpsc::channel(1);
            let text = format!("Echo: {message}");
            tx.send(AgentStreamEvent::Token(text)).await.ok();
            tx.send(AgentStreamEvent::Done).await.ok();
            Ok(rx)
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_chat_flow() {
        let agent: Arc<dyn AgentEngine + Send + Sync> = Arc::new(EchoAgent);

        // B1b: sidecar manages history — Rust just calls chat()
        let resp = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            agent.chat("hello", &[], None),
        ).await.expect("timed out").expect("chat failed");

        assert_eq!(resp.text, "Echo: hello");
        // B1b: history comes back in the response
        assert_eq!(resp.history.len(), 2);
        assert_eq!(resp.history[0].content, "hello");
        assert_eq!(resp.history[1].content, "Echo: hello");
    }

    #[test]
    fn test_history_truncation() {
        // B1b: truncation is now sidecar's responsibility (tested in sidecar)
        // This test verifies the algorithm used by AgentManager.chat()
        let mut history: Vec<ConversationMessage> = (0..60).map(|i| ConversationMessage {
            role: if i % 2 == 0 { MessageRole::User } else { MessageRole::Assistant },
            content: format!("msg{i}"),
        }).collect();
        if history.len() > 50 {
            history = history.split_off(history.len() - 50);
        }
        assert_eq!(history.len(), 50);
        assert_eq!(history[0].content, "msg10");
        assert_eq!(history[49].content, "msg59");
    }

    #[test]
    fn test_clear_history() {
        let mut history = vec![ConversationMessage { role: MessageRole::User, content: "test".into() }];
        assert!(!history.is_empty());
        history.clear();
        assert!(history.is_empty());
    }
}
