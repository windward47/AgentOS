//! Config IPC commands.

use std::sync::atomic::Ordering;
use companion_core::config::CompanionConfig;
use crate::state::{AgentState, ConfigState};

#[tauri::command]
pub async fn get_config(config: tauri::State<'_, ConfigState>) -> Result<CompanionConfig, String> {
    Ok(config.config.lock().await.clone())
}

#[tauri::command]
pub async fn update_config(
    config: tauri::State<'_, ConfigState>, agent: tauri::State<'_, AgentState>, new_config: CompanionConfig,
) -> Result<(), String> {
    let val = serde_json::to_value(&new_config).map_err(|e| format!("serialize: {e}"))?;
    config.push_to_sidecar(&agent.agent, val).await?;
    config.system_mode.swap(new_config.system_mode, Ordering::SeqCst);
    Ok(())
}
