use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Emotion labels detectable from speech.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EmotionLabel {
    Happy,
    Sad,
    Angry,
    Neutral,
    Surprised,
    Fearful,
}

/// Emotion recognition engine.
#[async_trait]
pub trait EmotionEngine: Send + Sync {
    /// Classify emotion from mono PCM f32 audio samples.
    async fn classify(&self, audio: &[f32]) -> Result<EmotionLabel, EmotionError>;
}

#[derive(Debug, Error)]
pub enum EmotionError {
    #[error("emotion recognition internal error: {0}")]
    Internal(String),
}
