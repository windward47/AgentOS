/// VAD (Voice Activity Detection) engine.
pub trait VadEngine: Send {
    /// Return `true` if the chunk contains speech.
    fn is_voice(&self, chunk: &[f32]) -> bool;

    /// Reset internal state (for new utterance).
    fn reset(&mut self);
}

/// Simple energy-based VAD — uses RMS threshold.
pub struct EnergyVad {
    threshold: f32,
}

impl EnergyVad {
    pub fn new(threshold: f32) -> Self {
        Self { threshold }
    }
}

impl VadEngine for EnergyVad {
    fn is_voice(&self, chunk: &[f32]) -> bool {
        let rms = (chunk.iter().map(|s| s * s).sum::<f32>() / chunk.len() as f32).sqrt();
        rms > self.threshold
    }

    fn reset(&mut self) {
        // stateless, no-op
    }
}

// ---------------------------------------------------------------------------
// VadState — utterance boundary state machine
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VadState {
    /// Waiting for speech to begin.
    Idle,
    /// Speech detected, still accumulating (before min_speech_ms).
    SpeechStart,
    /// In an active utterance.
    Speaking,
    /// Silence after speaking, waiting for timeout or renewed speech.
    Silence,
}

/// Frame size in ms used for all VAD processing.
pub const VAD_FRAME_MS: u32 = 30;

/// High-level VAD controller — drives utterance boundary detection.
pub struct Vad {
    pub engine: Box<dyn VadEngine>,
    /// Minimum speech duration to confirm utterance start (ms).
    pub min_speech_ms: u32,
    /// Silence duration to end utterance (ms).
    pub silence_timeout_ms: u32,
    /// Energy threshold (0.0 – 1.0).
    pub threshold: f32,
    // Internal state machine
    state: VadState,
    speech_frames: u32,
    silence_frames: u32,
    frame_ms: u32,
}

impl Vad {
    pub fn new(threshold: f32) -> Self {
        Self {
            engine: Box::new(EnergyVad::new(threshold)),
            min_speech_ms: 200,
            silence_timeout_ms: 600,
            threshold,
            state: VadState::Idle,
            speech_frames: 0,
            silence_frames: 0,
            frame_ms: VAD_FRAME_MS,
        }
    }

    /// Process one audio frame and return the updated VAD state.
    /// `audio` should be roughly `frame_ms` of audio at the sample rate.
    pub fn process_frame(&mut self, audio: &[f32]) -> VadState {
        let is_voice = self.engine.is_voice(audio);

        match self.state {
            VadState::Idle => {
                if is_voice {
                    self.speech_frames = 1;
                    self.state = VadState::SpeechStart;
                }
            }
            VadState::SpeechStart => {
                if is_voice {
                    self.speech_frames += 1;
                    let speech_ms = self.speech_frames * self.frame_ms;
                    if speech_ms >= self.min_speech_ms {
                        self.state = VadState::Speaking;
                    }
                } else {
                    // False start — not enough speech
                    self.speech_frames = 0;
                    self.state = VadState::Idle;
                }
            }
            VadState::Speaking => {
                if is_voice {
                    self.silence_frames = 0;
                } else {
                    self.silence_frames += 1;
                    self.state = VadState::Silence;
                }
            }
            VadState::Silence => {
                if is_voice {
                    // Speech resumed — go back
                    self.silence_frames = 0;
                    self.state = VadState::Speaking;
                } else {
                    self.silence_frames += 1;
                    let silence_ms = self.silence_frames * self.frame_ms;
                    if silence_ms >= self.silence_timeout_ms {
                        // Utterance ended
                        self.speech_frames = 0;
                        self.silence_frames = 0;
                        self.state = VadState::Idle;
                        return VadState::Idle; // signal "utterance complete"
                    }
                }
            }
        }

        self.state
    }

    /// Whether we are currently in an active utterance (Speaking or Silence).
    pub fn in_utterance(&self) -> bool {
        matches!(self.state, VadState::Speaking | VadState::Silence | VadState::SpeechStart)
    }

    /// Reset VAD to idle (e.g., when a new utterance starts externally).
    pub fn reset(&mut self) {
        self.state = VadState::Idle;
        self.speech_frames = 0;
        self.silence_frames = 0;
        self.engine.reset();
    }

    pub fn current_state(&self) -> VadState {
        self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_energy_vad_silence() {
        let vad = EnergyVad::new(0.1);
        let silence = vec![0.0; 480]; // 10ms of silence at 48kHz
        assert!(!vad.is_voice(&silence));
    }

    #[test]
    fn test_energy_vad_speech() {
        let vad = EnergyVad::new(0.1);
        let speech: Vec<f32> = (0..480).map(|i| (i as f32 / 480.0 * 0.5)).collect();
        assert!(vad.is_voice(&speech));
    }

    #[test]
    fn test_vad_state_machine() {
        let mut vad = Vad::new(0.1);
        assert_eq!(vad.current_state(), VadState::Idle);

        // First frame: still SpeechStart (need >200ms speech)
        let speech: Vec<f32> = vec![0.5; 480]; // 10ms
        let state = vad.process_frame(&speech);
        assert_eq!(state, VadState::SpeechStart);

        // After enough frames: Speaking
        for _ in 0..20 {
            vad.process_frame(&speech);
        }
        assert_eq!(vad.current_state(), VadState::Speaking);

        // Silence frame: silence
        let silence = vec![0.0; 480];
        let state = vad.process_frame(&silence);
        assert_eq!(state, VadState::Silence);

        // More silence frames should eventually trigger utterance end
        let mut ended = false;
        for _ in 0..70 {
            let s = vad.process_frame(&silence);
            if s == VadState::Idle && !vad.in_utterance() {
                ended = true;
                break;
            }
        }
        assert!(ended, "utterance should have ended after silence timeout");
    }
}
