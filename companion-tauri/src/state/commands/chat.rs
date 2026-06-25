//! Chat-related IPC commands.

use companion_core::agent::{AgentEngine, ConversationMessage, MessageRole};
use tauri::Emitter;
use crate::state::{AgentState, ConfigState};

#[tauri::command]
pub async fn chat(
    agent: tauri::State<'_, AgentState>,
    config: tauri::State<'_, ConfigState>,
    message: String,
) -> Result<String, String> {
    if !agent.agent.is_running().await {
        agent.agent.spawn().await.map_err(|e| format!("spawn: {e}"))?;
        config.sync_from_sidecar(&agent.agent).await?;
    }
    let system_prompt = config.config.lock().await.custom_system_prompt.clone();
    let response = agent.agent.chat(&message, &[], Some(&system_prompt)).await
        .map_err(|e| format!("Agent error: {e}"))?;
    Ok(response.text)
}

#[tauri::command]
pub async fn chat_stream(
    app: tauri::AppHandle,
    agent: tauri::State<'_, AgentState>,
    config: tauri::State<'_, ConfigState>,
    message: String,
    history: Option<Vec<serde_json::Value>>,
) -> Result<(), String> {
    log::info!("chat_stream: \"{}\"", &message[..message.len().min(30)]);
    if !agent.agent.is_running().await {
        agent.agent.spawn().await.map_err(|e| format!("spawn: {e}"))?;
        config.sync_from_sidecar(&agent.agent).await?;
    }
    let hist = history.unwrap_or_default();
    let mut rx = agent.agent.chat_stream_tokens(&message, &hist).await
        .map_err(|e| format!("stream: {e}"))?;

    let app2 = app.clone();
    tokio::spawn(async move {
        while let Some(token) = rx.recv().await {
            if token.starts_with('\0') {
                let text = &token[1..];
                let _ = app2.emit("chat_token", serde_json::json!({ "done": true, "token": text }));
                return;
            }
            let _ = app2.emit("chat_token", serde_json::json!({ "token": token }));
        }
        let _ = app2.emit("chat_token", serde_json::json!({ "done": true, "token": "⚠️ Connection lost." }));
    });
    Ok(())
}

#[tauri::command]
pub async fn get_history(agent: tauri::State<'_, AgentState>) -> Result<Vec<ConversationMessage>, String> {
    if !agent.agent.is_running().await { return Ok(vec![]); }
    let val = agent.agent.get_history().await.map_err(|e| format!("get_history: {e}"))?;
    let arr = val.get("history").and_then(|v| v.as_array()).ok_or("bad history response")?;
    let mut out = Vec::new();
    for entry in arr {
        let role_str = entry.get("role").and_then(|v| v.as_str()).unwrap_or("user");
        let content = entry.get("content").and_then(|v| v.as_str()).unwrap_or("");
        let role = match role_str {
            "assistant" => MessageRole::Assistant,
            "system" => MessageRole::System,
            "tool" => MessageRole::Tool,
            _ => MessageRole::User,
        };
        out.push(ConversationMessage { role, content: content.to_string() });
    }
    Ok(out)
}

#[tauri::command]
pub async fn clear_history(agent: tauri::State<'_, AgentState>) -> Result<(), String> {
    if agent.agent.is_running().await {
        agent.agent.clear_history().await.map_err(|e| format!("clear history: {e}"))?;
    }
    Ok(())
}
