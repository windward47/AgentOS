use crate::agent::{AgentEngine, ConversationMessage, MessageRole};
use crate::agent::omp_rpc::OmpRpcClient;
use crate::asr::AsrProvider;
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
    /// Application configuration (loaded at startup, mutable at runtime)
    pub config: Arc<Mutex<CompanionConfig>>,
    /// Configuration persistence manager
    pub config_manager: ConfigManager,
    /// The Agent engine — auto-spawns on first use
    pub agent: Arc<dyn AgentEngine + Send + Sync>,
    /// Conversation history (shared across commands)
    pub history: Arc<Mutex<Vec<ConversationMessage>>>,
}

impl AppState {
    pub fn new() -> Self {
        let config_manager = ConfigManager::new().expect("failed to init config manager");
        let config = config_manager.load().expect("failed to load config");

        Self {
            system_mode: AtomicBool::new(config.system_mode),
            is_speaking: AtomicBool::new(false),
            is_listening: AtomicBool::new(false),
            config: Arc::new(Mutex::new(config)),
            config_manager,
            agent: Arc::new(OmpRpcClient::new("omp")),
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

/// Tauri command: transcribe raw PCM f32 mono audio to text.
/// Audio should be 16kHz mono. Frontend handles microphone capture + VAD.
#[tauri::command]
pub async fn transcribe_audio(
    audio: Vec<f32>,
) -> Result<String, String> {
    let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_default();
    if api_key.is_empty() {
        return Err("请设置 OPENAI_API_KEY 环境变量以启用语音识别功能".into());
    }
    let asr = crate::asr::whisper_cloud::WhisperCloud::new(&api_key, "whisper-1");
    asr.transcribe(&audio).await
        .map_err(|e| format!("ASR 错误: {e}"))
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
