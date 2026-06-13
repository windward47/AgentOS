//! Domain-level application states for Tauri's managed state system.
//!
//! Split from a monolithic `AppState` into focused domain states so that
//! each Tauri command depends on exactly what it needs — no more, no less.

use companion_core::agent::omp_sidecar::OmpAgentSidecar;
use companion_core::agent::{AgentEngine, ConversationMessage, MessageRole, provider_to_model};
use companion_core::asr::AsrProvider;
use companion_core::tts::TtsProvider;
use companion_core::config::{CompanionConfig, ConfigManager};
use companion_core::permissions::audit::{AuditEvent, AuditLogger};
use companion_core::tools::ToolRegistry;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::{Manager, Emitter};

// ── AgentState ──────────────────────────────────────────────────────────

/// Manages the agent engine and conversation history.
pub struct AgentState {
    pub agent: Arc<OmpAgentSidecar>,
    pub history: Arc<Mutex<Vec<ConversationMessage>>>,
}

impl AgentState {
    pub fn new() -> Self {
        Self {
            agent: Arc::new(OmpAgentSidecar::new()),
            history: Arc::new(Mutex::new(Vec::new())),
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
}

/// Resolve API key: ProviderConfig.key → config.default_api_key → env COMPANION_API_TOKEN → ""
pub fn resolve_provider_key(prov: &companion_core::config::ProviderConfig, fallback_key: &str) -> String {
    if let Some(ref k) = prov.key { if !k.is_empty() { return k.clone(); } }
    if let Ok(tok) = std::env::var("COMPANION_API_TOKEN") { if !tok.is_empty() { return tok; } }
    fallback_key.to_string()
}

// ── AuditState ──────────────────────────────────────────────────────────

/// Security audit logging.
pub struct AuditState {
    pub audit: AuditLogger,
}

impl AuditState {
    pub fn new(data_root: &std::path::Path) -> Self {
        Self {
            audit: AuditLogger::new(&data_root.to_path_buf())
                .expect("failed to init audit logger"),
        }
    }
}

// ── ToolState ───────────────────────────────────────────────────────────

/// Tool registry — all MCP tools accessible by the agent.
pub struct ToolState {
    pub tools: Arc<ToolRegistry>,
}

impl ToolState {
    pub fn new(sandbox_root: std::path::PathBuf) -> Self {
        Self {
            tools: Arc::new(ToolRegistry::with_builtins(sandbox_root)),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tauri IPC Commands
// ═══════════════════════════════════════════════════════════════════════


#[tauri::command]
pub async fn chat(
    agent: tauri::State<'_, AgentState>,
    config: tauri::State<'_, ConfigState>,
    _voice: tauri::State<'_, VoiceState>,
    message: String,
) -> Result<String, String> {
    // Auto-spawn sidecar if not running
    if !agent.agent.is_running().await {
        agent
            .agent
            .spawn()
            .await
            .map_err(|e| format!("Sidecar spawn failed: {e}"))?;
    }
    let history_snapshot = {
        let hist = agent.history.lock().await;
        hist.clone()
    };
    let response = agent
        .agent
        .chat(&message, &history_snapshot, Some(&config.config.lock().await.custom_system_prompt))
        .await
        .map_err(|e| format!("Agent error: {e}"))?;
    {
        let mut hist = agent.history.lock().await;
        hist.push(ConversationMessage {
            role: MessageRole::User,
            content: message,
        });
        hist.push(ConversationMessage {
            role: MessageRole::Assistant,
            content: response.text.clone(),
        });
        if hist.len() > 50 {
            let keep = hist.len() - 50;
            hist.rotate_left(keep);
            hist.truncate(50);
        }
    }
    Ok(response.text)
}

#[tauri::command]
pub async fn chat_with_tools(
    agent: tauri::State<'_, AgentState>,
    config: tauri::State<'_, ConfigState>,
    message: String,
) -> Result<String, String> {
    if !agent.agent.is_running().await {
        agent
            .agent
            .spawn()
            .await
            .map_err(|e| format!("Sidecar spawn failed: {e}"))?;
    }
    let history_snapshot = {
        let hist = agent.history.lock().await;
        hist.clone()
    };
    let response = agent
        .agent
        .chat(&message, &history_snapshot, Some(&config.config.lock().await.custom_system_prompt))
        .await
        .map_err(|e| format!("Agent error: {e}"))?;
    {
        let mut hist = agent.history.lock().await;
        hist.push(ConversationMessage {
            role: MessageRole::User,
            content: message,
        });
        hist.push(ConversationMessage {
            role: MessageRole::Assistant,
            content: response.text.clone(),
        });
        if hist.len() > 50 {
            let keep = hist.len() - 50;
            hist.rotate_left(keep);
            hist.truncate(50);
        }
    }
    Ok(response.text)
}

#[tauri::command]
pub async fn get_history(
    agent: tauri::State<'_, AgentState>,
) -> Result<Vec<ConversationMessage>, String> {
    let hist = agent.history.lock().await;
    Ok(hist.clone())
}

#[tauri::command]
pub async fn clear_history(agent: tauri::State<'_, AgentState>) -> Result<(), String> {
    agent.history.lock().await.clear();
    if agent.agent.is_running().await {
        agent.agent.clear_history().await.map_err(|e| format!("clear history: {e}"))?;
    }
    Ok(())
}

fn ensure_chat_completions_url(url: &str) -> String {
    if url.contains("/chat/completions") { url.to_string() }
    else { format!("{}/chat/completions", url.trim_end_matches('/')) }
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
    audit: tauri::State<'_, AuditState>,
    new_config: CompanionConfig,
) -> Result<(), String> {
    {
        let mut current = config.config.lock().await;
        *current = new_config.clone();
    }
    config.save().await;
    let was_mode = config
        .system_mode
        .swap(new_config.system_mode, Ordering::SeqCst);
    if was_mode != new_config.system_mode {
        audit
            .audit
            .log(AuditEvent::ModeSwitch { from: was_mode, to: new_config.system_mode });
    }
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
    log::debug!("get_voice_state → {}", s);
    Ok(s.into())
}

/// List available Live2D models from web/public/live2d/models/.
/// Returns relative paths to .model3.json files for verified-compatible models.
#[tauri::command]
pub async fn list_live2d_models() -> Result<Vec<String>, String> {
    // Verified-compatible models (tested with Cubism SDK 5.1.0).
    // Excluded: Epsilon (.cmo3 format, not C3), ren (moc3 v6).
    let working: &[&str] = &[
        "haru/haru.model3.json",
        "haru_greeter_pro_jp/haru_greeter_pro_jp/runtime/haru_greeter_t05.model3.json",
        "hiyori_pro_zh/hiyori_pro_zh/runtime/hiyori_pro_t11.model3.json",
        "kei_zh/kei_zh/kei_basic_free/runtime/kei_basic_free.model3.json",
        "kei_zh/kei_zh/kei_vowels_pro/runtime/kei_vowels_pro.model3.json",
        "mao_pro_zh/mao_pro_zh/runtime/mao_pro.model3.json",
        "miara_pro_en/miara_pro_en/runtime/miara_pro_t03.model3.json",
        "miku_pro_jp/miku_pro_jp/runtime/miku_sample_t04.model3.json",
        "natori_pro_zh/natori_pro_zh/runtime/natori_pro_t06.model3.json",
        "rice_pro_zh/rice_pro_zh/runtime/rice_pro_t03.model3.json",
    ];
    Ok(working.iter().map(|s| s.to_string()).collect())
}

/// Tell the avatar window to switch to a different model.
#[tauri::command]
pub async fn set_live2d_model(app: tauri::AppHandle, model_path: String) -> Result<(), String> {
    app.emit("switch_live2d_model", model_path)
        .map_err(|e| format!("emit: {e}"))?;
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
pub async fn get_audit_log(
    audit: tauri::State<'_, AuditState>,
) -> Result<String, String> {
    let path = audit.audit.path();
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
