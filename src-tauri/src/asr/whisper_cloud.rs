//! Cloud ASR via OpenAI Whisper API.
//!
//! Sends audio as a multipart POST to `https://api.openai.com/v1/audio/transcriptions`.

use async_trait::async_trait;
use crate::asr::{AsrError, AsrProvider};

/// Cloud ASR via OpenAI Whisper API.
pub struct WhisperCloud {
    api_key: String,
    model: String,
    language: Option<String>,
    client: reqwest::Client,
}

impl WhisperCloud {
    pub fn new(api_key: &str, model: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            model: model.to_string(),
            language: None,
            client: reqwest::Client::new(),
        }
    }

    /// Set the language hint (e.g. "zh", "en").
    pub fn with_language(mut self, lang: &str) -> Self {
        self.language = Some(lang.to_string());
        self
    }

    /// Write PCM f32 mono audio as a WAV byte buffer (in memory).
    fn wav_bytes(&self, audio: &[f32]) -> Result<Vec<u8>, AsrError> {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut writer = hound::WavWriter::new(&mut cursor, spec)
                .map_err(|e| AsrError::Internal(format!("wav create: {e}")))?;
            for &sample in audio {
                let scaled = (sample * i16::MAX as f32) as i16;
                writer.write_sample(scaled)
                    .map_err(|e| AsrError::Internal(format!("wav write: {e}")))?;
            }
            writer.finalize()
                .map_err(|e| AsrError::Internal(format!("wav finalize: {e}")))?;
        }
        Ok(cursor.into_inner())
    }
}

#[async_trait]
impl AsrProvider for WhisperCloud {
    async fn transcribe(&self, audio: &[f32]) -> Result<String, AsrError> {
        if audio.len() < 1600 {
            return Err(AsrError::AudioTooShort(audio.len() as u32 * 1000 / 16000));
        }

        let wav_data = self.wav_bytes(audio)?;

        // Build multipart form
        let part = reqwest::multipart::Part::bytes(wav_data)
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| AsrError::Internal(e.to_string()))?;

        let mut form = reqwest::multipart::Form::new()
            .part("file", part)
            .text("model", self.model.clone());

        if let Some(ref lang) = self.language {
            form = form.text("language", lang.clone());
        }

        let response = self.client
            .post("https://api.openai.com/v1/audio/transcriptions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .multipart(form)
            .send()
            .await
            .map_err(|e| AsrError::Internal(format!("request failed: {e}")))?;

        let status = response.status();
        let body = response.text().await
            .map_err(|e| AsrError::Internal(format!("read body: {e}")))?;

        if !status.is_success() {
            return Err(AsrError::Internal(format!("API error {status}: {body}")));
        }

        let parsed: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| AsrError::Internal(format!("json parse: {e}")))?;

        let text = parsed["text"]
            .as_str()
            .unwrap_or("")
            .trim()
            .to_string();

        Ok(text)
    }

    fn switch_model(&mut self, model: &str) -> Result<(), AsrError> {
        self.model = model.to_string();
        Ok(())
    }
}
