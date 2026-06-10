//! Xiaomi Token Plan TTS — synthesizes speech via the chat completions endpoint.
//! Uses `mimo-v2.5-tts` model with `audio` modality.

use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use crate::tts::{TtsError, TtsProvider};
use serde_json::Value;

pub struct XiaomiTts {
    api_key: String,
    voice: String,
    base_url: String,
    client: reqwest::Client,
}

impl XiaomiTts {
    /// Available voices: mimo_default, 冰糖, 茉莉, 苏打, 白桦, Mia, Chloe, Milo, Dean
    pub fn new(api_key: &str, voice: &str) -> Self {
        Self::with_url(api_key, voice, "https://token-plan-cn.xiaomimimo.com/v1/chat/completions")
    }

    pub fn with_url(api_key: &str, voice: &str, base_url: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            voice: voice.to_string(),
            base_url: base_url.to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// Decode a base64 WAV byte array to PCM f32 mono samples.
    fn decode_wav_to_pcm(wav_bytes: &[u8]) -> Result<Vec<f32>, TtsError> {
        let mut reader = hound::WavReader::new(std::io::Cursor::new(wav_bytes))
            .map_err(|e| TtsError::Internal(format!("wav read: {e}")))?;

        let spec = reader.spec();
        if spec.sample_format != hound::SampleFormat::Int || spec.bits_per_sample != 16 {
            return Err(TtsError::Internal("expected 16-bit PCM WAV".into()));
        }

        let samples: Vec<f32> = match spec.channels {
            1 => reader.samples::<i16>()
                .map(|s| s.map(|v| v as f32 / i16::MAX as f32))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| TtsError::Internal(format!("read: {e}")))?,
            _ => {
                // Downmix to mono by averaging channels
                let all: Vec<i16> = reader.samples::<i16>()
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| TtsError::Internal(format!("read: {e}")))?;
                all.chunks(spec.channels as usize)
                    .map(|chunk| chunk.iter().map(|&s| s as f32).sum::<f32>() / spec.channels as f32 / i16::MAX as f32)
                    .collect()
            }
        };

        Ok(samples)
    }
}

#[async_trait]
impl TtsProvider for XiaomiTts {
    async fn synthesize(&self, text: &str) -> Result<Vec<f32>, TtsError> {
        if text.len() > 500 {
            return Err(TtsError::TextTooLong(text.len()));
        }

        let body = serde_json::json!({
            "model": "mimo-v2.5-tts",
            "messages": [
                {"role": "user", "content": format!("请说：{text}")},
                {"role": "assistant", "content": text}
            ],
            "modalities": ["text", "audio"],
            "audio": {"voice": self.voice, "format": "wav"},
            "max_tokens": 500
        });

        let response = self.client
            .post(&self.base_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| TtsError::Internal(format!("request: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(TtsError::Internal(format!("TTS API {status}: {text}")));
        }

        let parsed: Value = response.json().await
            .map_err(|e| TtsError::Internal(format!("json: {e}")))?;

        let audio_b64 = parsed["choices"][0]["message"]["audio"]["data"]
            .as_str()
            .ok_or_else(|| TtsError::Internal("no audio data in response".into()))?;

        let wav_bytes = BASE64.decode(audio_b64)
            .map_err(|e| TtsError::Internal(format!("base64: {e}")))?;

        Self::decode_wav_to_pcm(&wav_bytes)
    }

    fn voice_name(&self) -> &str {
        &self.voice
    }
}
