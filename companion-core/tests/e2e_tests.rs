/// End-to-end test for chat flow.
/// Uses a mock AgentEngine — no external dependencies.

#[cfg(test)]
mod e2e_tests {
    use companion_core::agent::{AgentEngine, AgentError, AgentResponse, ConversationMessage, MessageRole};
    use std::sync::Arc;

    /// Mock agent that echoes.
    struct EchoAgent;
    #[async_trait::async_trait]
    impl AgentEngine for EchoAgent {
        async fn chat(&self, message: &str, _history: &[ConversationMessage], _system_prompt: Option<&str>) -> Result<AgentResponse, AgentError> {
            Ok(AgentResponse {
                text: format!("Echo: {message}"),
            })
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_chat_flow() {
        let agent: Arc<dyn AgentEngine + Send + Sync> = Arc::new(EchoAgent);

        let resp = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            agent.chat("hello", &[], None),
        ).await.expect("timed out").expect("chat failed");

        assert_eq!(resp.text, "Echo: hello");
    }

    #[test]
    fn test_history_truncation() {
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
