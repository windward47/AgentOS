use async_trait::async_trait;
use thiserror::Error;

/// TTS provider — synthesize text into audio samples.
#[async_trait]
pub trait TtsProvider: Send + Sync {
    /// Synthesize text into mono PCM f32 samples.
    async fn synthesize(&self, text: &str) -> Result<Vec<f32>, TtsError>;

    /// Human-readable voice name for display (e.g. "Azure Xiaoxiao").
    fn voice_name(&self) -> &str;
}

#[derive(Debug, Error)]
pub enum TtsError {
    #[error("TTS internal error: {0}")]
    Internal(String),
    #[error("text too long: {0} chars")]
    TextTooLong(usize),
}

pub mod mock;
pub mod xiaomi_tts;
