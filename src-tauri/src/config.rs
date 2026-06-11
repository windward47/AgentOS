use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use thiserror::Error;

/// Top-level configuration for Companion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanionConfig {
    /// Sandbox root directory for file tools.
    #[serde(default = "default_sandbox_path")]
    pub sandbox_path: PathBuf,

    /// LLM provider: "openai" | "ollama" | "claude"
    #[serde(default = "default_llm")]
    pub llm_provider: String,

    /// ASR provider: "local" | "cloud"
    #[serde(default = "default_asr")]
    pub asr_provider: String,

    /// TTS provider: "local" | "cloud"
    #[serde(default = "default_tts")]
    pub tts_provider: String,

    /// System mode (false = sandbox, true = unrestricted)
    #[serde(default)]
    pub system_mode: bool,

    /// Enable accessibility features
    #[serde(default)]
    pub enable_accessibility: bool,

    /// VAD energy threshold (0.0 – 1.0)
    #[serde(default = "default_vad_threshold")]
    pub vad_threshold: f32,

    /// Voice input mode: "ptt" (push-to-talk, manual stop) or "auto" (VAD auto-stop)
    #[serde(default = "default_voice_mode")]
    pub voice_mode: String,

    /// Default TTS voice name (e.g. "茉莉", "冰糖", "mimo_default")
    #[serde(default = "default_tts_voice")]
    pub tts_voice: String,

    /// User's display name
    #[serde(default = "default_user_name")]
    pub user_name: String,

    /// Active conversation style template name
    #[serde(default = "default_style")]
    pub style_template: String,

    /// Custom system prompt override (optional)
    #[serde(default)]
    pub custom_system_prompt: Option<String>,

    /// Emotion → style mapping
    #[serde(default)]
    pub emotion_mapping: HashMap<String, String>,

    /// API token for Xiaomi/cloud services (env COMPANION_API_TOKEN overrides)
    #[serde(default)]
    pub api_token: Option<String>,

    // ── Custom provider overrides ──
    #[serde(default)]
    pub llm_custom_url: Option<String>,
    #[serde(default)]
    pub llm_custom_key: Option<String>,
    #[serde(default)]
    pub llm_custom_model: Option<String>,

    #[serde(default)]
    pub asr_custom_url: Option<String>,
    #[serde(default)]
    pub asr_custom_key: Option<String>,
    #[serde(default)]
    pub asr_custom_model: Option<String>,

    #[serde(default)]
    pub tts_custom_url: Option<String>,
    #[serde(default)]
    pub tts_custom_key: Option<String>,
    #[serde(default)]
    pub tts_custom_model: Option<String>,

    // ── Global voice (system-tray hotkey ASR/TTS) ──
    #[serde(default)]
    pub global_voice: GlobalVoiceConfig,
}

/// Configuration for global voice hotkey/inject/TTS features.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalVoiceConfig {
    /// Hotkey for recording toggle (e.g. "Alt+`")
    #[serde(default = "default_global_record_hotkey")]
    pub record_hotkey: String,

    /// Hotkey for TTS trigger (e.g. "Alt+T")
    #[serde(default = "default_global_tts_hotkey")]
    pub tts_hotkey: String,

    /// Hotkey to switch inject mode (e.g. "Alt+Shift+V")
    #[serde(default = "default_inject_switch_hotkey")]
    pub inject_mode_switch_hotkey: String,

    /// Hotkey to switch ASR engine (e.g. "Alt+Shift+E")
    #[serde(default = "default_engine_switch_hotkey")]
    pub engine_switch_hotkey: String,

    /// Inject mode: "keyboard" | "clipboard"
    #[serde(default = "default_inject_mode")]
    pub inject_mode: String,

    /// ASR engine for global recording: "mimo" | "openai" | "aliyun" | "whisper-local"
    #[serde(default = "default_global_asr_engine")]
    pub asr_engine: String,

    /// TTS engine for global TTS: "mimo-tts"
    #[serde(default = "default_global_tts_engine")]
    pub tts_engine: String,
}

impl Default for GlobalVoiceConfig {
    fn default() -> Self {
        Self {
            record_hotkey: default_global_record_hotkey(),
            tts_hotkey: default_global_tts_hotkey(),
            inject_mode_switch_hotkey: default_inject_switch_hotkey(),
            engine_switch_hotkey: default_engine_switch_hotkey(),
            inject_mode: default_inject_mode(),
            asr_engine: default_global_asr_engine(),
            tts_engine: default_global_tts_engine(),
        }
    }
}

fn default_global_record_hotkey() -> String { "Alt+`".into() }
fn default_global_tts_hotkey() -> String { "Alt+T".into() }
fn default_inject_switch_hotkey() -> String { "Alt+Shift+V".into() }
fn default_engine_switch_hotkey() -> String { "Alt+Shift+E".into() }
fn default_inject_mode() -> String { "keyboard".into() }
fn default_global_asr_engine() -> String { "mimo".into() }
fn default_global_tts_engine() -> String { "mimo-tts".into() }

fn default_sandbox_path() -> PathBuf {
    dirs::home_dir()
        .map(|p| p.join(".companion").join("sandbox"))
        .unwrap_or_else(|| PathBuf::from("./.companion/sandbox"))
}

fn default_llm() -> String { "siliconflow".into() }
fn default_asr() -> String { "local".into() }
fn default_tts() -> String { "local".into() }
fn default_vad_threshold() -> f32 { 0.3 }
fn default_voice_mode() -> String { "ptt".into() }
fn default_tts_voice() -> String { "茉莉".into() }
fn default_user_name() -> String { "User".into() }
fn default_style() -> String { "professional".into() }

impl Default for CompanionConfig {
    fn default() -> Self {
        Self {
            sandbox_path: default_sandbox_path(),
            llm_provider: default_llm(),
            asr_provider: default_asr(),
            tts_provider: default_tts(),
            system_mode: false,
            enable_accessibility: false,
            vad_threshold: default_vad_threshold(),
            voice_mode: default_voice_mode(),
            tts_voice: default_tts_voice(),
            user_name: default_user_name(),
            style_template: default_style(),
            custom_system_prompt: None,
            emotion_mapping: HashMap::new(),
            api_token: None,
            llm_custom_url: None, llm_custom_key: None, llm_custom_model: None,
            asr_custom_url: None, asr_custom_key: None, asr_custom_model: None,
            tts_custom_url: None, tts_custom_key: None, tts_custom_model: None,
            global_voice: GlobalVoiceConfig::default(),
        }
    }
}

/// Errors from the configuration system.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("cannot determine home directory")]
    NoHomeDir,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Configuration manager — loads, saves, and provides defaults.
#[derive(Clone)]
pub struct ConfigManager {
    config_path: PathBuf,
    data_root: PathBuf,
}

impl ConfigManager {
    /// Create a new manager using the default `~/.companion/` root.
    pub fn new() -> Result<Self, ConfigError> {
        let home = dirs::home_dir().ok_or(ConfigError::NoHomeDir)?;
        let data_root = home.join(".companion");
        Ok(Self {
            config_path: data_root.join("config.json"),
            data_root,
        })
    }

    /// Ensure the data directory exists.
    fn ensure_dirs(&self) -> Result<(), ConfigError> {
        fs::create_dir_all(&self.data_root)?;
        fs::create_dir_all(self.data_root.join("sandbox"))?;
        fs::create_dir_all(self.data_root.join("logs"))?;
        fs::create_dir_all(self.data_root.join("tools"))?;
        fs::create_dir_all(self.data_root.join("models"))?;
        Ok(())
    }

    /// Load config from disk, or create default if missing.
    pub fn load(&self) -> Result<CompanionConfig, ConfigError> {
        self.ensure_dirs()?;

        if !self.config_path.exists() {
            let config = CompanionConfig::default();
            self.save(&config)?;
            return Ok(config);
        }

        let content = fs::read_to_string(&self.config_path)?;
        serde_json::from_str(&content).or_else(|e| {
            // Backup corrupt config before overwriting
            let bak = self.config_path.with_extension("json.bak");
            let _ = fs::write(&bak, &content);
            eprintln!("[ConfigManager] corrupt config backed up to {bak:?}: {e}");
            let default = CompanionConfig::default();
            self.save(&default).ok();
            Ok(default)
        })
    }

    /// Save config to disk.
    pub fn save(&self, config: &CompanionConfig) -> Result<(), ConfigError> {
        self.ensure_dirs()?;
        let content = serde_json::to_string_pretty(config)?;
        fs::write(&self.config_path, content)?;
        Ok(())
    }

    /// Return the data root directory path.
    pub fn root_dir(&self) -> std::path::PathBuf {
        self.data_root.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_default_config() {
        let config = CompanionConfig::default();
        assert!(!config.system_mode);
        assert_eq!(config.vad_threshold, 0.3);
        assert_eq!(config.llm_provider, "siliconflow");
    }

    #[test]
    fn test_config_roundtrip() {
        // Use a temp dir to avoid polluting ~/.companion
        let tmp = env::temp_dir().join("companion_test_config");
        fs::create_dir_all(&tmp).unwrap();
        let mgr = ConfigManager {
            config_path: tmp.join("config.json"),
            data_root: tmp.clone(),
        };

        let config = CompanionConfig {
            user_name: "测试用户".into(),
            ..Default::default()
        };
        mgr.save(&config).unwrap();
        let loaded = mgr.load().unwrap();
        assert_eq!(loaded.user_name, "测试用户");
        assert!(!loaded.system_mode);

        fs::remove_dir_all(&tmp).ok();
    }
}
