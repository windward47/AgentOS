use async_trait::async_trait;
use crate::tts::{TtsError, TtsProvider};

/// Mock TTS provider for testing — returns fixed-length silent audio.
pub struct MockTts {
    voice: String,
    sample_count: usize,
}

impl MockTts {
    pub fn new(sample_count: usize) -> Self {
        Self { voice: "Mock".into(), sample_count }
    }
}

#[async_trait]
impl TtsProvider for MockTts {
    async fn synthesize(&self, _text: &str) -> Result<Vec<f32>, TtsError> {
        Ok(vec![0.0; self.sample_count])
    }

    fn voice_name(&self) -> &str {
        &self.voice
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_tts() {
        let mock = MockTts::new(16000);
        let audio = mock.synthesize("test").await.unwrap();
        assert_eq!(audio.len(), 16000);
        assert_eq!(mock.voice_name(), "Mock");
    }
}
