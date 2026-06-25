//! Memory management IPC commands.

use crate::state::{AgentState, ConfigState};

#[tauri::command]
pub async fn list_memories(
    agent: tauri::State<'_, AgentState>, config: tauri::State<'_, ConfigState>,
) -> Result<serde_json::Value, String> {
    if !agent.agent.is_running().await { agent.agent.spawn().await.map_err(|e| format!("spawn: {e}"))?; config.sync_from_sidecar(&agent.agent).await?; }
    agent.agent.list_memories().await.map_err(|e| format!("list memories: {e}"))
}

#[tauri::command]
pub async fn forget_memory(
    agent: tauri::State<'_, AgentState>, config: tauri::State<'_, ConfigState>, id: String,
) -> Result<serde_json::Value, String> {
    if !agent.agent.is_running().await { agent.agent.spawn().await.map_err(|e| format!("spawn: {e}"))?; config.sync_from_sidecar(&agent.agent).await?; }
    agent.agent.forget_memory(&id).await.map_err(|e| format!("forget memory: {e}"))
}
