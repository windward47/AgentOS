//! Audio capture and playback module.
//!
//! Provides microphone input via `cpal` and speaker output.
//! The capture loop writes into a short (2s) ring buffer for VAD/ASR consumption.

pub mod capture;
pub mod vad;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AudioError {
    #[error("audio device not found: {0}")]
    DeviceNotFound(String),
    #[error("stream creation failed: {0}")]
    StreamError(String),
}
