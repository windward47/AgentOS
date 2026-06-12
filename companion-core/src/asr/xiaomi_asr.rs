//! Xiaomi Token Plan ASR — transcribes audio via the chat completions endpoint.
//! Uses `mimo-v2.5-asr` model with `input_audio` content type.

use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use crate::asr::{AsrError, AsrProvider};
use serde_json::Value;

pub struct XiaomiAsr {
    api_key: String,
    base_url: String,
    client: reqwest::Client,
}

impl XiaomiAsr {
    pub fn new(api_key: &str) -> Self {
        Self::with_url(api_key, "https://token-plan-cn.xiaomimimo.com/v1/chat/completions")
    }

    pub fn with_url(api_key: &str, base_url: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            base_url: base_url.to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// Encode PCM f32 mono 16kHz audio to a WAV base64 data URL.
    fn pcm_to_wav_data_url(audio: &[f32]) -> Result<String, AsrError> {
        use std::io::Cursor;

        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut buf = Cursor::new(Vec::new());
        {
            let mut writer = hound::WavWriter::new(&mut buf, spec)
                .map_err(|e| AsrError::Internal(format!("wav create: {e}")))?;
            for &s in audio {
                let scaled = (s * i16::MAX as f32) as i16;
                writer.write_sample(scaled)
                    .map_err(|e| AsrError::Internal(format!("wav write: {e}")))?;
            }
            writer.finalize()
                .map_err(|e| AsrError::Internal(format!("wav finalize: {e}")))?;
        }
        let wav_bytes = buf.into_inner();
        let b64 = BASE64.encode(&wav_bytes);
        Ok(format!("data:audio/wav;base64,{b64}"))
    }
}

#[async_trait]
impl AsrProvider for XiaomiAsr {
    async fn transcribe(&self, audio: &[f32]) -> Result<String, AsrError> {
        if audio.len() < 1600 {
            return Err(AsrError::AudioTooShort(audio.len() as u32 * 1000 / 16000));
        }

        let data_url = Self::pcm_to_wav_data_url(audio)?;

        let body = serde_json::json!({
            "model": "mimo-v2.5-asr",
            "messages": [{
                "role": "user",
                "content": [{
                    "type": "input_audio",
                    "input_audio": {
                        "data": data_url,
                        "format": "wav"
                    }
                }]
            }],
            "max_tokens": 100
        });

        let response = self.client
            .post(&self.base_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| AsrError::Internal(format!("request: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(AsrError::Internal(format!("ASR API {status}: {text}")));
        }

        let parsed: Value = response.json().await
            .map_err(|e| AsrError::Internal(format!("json: {e}")))?;

        let text = parsed["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .trim()
            .to_string();

        Ok(text)
    }

    fn switch_model(&mut self, _model: &str) -> Result<(), AsrError> {
        Ok(())
    }
}
