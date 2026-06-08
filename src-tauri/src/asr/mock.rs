use async_trait::async_trait;
use crate::asr::{AsrError, AsrProvider};

/// Mock ASR provider for testing — returns fixed transcription.
pub struct MockAsr {
    response: String,
}

impl MockAsr {
    pub fn new(response: &str) -> Self {
        Self { response: response.to_string() }
    }
}

#[async_trait]
impl AsrProvider for MockAsr {
    async fn transcribe(&self, _audio: &[f32]) -> Result<String, AsrError> {
        Ok(self.response.clone())
    }

    fn switch_model(&mut self, _model: &str) -> Result<(), AsrError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_asr() {
        let mock = MockAsr::new("你好");
        let result = mock.transcribe(&[0.0; 16000]).await.unwrap();
        assert_eq!(result, "你好");
    }
}
