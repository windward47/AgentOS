use async_trait::async_trait;
use thiserror::Error;

/// ASR provider — transcribe audio samples into text.
#[async_trait]
pub trait AsrProvider: Send + Sync {
    /// Transcribe a mono PCM f32 audio buffer to text.
    async fn transcribe(&self, audio: &[f32]) -> Result<String, AsrError>;

    /// Switch the underlying model (if applicable).
    fn switch_model(&mut self, model: &str) -> Result<(), AsrError>;
}

#[derive(Debug, Error)]
pub enum AsrError {
    #[error("ASR internal error: {0}")]
    Internal(String),
    #[error("model not found: {0}")]
    ModelNotFound(String),
    #[error("audio too short: {0}ms < minimum")]
    AudioTooShort(u32),
}

pub mod aliyun_asr;
pub mod whisper_cloud;
pub mod whisper_local;
pub mod xiaomi_asr;
