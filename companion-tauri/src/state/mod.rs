//! Domain states + command submodules.

use companion_core::agent::omp_sidecar::OmpAgentSidecar;
use companion_core::config::{CompanionConfig, ConfigManager};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

pub mod commands;

// Re-export all commands for lib.rs invoke_handler
pub use commands::chat::*;
pub use commands::conversation::*;
pub use commands::media::*;
pub use commands::memory::*;
pub use commands::avatar::*;
pub use commands::system::*;
pub use commands::config_cmd::*;

// ── AgentState ──────────────────────────────────────────────────────────

pub struct AgentState {
    pub agent: Arc<OmpAgentSidecar>,
}

impl AgentState {
    pub fn new() -> Self {
        Self { agent: Arc::new(OmpAgentSidecar::new()) }
    }
}

// ── VoiceState ──────────────────────────────────────────────────────────

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

    pub async fn sync_from_sidecar(&self, agent: &OmpAgentSidecar) -> Result<(), String> {
        let val = agent.get_config().await.map_err(|e| format!("get_config RPC: {e}"))?;
        let cfg: CompanionConfig = serde_json::from_value(val).map_err(|e| format!("parse config: {e}"))?;
        self.system_mode.store(cfg.system_mode, Ordering::Relaxed);
        *self.config.lock().await = cfg;
        Ok(())
    }

    pub async fn push_to_sidecar(&self, agent: &OmpAgentSidecar, partial: serde_json::Value) -> Result<(), String> {
        let val = agent.update_config(partial).await.map_err(|e| format!("update_config RPC: {e}"))?;
        let cfg: CompanionConfig = serde_json::from_value(val).map_err(|e| format!("parse config: {e}"))?;
        self.system_mode.store(cfg.system_mode, Ordering::Relaxed);
        *self.config.lock().await = cfg;
        Ok(())
    }
}
