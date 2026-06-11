//! Text injection into the currently focused text field.
//!
//! Supports Keyboard (enigo type simulation) and Clipboard (save → Ctrl+V → restore).
//! Also provides `read_selected_text` via Ctrl+C simulation.

pub mod keyboard;
pub mod clipboard;
pub mod text_reader;

use crate::config::CompanionConfig;

#[derive(Debug, thiserror::Error)]
pub enum InjectError {
    #[error("Keyboard simulation failed: {0}")]
    Keyboard(String),
    #[error("Clipboard operation failed: {0}")]
    Clipboard(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InjectMode {
    Keyboard,
    Clipboard,
}

impl InjectMode {
    pub fn from_config(cfg: &CompanionConfig) -> Self {
        match cfg.global_voice.inject_mode.as_str() {
            "clipboard" => InjectMode::Clipboard,
            _ => InjectMode::Keyboard,
        }
    }

    pub fn toggle(&self) -> Self {
        match self {
            InjectMode::Keyboard => InjectMode::Clipboard,
            InjectMode::Clipboard => InjectMode::Keyboard,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            InjectMode::Keyboard => "keyboard",
            InjectMode::Clipboard => "clipboard",
        }
    }
}

/// Inject text at the current cursor position.
pub fn inject_text(text: &str, mode: InjectMode) -> Result<(), InjectError> {
    match mode {
        InjectMode::Keyboard => keyboard::inject_keyboard(text),
        InjectMode::Clipboard => clipboard::inject_clipboard(text),
    }
}
