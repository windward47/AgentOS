/// Sidecar integration tests — validate RPC round-trips.
/// Requires: bun installed, sidecar script present.
#[cfg(test)]
mod sidecar_tests {
    use companion_core::agent::omp_sidecar::OmpAgentSidecar;
    use companion_core::agent::AgentEngine;

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
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_chat_returns_text() {
        let agent = spawn().await;
        let response = agent.chat("Hello", &[], None).await.expect("chat");
        assert!(!response.text.is_empty(), "chat returns text");
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
}
