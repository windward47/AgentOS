//! Audio utility functions (WAV encoding, RMS level, etc.)

use hound::WavSpec;

/// Encode PCM i16 samples (16 kHz, mono) into WAV bytes.
pub fn pcm_i16_to_wav(samples: &[i16], sample_rate: u32) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut buf = Vec::new();
    {
        let mut writer = hound::WavWriter::new(std::io::Cursor::new(&mut buf), spec)?;
        for &sample in samples {
            writer.write_sample(sample)?;
        }
        writer.finalize()?;
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

/// Simple RMS energy level of a PCM i16 buffer.
#[allow(dead_code)]
pub fn rms_level_i16(samples: &[i16]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f64 = samples.iter().map(|&s| (s as f64).powi(2)).sum();
    (sum_sq / samples.len() as f64).sqrt()
}

/// Convert RMS to a normalized 0..1 level.
#[allow(dead_code)]
pub fn normalized_level(rms: f64) -> f64 {
    let level = rms / 16384.0;
    level.clamp(0.0, 1.0)
}

/// Duration in seconds given sample count and rate.
#[allow(dead_code)]
pub fn duration_seconds(sample_count: usize, sample_rate: u32) -> f64 {
    sample_count as f64 / sample_rate as f64
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

    #[test]
    fn test_rms_silence() {
        let samples = vec![0i16; 100];
        assert!(rms_level_i16(&samples) < 1.0);
    }

    #[test]
    fn test_duration() {
        assert!((duration_seconds(16000, 16000) - 1.0).abs() < 0.001);
    }
}
