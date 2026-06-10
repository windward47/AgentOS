use crate::agent::{AgentEngine, ConversationMessage, MessageRole};
use crate::agent::omp_rpc::OmpRpcClient;
use crate::asr::AsrProvider;
use crate::tts::TtsProvider;
use crate::config::{CompanionConfig, ConfigManager};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Global application state shared across Tauri commands and modules.
pub struct AppState {
    /// System mode toggle (false = sandbox mode, true = system mode)
    pub system_mode: AtomicBool,
    /// Whether the agent is currently speaking (for TTS interrupt logic)
    pub is_speaking: AtomicBool,
    /// Whether the agent is currently listening (for VAD/ASR gate)
    pub is_listening: AtomicBool,
    /// Lip-sync level shared between main window and avatar window
    pub lip_level: std::sync::Mutex<f32>,
    /// Application configuration (loaded at startup, mutable at runtime)
    pub config: Arc<Mutex<CompanionConfig>>,
    /// Configuration persistence manager
    pub config_manager: ConfigManager,
    /// The Agent engine — auto-spawns on first use
    pub agent: Arc<dyn AgentEngine + Send + Sync>,
    /// Conversation history (shared across commands)
    pub history: Arc<Mutex<Vec<ConversationMessage>>>,
}

fn resolve_omp_binary() -> String {
    #[cfg(target_os = "windows")]
    {
        // On Windows, the npm global `omp` is a POSIX shell script (not executable).
        // `omp.cmd` is the actual batch wrapper that Windows uses.
        // `Command::new(\"omp\")` on Windows automatically tries .cmd/.exe/.bat,
        // so the simplest fix is to leave it as bare `omp` without full path.
        let candidates = [
            // npm global: %APPDATA%/npm/omp.cmd (Windows batch wrapper)
            {
                let appdata = std::env::var("APPDATA").unwrap_or_default();
                format!("{appdata}\\npm\\omp.cmd")
            },
        ];
        for path in &candidates {
            if std::path::Path::new(path).exists() {
                log::info!("found omp at: {path}");
                return path.clone();
            }
        }
    }

    // Non-Windows: the npm global omp script
    let candidates = [
        {
            let home = dirs::home_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            format!("{home}/.local/bin/omp")
        },
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

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        let config_manager = ConfigManager::new().expect("failed to init config manager");
        let config = config_manager.load().expect("failed to load config");
        let binary = resolve_omp_binary();

        Self {
            system_mode: AtomicBool::new(config.system_mode),
            is_speaking: AtomicBool::new(false),
            is_listening: AtomicBool::new(false),
            lip_level: std::sync::Mutex::new(0.0),
            config: Arc::new(Mutex::new(config)),
            config_manager,
            agent: Arc::new(OmpRpcClient::new(&binary)),
            history: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Persist the current config to disk.
    pub async fn save_config(&self) {
        let config = self.config.lock().await;
        self.config_manager.save(&config).ok();
    }
}

/// Tauri command: send a chat message to the agent and get a reply.
#[tauri::command]
pub async fn chat(
    state: tauri::State<'_, AppState>,
    message: String,
) -> Result<String, String> {
    let history_snapshot = {
        let hist = state.history.lock().await;
        hist.clone()
    };

    let response = state.agent.chat(&message, &history_snapshot).await
        .map_err(|e| format!("Agent error: {e}"))?;

    // Append user message + assistant reply to history
    {
        let mut hist = state.history.lock().await;
        hist.push(ConversationMessage {
            role: MessageRole::User,
            content: message,
        });
        hist.push(ConversationMessage {
            role: MessageRole::Assistant,
            content: response.text.clone(),
        });
        // Keep at most last 50 messages by trimming from the front
        if hist.len() > 50 {
            let keep = hist.len() - 50;
            hist.rotate_left(keep);
            hist.truncate(50);
        }
    }

    Ok(response.text)
}

/// Tauri command: return the conversation history.
#[tauri::command]
pub async fn get_history(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ConversationMessage>, String> {
    let hist = state.history.lock().await;
    Ok(hist.clone())
}

/// Tauri command: clear conversation history.
#[tauri::command]
pub async fn clear_history(
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut hist = state.history.lock().await;
    hist.clear();
    Ok(())
}

/// Tauri command: transcribe raw PCM f32 mono audio to text using Xiaomi ASR.
/// Audio should be 16kHz mono PCM f32 samples. Frontend handles microphone capture + VAD.
#[tauri::command]
pub async fn transcribe_audio(
    audio: Vec<f32>,
) -> Result<String, String> {
    let asr = crate::asr::xiaomi_asr::XiaomiAsr::new(
        "REDACTED_TOKEN_PLACEHOLDER",
    );
    asr.transcribe(&audio).await
        .map_err(|e| format!("ASR error: {e}"))
}

/// Tauri command: synthesize text to audio using Xiaomi TTS.
/// Returns PCM f32 mono samples (16kHz).
#[tauri::command]
pub async fn synthesize_audio(
    text: String,
    voice: Option<String>,
) -> Result<Vec<f32>, String> {
    let tts = crate::tts::xiaomi_tts::XiaomiTts::new(
        "REDACTED_TOKEN_PLACEHOLDER",
        &voice.unwrap_or_else(|| "茉莉".into()),
    );
    tts.synthesize(&text).await
        .map_err(|e| format!("TTS error: {e}"))
}

/// Tauri command: return the current configuration (for Settings UI).
#[tauri::command]
pub async fn get_config(
    state: tauri::State<'_, AppState>,
) -> Result<CompanionConfig, String> {
    let config = state.config.lock().await;
    Ok(config.clone())
}

/// Tauri command: update the configuration.
#[tauri::command]
pub async fn update_config(
    state: tauri::State<'_, AppState>,
    config: CompanionConfig,
) -> Result<(), String> {
    {
        let mut current = state.config.lock().await;
        *current = config.clone();
    }
    state.save_config().await;
    state.system_mode.store(config.system_mode, Ordering::SeqCst);
    Ok(())
}

/// Tauri command: set the lip-sync level (0.0–1.0). Main window calls this during TTS playback.
#[tauri::command]
pub async fn set_lip_level(
    state: tauri::State<'_, AppState>,
    level: f32,
) -> Result<(), String> {
    *state.lip_level.lock().unwrap() = level.clamp(0.0, 1.0);
    Ok(())
}

/// Tauri command: get the current lip-sync level. Avatar window polls this at ~30fps.
#[tauri::command]
pub async fn get_lip_level(
    state: tauri::State<'_, AppState>,
) -> Result<f32, String> {
    Ok(*state.lip_level.lock().unwrap())
}

/// Tauri command: take a screenshot of a given URL via Playwright.
/// Returns a base64-encoded PNG image.
#[tauri::command]
pub async fn browse_screenshot(
    url: String,
) -> Result<String, String> {
    let script = std::env::current_dir()
        .unwrap_or_default()
        .join("scripts")
        .join("browser-screenshot.mjs");

    let tmp = std::env::temp_dir().join(format!("companion_browse_{}.png", std::process::id()));

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
