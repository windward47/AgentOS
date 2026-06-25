//! Conversation session management IPC commands.

use crate::state::{AgentState, ConfigState};

#[tauri::command]
pub async fn list_conversations(
    agent: tauri::State<'_, AgentState>, config: tauri::State<'_, ConfigState>,
) -> Result<serde_json::Value, String> {
    if !agent.agent.is_running().await { agent.agent.spawn().await.map_err(|e| format!("spawn: {e}"))?; config.sync_from_sidecar(&agent.agent).await?; }
    agent.agent.list_conversations().await.map_err(|e| format!("list conversations: {e}"))
}

#[tauri::command]
pub async fn get_current_conversation(
    agent: tauri::State<'_, AgentState>, config: tauri::State<'_, ConfigState>,
) -> Result<serde_json::Value, String> {
    if !agent.agent.is_running().await { agent.agent.spawn().await.map_err(|e| format!("spawn: {e}"))?; config.sync_from_sidecar(&agent.agent).await?; }
    agent.agent.get_current_conversation().await.map_err(|e| format!("get current: {e}"))
}

#[tauri::command]
pub async fn create_conversation(
    agent: tauri::State<'_, AgentState>, config: tauri::State<'_, ConfigState>, title: Option<String>,
) -> Result<serde_json::Value, String> {
    if !agent.agent.is_running().await { agent.agent.spawn().await.map_err(|e| format!("spawn: {e}"))?; config.sync_from_sidecar(&agent.agent).await?; }
    agent.agent.create_conversation(title.as_deref()).await.map_err(|e| format!("create: {e}"))
}

#[tauri::command]
pub async fn switch_conversation(
    agent: tauri::State<'_, AgentState>, config: tauri::State<'_, ConfigState>, id: String,
) -> Result<serde_json::Value, String> {
    if !agent.agent.is_running().await { agent.agent.spawn().await.map_err(|e| format!("spawn: {e}"))?; config.sync_from_sidecar(&agent.agent).await?; }
    agent.agent.switch_conversation(&id).await.map_err(|e| format!("switch: {e}"))
}

#[tauri::command]
pub async fn delete_conversation(
    agent: tauri::State<'_, AgentState>, config: tauri::State<'_, ConfigState>, id: String,
) -> Result<serde_json::Value, String> {
    if !agent.agent.is_running().await { agent.agent.spawn().await.map_err(|e| format!("spawn: {e}"))?; config.sync_from_sidecar(&agent.agent).await?; }
    agent.agent.delete_conversation(&id).await.map_err(|e| format!("delete: {e}"))
}

#[tauri::command]
pub async fn rename_conversation(
    agent: tauri::State<'_, AgentState>, config: tauri::State<'_, ConfigState>, id: String, title: String,
) -> Result<serde_json::Value, String> {
    if !agent.agent.is_running().await { agent.agent.spawn().await.map_err(|e| format!("spawn: {e}"))?; config.sync_from_sidecar(&agent.agent).await?; }
    agent.agent.rename_conversation(&id, &title).await.map_err(|e| format!("rename: {e}"))
}
