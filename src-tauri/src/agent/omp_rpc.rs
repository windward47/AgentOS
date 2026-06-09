//! oh-my-pi (omp) subprocess integration.
//!
//! Uses `omp -p` (print mode) for synchronous chat: spawns a fresh omp process
//! per request, reads stdout, and returns the text.  No persistent sessions —
//! each `chat()` call is a clean `omp -p "message"` invocation.

use std::process::{Command, Stdio};
use super::{AgentEngine, AgentError, AgentResponse, AgentStreamEvent, ConversationMessage};
use async_trait::async_trait;

/// A client that spawns `omp -p` on every chat call.
/// Stateless — no subprocess persistence.
pub struct OmpRpcClient {
    /// Path to the omp binary.
    binary: String,
}

impl OmpRpcClient {
    pub fn new(binary: &str) -> Self {
        Self { binary: binary.to_string() }
    }

    /// Run `omp -p <message> --no-session` and return stdout.
    async fn run_omp(&self, message: &str) -> Result<String, AgentError> {
        let binary = self.binary.clone();
        let msg = message.to_string();

        let output = tokio::task::spawn_blocking(move || {
            Command::new(&binary)
                .arg("-p")
                .arg(&msg)
                .arg("--no-session")
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .output()
        })
        .await
        .map_err(|e| AgentError::SubprocessCrashed(format!("spawn_blocking failed: {e}")))?
        .map_err(|e| AgentError::SubprocessCrashed(format!("omp process failed: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AgentError::AgentReturnedError(
                format!("omp exited with {}: {stderr}", output.status.code().unwrap_or(-1))
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(stdout)
    }
}

#[async_trait]
impl AgentEngine for OmpRpcClient {
    async fn chat(
        &self,
        message: &str,
        _history: &[ConversationMessage],
    ) -> Result<AgentResponse, AgentError> {
        let text = self.run_omp(message).await?;
        Ok(AgentResponse { text, tool_calls: vec![] })
    }

    async fn chat_stream(
        &self,
        message: &str,
        history: &[ConversationMessage],
    ) -> Result<tokio::sync::mpsc::Receiver<AgentStreamEvent>, AgentError> {
        let response = self.chat(message, history).await?;
        let (tx, rx) = tokio::sync::mpsc::channel(16);
        let text = response.text;
        tokio::spawn(async move {
            for word in text.split(' ') {
                if tx.send(AgentStreamEvent::Token(word.to_string())).await.is_err() {
                    break;
                }
            }
            let _ = tx.send(AgentStreamEvent::Done).await;
        });
        Ok(rx)
    }
}
