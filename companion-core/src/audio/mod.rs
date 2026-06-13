//! Audio capture and playback module.
//!
//! Provides audio utilities for format conversion (PCM / WAV).

pub mod utils;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AudioError {
    #[error("audio device not found: {0}")]
    DeviceNotFound(String),
    #[error("stream creation failed: {0}")]
    StreamError(String),
}
