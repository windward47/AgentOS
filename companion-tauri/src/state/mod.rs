//! Domain-level application states for Tauri's managed state system.
//!
//! Split from a monolithic `AppState` into focused domain states so that
//! each Tauri command depends on exactly what it needs — no more, no less.

use companion_core::agent::omp_sidecar::OmpAgentSidecar;
use companion_core::agent::{AgentEngine, ConversationMessage, MessageRole, provider_to_model};
use companion_core::asr::AsrProvider;
use companion_core::tts::TtsProvider;
use companion_core::config::{CompanionConfig, ConfigManager, resolve_provider_key, ensure_chat_completions_url};
use companion_core::downloader::{download_model, DownloadProgress};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::{Manager, Emitter};

// ── AgentState ──────────────────────────────────────────────────────────

/// Manages the agent engine and conversation history.
pub struct AgentState {
    pub agent: Arc<OmpAgentSidecar>,
}

impl AgentState {
    pub fn new() -> Self {
        Self {
            agent: Arc::new(OmpAgentSidecar::new()),
        }
    }
}

// ── VoiceState ──────────────────────────────────────────────────────────

/// Tracks the current audio/voice state for Live2D animation.
pub struct VoiceState {
    pub is_speaking: AtomicBool,
    pub is_listening: AtomicBool,
    pub lip_level: std::sync::Mutex<f32>,
}

impl VoiceState {
    pub fn new() -> Self {
        Self {
            is_speaking: AtomicBool::new(false),
            is_listening: AtomicBool::new(false),
            lip_level: std::sync::Mutex::new(0.0),
        }
    }
}

// ── ConfigState ──────────────────────────────────────────────────────────

/// Application configuration with persistence.
pub struct ConfigState {
    pub config: Arc<Mutex<CompanionConfig>>,
    pub config_manager: ConfigManager,
    pub system_mode: AtomicBool,
}

impl ConfigState {
    pub fn new() -> Self {
        let config_manager = ConfigManager::new().expect("failed to init config manager");
        let config = config_manager.load().expect("failed to load config");
        Self {
            system_mode: AtomicBool::new(config.system_mode),
            config: Arc::new(Mutex::new(config)),
            config_manager,
        }
    }

    pub async fn save(&self) {
        let config = self.config.lock().await;
        self.config_manager.save(&config).ok();
    }

    /// Pull config from sidecar (B1a: sidecar owns config.json).
    pub async fn sync_from_sidecar(&self, agent: &OmpAgentSidecar) -> Result<(), String> {
        let val = agent.get_config().await.map_err(|e| format!("get_config RPC: {e}"))?;
        let cfg: CompanionConfig = serde_json::from_value(val)
            .map_err(|e| format!("parse config: {e}"))?;
        self.system_mode.store(cfg.system_mode, Ordering::Relaxed);
        *self.config.lock().await = cfg;
        Ok(())
    }

    /// Push config update to sidecar, get merged result back.
    pub async fn push_to_sidecar(&self, agent: &OmpAgentSidecar, partial: serde_json::Value) -> Result<(), String> {
        let val = agent.update_config(partial).await.map_err(|e| format!("update_config RPC: {e}"))?;
        let cfg: CompanionConfig = serde_json::from_value(val)
            .map_err(|e| format!("parse config: {e}"))?;
        self.system_mode.store(cfg.system_mode, Ordering::Relaxed);
        *self.config.lock().await = cfg;
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tauri IPC Commands
// ═══════════════════════════════════════════════════════════════════════

/// Shared core: spawn sidecar, chat, sync config on first spawn.
async fn do_chat(
    agent: &AgentState,
    config: &ConfigState,
    message: String,
) -> Result<String, String> {
    if !agent.agent.is_running().await {
        agent.agent.spawn().await
            .map_err(|e| format!("Sidecar spawn failed: {e}"))?;
        // B1a: sync config from sidecar (now the single source of truth)
        config.sync_from_sidecar(&agent.agent).await?;
    }
    let system_prompt = config.config.lock().await.custom_system_prompt.clone();
    // B1b: history is managed by sidecar; pass empty (sidecar has its own)
    let response = agent.agent.chat(&message, &[], Some(&system_prompt)).await
        .map_err(|e| format!("Agent error: {e}"))?;
    Ok(response.text)
}

#[tauri::command]
pub async fn chat(
    agent: tauri::State<'_, AgentState>,
    config: tauri::State<'_, ConfigState>,
    message: String,
) -> Result<String, String> {
    do_chat(&agent, &config, message).await
}

#[tauri::command]
pub async fn get_history(
    agent: tauri::State<'_, AgentState>,
) -> Result<Vec<ConversationMessage>, String> {
    if !agent.agent.is_running().await {
        return Ok(vec![]);
    }
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

#[tauri::command]
pub async fn transcribe_audio(
    config: tauri::State<'_, ConfigState>,
    audio: Vec<f32>,
) -> Result<String, String> {
    let cfg = config.config.lock().await;
    let base_url = ensure_chat_completions_url(
        &cfg.asr.url.clone().unwrap_or_else(||
            "https://token-plan-cn.xiaomimimo.com/v1".into()
        )
    );
    let api_key = resolve_provider_key(&cfg.asr, &cfg.default_api_key);
    let asr = companion_core::asr::xiaomi_asr::XiaomiAsr::with_url(&api_key, &base_url);
    asr.transcribe(&audio)
        .await
        .map_err(|e| format!("ASR error: {e}"))
}

#[tauri::command]
pub async fn synthesize_audio(
    config: tauri::State<'_, ConfigState>,
    text: String,
    voice: Option<String>,
) -> Result<Vec<f32>, String> {
    let cfg = config.config.lock().await;
    let v = voice.unwrap_or_else(|| cfg.tts_voice.clone());
    let baser_url = ensure_chat_completions_url(
        &cfg.tts.url.clone().unwrap_or_else(||
            "https://token-plan-cn.xiaomimimo.com/v1".into()
        )
    );
    let api_key = resolve_provider_key(&cfg.tts, &cfg.default_api_key);
    let tts = companion_core::tts::xiaomi_tts::XiaomiTts::with_url(&api_key, &v, &baser_url);
    tts.synthesize(&text)
        .await
        .map_err(|e| format!("TTS error: {e}"))
}

#[tauri::command]
pub async fn get_config(
    config: tauri::State<'_, ConfigState>,
) -> Result<CompanionConfig, String> {
    let cfg = config.config.lock().await;
    Ok(cfg.clone())
}

#[tauri::command]
pub async fn update_config(
    config: tauri::State<'_, ConfigState>,
    agent: tauri::State<'_, AgentState>,
    new_config: CompanionConfig,
) -> Result<(), String> {
    // B1a: sidecar owns config.json — push update to sidecar
    let val = serde_json::to_value(&new_config).map_err(|e| format!("serialize: {e}"))?;
    config.push_to_sidecar(&agent.agent, val).await?;

    let _was_mode = config
        .system_mode
        .swap(new_config.system_mode, Ordering::SeqCst);
    let model = provider_to_model(&new_config.llm_provider).to_string();
    agent.agent.set_model(model).await;
    Ok(())
}

#[tauri::command]
pub async fn set_lip_level(
    voice: tauri::State<'_, VoiceState>,
    level: f32,
) -> Result<(), String> {
    *voice.lip_level.lock().unwrap() = level.clamp(0.0, 1.0);
    Ok(())
}

#[tauri::command]
pub async fn get_lip_level(voice: tauri::State<'_, VoiceState>) -> Result<f32, String> {
    Ok(*voice.lip_level.lock().unwrap())
}

#[tauri::command]
pub async fn get_voice_state(voice: tauri::State<'_, VoiceState>) -> Result<String, String> {
    let s = if voice.is_speaking.load(Ordering::Relaxed) {
        "speaking"
    } else if voice.is_listening.load(Ordering::Relaxed) {
        "listening"
    } else {
        "idle"
    };
    log::trace!("get_voice_state → {}", s);
    Ok(s.into())
}

/// List available Live2D models from ~/.companion/models/ and web/public/live2d/models/.
#[tauri::command]
pub async fn list_live2d_models() -> Result<Vec<String>, String> {
    let mut models = Vec::new();
    let home = dirs::home_dir().unwrap_or_default();
    let user_dir = home.join(".companion").join("models");

    // Scan ~/.companion/models/ for model3.json files
    if user_dir.exists() {
        scan_models(&user_dir, &user_dir, &mut models);
    }
    // Also scan web/public/live2d/models/ (for bundled haru)
    let base = std::env::current_dir().unwrap_or_default();
    let root = if base.ends_with("companion-tauri") { base.parent().unwrap_or(&base).to_path_buf() } else { base };
    let web_dir = root.join("web").join("public").join("live2d").join("models");
    if web_dir.exists() {
        scan_models(&web_dir, &web_dir, &mut models);
    }

    // Fallback
    if models.is_empty() {
        models.push("haru/haru.model3.json".into());
    }
    // Filter: exclude known-incompatible models (.cmo3 format, moc3 v6)
    models.retain(|p| {
        let lower = p.to_lowercase();
        !lower.contains("epsilon")
        && !(lower.starts_with("ren_") || lower.contains("/ren."))
        && !lower.contains("miku_pro")
    });
    models.dedup();
    // Also deduplicate by stripping intermediate "kei_zh/" prefix (home vs web dirs differ)
    let mut seen = std::collections::HashSet::new();
    models.retain(|p| {
        let key = p.split('/').last().unwrap_or(p).to_string();
        seen.insert(key)
    });
    models.sort();
    Ok(models)
}

fn scan_models(dir: &std::path::Path, base: &std::path::Path, out: &mut Vec<String>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name.starts_with('.') { continue; }
            if path.is_dir() {
                scan_models(&path, base, out);
            } else if name.ends_with(".model3.json") {
                if let Ok(rel) = path.strip_prefix(base) {
                    let r = rel.to_string_lossy().replace('\\', "/");
                    if !out.contains(&r) { out.push(r); }
                }
            }
        }
    }
}

/// Tell the avatar window to switch to a different model.
#[tauri::command]
pub async fn set_live2d_model(app: tauri::AppHandle, model_path: String) -> Result<(), String> {
    app.emit("switch_live2d_model", model_path)
        .map_err(|e| format!("emit: {e}"))?;
    Ok(())
}

/// Download a Live2D model zip from a URL and extract to ~/.companion/models/.
/// Emits `download_progress` events: { phase: "downloading"|"extracting"|"done"|"error", ... }
#[tauri::command]
pub async fn cmd_download_model(
    app: tauri::AppHandle,
    url: String,
    model_id: String,
) -> Result<(), String> {
    let home = dirs::home_dir().unwrap_or_default();
    let dest = home.join(".companion").join("models");
    std::fs::create_dir_all(&dest).map_err(|e| format!("mkdir models: {e}"))?;

    let (tx, mut rx) = tokio::sync::mpsc::channel::<DownloadProgress>(32);
    let app_clone = app.clone();

    // Spawn download on a background task
    tokio::spawn(async move {
        if let Err(e) = download_model(&url, &model_id, &dest, tx).await {
            let _ = app_clone.emit("download_progress", DownloadProgress::Error { message: e });
        }
    });

    // Forward progress events to the frontend
    tokio::spawn(async move {
        while let Some(evt) = rx.recv().await {
            let _ = app.emit("download_progress", &evt);
        }
    });

    Ok(())
}

/// Show or hide the avatar (Live2D) window.
#[tauri::command]
pub async fn set_avatar_visible(app: tauri::AppHandle, visible: bool) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("avatar") {
        if visible {
            win.show().map_err(|e| format!("show avatar: {e}"))?;
        } else {
            win.hide().map_err(|e| format!("hide avatar: {e}"))?;
        }
    }
    Ok(())
}

/// Returns whether the avatar window is visible.
#[tauri::command]
pub async fn get_avatar_visible(app: tauri::AppHandle) -> Result<bool, String> {
    match app.get_webview_window("avatar") {
        Some(win) => win.is_visible().map_err(|e| format!("avatar visible: {e}")),
        None => Ok(false),
    }
}

/// Toggle always-on-top for the avatar window.
#[tauri::command]
pub async fn set_avatar_always_on_top(app: tauri::AppHandle, on_top: bool) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("avatar") {
        win.set_always_on_top(on_top)
            .map_err(|e| format!("set always on top: {e}"))?;
    }
    Ok(())
}

/// Reset avatar window position and notify the avatar to reset model position.
#[tauri::command]
pub async fn reset_avatar_position(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("main") {
        let main_size = win.outer_size().map_err(|e| format!("main size: {e}"))?;
        if let Some(avatar) = app.get_webview_window("avatar") {
            avatar
                .set_position(tauri::PhysicalPosition::new(
                    main_size.width as i32 + 100,
                    100,
                ))
                .map_err(|e| format!("set avatar pos: {e}"))?;
        }
    }
    // Tell the avatar window to reset model position + scale
    app.emit("reset_model_position", ())
        .map_err(|e| format!("emit reset_model_position: {e}"))?;
    Ok(())
}

#[tauri::command]
pub async fn get_cursor_pos() -> Result<(i32, i32), String> {
    use enigo::{Enigo, Mouse, Settings};
    let enigo =
        Enigo::new(&Settings::default()).map_err(|e| format!("enigo: {:?}", e))?;
    enigo.location().map_err(|e| format!("location: {:?}", e))
}

#[tauri::command]
pub async fn browse_screenshot(url: String) -> Result<String, String> {
    let lower = url.to_lowercase();
    if !lower.starts_with("http://") && !lower.starts_with("https://") {
        return Err("Only http:// and https:// URLs are allowed".into());
    }
    let cwd = std::env::current_dir().unwrap_or_default();
    // Tauri runs from companion-tauri/ — go up to project root to find scripts/
    let script = if cwd.ends_with("companion-tauri") {
        cwd.parent().unwrap_or(&cwd).join("scripts")
    } else {
        cwd.join("scripts")
    }
    .join("browser-screenshot.mjs");
    let tmp = std::env::temp_dir().join(format!(
        "companion_browse_{}_{}.png",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0)
    ));
    let output = std::process::Command::new("node")
        .arg(&script)
        .arg(&url)
        .arg(&tmp)
        .arg("15000")
        .output()
        .map_err(|e| format!("Browser script failed: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Browser failed: {stderr}"));
    }
    let bytes = std::fs::read(&tmp).map_err(|e| format!("Read screenshot: {e}"))?;
    std::fs::remove_file(&tmp).ok();
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    Ok(format!("data:image/png;base64,{}", STANDARD.encode(&bytes)))
}

#[tauri::command]
pub async fn get_audit_log() -> Result<String, String> {
    let path = dirs::home_dir()
        .unwrap_or_default()
        .join(".companion")
        .join("logs")
        .join("command.log");
    if !path.exists() {
        return Ok("(no logs yet)".into());
    }
    std::fs::read_to_string(&path).map_err(|e| format!("read log: {e}"))
}

#[tauri::command]
pub async fn list_models(base_url: String, api_key: String) -> Result<Vec<String>, String> {
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!(
            "HTTP {}: {}",
            resp.status().as_u16(),
            resp.text().await.unwrap_or_default()
        ));
    }
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Parse error: {e}"))?;
    let models: Vec<String> = body["data"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|v| v["id"].as_str().map(String::from))
        .collect();
    Ok(models)
}
