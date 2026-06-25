//! ASR/TTS media IPC commands.

use companion_core::config::{resolve_provider_key, ensure_chat_completions_url};
use crate::state::{AgentState, ConfigState};

#[tauri::command]
pub async fn transcribe_audio(
    config: tauri::State<'_, ConfigState>, agent: tauri::State<'_, AgentState>, audio: Vec<f32>,
) -> Result<String, String> {
    if !agent.agent.is_running().await { agent.agent.spawn().await.map_err(|e| format!("spawn: {e}"))?; config.sync_from_sidecar(&agent.agent).await?; }
    let cfg = config.config.lock().await;
    let api_key = resolve_provider_key(&cfg.asr, &cfg.default_api_key);
    let base_url = ensure_chat_completions_url(&cfg.asr.url.clone().unwrap_or_else(|| "https://token-plan-cn.xiaomimimo.com/v1".into()));
    agent.agent.transcribe_audio(&audio, &api_key, &base_url).await.map_err(|e| format!("ASR: {e}"))
}

#[tauri::command]
pub async fn synthesize_audio(
    config: tauri::State<'_, ConfigState>, agent: tauri::State<'_, AgentState>, text: String, voice: Option<String>,
) -> Result<Vec<f32>, String> {
    if !agent.agent.is_running().await { agent.agent.spawn().await.map_err(|e| format!("spawn: {e}"))?; config.sync_from_sidecar(&agent.agent).await?; }
    let cfg = config.config.lock().await;
    let v = voice.unwrap_or_else(|| cfg.tts_voice.clone());
    let api_key = resolve_provider_key(&cfg.tts, &cfg.default_api_key);
    let base_url = ensure_chat_completions_url(&cfg.tts.url.clone().unwrap_or_else(|| "https://token-plan-cn.xiaomimimo.com/v1".into()));
    agent.agent.synthesize_audio(&text, &v, &api_key, &base_url).await.map_err(|e| format!("TTS: {e}"))
}
