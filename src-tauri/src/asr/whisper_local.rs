//! Local ASR via Whisper.cpp subprocess.
//!
//! Saves audio as a temp WAV file, calls `whisper-cli` with the model,
//! and parses the transcribed text from stdout.

use std::path::PathBuf;
use std::process::Command;
use async_trait::async_trait;
use crate::asr::{AsrError, AsrProvider};

/// Local Whisper ASR via subprocess.
pub struct WhisperLocal {
    /// Path to `whisper-cli` or `main` binary.
    binary_path: PathBuf,
    /// Path to the ggml model file.
    model_path: PathBuf,
    /// Temp directory for WAV files.
    temp_dir: PathBuf,
}

impl WhisperLocal {
    pub fn new(binary_path: PathBuf, model_path: PathBuf) -> Self {
        Self {
            temp_dir: std::env::temp_dir().join("companion_whisper"),
            binary_path,
            model_path,
        }
    }

    /// Write PCM f32 mono audio as a WAV file.
    fn write_wav(&self, audio: &[f32], path: &std::path::Path) -> Result<(), AsrError> {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec)
            .map_err(|e| AsrError::Internal(format!("wav create: {e}")))?;

        // Convert f32 [-1..1] to i16
        for &sample in audio {
            let scaled = (sample * i16::MAX as f32) as i16;
            writer.write_sample(scaled)
                .map_err(|e| AsrError::Internal(format!("wav write: {e}")))?;
        }
        writer.finalize()
            .map_err(|e| AsrError::Internal(format!("wav finalize: {e}")))?;
        Ok(())
    }
}

#[async_trait]
impl AsrProvider for WhisperLocal {
    async fn transcribe(&self, audio: &[f32]) -> Result<String, AsrError> {
        if audio.len() < 1600 {
            // Less than ~100ms at 16kHz
            return Err(AsrError::AudioTooShort(audio.len() as u32 * 1000 / 16000));
        }

        // Ensure temp dir exists
        std::fs::create_dir_all(&self.temp_dir)
            .map_err(|e| AsrError::Internal(format!("temp dir: {e}")))?;

        let wav_path = self.temp_dir.join("input.wav");
        self.write_wav(audio, &wav_path)?;

        // Spawn whisper-cli subprocess
        let output = tokio::task::spawn_blocking({
            let binary = self.binary_path.clone();
            let model = self.model_path.clone();
            let wav = wav_path.clone();
            move || {
                Command::new(&binary)
                    .arg("-m").arg(&model)
                    .arg("-f").arg(&wav)
                    .arg("-oj")     // JSON output
                    .arg("-nt")     // no timestamps
                    .output()
            }
        }).await.map_err(|e| AsrError::Internal(format!("join: {e}")))?
          .map_err(|e| AsrError::Internal(format!("whisper subprocess: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AsrError::Internal(format!("whisper failed: {stderr}")));
        }

        // Parse JSON output: { "text": "..." }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let parsed: serde_json::Value = serde_json::from_str(&stdout)
            .map_err(|e| AsrError::Internal(format!("json parse: {e}")))?;

        let text = parsed["text"]
            .as_str()
            .unwrap_or("")
            .trim()
            .to_string();

        // Cleanup temp file
        std::fs::remove_file(&wav_path).ok();

        Ok(text)
    }

    fn switch_model(&mut self, model: &str) -> Result<(), AsrError> {
        self.model_path = PathBuf::from(model);
        Ok(())
    }
}
