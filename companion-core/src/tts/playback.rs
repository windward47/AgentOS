//! TTS audio playback via system native player.
//!
//! Writes WAV data to a temp file and plays with:
//! - Windows: PowerShell Media.SoundPlayer
//! - macOS: afplay
//! - Linux: aplay

use std::io::Write;
use std::process::Command;
use tempfile::NamedTempFile;

use crate::tts::TtsError;

/// Play WAV audio synchronously.
pub fn play_wav(wav_data: &[u8]) -> Result<(), TtsError> {
    let mut tmp = NamedTempFile::with_suffix(".wav")
        .map_err(|e| TtsError::Internal(format!("temp file: {}", e)))?;

    tmp.write_all(wav_data)
        .map_err(|e| TtsError::Internal(format!("write temp: {}", e)))?;

    let path = tmp.path().to_path_buf();

    let status = if cfg!(target_os = "windows") {
        Command::new("powershell")
            .args([
                "-c",
                &format!(
                    "(New-Object Media.SoundPlayer '{}').PlaySync()",
                    path.display()
                ),
            ])
            .status()
    } else if cfg!(target_os = "macos") {
        Command::new("afplay").arg(&path).status()
    } else {
        Command::new("aplay").arg(&path).status()
    };

    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(TtsError::Internal(format!("playback exited: {}", s))),
        Err(e) => Err(TtsError::Internal(format!("playback cmd: {}", e))),
    }
}

/// Play WAV audio on a background thread. Returns immediately.
pub fn play_wav_async(wav_data: Vec<u8>) {
    std::thread::spawn(move || {
        if let Err(e) = play_wav(&wav_data) {
            log::error!("TTS playback failed: {}", e);
        }
    });
}
