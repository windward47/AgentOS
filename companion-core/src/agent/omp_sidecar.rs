//! Bun Agent Sidecar client — communicates via HTTP (localhost:PORT).
//! POST /rpc for regular RPC, POST /chat_stream for streaming NDJSON.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::Arc;
use tokio::sync::Mutex;

use async_trait::async_trait;
use serde_json::Value;

use super::{AgentEngine, AgentError, AgentResponse, ConversationMessage};

#[derive(serde::Serialize, Debug)]
struct JsonRpcRequest {
    id: String,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
}

pub struct OmpAgentSidecar {
    bun_binary: String,
    sidecar_script: String,
    port: Arc<Mutex<Option<u16>>>,
    client: reqwest::Client,
    next_id: Arc<Mutex<u64>>,
}

impl OmpAgentSidecar {
    const SIDECAR_SCRIPT_RELATIVE: &str = "services/agent-sidecar/src/index.ts";
    const DEFAULT_PORT: u16 = 9876;

    #[cfg(debug_assertions)]
    fn project_root() -> std::path::PathBuf {
        let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.pop();
        path
    }

    #[cfg(not(debug_assertions))]
    fn project_root() -> std::path::PathBuf {
        std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf())).unwrap_or_default()
    }

    fn resolve_bun() -> String {
        #[cfg(target_os = "windows")] {
            for p in &[format!(r"{}\npm\bun.cmd", std::env::var("APPDATA").unwrap_or_default()), "bun.cmd".into(), "bun".into()] {
                if std::path::Path::new(p).exists() { return p.clone(); }
            }
        }
        "bun".into()
    }

    pub fn new() -> Self {
        Self {
            bun_binary: Self::resolve_bun(),
            sidecar_script: Self::project_root().join(Self::SIDECAR_SCRIPT_RELATIVE).to_string_lossy().to_string(),
            port: Arc::new(Mutex::new(None)),
            client: reqwest::Client::new(),
            next_id: Arc::new(Mutex::new(0)),
        }
    }

    fn base_url(&self) -> String {
        let port = self.port.try_lock().ok().and_then(|g| *g).unwrap_or(Self::DEFAULT_PORT);
        format!("http://127.0.0.1:{}", port)
    }

    pub async fn spawn(&self) -> Result<(), AgentError> {
        let mut port_guard = self.port.lock().await;
        if port_guard.is_some() { return Ok(()); }

        let script = &self.sidecar_script;
        if !std::path::Path::new(script).exists() {
            return Err(AgentError::SubprocessCrashed(format!("Sidecar not found: {script}")));
        }

        let mut child = Command::new(&self.bun_binary).arg("run").arg(script)
            .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::inherit())
            .spawn().map_err(|e| AgentError::SubprocessCrashed(format!("Spawn: {e}")))?;

        let stdout = child.stdout.take().ok_or(AgentError::SubprocessCrashed("No stdout".into()))?;
        let mut reader = BufReader::new(stdout);
        let mut port_line = String::new();
        reader.read_line(&mut port_line).map_err(|e| AgentError::SubprocessCrashed(format!("Read port: {e}")))?;
        let port: u16 = port_line.trim().parse().map_err(|_| AgentError::SubprocessCrashed("Parse port".into()))?;

        *port_guard = Some(port);
        log::info!("Agent sidecar spawned on port {port}");
        Ok(())
    }

    pub async fn is_running(&self) -> bool { self.port.lock().await.is_some() }

    async fn next_id_val(&self) -> u64 { let mut g = self.next_id.lock().await; let id = *g; *g += 1; id }

    async fn rpc(&self, method: &str, params: Option<Value>) -> Result<Value, AgentError> {
        let id = format!("r{}", self.next_id_val().await);
        let req = JsonRpcRequest { id, method: method.to_string(), params };
        let url = format!("{}/rpc", self.base_url());
        let resp = self.client.post(&url).json(&req).send().await
            .map_err(|e| AgentError::SubprocessCrashed(format!("HTTP: {e}")))?;
        let body: Value = resp.json().await
            .map_err(|e| AgentError::RpcError(format!("Parse: {e}")))?;
        if body.get("type").and_then(|v| v.as_str()) == Some("error") {
            return Err(AgentError::AgentReturnedError(
                body.get("error").and_then(|e| e.get("message")).and_then(|v| v.as_str()).unwrap_or("Unknown").into()
            ));
        }
        Ok(body.get("result").cloned().unwrap_or(Value::Null))
    }

    pub async fn chat_stream_tokens(&self, message: &str, history: &[Value]) -> Result<tokio::sync::mpsc::Receiver<String>, AgentError> {
        let id = format!("s{}", self.next_id_val().await);
        let req = JsonRpcRequest { id: id.clone(), method: "chat_stream".to_string(), params: Some(serde_json::json!({ "message": message, "history": history })) };
        let (tx, rx) = tokio::sync::mpsc::channel(256);
        let url = format!("{}/chat_stream", self.base_url());
        log::info!("chat_stream_tokens: POST {}", url);
        let client = self.client.clone();

        tokio::spawn(async move {
            let resp = match client.post(&url).json(&req).send().await {
                Ok(r) => r,
                Err(e) => { let _ = tx.send(format!("\0⚠️ HTTP {}", e)).await; return; }
            };
            let mut stream = resp.bytes_stream();
            use futures_util::StreamExt;
            while let Some(chunk) = stream.next().await {
                let chunk = match chunk {
                    Ok(c) => c,
                    Err(_) => { let _ = tx.send("\0⚠️ Stream error".to_string()).await; return; }
                };
                let text = String::from_utf8_lossy(&chunk);
                for line in text.lines() {
                    let line = line.strip_prefix("data: ").unwrap_or(line);
                    if line.is_empty() || line.starts_with(":") { continue; }
                    let resp: Value = match serde_json::from_str(line) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };
                    let typ = resp.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    match typ {
                        "event" => {
                            let evt = resp.get("event").and_then(|v| v.as_str()).unwrap_or("");
                            match evt {
                                "token" => {
                                    if let Some(t) = resp.get("data").and_then(|d| d.get("token")).and_then(|v| v.as_str()) {
                                        if tx.send(t.to_string()).await.is_err() { return; }
                                    }
                                }
                                "done" => {
                                    let t = resp.get("data").and_then(|d| d.get("text")).and_then(|v| v.as_str()).unwrap_or("");
                                    let _ = tx.send(format!("\0{}", t)).await;
                                    return;
                                }
                                _ => {}
                            }
                        }
                        "error" => {
                            let msg = resp.get("error").and_then(|e| e.get("message")).and_then(|v| v.as_str()).unwrap_or("Error");
                            let _ = tx.send(format!("\0⚠️ {}", msg)).await;
                            return;
                        }
                        _ => {}
                    }
                }
            }
            let _ = tx.send("\0⚠️ Stream ended".to_string()).await;
        });

        Ok(rx)
    }

    // ── Passthrough RPC ──
    pub async fn clear_history(&self) -> Result<(), AgentError> { self.rpc("clear_history", None).await.map(|_| ()) }
    pub async fn get_history(&self) -> Result<Value, AgentError> { self.rpc("get_history", None).await }
    pub async fn get_config(&self) -> Result<Value, AgentError> { self.rpc("get_config", None).await }
    pub async fn update_config(&self, p: Value) -> Result<Value, AgentError> { self.rpc("update_config", Some(p)).await }
    pub async fn list_conversations(&self) -> Result<Value, AgentError> { self.rpc("list_conversations", None).await }
    pub async fn get_current_conversation(&self) -> Result<Value, AgentError> { self.rpc("get_current_conversation", None).await }
    pub async fn create_conversation(&self, t: Option<&str>) -> Result<Value, AgentError> { self.rpc("create_conversation", t.map(|v| serde_json::json!({ "title": v }))).await }
    pub async fn switch_conversation(&self, id: &str) -> Result<Value, AgentError> { self.rpc("switch_conversation", Some(serde_json::json!({ "id": id }))).await }
    pub async fn delete_conversation(&self, id: &str) -> Result<Value, AgentError> { self.rpc("delete_conversation", Some(serde_json::json!({ "id": id }))).await }
    pub async fn rename_conversation(&self, id: &str, title: &str) -> Result<Value, AgentError> { self.rpc("rename_conversation", Some(serde_json::json!({ "id": id, "title": title }))).await }
    pub async fn list_memories(&self) -> Result<Value, AgentError> { self.rpc("list_memories", None).await }
    pub async fn forget_memory(&self, id: &str) -> Result<Value, AgentError> { self.rpc("forget_memory", Some(serde_json::json!({ "id": id }))).await }
    pub async fn transcribe_audio(&self, audio: &[f32], api_key: &str, base_url: &str) -> Result<String, AgentError> {
        let r = self.rpc("transcribe_audio", Some(serde_json::json!({ "audio": audio, "api_key": api_key, "base_url": base_url }))).await?;
        Ok(r.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string())
    }
    pub async fn synthesize_audio(&self, text: &str, voice: &str, api_key: &str, base_url: &str) -> Result<Vec<f32>, AgentError> {
        let r = self.rpc("synthesize_audio", Some(serde_json::json!({ "text": text, "voice": voice, "api_key": api_key, "base_url": base_url }))).await?;
        Ok(r.get("pcm").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect()).unwrap_or_default())
    }
}

#[async_trait]
impl AgentEngine for OmpAgentSidecar {
    async fn chat(&self, message: &str, history: &[ConversationMessage], sp: Option<&str>) -> Result<AgentResponse, AgentError> {
        let hj: Vec<Value> = history.iter().map(|m| serde_json::json!({ "role": match m.role { super::MessageRole::User=>"user", super::MessageRole::Assistant=>"assistant", super::MessageRole::System=>"system", super::MessageRole::Tool=>"tool" }, "content": m.content })).collect();
        let mut params = serde_json::json!({ "message": message, "history": hj });
        if let Some(s) = sp { params["system_prompt"] = Value::String(s.to_string()); }
        let r = self.rpc("chat", Some(params)).await?;
        Ok(AgentResponse { text: r.get("text").and_then(|v| v.as_str()).unwrap_or("").into() })
    }
}
