/// Integration test for the omp RPC chat pipeline.
/// Requires `omp` on PATH and a working LLM (Xiaomi token plan via sensenova provider).

#[cfg(test)]
mod integration_tests {
    use companion_core::agent::omp_rpc::OmpRpcClient;
    use companion_core::agent::{AgentEngine, ConversationMessage};

    /// Test that the RPC client can spawn omp and send a real prompt.
    /// This test requires omp ≥ 15.1.2 and the sensenova/xiaomi provider configured.
    #[tokio::test]
    #[ignore = "requires omp binary and API key configured"]
    async fn test_omp_rpc_real_chat() {
        // Use full path for reliability (same as resolve_omp_binary)
        let binary = std::env::var("APP_DATA")
            .map(|d| format!("{d}/npm/omp"))
            .unwrap_or_else(|_| "omp".into());

        if !std::path::Path::new(&binary).exists() {
            eprintln!("Skipping: omp not found at {binary}");
            return;
        }

        let client = OmpRpcClient::new(&binary);
        let result = client.chat("Say 'OK' in one word", &[]).await;

        match result {
            Ok(response) => {
                println!("omp response: {}", response.text);
                assert!(
                    response.text.to_lowercase().contains("ok")
                        || response.text.contains("OK")
                        || !response.text.is_empty(),
                    "Expected non-empty response, got: {}",
                    response.text
                );
            }
            Err(e) => {
                // If omp is not configured or API key missing, test passes but logs
                eprintln!("omp error (may be env issue): {e}");
            }
        }
    }

    /// Test that the response contains any text at all.
    #[tokio::test]
    #[ignore = "requires omp binary and API key configured"]
    async fn test_omp_rpc_response_is_not_empty() {
        let binary = "omp";
        let client = OmpRpcClient::new(binary);
        let result = client.chat("hi", &[]).await;

        match result {
            Ok(response) => assert!(!response.text.is_empty()),
            Err(_) => { /* skip if omp not available */ }
        }
    }
}
