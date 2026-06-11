//! 阿里云智能语音交互 — 一句话识别 REST API.
//!
//! API: POST https://nls-gateway.{region}.aliyuncs.com/stream/v1/asr
//! 鉴权: appkey + token
//! 音频: 16kHz 单声道 PCM（跳过 WAV 头 44 字节）

use async_trait::async_trait;
use crate::asr::{AsrError, AsrProvider};

pub struct AliyunAsr {
    appkey: String,
    token: String,
    region: String,
    client: reqwest::Client,
}

impl AliyunAsr {
    pub fn new(appkey: &str, token: &str) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .expect("Failed to build HTTP client");

        Self {
            appkey: appkey.to_string(),
            token: token.to_string(),
            region: "cn-shanghai".to_string(),
            client,
        }
    }

    /// Set Aliyun region (default: cn-shanghai).
    #[allow(dead_code)]
    pub fn with_region(mut self, region: &str) -> Self {
        self.region = region.to_string();
        self
    }

    /// Wrap PCM f32 audio as a WAV file for Aliyun API.
    fn encode_wav(audio: &[f32]) -> Result<Vec<u8>, AsrError> {
        use hound::{WavSpec, WavWriter, SampleFormat};
        use std::io::Cursor;

        let spec = WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };

        let mut buf = Vec::new();
        {
            let mut writer = WavWriter::new(Cursor::new(&mut buf), spec)
                .map_err(|e| AsrError::Internal(format!("wav create: {e}")))?;
            for &s in audio {
                let scaled = (s * i16::MAX as f32) as i16;
                writer.write_sample(scaled)
                    .map_err(|e| AsrError::Internal(format!("wav write: {e}")))?;
            }
            writer.finalize()
                .map_err(|e| AsrError::Internal(format!("wav finalize: {e}")))?;
        }
        Ok(buf)
    }
}

#[async_trait]
impl AsrProvider for AliyunAsr {
    async fn transcribe(&self, audio: &[f32]) -> Result<String, AsrError> {
        // Encode f32 PCM to WAV, then strip header to get raw PCM for Aliyun
        let wav_bytes = Self::encode_wav(audio)?;

        // Skip 44-byte WAV header, Aliyun expects raw PCM
        let pcm_data = if wav_bytes.len() > 44 {
            wav_bytes[44..].to_vec()
        } else {
            return Err(AsrError::Internal("audio too short".into()));
        };

        let url = format!(
            "https://nls-gateway.{}.aliyuncs.com/stream/v1/asr?appkey={}&Format=pcm&SampleRate=16000&EnableIntermediateResult=false",
            self.region, self.appkey
        );

        let response = self
            .client
            .post(&url)
            .header("X-NLS-Token", &self.token)
            .header("Content-Type", "application/octet-stream")
            .body(pcm_data)
            .send()
            .await
            .map_err(|e| AsrError::Internal(format!("request: {}", e)))?;

        let status = response.status();
        let body = response.text().await.map_err(|e| {
            AsrError::Internal(format!("read body: {}", e))
        })?;

        if !status.is_success() {
            return Err(AsrError::Internal(format!(
                "API {}: {}",
                status.as_u16(),
                body.chars().take(300).collect::<String>()
            )));
        }

        #[derive(serde::Deserialize)]
        struct AliyunResponse {
            status: i32,
            result: Option<String>,
            message: Option<String>,
        }

        let resp: AliyunResponse = serde_json::from_str(&body).map_err(|e| {
            AsrError::Internal(format!("parse response: {} — body: {}", e, body.chars().take(200).collect::<String>()))
        })?;

        if resp.status != 20000000 {
            return Err(AsrError::Internal(format!(
                "API error {}: {}",
                resp.status,
                resp.message.unwrap_or_default()
            )));
        }

        Ok(resp.result.unwrap_or_default().trim().to_string())
    }

    fn switch_model(&mut self, _model: &str) -> Result<(), AsrError> {
        Ok(())
    }
}
