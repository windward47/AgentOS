use crate::agent::{AgentEngine, ConversationMessage, MessageRole};
use crate::agent::omp_sidecar::OmpAgentSidecar;
use crate::agent::omp_rpc::provider_to_model;
use crate::asr::AsrProvider;
use crate::tts::TtsProvider;
use crate::config::{CompanionConfig, ConfigManager};
use crate::permissions::audit::{AuditEvent, AuditLogger};
use crate::tools::ToolRegistry;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Global application state shared across Tauri commands and modules.
pub struct AppState {
    pub system_mode: AtomicBool,
    pub is_speaking: AtomicBool,
    pub is_listening: AtomicBool,
    pub lip_level: std::sync::Mutex<f32>,
    pub config: Arc<Mutex<CompanionConfig>>,
    pub config_manager: ConfigManager,
    pub agent: Arc<OmpAgentSidecar>,
    pub tools: Arc<ToolRegistry>,
    pub history: Arc<Mutex<Vec<ConversationMessage>>>,
    pub audit: AuditLogger,
}

fn resolve_api_token(config: &CompanionConfig) -> String {
    if let Ok(tok) = std::env::var("COMPANION_API_TOKEN") {
        if !tok.is_empty() { return tok; }
    }
    if let Some(ref tok) = config.api_token {
        if !tok.is_empty() { return tok.clone(); }
    }
    String::new()
}

#[allow(dead_code)]
fn resolve_omp_binary() -> String {
    #[cfg(target_os = "windows")]
    {
        let candidates = [{
            let appdata = std::env::var("APPDATA").unwrap_or_default();
            format!("{appdata}\\npm\\omp.cmd")
        }];
        for path in &candidates {
            if std::path::Path::new(path).exists() {
                log::info!("found omp at: {path}");
                return path.clone();
            }
        }
    }
    let candidates = [
        { let home = dirs::home_dir().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
          format!("{home}/.local/bin/omp") },
        "/usr/local/bin/omp".to_string(),
        "/opt/homebrew/bin/omp".to_string(),
    ];
    for path in &candidates {
        if std::path::Path::new(path).exists() {
            log::info!("found omp at: {path}");
            return path.clone();
        }
    }
    log::info!("omp not found at known paths; using bare 'omp' (PATH lookup)");
    "omp".to_string()
}

impl Default for AppState { fn default() -> Self { Self::new() } }

impl AppState {
    pub fn new() -> Self {
        let config_manager = ConfigManager::new().expect("failed to init config manager");
        let config = config_manager.load().expect("failed to load config");
        let audit = AuditLogger::new(&config_manager.root_dir()).expect("failed to init audit logger");
        let sandbox_root = config_manager.root_dir().join("sandbox");
        let tools = Arc::new(ToolRegistry::with_builtins(sandbox_root));
        let agent = Arc::new(OmpAgentSidecar::new());
        // Sidecar is spawned lazily on first chat request via spawn_if_needed()
        Self {
            system_mode: AtomicBool::new(config.system_mode),
            is_speaking: AtomicBool::new(false),
            is_listening: AtomicBool::new(false),
            lip_level: std::sync::Mutex::new(0.0),
            config: Arc::new(Mutex::new(config)),
            config_manager, agent, tools,
            history: Arc::new(Mutex::new(Vec::new())),
            audit,
        }
    }
    pub async fn save_config(&self) {
        let config = self.config.lock().await;
        self.config_manager.save(&config).ok();
    }
}

/// Tauri command: send a chat message to the agent and get a reply.
#[tauri::command]
pub async fn chat(state: tauri::State<'_, AppState>, message: String) -> Result<String, String> {
    // Auto-spawn sidecar if not running
    if !state.agent.is_running().await {
        state.agent.spawn().await.map_err(|e| format!("Sidecar spawn failed: {e}"))?;
    }
    let history_snapshot = { let hist = state.history.lock().await; hist.clone() };
    let response = state.agent.chat(&message, &history_snapshot).await
        .map_err(|e| format!("Agent error: {e}"))?;
    {
        let mut hist = state.history.lock().await;
        hist.push(ConversationMessage { role: MessageRole::User, content: message });
        hist.push(ConversationMessage { role: MessageRole::Assistant, content: response.text.clone() });
        if hist.len() > 50 { let keep = hist.len() - 50; hist.rotate_left(keep); hist.truncate(50); }
    }
    Ok(response.text)
}

/// Tauri command: send a chat message with tool support.
/// The sidecar handles LLM orchestration and tool execution internally.
/// This function wraps the sidecar interaction and manages conversation history.
#[tauri::command]
pub async fn chat_with_tools(state: tauri::State<'_, AppState>, message: String) -> Result<String, String> {
    // Same as chat() but explicitly handles tool-augmented responses
    if !state.agent.is_running().await {
        state.agent.spawn().await.map_err(|e| format!("Sidecar spawn failed: {e}"))?;
    }
    let history_snapshot = { let hist = state.history.lock().await; hist.clone() };
    let response = state.agent.chat(&message, &history_snapshot).await
        .map_err(|e| format!("Agent error: {e}"))?;
    {
        let mut hist = state.history.lock().await;
        hist.push(ConversationMessage { role: MessageRole::User, content: message });
        hist.push(ConversationMessage { role: MessageRole::Assistant, content: response.text.clone() });
        if hist.len() > 50 { let keep = hist.len() - 50; hist.rotate_left(keep); hist.truncate(50); }
    }
    Ok(response.text)
}

#[tauri::command]
pub async fn get_history(state: tauri::State<'_, AppState>) -> Result<Vec<ConversationMessage>, String> {
    let hist = state.history.lock().await;
    Ok(hist.clone())
}

#[tauri::command]
pub async fn clear_history(state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.history.lock().await.clear();
    Ok(())
}

/// Tauri command: transcribe audio. Uses custom URL/key if provider is "custom".
#[tauri::command]
pub async fn transcribe_audio(state: tauri::State<'_, AppState>, audio: Vec<f32>) -> Result<String, String> {
    let config = state.config.lock().await;
    let token = resolve_api_token(&config);
    let asr = if config.asr_provider == "custom" && config.asr_custom_url.is_some() {
        let key = config.asr_custom_key.clone().unwrap_or_else(|| token.clone());
        crate::asr::xiaomi_asr::XiaomiAsr::with_url(&key, config.asr_custom_url.as_deref().unwrap_or(""))
    } else {
        crate::asr::xiaomi_asr::XiaomiAsr::new(&token)
    };
    asr.transcribe(&audio).await.map_err(|e| format!("ASR error: {e}"))
}

/// Tauri command: synthesize text to audio. Uses custom URL/key if provider is "custom".
#[tauri::command]
pub async fn synthesize_audio(state: tauri::State<'_, AppState>, text: String, voice: Option<String>) -> Result<Vec<f32>, String> {
    let config = state.config.lock().await;
    let v = voice.unwrap_or_else(|| "茉莉".into());
    let tts = if config.tts_provider == "custom" && config.tts_custom_url.is_some() {
        let key = config.tts_custom_key.as_deref().map(String::from).unwrap_or_else(|| resolve_api_token(&config));
        crate::tts::xiaomi_tts::XiaomiTts::with_url(&key, &v, config.tts_custom_url.as_deref().unwrap_or(""))
    } else {
        let token = resolve_api_token(&config);
        crate::tts::xiaomi_tts::XiaomiTts::new(&token, &v)
    };
    tts.synthesize(&text).await.map_err(|e| format!("TTS error: {e}"))
}

#[tauri::command]
pub async fn get_config(state: tauri::State<'_, AppState>) -> Result<CompanionConfig, String> {
    let config = state.config.lock().await;
    Ok(config.clone())
}

#[tauri::command]
pub async fn update_config(state: tauri::State<'_, AppState>, config: CompanionConfig) -> Result<(), String> {
    {
        let mut current = state.config.lock().await;
        *current = config.clone();
    }
    state.save_config().await;
    let was_mode = state.system_mode.swap(config.system_mode, Ordering::SeqCst);
    if was_mode != config.system_mode {
        state.audit.log(AuditEvent::ModeSwitch { from: was_mode, to: config.system_mode });
    }
    let model = provider_to_model(&config.llm_provider).to_string();
    state.agent.set_model(model).await;
    Ok(())
}

#[tauri::command]
pub async fn set_lip_level(state: tauri::State<'_, AppState>, level: f32) -> Result<(), String> {
    *state.lip_level.lock().unwrap() = level.clamp(0.0, 1.0);
    Ok(())
}

#[tauri::command]
pub async fn get_lip_level(state: tauri::State<'_, AppState>) -> Result<f32, String> {
    Ok(*state.lip_level.lock().unwrap())
}

/// Returns the current avatar animation state: "idle" | "listening" | "speaking".
#[tauri::command]
pub async fn get_voice_state(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let s = if state.is_speaking.load(Ordering::Relaxed) {
        "speaking"
    } else if state.is_listening.load(Ordering::Relaxed) {
        "listening"
    } else {
        "idle"
    };
    log::debug!("get_voice_state → {}", s);
    Ok(s.into())
}

/// Returns the global cursor position (screen coordinates).
#[tauri::command]
pub async fn get_cursor_pos() -> Result<(i32, i32), String> {
    use enigo::{Enigo, Mouse, Settings};
    let enigo = Enigo::new(&Settings::default()).map_err(|e| format!("enigo: {:?}", e))?;
    enigo.location().map_err(|e| format!("location: {:?}", e))
}

/// Tauri command: take a screenshot of a given URL via Playwright.
#[tauri::command]
pub async fn browse_screenshot(url: String) -> Result<String, String> {
    let lower = url.to_lowercase();
    if !lower.starts_with("http://") && !lower.starts_with("https://") {
        return Err("Only http:// and https:// URLs are allowed".into());
    }
    let script = std::env::current_dir().unwrap_or_default().join("scripts").join("browser-screenshot.mjs");
    let tmp = std::env::temp_dir().join(format!("companion_browse_{}_{}.png", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0)));
    let output = std::process::Command::new("node").arg(&script).arg(&url).arg(&tmp).arg("15000")
        .output().map_err(|e| format!("Browser script failed: {e}"))?;
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
pub async fn get_audit_log(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let path = state.audit.path();
    if !path.exists() { return Ok("(no logs yet)".into()); }
    std::fs::read_to_string(&path).map_err(|e| format!("read log: {e}"))
}

/// Tauri command: list models from a custom OpenAI-compatible endpoint.
#[tauri::command]
pub async fn list_models(base_url: String, api_key: String) -> Result<Vec<String>, String> {
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let client = reqwest::Client::new();
    let resp = client.get(&url)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .timeout(std::time::Duration::from_secs(10))
        .send().await.map_err(|e| format!("Request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}: {}", resp.status().as_u16(), resp.text().await.unwrap_or_default()));
    }
    let body: serde_json::Value = resp.json().await.map_err(|e| format!("Parse error: {e}"))?;
    let models: Vec<String> = body["data"].as_array().unwrap_or(&vec![]).iter()
        .filter_map(|v| v["id"].as_str().map(String::from)).collect();
    Ok(models)
}
