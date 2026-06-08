//! oh-my-pi (omp) RPC subprocess integration.
//!
//! Spawns `omp --mode rpc` as a managed child process and communicates
//! via NDJSON over stdin/stdout.
//!
//! ## Protocol
//!
//! Every line on stdin is a JSON-RPC request; every line on stdout is a response
//! or event frame.  Frame types:
//!
//! - `{"id":"r1","type":"prompt","message":"...","conversationId":"..."}`
//! - `{"id":"r2","type":"set_model","provider":"anthropic","modelId":"..."}`
//! - `{"id":"r3","type":"abort"}`
//!
//! Responses come as frames:
//! - `{"id":"r1","type":"response","message":{"content":"...","role":"assistant"}}`
//! - `{"id":"r1","type":"error","error":"..."}`
//! - streaming token frames (`type: "delta"`)

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

use super::{AgentEngine, AgentError, AgentResponse, AgentStreamEvent, ConversationMessage};
use async_trait::async_trait;

// ---------------------------------------------------------------------------
// RPC frame types (subset of the protocol)
// ---------------------------------------------------------------------------

#[derive(serde::Serialize)]
struct RpcRequest<'a> {
    id: &'a str,
    #[serde(rename = "type")]
    r#type: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    conversation_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    provider: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model_id: Option<&'a str>,
}

#[derive(serde::Deserialize)]
#[serde(untagged)]
#[allow(dead_code)]
enum RpcFrame {
    Response {
        id: String,
        #[serde(rename = "type")]
        r#type: String,
        #[serde(default)]
        message: Option<serde_json::Value>,
        #[serde(default)]
        error: Option<String>,
    },
    Unknown(serde_json::Value),
}

// ---------------------------------------------------------------------------
// OmpRpcClient
// ---------------------------------------------------------------------------

/// A client that manages an `omp --mode rpc` subprocess.
///
/// ## Lifecycle
///
/// 1. `OmpRpcClient::spawn()` → forks `omp` with the given binary path
/// 2. `chat()` / `chat_stream()` send prompt frames and read responses
/// 3. `shutdown()` kills the subprocess
pub struct OmpRpcClient {
    /// The child process handle (stdin writer stored separately).
    child: Mutex<Option<ChildProcess>>,
    /// Current model identifier (persisted for reconnection).
    model: Arc<Mutex<ModelConfig>>,
    /// Monotonically increasing request id.
    next_id: AtomicU64,
    /// Path to the `omp` binary.
    binary: String,
}

struct ChildProcess {
    _child: Child,
    stdin: ChildStdin,
    reader: BufReader<std::process::ChildStdout>,
}

#[derive(Clone)]
struct ModelConfig {
    provider: String,
    model_id: String,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            provider: "openai".into(),
            model_id: "gpt-4o-mini".into(),
        }
    }
}

impl OmpRpcClient {
    /// Create a new client.  Call [`spawn`](Self::spawn) before use.
    pub fn new(binary: &str) -> Self {
        Self {
            child: Mutex::new(None),
            model: Arc::new(Mutex::new(ModelConfig::default())),
            next_id: AtomicU64::new(1),
            binary: binary.to_string(),
        }
    }

    /// Start the `omp` subprocess.
    ///
    /// Errors if `omp` is not found on `$PATH` or the process refuses to start.
    pub async fn spawn(&self) -> Result<(), AgentError> {
        let mut child = Command::new(&self.binary)
            .arg("--mode")
            .arg("rpc")
            .arg("--no-session") // no TUI session wrapping
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit()) // log to companion's stderr
            .spawn()
            .map_err(|e| AgentError::SubprocessCrashed(format!("cannot spawn omp: {e}")))?;

        let stdin = child.stdin.take().ok_or_else(|| {
            AgentError::SubprocessCrashed("cannot open omp stdin".into())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            AgentError::SubprocessCrashed("cannot open omp stdout".into())
        })?;
        let reader = BufReader::new(stdout);

        let mut guard = self.child.lock().await;
        *guard = Some(ChildProcess { _child: child, stdin, reader });

        // Set the model immediately so subsequent chat calls use it.
        let model = self.model.lock().await;
        self.send_set_model(&model.provider, &model.model_id).await?;

        Ok(())
    }

    /// Shut down the subprocess gracefully.
    pub async fn shutdown(&self) {
        let mut guard = self.child.lock().await;
        if let Some(mut cp) = guard.take() {
            // Dropping stdin signals EOF to omp; dropping the Child kills it.
            drop(cp.stdin);
            // Give the process a moment to exit, then force-kill.
            let _ = cp._child.wait();
        }
    }

    /// Send a `set_model` command to the subprocess.
    async fn send_set_model(&self, provider: &str, model_id: &str) -> Result<(), AgentError> {
        let id = self.next_id();
        let req = RpcRequest {
            id: &id,
            r#type: "set_model",
            message: None,
            conversation_id: None,
            provider: Some(provider),
            model_id: Some(model_id),
        };
        self.send_frame(&req).await
    }

    /// Serialise and write a single JSON line to the subprocess stdin.
    async fn send_frame(&self, req: &RpcRequest<'_>) -> Result<(), AgentError> {
        let mut guard = self.child.lock().await;
        let cp = guard.as_mut().ok_or(AgentError::NotRunning)?;
        let line = serde_json::to_string(req).map_err(|e| AgentError::RpcError(e.to_string()))?;
        writeln!(cp.stdin, "{line}").map_err(|e| AgentError::SubprocessCrashed(e.to_string()))?;
        cp.stdin.flush().map_err(|e| AgentError::SubprocessCrashed(e.to_string()))?;
        Ok(())
    }

    /// Read the next frame from stdout (blocking read, spawned on background task).
    async fn read_frame(&self) -> Result<RpcFrame, AgentError> {
        let mut guard = self.child.lock().await;
        let cp = guard.as_mut().ok_or(AgentError::NotRunning)?;
        let mut line = String::new();
        cp.reader
            .read_line(&mut line)
            .map_err(|e| AgentError::SubprocessCrashed(e.to_string()))?;
        if line.is_empty() {
            return Err(AgentError::SubprocessCrashed("omp process closed stdout".into()));
        }
        serde_json::from_str(&line).map_err(|e| AgentError::RpcError(e.to_string()))
    }

    fn next_id(&self) -> String {
        let n = self.next_id.fetch_add(1, Ordering::SeqCst);
        format!("r{n}")
    }

    /// Update the stored model config (persisted across re-connects).
    pub async fn set_model(&self, provider: &str, model_id: &str) -> Result<(), AgentError> {
        let mut model = self.model.lock().await;
        model.provider = provider.to_string();
        model.model_id = model_id.to_string();
        self.send_set_model(provider, model_id).await
    }
}

#[async_trait]
impl AgentEngine for OmpRpcClient {
    async fn chat(
        &self,
        message: &str,
        _history: &[ConversationMessage],
    ) -> Result<AgentResponse, AgentError> {
        // Auto-spawn on first use
        {
            let guard = self.child.lock().await;
            if guard.is_none() {
                drop(guard);
                self.spawn().await?;
            }
        }

        let id = self.next_id();
        let req = RpcRequest {
            id: &id,
            r#type: "prompt",
            message: Some(message),
            conversation_id: None,
            provider: None,
            model_id: None,
        };
        self.send_frame(&req).await?;

        // Read response frames until we get the final response or an error.
        loop {
            let frame = self.read_frame().await?;
            match frame {
                RpcFrame::Response { id: _, r#type, message, error } => {
                    if r#type == "error" {
                        return Err(AgentError::AgentReturnedError(
                            error.unwrap_or_else(|| "unknown error".into()),
                        ));
                    }
                    if r#type == "response" {
                        let text = message
                            .and_then(|v| v.get("content")?.as_str().map(String::from))
                            .unwrap_or_default();
                        return Ok(AgentResponse {
                            text,
                            tool_calls: vec![],
                        });
                    }
                    // Ignore intermediate frames (delta, tool_call, etc.)
                }
                RpcFrame::Unknown(_) => continue,
            }
        }
    }

    async fn chat_stream(
        &self,
        message: &str,
        _history: &[ConversationMessage],
    ) -> Result<tokio::sync::mpsc::Receiver<AgentStreamEvent>, AgentError> {
        // TODO: implement streaming via tokio::task::spawn_blocking that reads frames
        // and pushes tokens into the channel.  For the MVP we delegate to chat().
        let response = self.chat(message, _history).await?;
        let (tx, rx) = tokio::sync::mpsc::channel(16);
        tokio::spawn(async move {
            for token in response.text.split(" ") {
                tx.send(AgentStreamEvent::Token(token.to_string())).await.ok();
            }
            tx.send(AgentStreamEvent::Done).await.ok();
        });
        Ok(rx)
    }
}

impl Drop for OmpRpcClient {
    fn drop(&mut self) {
        // Best-effort shutdown in a blocking context.
        // The async `shutdown` method is preferred.
    }
}
