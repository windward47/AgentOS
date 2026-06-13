//! Global voice command handler — bridges hotkey events → capture → ASR → inject / TTS playback.
//!
//! Extracted from the original monolithic `lib.rs` to keep the Tauri setup focused.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use companion_core::audio::utils::{f32_to_i16, pcm_i16_to_wav};
use companion_core::capture_mgr::{CaptureHandle, i16_to_f32};
use companion_core::asr::{AsrProvider, xiaomi_asr::XiaomiAsr, whisper_cloud::WhisperCloud, whisper_local::WhisperLocal, aliyun_asr::AliyunAsr};
use companion_core::inject::{inject_text, text_reader, InjectMode};
use companion_core::tts::{TtsProvider, xiaomi_tts::XiaomiTts};
use companion_core::tts::playback;
use companion_core::config::{CompanionConfig, resolve_provider_key};
use tauri::Manager;
use tauri::Emitter;
use enigo::Keyboard;

use crate::state::{VoiceState, ConfigState};

/// Commands dispatched from hotkey listener or tray menu to the voice handler loop.
pub enum VoiceCommand {
    RecordStart,
    RecordStop,
    TtsTrigger,
    SetAsrEngine(String),
    SetInjectMode(String),
    ToggleInjectMode,
    CycleAsrEngine,
}

/// Handle one voice command — may involve capture, ASR, inject, or TTS.
pub async fn handle_voice_command(
    app: &tauri::AppHandle,
    cmd: VoiceCommand,
    capture_handle: &Arc<CaptureHandle>,
    inject_mode: &Arc<std::sync::Mutex<InjectMode>>,
    asr_engine_name: &Arc<std::sync::Mutex<String>>,
    engines: &Arc<std::collections::HashMap<String, Box<dyn AsrProvider + Send + Sync>>>,
    _tts_engine: &str,
) {
    match cmd {
        VoiceCommand::RecordStart => {
            log::info!("[GlobalVoice] Push-to-talk: starting recording");
            app.state::<VoiceState>().is_listening.store(true, Ordering::Release);
            if !capture_handle.start() {
                log::error!("[GlobalVoice] Failed to start capture");
                app.state::<VoiceState>().is_listening.store(false, Ordering::Release);
            }
        }

        VoiceCommand::RecordStop => {
            log::info!("[GlobalVoice] Push-to-talk: stopping recording");
            app.state::<VoiceState>().is_listening.store(false, Ordering::Release);

            let i16_samples = capture_handle.stop();
            if i16_samples.is_empty() {
                log::warn!("[GlobalVoice] No audio captured");
                return;
            }
            let f32_samples = i16_to_f32(&i16_samples);
            log::info!(
                "[GlobalVoice] Captured {} samples ({:.1}s)",
                f32_samples.len(),
                f32_samples.len() as f64 / 16000.0
            );

            let engine_name = asr_engine_name.lock().unwrap().clone();
            log::info!("[GlobalVoice] Running ASR with '{}'...", engine_name);

            let engine = engines.get(&engine_name);
            let text = match engine {
                Some(e) => match e.transcribe(&f32_samples).await {
                    Ok(t) => t,
                    Err(err) => {
                        log::error!("[GlobalVoice] ASR failed: {}", err);
                        return;
                    }
                },
                None => {
                    log::error!("[GlobalVoice] ASR engine '{}' not found", engine_name);
                    return;
                }
            };

            if text.is_empty() {
                log::warn!("[GlobalVoice] ASR returned empty text");
                return;
            }
            log::info!("[GlobalVoice] ASR result: {} chars", text.len());

            // Check if Companion window is focused
            let companion_focused = app.get_webview_window("main")
                .map(|w| w.is_focused().unwrap_or(false))
                .unwrap_or(false);
            // Write debug to file since terminal is too noisy
            let _ = std::fs::write(
                dirs::home_dir().unwrap_or_default().join(".companion").join("logs").join("voice_debug.log"),
                format!("[{}] focused={} text_len={}\n", 
                    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                    companion_focused, text.len()),
            );

            // Always emit to chat window (works even when chat is focused OR not)
            let _ = app.emit("voice_asr_result", serde_json::json!({ "text": &text }));

            if !companion_focused {
                let mode = *inject_mode.lock().unwrap();
                log::info!("[GlobalVoice] Injecting via {:?}", mode);
                if let Err(e) = inject_text(&text, mode) {
                    log::error!("[GlobalVoice] Injection failed: {}", e);
                }
            }
        }

        VoiceCommand::TtsTrigger => {
            log::info!("[GlobalVoice] TTS trigger: reading selected text...");

            // Explicitly release Alt key so enigo's Ctrl+C simulation
            // doesn't become Alt+Ctrl+C due to the still-held Alt.
            if let Ok(mut enigo) = enigo::Enigo::new(&enigo::Settings::default()) {
                let _ = enigo.key(enigo::Key::Alt, enigo::Direction::Release);
            }
            std::thread::sleep(std::time::Duration::from_millis(100));

            let text = match text_reader::read_selected_text() {
                Ok(t) if !t.is_empty() => {
                    log::info!("[GlobalVoice] Got {} chars from selection", t.len());
                    t
                }
                Ok(_) => {
                    log::warn!("[GlobalVoice] No text selected");
                    return;
                }
                Err(e) => {
                    log::error!("[GlobalVoice] Failed to read selection: {}", e);
                    return;
                }
            };

            let state = app.state::<ConfigState>();
            let cfg_guard = state.config.lock().await;
            let api_token = resolve_provider_key(&cfg_guard.tts, &cfg_guard.default_api_key);

            if api_token.is_empty() {
                log::error!("[GlobalVoice] COMPANION_API_TOKEN not set (neither in config nor env), cannot use TTS");
                return;
            }

            let tts = XiaomiTts::new(&api_token, "茉莉");
            match tts.synthesize(&text).await {
                Ok(pcm_f32) => {
                    log::info!("[GlobalVoice] TTS synthesized {} f32 samples", pcm_f32.len());
                    app.state::<VoiceState>().is_speaking.store(true, Ordering::Release);
                    animate_lip_sync(&pcm_f32, 24000, app);

                    let i16 = f32_to_i16(&pcm_f32);
                    match pcm_i16_to_wav(&i16, 24000) {
                        Ok(wav) => {
                            log::info!("[GlobalVoice] WAV encoded {} bytes, starting playback", wav.len());
                            playback::play_wav_async(wav);
                        }
                        Err(e) => log::error!("[GlobalVoice] WAV encoding failed: {}", e),
                    }
                }
                Err(e) => log::error!("[GlobalVoice] TTS synthesis failed: {}", e),
            }
        }

        VoiceCommand::SetAsrEngine(name) => {
            log::info!("[GlobalVoice] Switching ASR engine to: {}", name);
            {
                let state = app.state::<ConfigState>();
                let mut cfg = state.config.lock().await;
                cfg.global_voice.asr_engine = name.clone();
                state.save().await;
            }
            let mut guard = asr_engine_name.lock().unwrap();
            *guard = name;
        }

        VoiceCommand::SetInjectMode(mode_str) => {
            log::info!("[GlobalVoice] Switching inject mode to: {}", mode_str);
            let mut guard = inject_mode.lock().unwrap();
            *guard = match mode_str.as_str() {
                "clipboard" => InjectMode::Clipboard,
                _ => InjectMode::Keyboard,
            };
            { let state = app.state::<ConfigState>(); let mut cfg = state.config.lock().await;
                cfg.global_voice.inject_mode = mode_str;
                state.save().await;
            }
        }

        VoiceCommand::ToggleInjectMode => {
            let mut guard = inject_mode.lock().unwrap();
            *guard = guard.toggle();
            let mode_str = guard.as_str().to_string();
            log::info!("[GlobalVoice] Toggled inject mode to: {}", mode_str);
            { let state = app.state::<ConfigState>(); let mut cfg = state.config.lock().await;
                cfg.global_voice.inject_mode = mode_str;
                state.save().await;
            }
        }

        VoiceCommand::CycleAsrEngine => {
            let mut guard = asr_engine_name.lock().unwrap();
            let names: Vec<&String> = engines.keys().collect();
            if names.is_empty() {
                return;
            }
            let pos = names.iter().position(|n| *n == &*guard);
            let next_idx = match pos {
                Some(i) => (i + 1) % names.len(),
                None => 0,
            };
            let next = names[next_idx].clone();
            log::info!("[GlobalVoice] Cycling ASR engine to: {}", next);
            *guard = next.clone();
            { let state = app.state::<ConfigState>(); let mut cfg = state.config.lock().await;
                cfg.global_voice.asr_engine = next;
                state.save().await;
            }
        }
    }
}

/// Compute RMS envelope from PCM f32 audio and animate `lip_level` in sync with playback.
pub fn animate_lip_sync(
    pcm: &[f32],
    sample_rate: u32,
    app_handle: &tauri::AppHandle,
) {
    let frame_ms = 50u64;
    let frame_samples = (sample_rate as u64 * frame_ms / 1000) as usize;
    if frame_samples == 0 {
        return;
    }
    let total_frames = pcm.len() / frame_samples;
    if total_frames == 0 {
        return;
    }

    let rms_values: Vec<f32> = pcm
        .chunks(frame_samples)
        .map(|chunk| {
            let sum_sq: f32 = chunk.iter().map(|&s| s * s).sum();
            let rms = (sum_sq / chunk.len() as f32).sqrt();
            (rms * 6.0).clamp(0.0, 1.0)
        })
        .collect();

    let app_handle = app_handle.clone();
    std::thread::spawn(move || {
        let voice = app_handle.state::<VoiceState>();
        let lip_level = &voice.lip_level;
        for &level in &rms_values {
            *lip_level.lock().unwrap() = level;
            std::thread::sleep(std::time::Duration::from_millis(frame_ms));
        }
        *lip_level.lock().unwrap() = 0.0;
        voice.is_speaking.store(false, Ordering::Release);
    });
}

/// Build a map of ASR engine name → Box<dyn AsrProvider>.
pub fn build_global_asr_engines(
    cfg: &CompanionConfig,
) -> std::collections::HashMap<String, Box<dyn AsrProvider + Send + Sync>> {
    use std::collections::HashMap;

    let mut engines: HashMap<String, Box<dyn AsrProvider + Send + Sync>> = HashMap::new();
    let api_token = resolve_provider_key(&cfg.asr, &cfg.default_api_key);

    // Mimo ASR (Xiaomi) — always use Xiaomi URL for global hotkeys
    if !api_token.is_empty() {
        // Global hotkey always uses Xiaomi cloud, not local provider
        let base_url = "https://token-plan-cn.xiaomimimo.com/v1/chat/completions".to_string();
        log::info!("[GlobalVoice] Registering Mimo ASR engine (cloud)");
        engines.insert("mimo".into(), Box::new(XiaomiAsr::with_url(&api_token, &base_url)));
    }

    // OpenAI Whisper
    if !api_token.is_empty() {
        log::info!("[GlobalVoice] Registering OpenAI Whisper engine");
        engines.insert("openai".into(), Box::new(WhisperCloud::new(&api_token, "whisper-1")));
    }

    // Aliyun ASR
    if let Some(key) = &cfg.asr.key {
        if let Some(_model) = &cfg.asr.model {
            if let Some((appkey, token)) = key.split_once(':') {
                log::info!("[GlobalVoice] Registering Aliyun ASR engine");
                engines.insert("aliyun".into(), Box::new(AliyunAsr::new(appkey, token)));
            }
        }
    }

    // Whisper Local (if binary and model are configured)
    if let Some(path_str) = &cfg.asr.url {
        let binary_path = std::path::PathBuf::from(path_str);
        let model_path = cfg
            .asr
            .model
            .as_ref()
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                let home = dirs::home_dir().unwrap_or_default();
                home.join(".companion").join("models").join("ggml-base.bin")
            });
        if binary_path.exists() && model_path.exists() {
            log::info!("[GlobalVoice] Registering Whisper Local engine");
            engines.insert(
                "whisper-local".into(),
                Box::new(WhisperLocal::new(binary_path, model_path)),
            );
        }
    }

    engines
}
