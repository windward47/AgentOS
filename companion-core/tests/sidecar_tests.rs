/// Sidecar integration tests — validate RPC round-trips.
/// Requires: bun installed, sidecar script present.
#[cfg(test)]
mod sidecar_tests {
    use companion_core::agent::omp_sidecar::OmpAgentSidecar;
    use companion_core::agent::AgentEngine;
    use serde_json::Value;

    async fn spawn() -> OmpAgentSidecar {
        let agent = OmpAgentSidecar::new();
        agent.spawn().await.expect("sidecar spawn");
        agent
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_get_config_roundtrip() {
        let agent = spawn().await;
        let config = agent.get_config().await.expect("get_config");
        assert!(config.get("sandbox_path").is_some(), "config has sandbox_path");
        assert!(config.get("custom_system_prompt").is_some(), "config has system_prompt");
        let sp = config["custom_system_prompt"].as_str().unwrap();
        assert!(sp.contains("web_search"), "system prompt mentions web_search");
        assert!(sp.contains("Never say"), "system prompt says 'Never say you can't'");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_chat_returns_text_and_history() {
        let agent = spawn().await;
        let response = agent.chat("Hello", &[], None).await.expect("chat");
        assert!(!response.text.is_empty(), "chat returns text");
        assert!(!response.history.is_empty(), "chat returns history");
        assert_eq!(response.history[response.history.len() - 1].content, response.text,
            "last history entry matches response text");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_clear_history() {
        let agent = spawn().await;
        agent.chat("Test message", &[], None).await.expect("chat");
        let hist_before = agent.get_history().await.expect("get_history");
        let arr = hist_before["history"].as_array().unwrap();
        assert!(arr.len() >= 2, "history has user + assistant");

        agent.clear_history().await.expect("clear_history");
        let hist_after = agent.get_history().await.expect("get_history");
        let arr2 = hist_after["history"].as_array().unwrap();
        assert!(arr2.is_empty(), "history empty after clear");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_agent_action_chat() {
        let agent = spawn().await;
        let result = agent.agent_action("chat", serde_json::json!({
            "message": "Say hello in one word",
        })).await.expect("agent_action chat");
        let text = result["text"].as_str().unwrap();
        assert!(!text.is_empty(), "agent_action chat returns text");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_agent_action_get_config() {
        let agent = spawn().await;
        let result = agent.agent_action("get_config", Value::Null).await.expect("agent_action");
        assert!(result.get("sandbox_path").is_some());
    }
}
