//! Bun Agent Sidecar client — communicates with the Bun sidecar process
//! via stdin/stdout JSON-RPC (NDJSON protocol).
//!
//! Replaces the old `omp -p` subprocess approach with a persistent Bun
//! sidecar powered by `@oh-my-pi/pi-agent-core`.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use tokio::sync::Mutex;

use async_trait::async_trait;
use serde_json::Value;

use super::{AgentEngine, AgentError, AgentResponse, AgentStreamEvent, ConversationMessage};

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct JsonRpcRequest {
    id: String,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
}

#[derive(serde::Deserialize, Debug)]
struct JsonRpcResponse {
    id: String,
    #[serde(rename = "type")]
    r#type: String,
    result: Option<Value>,
    event: Option<String>,
    data: Option<Value>,
    error: Option<JsonRpcError>,
}

#[derive(serde::Deserialize, Debug)]
#[allow(dead_code)]
struct JsonRpcError {
    message: String,
    code: Option<i64>,
}

pub struct OmpAgentSidecar {
    bun_binary: String,
    sidecar_script: String,
    process: Arc<Mutex<Option<SidecarProcess>>>,
    model_info: Arc<Mutex<Option<Value>>>,
}

#[allow(dead_code)]
struct SidecarProcess {
    child: Child,
    stdin_writer: Box<dyn Write + Send>,
    stdout_reader: BufReader<Box<dyn std::io::Read + Send>>,
    next_id: u64,
}

impl OmpAgentSidecar {
    const SIDECAR_SCRIPT_RELATIVE: &str = "services/agent-sidecar/src/index.ts";

    #[cfg(debug_assertions)]
    fn project_root() -> std::path::PathBuf {
        let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.pop();
        path
    }

    #[cfg(not(debug_assertions))]
    fn project_root() -> std::path::PathBuf {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| std::path::PathBuf::from("."))
    }

    fn resolve_bun() -> String {
        #[cfg(target_os = "windows")]
        {
            let candidates = [
                format!(r"{}\npm\bun.cmd", std::env::var("APPDATA").unwrap_or_default()),
                format!(r"{}\bun\bin\bun.exe", std::env::var("USERPROFILE").unwrap_or_default()),
                "bun.cmd".to_string(),
                "bun".to_string(),
            ];
            for path in &candidates {
                if std::path::Path::new(path).exists() {
                    log::info!("found bun at: {path}");
                    return path.clone();
                }
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let candidates = [
                std::env::var("HOME").unwrap_or_default() + "/.bun/bin/bun",
                "/usr/local/bin/bun",
                "/opt/homebrew/bin/bun",
                "bun",
            ];
            for path in &candidates {
                if std::path::Path::new(path).exists() {
                    log::info!("found bun at: {path}");
                    return path.to_string();
                }
            }
        }
        log::info!("bun not found at known paths; trying PATH lookup");
        "bun".to_string()
    }

    pub fn new() -> Self {
        let bun = Self::resolve_bun();
        let script = Self::project_root().join(Self::SIDECAR_SCRIPT_RELATIVE);
        log::info!("Agent sidecar script: {}", script.display());
        Self {
            bun_binary: bun,
            sidecar_script: script.to_string_lossy().to_string(),
            process: Arc::new(Mutex::new(None)),
            model_info: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn spawn(&self) -> Result<(), AgentError> {
        if !std::path::Path::new(&self.sidecar_script).exists() {
            return Err(AgentError::SubprocessCrashed(
                format!("Sidecar not found: {}", self.sidecar_script)
            ));
        }

        let mut child = Command::new(&self.bun_binary)
            .arg("run")
            .arg(&self.sidecar_script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| AgentError::SubprocessCrashed(
                format!("Spawn sidecar: {e}")
            ))?;

        let stdin = child.stdin.take().ok_or_else(|| {
            AgentError::SubprocessCrashed("No sidecar stdin".into())
        })?;

        let stdout = child.stdout.take().ok_or_else(|| {
            AgentError::SubprocessCrashed("No sidecar stdout".into())
        })?;

        let proc = SidecarProcess {
            child,
            stdin_writer: Box::new(stdin),
            stdout_reader: BufReader::new(Box::new(stdout)),
            next_id: 1,
        };

        *self.process.lock().await = Some(proc);
        log::info!("Agent sidecar spawned");

        // Ping to verify
        let ping_result = self.send_request("ping", None).await?;
        if let Some(info) = ping_result.get("model") {
            *self.model_info.lock().await = Some(info.clone());
        }

        Ok(())
    }

    async fn send_request(&self, method: &str, params: Option<Value>) -> Result<Value, AgentError> {
        let mut guard = self.process.lock().await;
        let proc = guard.as_mut().ok_or(AgentError::NotRunning)?;

        let id = format!("r{}", proc.next_id);
        proc.next_id += 1;

        let request = JsonRpcRequest {
            id: id.clone(),
            method: method.to_string(),
            params,
        };

        let json = serde_json::to_string(&request)
            .map_err(|e| AgentError::RpcError(format!("Serialize: {e}")))?;

        writeln!(proc.stdin_writer, "{json}")
            .map_err(|e| AgentError::SubprocessCrashed(format!("Write: {e}")))?;
        proc.stdin_writer.flush()
            .map_err(|e| AgentError::SubprocessCrashed(format!("Flush: {e}")))?;

        let mut line = String::new();
        loop {
            line.clear();
            proc.stdout_reader.read_line(&mut line)
                .map_err(|e| AgentError::SubprocessCrashed(format!("Read: {e}")))?;
            if line.is_empty() {
                return Err(AgentError::SubprocessCrashed("Sidecar ended".into()));
            }
            let trimmed = line.trim();
            if trimmed.is_empty() { continue; }

            let resp: JsonRpcResponse = serde_json::from_str(trimmed)
                .map_err(|e| AgentError::RpcError(format!("Parse: {e}")))?;
            if resp.id != id { continue; }

            return match resp.r#type.as_str() {
                "result" => Ok(resp.result.unwrap_or(Value::Null)),
                "error" => {
                    let msg = resp.error.map(|e| e.message).unwrap_or_else(|| "Unknown".into());
                    Err(AgentError::AgentReturnedError(msg))
                }
                _ => continue,
            };
        }
    }

    #[allow(dead_code)]
    async fn send_stream_request<F>(&self, method: &str, params: Option<Value>, mut on_event: F) -> Result<(), AgentError>
    where F: FnMut(&str, &Value) {
        let mut guard = self.process.lock().await;
        let proc = guard.as_mut().ok_or(AgentError::NotRunning)?;

        let id = format!("r{}", proc.next_id);
        proc.next_id += 1;

        let request = JsonRpcRequest {
            id: id.clone(),
            method: method.to_string(),
            params,
        };

        let json = serde_json::to_string(&request)
            .map_err(|e| AgentError::RpcError(format!("Serialize: {e}")))?;

        writeln!(proc.stdin_writer, "{json}")
            .map_err(|e| AgentError::SubprocessCrashed(format!("Write: {e}")))?;
        proc.stdin_writer.flush()
            .map_err(|e| AgentError::SubprocessCrashed(format!("Flush: {e}")))?;

        let mut line = String::new();
        loop {
            line.clear();
            proc.stdout_reader.read_line(&mut line)
                .map_err(|e| AgentError::SubprocessCrashed(format!("Read: {e}")))?;
            if line.is_empty() {
                return Err(AgentError::SubprocessCrashed("Sidecar ended".into()));
            }
            let trimmed = line.trim();
            if trimmed.is_empty() { continue; }

            let resp: JsonRpcResponse = serde_json::from_str(trimmed)
                .map_err(|e| AgentError::RpcError(format!("Parse: {e}")))?;
            if resp.id != id { continue; }

            match resp.r#type.as_str() {
                "event" => {
                    let name = match &resp.event { Some(n) => n.as_str(), None => continue };
                    let data = match &resp.data { Some(d) => d.clone(), None => Value::Null };
                    if name == "done" { return Ok(()) }
                    on_event(name, &data);
                }
                "error" => {
                    let msg = resp.error.map(|e| e.message).unwrap_or_else(|| "Stream error".into());
                    return Err(AgentError::AgentReturnedError(msg));
                }
                _ => {}
            }
        }
    }

    pub async fn is_running(&self) -> bool {
        self.process.lock().await.is_some()
    }

    pub async fn set_model(&self, _model: String) {
        // The sidecar handles model switching via the config file.
        // Full runtime model switching requires a restart of the sidecar,
        // which we will implement in a future update.
        log::info!("model switch to {} requested (sidecar restart needed)", _model);
        // For now, the sidecar loads from ~/.omp/agent/config.yml on each `chat` call
        // since it reads the default model role from there.
    }

    pub async fn get_model_info(&self) -> Option<Value> {
        self.model_info.lock().await.clone()
    }

    /// Tell the sidecar to clear its conversation history.
    pub async fn clear_history(&self) -> Result<(), AgentError> {
        self.send_request("clear_history", None).await?;
        Ok(())
    }
}

#[async_trait]
impl AgentEngine for OmpAgentSidecar {
    async fn chat(&self, message: &str, history: &[ConversationMessage]) -> Result<AgentResponse, AgentError> {
        let history_json: Vec<Value> = history.iter().map(|msg| {
            let role = match msg.role {
                super::MessageRole::User => "user",
                super::MessageRole::Assistant => "assistant",
                super::MessageRole::System => "system",
                super::MessageRole::Tool => "tool",
            };
            serde_json::json!({ "role": role, "content": msg.content })
        }).collect();

        let params = serde_json::json!({ "message": message, "history": history_json });
        let result = self.send_request("chat", Some(params)).await?;
        let text = result.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string();
        Ok(AgentResponse { text, tool_calls: vec![] })
    }

    async fn chat_stream(&self, message: &str, history: &[ConversationMessage]) -> Result<tokio::sync::mpsc::Receiver<AgentStreamEvent>, AgentError> {
        let (tx, rx) = tokio::sync::mpsc::channel(128);

        let history_json: Vec<Value> = history.iter().map(|msg| {
            let role = match msg.role {
                super::MessageRole::User => "user",
                super::MessageRole::Assistant => "assistant",
                super::MessageRole::System => "system",
                super::MessageRole::Tool => "tool",
            };
            serde_json::json!({ "role": role, "content": msg.content })
        }).collect();

        let params = serde_json::json!({ "message": message, "history": history_json });
        let bun = self.bun_binary.clone();
        let script = self.sidecar_script.clone();

        tokio::spawn(async move {
            let child = match Command::new(&bun).arg("run").arg(&script)
                .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()
            {
                Ok(c) => c,
                Err(e) => { let _ = tx.send(AgentStreamEvent::Error(e.to_string())).await; return; }
            };

            let mut stdin = match child.stdin {
                Some(s) => s,
                None => { let _ = tx.send(AgentStreamEvent::Error("No stdin".into())).await; return; }
            };

            let stdout = match child.stdout {
                Some(s) => s,
                None => { let _ = tx.send(AgentStreamEvent::Error("No stdout".into())).await; return; }
            };

            let request = JsonRpcRequest {
                id: "s1".to_string(),
                method: "chat_stream".to_string(),
                params: Some(params),
            };

            let json = match serde_json::to_string(&request) {
                Ok(j) => j,
                Err(e) => { let _ = tx.send(AgentStreamEvent::Error(e.to_string())).await; return; }
            };

            if writeln!(stdin, "{json}").is_err() || stdin.flush().is_err() {
                let _ = tx.send(AgentStreamEvent::Error("Write to sidecar failed".into())).await;
                return;
            }

            let mut reader = BufReader::new(stdout);
            let mut line = String::new();

            loop {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) => { let _ = tx.send(AgentStreamEvent::Done).await; return; }
                    Err(e) => { let _ = tx.send(AgentStreamEvent::Error(e.to_string())).await; return; }
                    Ok(_) => {}
                }

                let trimmed = line.trim();
                if trimmed.is_empty() { continue; }

                let resp: JsonRpcResponse = match serde_json::from_str(trimmed) {
                    Ok(r) => r,
                    Err(_) => continue,
                };

                if resp.id != "s1" { continue; }

                match resp.r#type.as_str() {
                    "event" => match resp.event.as_deref() {
                        Some("token") => {
                            if let Some(t) = resp.data.as_ref().and_then(|d| d.get("token")).and_then(|v| v.as_str()) {
                                let _ = tx.send(AgentStreamEvent::Token(t.to_string())).await;
                            }
                        }
                        Some("tool_start") => {
                            if let Some(n) = resp.data.as_ref().and_then(|d| d.get("name")).and_then(|v| v.as_str()) {
                                let _ = tx.send(AgentStreamEvent::ToolStarted { name: n.to_string() }).await;
                            }
                        }
                        Some("tool_end") => {
                            let name = resp.data.as_ref().and_then(|d| d.get("name")).and_then(|v| v.as_str());
                            let result = resp.data.as_ref().and_then(|d| d.get("result")).and_then(|v| v.as_str());
                            if let Some(n) = name {
                                let _ = tx.send(AgentStreamEvent::ToolCompleted {
                                    name: n.to_string(),
                                    result: result.unwrap_or("").to_string(),
                                }).await;
                            }
                        }
                        Some("done") => { let _ = tx.send(AgentStreamEvent::Done).await; return; }
                        _ => {}
                    },
                    "error" => {
                        let msg = resp.error.map(|e| e.message).unwrap_or_else(|| "Stream error".into());
                        let _ = tx.send(AgentStreamEvent::Error(msg)).await;
                        return;
                    }
                    _ => {}
                }
            }
        });

        Ok(rx)
    }
}
