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

pub mod mock;
pub mod whisper_cloud;
pub mod whisper_local;

use std::sync::Arc;
use crate::audio::capture::AudioCapture;
use crate::audio::vad::{Vad, VadState, VAD_FRAME_MS};

/// Coordinates: mic capture → VAD detection → ASR transcription.
pub struct VoiceInputService {
    capture: Option<AudioCapture>,
    vad: Vad,
    asr: Option<Arc<dyn AsrProvider + Send + Sync>>,
}

impl VoiceInputService {
    pub fn new() -> Self {
        Self {
            capture: None,
            vad: Vad::new(0.3),
            asr: None,
        }
    }

    /// Start the microphone capture.
    pub fn start_capture(&mut self) -> Result<(), crate::audio::AudioError> {
        let capture = AudioCapture::start(16000)?;
        self.capture = Some(capture);
        Ok(())
    }

    /// Stop capture.
    pub fn stop_capture(&mut self) {
        if let Some(mut cap) = self.capture.take() {
            cap.stop();
        }
    }

    /// Set the ASR provider.
    pub fn set_asr(&mut self, asr: Arc<dyn AsrProvider + Send + Sync>) {
        self.asr = Some(asr);
    }

    /// Read one VAD-length frame from the capture buffer and process it.
    /// Returns `Some(audio)` when an utterance completes (silence timeout reached).
    pub fn process_frame(&mut self) -> Result<Option<Vec<f32>>, AsrError> {
        let capture = self.capture.as_ref().ok_or_else(|| {
            AsrError::Internal("capture not started".into())
        })?;

        let sample_rate = capture.sample_rate();
        let frame_samples = (sample_rate as u64 * VAD_FRAME_MS as u64 / 1000) as usize;
        let frame = capture.read_chunk(VAD_FRAME_MS);

        if frame.len() < frame_samples {
            return Ok(None); // not enough data yet
        }

        let prev_was_idle = matches!(self.vad.current_state(), VadState::Idle);
        let new_state = self.vad.process_frame(&frame);

        // Check if utterance just ended (transition from utterance to idle)
        if !prev_was_idle && new_state == VadState::Idle {
            // Collect the full utterance from the capture buffer
            let utterance_duration_ms = 3000; // at most 3 seconds
            let audio = capture.read_chunk(utterance_duration_ms);
            self.vad.reset();
            return Ok(Some(audio));
        }

        Ok(None)
    }

    /// Run the full voice input pipeline synchronously:
    /// Processes frames until an utterance is detected, then transcribes it.
    pub async fn listen_and_transcribe(&mut self) -> Result<String, AsrError> {
        let asr = self.asr.clone().ok_or_else(|| {
            AsrError::Internal("ASR not configured".into())
        })?;

        // Poll frames until utterance complete
        let audio = loop {
            if let Some(audio) = self.process_frame()? {
                break audio;
            }
            // Sleep briefly to avoid busy-wait
            tokio::time::sleep(std::time::Duration::from_millis(VAD_FRAME_MS as u64)).await;
        };

        asr.transcribe(&audio).await
    }
}
