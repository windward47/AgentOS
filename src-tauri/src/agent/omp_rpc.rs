//! oh-my-pi (omp) subprocess integration.
//!
//! Uses `omp -p` (print mode) for synchronous chat: spawns a fresh omp process
//! per request, reads stdout, and returns the text.  Supports model switching
//! via `--model` and tool injection via a tool-calling loop with `ToolRegistry`.
//!
//! ## Tool-calling loop
//!
//! 1. Append tool definitions (JSON Schema) to the system prompt
//! 2. Call `omp -p` with the user's message
//! 3. If the response is a JSON tool-call object, execute it in `ToolRegistry`
//! 4. Feed the tool result back into omp as a follow-up message
//! 5. Repeat until omp produces a natural-language response
//!
//! Max 5 tool-calling iterations per chat (safety limit).

use std::process::{Command, Stdio};
use std::sync::Arc;
use super::{AgentEngine, AgentError, AgentResponse, AgentStreamEvent, ConversationMessage};
use crate::tools::ToolRegistry;
use async_trait::async_trait;

/// Map CompanionConfig `llm_provider` value to an omp `--model` argument.
///
/// omp accepts fuzzy model names — we pass the canonical short name
/// (e.g. "mimo-v2.5", "nex-agi/Nex-N2-Pro") and omp resolves it.
pub fn provider_to_model(provider: &str) -> &'static str {
    match provider {
        "siliconflow" => "nex-agi/Nex-N2-Pro",
        "xiaomi" => "mimo-v2.5-pro",
        _ => "mimo-v2.5", // openai / ollama / claude / default → sensenova fallback
    }
}

/// Format conversation history as a transcript to prepend to the message.
fn format_history(history: &[ConversationMessage]) -> String {
    if history.is_empty() {
        return String::new();
    }

    let mut transcript = String::from("## Previous conversation\n\n");
    for msg in history {
        let role = match msg.role {
            super::MessageRole::User => "User",
            super::MessageRole::Assistant => "Assistant",
            super::MessageRole::System => "System",
            super::MessageRole::Tool => "Tool",
        };
        transcript.push_str(&format!("**{role}:** {}\n\n", msg.content));
    }
    transcript.push_str("## Current message\n\n");
    transcript
}

/// Build the full prompt: history transcript + current message.
fn build_prompt(message: &str, history: &[ConversationMessage]) -> String {
    let hist = format_history(history);
    format!("{hist}{message}")
}
fn build_tool_prompt(registry: &ToolRegistry) -> String {
    let defs = registry.definitions();
    if defs.is_empty() {
        return String::new();
    }

    let mut prompt = String::from(
        "\n\n## Available Tools\n\n"
    );

    for def in &defs {
        let func = &def["function"];
        let name = func["name"].as_str().unwrap_or("?");
        let desc = func["description"].as_str().unwrap_or("");
        let params = serde_json::to_string_pretty(&func["parameters"]).unwrap_or_default();
        prompt.push_str(&format!("### {name}\n{desc}\n```json\n{params}\n```\n\n"));
    }

    prompt.push_str(
        "To use a tool, respond with ONLY a JSON object on a single line:\n\
         {\"name\":\"<tool_name>\",\"arguments\":{...}}\n\
         After calling a tool you will receive the result. Continue until done.\n"
    );

    prompt
}

/// Try to parse a tool-call JSON from omp's text response.
/// Returns `Some((name, args))` if the entire response is a single JSON tool call.
fn parse_tool_call(text: &str) -> Option<(String, serde_json::Value)> {
    let trimmed = text.trim();
    // Only parse as tool call if it starts with { and looks like {"name":
    if !trimmed.starts_with('{') {
        return None;
    }
    let parsed: serde_json::Value = serde_json::from_str(trimmed).ok()?;
    let name = parsed.get("name")?.as_str()?.to_string();
    let args = parsed.get("arguments").cloned().unwrap_or(serde_json::Value::Null);
    Some((name, args))
}

/// A client that spawns `omp -p` on every chat call.
/// Stateless — no subprocess persistence. Supports model switching and tool injection.
pub struct OmpRpcClient {
    /// Path to the omp binary.
    binary: String,
    /// Model name for omp `--model` flag (default: "mimo-v2.5").
    model: std::sync::Mutex<String>,
    /// Tool registry for the tool-calling loop (may be empty).
    tools: Option<Arc<ToolRegistry>>,
}

impl OmpRpcClient {
    pub fn new(binary: &str) -> Self {
        Self {
            binary: binary.to_string(),
            model: std::sync::Mutex::new("mimo-v2.5".to_string()),
            tools: None,
        }
    }

    /// Attach a tool registry for the tool-calling loop.
    pub fn with_tools(binary: &str, tools: Arc<ToolRegistry>) -> Self {
        Self {
            binary: binary.to_string(),
            model: std::sync::Mutex::new("mimo-v2.5".to_string()),
            tools: Some(tools),
        }
    }

    /// Update the model name dynamically (called when config changes).
    pub fn set_model(&self, model: String) {
        if let Ok(mut m) = self.model.lock() {
            *m = model;
        }
    }

    /// Run `omp -p` with optional tool prompts. 60-second timeout.
    async fn run_omp(
        &self,
        message: &str,
        tool_prompt: &str,
    ) -> Result<String, AgentError> {
        let binary = self.binary.clone();
        let msg = message.to_string();
        let model = self.model.lock().unwrap().clone();
        let tool_prompt = tool_prompt.to_string();

        let output = tokio::time::timeout(
            std::time::Duration::from_secs(60),
            tokio::task::spawn_blocking(move || {
                let mut cmd = Command::new(&binary);
                cmd.arg("-p")
                   .arg(&msg)
                   .arg("--no-session")
                   .arg("--model").arg(&model)
                   .stdout(Stdio::piped())
                   .stderr(Stdio::inherit());

                if !tool_prompt.is_empty() {
                    cmd.arg("--append-system-prompt").arg(&tool_prompt);
                }

                cmd.output()
            }),
        )
        .await
        .map_err(|_| AgentError::Timeout)?  // 60s elapsed
        .map_err(|e| AgentError::SubprocessCrashed(format!("spawn_blocking failed: {e}")))?
        .map_err(|e| AgentError::SubprocessCrashed(format!("omp process failed: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AgentError::AgentReturnedError(
                format!("omp exited with {}: {stderr}",
                    output.status.code().unwrap_or(-1))
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(stdout)
    }

    /// Tool-calling loop: call omp, parse tool calls, execute, feed back.
    /// Max 5 iterations.
    async fn chat_with_tools(
        &self,
        message: &str,
    ) -> Result<AgentResponse, AgentError> {
        const MAX_ITERATIONS: usize = 5;

        let tool_prompt = match &self.tools {
            Some(reg) => build_tool_prompt(reg),
            None => String::new(),
        };

        let mut current_message = message.to_string();
        let mut tool_calls_made: Vec<String> = Vec::new();

        for _ in 0..MAX_ITERATIONS {
            let response = self.run_omp(&current_message, &tool_prompt).await?;

            // Try to parse as tool call
            if let Some((tool_name, args)) = parse_tool_call(&response) {
                if let Some(reg) = &self.tools {
                    match reg.execute(&tool_name, args).await {
                        Ok(result) => {
                            let result_str = serde_json::to_string(&result)
                                .unwrap_or_else(|_| "{}".to_string());
                            tool_calls_made.push(tool_name.clone());
                            current_message = format!(
                                "Tool result for {tool_name}: {result_str}\n\nPlease continue."
                            );
                            continue; // loop back with tool result
                        }
                        Err(e) => {
                            current_message = format!(
                                "Tool error for {tool_name}: {e}\n\nPlease respond to the user."
                            );
                            continue;
                        }
                    }
                }
            }

            // Not a tool call — this is the final natural-language response
            return Ok(AgentResponse {
                text: response,
                tool_calls: tool_calls_made,
            });
        }

        Ok(AgentResponse {
            text: "I tried to help but reached the maximum number of tool calls. Please try again with a simpler request.".into(),
            tool_calls: tool_calls_made,
        })
    }
}

#[async_trait]
impl AgentEngine for OmpRpcClient {
    async fn chat(
        &self,
        message: &str,
        history: &[ConversationMessage],
    ) -> Result<AgentResponse, AgentError> {
        let prompt = build_prompt(message, history);
        if self.tools.is_some() {
            self.chat_with_tools(&prompt).await
        } else {
            let text = self.run_omp(&prompt, "").await?;
            Ok(AgentResponse { text, tool_calls: vec![] })
        }
    }

    async fn chat_stream(
        &self,
        message: &str,
        history: &[ConversationMessage],
    ) -> Result<tokio::sync::mpsc::Receiver<AgentStreamEvent>, AgentError> {
        let response = self.chat(message, history).await?;
        let (tx, rx) = tokio::sync::mpsc::channel(32);
        let text = response.text;
        let tool_calls = response.tool_calls;
        tokio::spawn(async move {
            // Emit tool-call events
            for tool in &tool_calls {
                let _ = tx.send(AgentStreamEvent::ToolStarted { name: tool.clone() }).await;
                let _ = tx.send(AgentStreamEvent::ToolCompleted { name: tool.clone(), result: String::new() }).await;
            }
            // Emit text tokens word-by-word
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
