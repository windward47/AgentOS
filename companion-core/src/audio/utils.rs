//! Audio utility functions (WAV encoding, RMS level, etc.)

use hound::WavSpec;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AudioUtilError {
    #[error("WAV encode: {0}")]
    Wav(String),
}

/// Encode PCM i16 samples (16 kHz, mono) into WAV bytes.
pub fn pcm_i16_to_wav(samples: &[i16], sample_rate: u32) -> Result<Vec<u8>, AudioUtilError> {
    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut buf = Vec::new();
    {
        let mut writer = hound::WavWriter::new(std::io::Cursor::new(&mut buf), spec).map_err(|e| AudioUtilError::Wav(e.to_string()))?;
        for &sample in samples {
            writer.write_sample(sample).map_err(|e| AudioUtilError::Wav(e.to_string()))?;
        }
        writer.finalize().map_err(|e| AudioUtilError::Wav(e.to_string()))?;
    }
    Ok(buf)
}

/// Convert PCM f32 samples to PCM i16 (scaling by i16::MAX).
pub fn f32_to_i16(audio: &[f32]) -> Vec<i16> {
    audio
        .iter()
        .map(|&s| (s * i16::MAX as f32).clamp(-32768.0, 32767.0) as i16)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pcm_i16_to_wav_roundtrip() {
        let samples: Vec<i16> = vec![0, 1000, -1000, 0, 5000, -5000, 0];
        let wav_bytes = pcm_i16_to_wav(&samples, 16000).unwrap();
        assert!(!wav_bytes.is_empty());
        assert_eq!(&wav_bytes[0..4], b"RIFF");
        assert_eq!(&wav_bytes[8..12], b"WAVE");
    }

    #[test]
    fn test_f32_to_i16() {
        let f32_samples = vec![0.0, 0.5, -0.5, 1.0, -1.0];
        let i16_samples = f32_to_i16(&f32_samples);
        assert_eq!(i16_samples.len(), 5);
        assert_eq!(i16_samples[0], 0);
        assert!(i16_samples[1] > 16000);
        assert!(i16_samples[3] >= 32767);
    }
}
