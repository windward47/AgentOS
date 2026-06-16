//! Voice module — hotkey listener → mic capture → ASR.
//!
//! Reuses companion-core's CaptureHandle, HotkeyBinding, and AsrProvider trait.
//! The VoiceController manages the push-to-talk lifecycle:
//!   hotkey press  → CaptureHandle::start()  → [listening]
//!   hotkey release → CaptureHandle::stop()  → ASR → text

use std::collections::HashMap;
use std::sync::Arc;

use companion_core::asr::AsrProvider;
use companion_core::capture_mgr::{self};
use companion_core::config::CompanionConfig;
use companion_core::hotkey::{start_hotkey_listener, HotkeyBinding, HotkeyEvent};

use crossbeam::channel::{self, Receiver, Sender};
use std::sync::atomic::AtomicBool;

/// Events produced by the voice pipeline for the TUI to consume.
#[derive(Debug, Clone)]
pub enum VoiceEvent {
    /// Recording started (hotkey pressed).
    ListeningStarted,
    /// Recording stopped, about to transcribe.
    ListeningStopped,
    /// ASR result ready.
    TextReady(String),
    /// Voice pipeline error.
    Error(String),
}

/// Controller for the push-to-talk voice pipeline.
pub struct VoiceController {
    /// Channel to send events to the TUI event loop.
    event_tx: Sender<VoiceEvent>,
}

impl VoiceController {
    /// Spawn the voice pipeline on background threads.
    /// Returns a Receiver for the TUI to poll.
    pub fn spawn(
        config: &CompanionConfig,
    ) -> Receiver<VoiceEvent> {
        let (event_tx, event_rx) = channel::unbounded::<VoiceEvent>();

        // ── Build ASR engines ──
        let asr_engines = build_asr_engines(config);
        if asr_engines.is_empty() {
            log::warn!("[VoiceController] No ASR engines configured");
            let _ = event_tx.send(VoiceEvent::Error("No ASR engines configured".into()));
            return event_rx;
        }

        let engine_name = config.global_voice.asr_engine.clone();
        log::info!("[VoiceController] Using ASR engine: {}", engine_name);

        // ── Capture handle ──
        let capture = Arc::new(capture_mgr::spawn_capture_manager());

        // ── Hotkey bindings ──
        let record_binding = HotkeyBinding::parse(&config.global_voice.record_hotkey)
            .unwrap_or_else(|| HotkeyBinding::parse("Alt+`").unwrap());

        // ── Hotkey channel ──
        let (hotkey_tx, hotkey_rx) = channel::unbounded::<HotkeyEvent>();
        let stop_flag = Arc::new(AtomicBool::new(false));

        start_hotkey_listener(
            record_binding,
            // Auxiliary hotkeys — not used in TUI v1, but the listener requires them
            HotkeyBinding::parse("Alt+T").unwrap_or_else(|| HotkeyBinding::parse("Alt+`").unwrap()),
            HotkeyBinding::parse("Alt+Shift+V").unwrap_or_else(|| HotkeyBinding::parse("Alt+`").unwrap()),
            HotkeyBinding::parse("Alt+Shift+E").unwrap_or_else(|| HotkeyBinding::parse("Alt+`").unwrap()),
            hotkey_tx,
            stop_flag,
        );

        // ── Background thread: hotkey events → capture → ASR ──
        let tx = event_tx.clone();
        let engines = Arc::new(asr_engines);
        let eng_name = engine_name;

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("voice rt");
            let mut recording = false;

            while let Ok(evt) = hotkey_rx.recv() {
                match evt {
                    HotkeyEvent::RecordStart => {
                        if recording {
                            continue;
                        }
                        recording = true;
                        log::info!("[Voice] PTT start");
                        let _ = tx.send(VoiceEvent::ListeningStarted);
                        if !capture.start() {
                            log::error!("[Voice] Capture start failed");
                            let _ = tx.send(VoiceEvent::Error("Mic start failed".into()));
                            recording = false;
                        }
                    }
                    HotkeyEvent::RecordStop => {
                        if !recording {
                            continue;
                        }
                        recording = false;
                        log::info!("[Voice] PTT stop");
                        let _ = tx.send(VoiceEvent::ListeningStopped);

                        let i16_samples = capture.stop();
                        if i16_samples.is_empty() {
                            log::warn!("[Voice] No audio captured");
                            let _ = tx.send(VoiceEvent::Error("No audio captured".into()));
                            continue;
                        }

                        let f32_samples = capture_mgr::i16_to_f32(&i16_samples);
                        log::info!(
                            "[Voice] Captured {} samples ({:.1}s)",
                            f32_samples.len(),
                            f32_samples.len() as f64 / 16000.0
                        );

                        let text = {
                            let engine = engines.get(&eng_name);
                            match engine {
                                Some(e) => {
                                    match rt.block_on(e.transcribe(&f32_samples)) {
                                        Ok(t) => t,
                                        Err(err) => {
                                            log::error!("[Voice] ASR failed: {}", err);
                                            let _ = tx.send(VoiceEvent::Error(format!("ASR: {err}")));
                                            continue;
                                        }
                                    }
                                }
                                None => {
                                    log::error!("[Voice] ASR engine '{}' not found", eng_name);
                                    let _ = tx.send(VoiceEvent::Error(format!("ASR engine '{eng_name}' not found")));
                                    continue;
                                }
                            }
                        };

                        if text.is_empty() {
                            log::warn!("[Voice] ASR returned empty text");
                            continue;
                        }
                        log::info!("[Voice] ASR result: {} chars", text.len());
                        let _ = tx.send(VoiceEvent::TextReady(text));
                    }
                    _ => {
                        // TtsTrigger, InjectModeSwitch, EngineSwitch — ignored in TUI v1
                    }
                }
            }
            log::info!("[Voice] Hotkey listener exited");
        });

        event_rx
    }
}

/// Build ASR engine map (lightweight version of voice_handler::build_global_asr_engines).
fn build_asr_engines(
    cfg: &CompanionConfig,
) -> HashMap<String, Box<dyn AsrProvider + Send + Sync>> {
    use companion_core::asr::xiaomi_asr::XiaomiAsr;
    use companion_core::asr::whisper_cloud::WhisperCloud;
    use companion_core::config::resolve_provider_key;

    let mut engines: HashMap<String, Box<dyn AsrProvider + Send + Sync>> = HashMap::new();
    let api_token = resolve_provider_key(&cfg.asr, &cfg.default_api_key);

    // Mimo ASR (Xiaomi)
    if !api_token.is_empty() {
        let base_url = "https://token-plan-cn.xiaomimimo.com/v1/chat/completions".to_string();
        engines.insert("mimo".into(), Box::new(XiaomiAsr::with_url(&api_token, &base_url)));
    }

    // OpenAI Whisper
    if !api_token.is_empty() {
        engines.insert("openai".into(), Box::new(WhisperCloud::new(&api_token, "whisper-1")));
    }

    engines
}
