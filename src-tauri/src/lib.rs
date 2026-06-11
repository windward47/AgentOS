#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// ---------------------------------------------------------------------------
// Companion Core — modular desktop agent backend
// ---------------------------------------------------------------------------

pub mod agent;
pub mod audio;
pub mod asr;
pub mod capture_mgr;
pub mod config;
pub mod emotion;
pub mod hotkey;
pub mod inject;
pub mod llm;
pub mod mcp;
pub mod permissions;
pub mod sandbox;
pub mod state;
pub mod tools;
pub mod tts;
pub mod websocket;

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use capture_mgr::{CaptureHandle, i16_to_f32};
use audio::utils::f32_to_i16;
use config::{CompanionConfig, ConfigManager};
use hotkey::{start_hotkey_listener, HotkeyBinding};
use inject::{inject_text, text_reader, InjectMode};
use enigo::{Enigo, Key, Keyboard, Direction, Settings};
use state::AppState;
use tauri::{
    image::Image,
    menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};
use tts::playback;
use asr::{AsrProvider, xiaomi_asr::XiaomiAsr, whisper_cloud::WhisperCloud, whisper_local::WhisperLocal, aliyun_asr::AliyunAsr};
use tts::{TtsProvider, xiaomi_tts::XiaomiTts};

// ── Global voice shared state ──────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(if cfg!(debug_assertions) {
                    log::LevelFilter::Debug
                } else {
                    log::LevelFilter::Info
                })
                .build(),
        )
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            state::chat,
            state::chat_with_tools,
            state::get_history,
            state::clear_history,
            state::transcribe_audio,
            state::synthesize_audio,
            state::get_config,
            state::update_config,
            state::set_lip_level,
            state::get_lip_level,
            state::get_voice_state,
            state::browse_screenshot,
            state::get_audit_log,
            state::list_models,
        ])
        .setup(|app| {
            // ---- Load config ----
            let config_mgr = ConfigManager::new().ok();
            let config = config_mgr.as_ref().and_then(|c| c.load().ok());
            let cfg = config.as_ref().cloned().unwrap_or_default();

            // ---- Main window ----
            let main_win = app.get_webview_window("main").unwrap();
            main_win.set_title("Companion v0.1.0").ok();

            // Intercept close → hide to tray instead of quitting
            {
                let main_win = main_win.clone();
                main_win.clone().on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = main_win.hide();
                    }
                });
            }

            // ---- Avatar window ----
            if let Some(avatar) = app.get_webview_window("avatar") {
                let _ = avatar.set_position(tauri::PhysicalPosition::new(
                    main_win.outer_size().unwrap().width as i32 + 100,
                    100,
                ));
                avatar.open_devtools();
            }

            // ---- Global voice state ----
            let capture_handle = Arc::new(capture_mgr::spawn_capture_manager());
            let global_inject_mode: std::sync::Mutex<InjectMode> =
                std::sync::Mutex::new(InjectMode::from_config(&cfg));
            let global_inject_mode = Arc::new(global_inject_mode);
            let global_asr_engine: std::sync::Mutex<String> =
                std::sync::Mutex::new(cfg.global_voice.asr_engine.clone());
            let global_asr_engine = Arc::new(global_asr_engine);

            // ---- Build ASR engines ----
            let asr_engines = build_global_asr_engines(&cfg);

            // ---- System tray icon & menu (with voice items) ----
            let icon_img = image::load_from_memory(include_bytes!("../icons/icon.png"))
                .expect("failed to decode tray icon")
                .into_rgba8();
            let (w, h) = icon_img.dimensions();
            let tray_icon = Image::new_owned(icon_img.into_raw(), w, h);

            // ASR Engine submenu
            let asr_engines_names: Vec<&str> = asr_engines.keys().map(|s| s.as_str()).collect();
            let asr_submenu = {
                let mut builder = SubmenuBuilder::new(app, "ASR Engine");
                for name in &asr_engines_names {
                    builder =
                        builder.item(&MenuItemBuilder::with_id(format!("asr_{}", name), name).build(app).unwrap());
                }
                builder.build().unwrap()
            };

            // Inject Mode submenu
            let inject_submenu = SubmenuBuilder::new(app, "Inject Mode")
                .item(&MenuItemBuilder::with_id("inject_keyboard", "Keyboard").build(app).unwrap())
                .item(&MenuItemBuilder::with_id("inject_clipboard", "Clipboard").build(app).unwrap())
                .build()
                .unwrap();

            let separator0 = PredefinedMenuItem::separator(app).unwrap();
            let record_item = MenuItemBuilder::with_id("record", "🎤 Record (Alt+`)")
                .build(app)
                .unwrap();
            let tts_item = MenuItemBuilder::with_id("tts_selection", "🔊 TTS Selected (Alt+T)")
                .build(app)
                .unwrap();
            let separator1 = PredefinedMenuItem::separator(app).unwrap();
            let show_hide = MenuItemBuilder::with_id("show_hide", "Show/Hide")
                .build(app)
                .unwrap();
            let separator2 = PredefinedMenuItem::separator(app).unwrap();
            let settings = MenuItemBuilder::with_id("settings", "Settings")
                .build(app)
                .unwrap();
            let separator3 = PredefinedMenuItem::separator(app).unwrap();
            let quit = MenuItemBuilder::with_id("quit", "Quit")
                .accelerator("CmdOrCtrl+Q")
                .build(app)
                .unwrap();

            let menu = MenuBuilder::new(app)
                .item(&asr_submenu)
                .item(&inject_submenu)
                .item(&separator0)
                .item(&record_item)
                .item(&tts_item)
                .item(&separator1)
                .item(&show_hide)
                .item(&separator2)
                .item(&settings)
                .item(&separator3)
                .item(&quit)
                .build()
                .unwrap();

            // ── Global voice channel (crossbeam for !Send safety) ──
            let (voice_tx, voice_rx) = crossbeam::channel::unbounded::<VoiceCommand>();
            let voice_tx_for_menu = voice_tx.clone();

            let _tray = TrayIconBuilder::new()
                .icon(tray_icon)
                .tooltip("Companion")
                .menu(&menu)
                .on_menu_event(move |app, event| {
                    let id = event.id.as_ref();
                    match id {
                        "show_hide" => {
                            if let Some(win) = app.get_webview_window("main") {
                                if win.is_visible().unwrap_or(true) {
                                    let _ = win.hide();
                                } else {
                                    let _ = win.show();
                                    let _ = win.set_focus();
                                }
                            }
                        }
                        "settings" => {
                            if let Some(win) = app.get_webview_window("main") {
                                let _ = win.show();
                                let _ = win.set_focus();
                                let _ = win.eval("window.location.hash = '#/settings'");
                            }
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        "record" => {
                            let _ = voice_tx_for_menu.send(VoiceCommand::RecordStart);
                        }
                        "tts_selection" => {
                            let _ = voice_tx_for_menu.send(VoiceCommand::TtsTrigger);
                        }
                        name if name.starts_with("asr_") => {
                            let engine = name.strip_prefix("asr_").unwrap_or("mimo").to_string();
                            let _ = voice_tx_for_menu.send(VoiceCommand::SetAsrEngine(engine));
                        }
                        "inject_keyboard" => {
                            let _ = voice_tx_for_menu.send(VoiceCommand::SetInjectMode("keyboard".into()));
                        }
                        "inject_clipboard" => {
                            let _ = voice_tx_for_menu.send(VoiceCommand::SetInjectMode("clipboard".into()));
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(win) = app.get_webview_window("main") {
                            if win.is_visible().unwrap_or(false) {
                                let _ = win.hide();
                            } else {
                                let _ = win.show();
                                let _ = win.set_focus();
                            }
                        }
                    }
                })
                .build(app)
                .unwrap();

            // ── Start global hotkey listener ──────────────────────
            let (hotkey_tx, hotkey_rx) =
                crossbeam::channel::unbounded::<hotkey::HotkeyEvent>();

            let record_binding = HotkeyBinding::parse(&cfg.global_voice.record_hotkey)
                .unwrap_or_else(|| HotkeyBinding::parse("Alt+`").unwrap());
            let tts_binding = HotkeyBinding::parse(&cfg.global_voice.tts_hotkey)
                .unwrap_or_else(|| HotkeyBinding::parse("Alt+T").unwrap());
            let inject_switch = HotkeyBinding::parse(&cfg.global_voice.inject_mode_switch_hotkey)
                .unwrap_or_else(|| HotkeyBinding::parse("Alt+Shift+V").unwrap());
            let engine_switch = HotkeyBinding::parse(&cfg.global_voice.engine_switch_hotkey)
                .unwrap_or_else(|| HotkeyBinding::parse("Alt+Shift+E").unwrap());

            let stop_flag = Arc::new(AtomicBool::new(false));

            start_hotkey_listener(
                record_binding,
                tts_binding,
                inject_switch,
                engine_switch,
                hotkey_tx.clone(),
                stop_flag,
            );

            // Bridge: hotkey events → voice commands
            let voice_tx2 = voice_tx.clone();
            std::thread::spawn(move || {
                while let Ok(evt) = hotkey_rx.recv() {
                    let cmd = match evt {
                        hotkey::HotkeyEvent::RecordStart => VoiceCommand::RecordStart,
                        hotkey::HotkeyEvent::RecordStop => VoiceCommand::RecordStop,
                        hotkey::HotkeyEvent::TtsTrigger => VoiceCommand::TtsTrigger,
                        hotkey::HotkeyEvent::InjectModeSwitch => VoiceCommand::ToggleInjectMode,
                        hotkey::HotkeyEvent::EngineSwitch => VoiceCommand::CycleAsrEngine,
                    };
                    if voice_tx2.send(cmd).is_err() {
                        break;
                    }
                }
            });

            // ── Spawn event handler (std::thread, not tokio, due to !Send AudioCapture) ──
            let app_handle = app.handle().clone();
            let the_capture = capture_handle.clone();
            let the_inject_mode = global_inject_mode.clone();
            let the_asr_engine = global_asr_engine.clone();
            let the_engines = Arc::new(asr_engines);
            let tts_cfg = cfg.global_voice.tts_engine.clone();

            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().expect("create voice handler rt");
                loop {
                    let cmd = match voice_rx.recv() {
                        Ok(c) => c,
                        Err(_) => break,
                    };

                    rt.block_on(handle_voice_command(
                        &app_handle,
                        cmd,
                        &the_capture,
                        &the_inject_mode,
                        &the_asr_engine,
                        &the_engines,
                        &tts_cfg,
                    ));
                }
            });

            log::info!(
                "Companion v{} started (tray + global hotkeys)",
                env!("CARGO_PKG_VERSION")
            );
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running companion application");
}

// ── Voice command channel ─────────────────────────────────────────────

enum VoiceCommand {
    RecordStart,
    RecordStop,
    TtsTrigger,
    SetAsrEngine(String),
    SetInjectMode(String),
    ToggleInjectMode,
    CycleAsrEngine,
}

// ── Event handler ─────────────────────────────────────────────────────

async fn handle_voice_command(
    app: &tauri::AppHandle,
    cmd: VoiceCommand,
    capture_handle: &Arc<CaptureHandle>,
    inject_mode: &Arc<std::sync::Mutex<InjectMode>>,
    asr_engine_name: &Arc<std::sync::Mutex<String>>,
    engines: &Arc<std::collections::HashMap<String, Box<dyn AsrProvider + Send + Sync>>>,
    tts_engine: &str,
) {
    match cmd {
        VoiceCommand::RecordStart => {
            log::info!("[GlobalVoice] Push-to-talk: starting recording");
            app.state::<AppState>().is_listening.store(true, std::sync::atomic::Ordering::Release);
            log::info!("[GlobalVoice] is_listening set to TRUE");
            if !capture_handle.start() {
                log::error!("[GlobalVoice] Failed to start capture");
                app.state::<AppState>().is_listening.store(false, std::sync::atomic::Ordering::Release);
            }
        }

        VoiceCommand::RecordStop => {
            log::info!("[GlobalVoice] Push-to-talk: stopping recording");
            app.state::<AppState>().is_listening.store(false, std::sync::atomic::Ordering::Release);

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

                // Run ASR directly (Companion AsrProvider takes &[f32])
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

                // Inject text at cursor
                let mode = *inject_mode.lock().unwrap();
                log::info!("[GlobalVoice] Injecting via {:?}", mode);
                if let Err(e) = inject_text(&text, mode) {
                    log::error!("[GlobalVoice] Injection failed: {}", e);
                }
        }

        VoiceCommand::TtsTrigger => {
            log::info!("[GlobalVoice] TTS trigger: reading selected text...");

            // Explicitly release Alt key so enigo's Ctrl+C simulation
            // doesn't become Alt+Ctrl+C due to the still-held Alt.
            if let Ok(mut enigo) = Enigo::new(&Settings::default()) {
                let _ = enigo.key(Key::Alt, Direction::Release);
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

            // Build TTS provider and synthesize
            log::info!(
                "[GlobalVoice] TTS: {} chars with engine '{}'",
                text.len(),
                tts_engine
            );

            // Get API token from config (preferred) or env var
            let api_token = if let Some(mgr) = ConfigManager::new().ok() {
                if let Ok(cfg) = mgr.load() {
                    cfg.api_token.clone()
                        .filter(|t| !t.is_empty())
                        .or_else(|| std::env::var("COMPANION_API_TOKEN").ok())
                        .unwrap_or_default()
                } else {
                    std::env::var("COMPANION_API_TOKEN").unwrap_or_default()
                }
            } else {
                std::env::var("COMPANION_API_TOKEN").unwrap_or_default()
            };

            if api_token.is_empty() {
                log::error!("[GlobalVoice] COMPANION_API_TOKEN not set (neither in config nor env), cannot use TTS");
                return;
            }

            let tts = XiaomiTts::new(&api_token, "茉莉");
            match tts.synthesize(&text).await {
                Ok(pcm_f32) => {
                    log::info!("[GlobalVoice] TTS synthesized {} f32 samples", pcm_f32.len());

                    // Emit speaking state for Live2D
                    app.state::<AppState>().is_speaking.store(true, std::sync::atomic::Ordering::Release);

                    // Start RMS-based lip sync animation alongside playback
                    animate_lip_sync(&pcm_f32, 24000, app);

                    let i16 = f32_to_i16(&pcm_f32);
                    match audio::utils::pcm_i16_to_wav(&i16, 24000) {
                        Ok(wav) => {
                            log::info!("[GlobalVoice] WAV encoded {} bytes, starting playback", wav.len());
                            playback::play_wav_async(wav);
                        }
                        Err(e) => {
                            log::error!("[GlobalVoice] WAV encoding failed: {}", e);
                        }
                    }
                }
                Err(e) => {
                    log::error!("[GlobalVoice] TTS synthesis failed: {}", e);
                }
            }
        }

        VoiceCommand::SetAsrEngine(name) => {
            log::info!("[GlobalVoice] Switching ASR engine to: {}", name);
            let mut guard = asr_engine_name.lock().unwrap();
            *guard = name;
            // Persist to config
            if let Some(mgr) = ConfigManager::new().ok() {
                if let Ok(mut cfg) = mgr.load() {
                    cfg.global_voice.asr_engine = guard.clone();
                    let _ = mgr.save(&cfg);
                }
            }
        }

        VoiceCommand::SetInjectMode(mode_str) => {
            log::info!("[GlobalVoice] Switching inject mode to: {}", mode_str);
            let mut guard = inject_mode.lock().unwrap();
            *guard = match mode_str.as_str() {
                "clipboard" => InjectMode::Clipboard,
                _ => InjectMode::Keyboard,
            };
            if let Some(mgr) = ConfigManager::new().ok() {
                if let Ok(mut cfg) = mgr.load() {
                    cfg.global_voice.inject_mode = mode_str;
                    let _ = mgr.save(&cfg);
                }
            }
        }

        VoiceCommand::ToggleInjectMode => {
            let mut guard = inject_mode.lock().unwrap();
            *guard = guard.toggle();
            let mode_str = guard.as_str().to_string();
            log::info!("[GlobalVoice] Toggled inject mode to: {}", mode_str);
            if let Some(mgr) = ConfigManager::new().ok() {
                if let Ok(mut cfg) = mgr.load() {
                    cfg.global_voice.inject_mode = mode_str;
                    let _ = mgr.save(&cfg);
                }
            }
        }

        VoiceCommand::CycleAsrEngine => {
            let mut guard = asr_engine_name.lock().unwrap();
            let names: Vec<&String> = engines.keys().collect();
            if names.is_empty() { return; }
            let pos = names.iter().position(|n| *n == &*guard);
            let next_idx = match pos {
                Some(i) => (i + 1) % names.len(),
                None => 0,
            };
            let next = names[next_idx].clone();
            log::info!("[GlobalVoice] Cycling ASR engine to: {}", next);
            *guard = next.clone();
            if let Some(mgr) = ConfigManager::new().ok() {
                if let Ok(mut cfg) = mgr.load() {
                    cfg.global_voice.asr_engine = next;
                    let _ = mgr.save(&cfg);
                }
            }
        }
    }
}

// ── Live2D lip sync: extract RMS envelope and drive lip_level ────────


/// Compute RMS envelope from PCM f32 audio and animate `lip_level` in sync with playback.
/// Spawns a background thread that updates lip_level every 50ms,
/// then resets to 0 and emits "idle" via the app handle.
fn animate_lip_sync(
    pcm: &[f32],
    sample_rate: u32,
    app_handle: &tauri::AppHandle,
) {
    let frame_ms = 50u64;
    let frame_samples = (sample_rate as u64 * frame_ms / 1000) as usize;
    if frame_samples == 0 { return; }
    let total_frames = pcm.len() / frame_samples;
    if total_frames == 0 { return; }

    // Pre-compute RMS for each frame (normalized to 0..1)
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
        let lip_level = &app_handle.state::<state::AppState>().lip_level;
        for &level in &rms_values {
            *lip_level.lock().unwrap() = level;
            std::thread::sleep(std::time::Duration::from_millis(frame_ms));
        }
        // Reset and signal idle
        *lip_level.lock().unwrap() = 0.0;
        app_handle.state::<state::AppState>().is_speaking.store(false, std::sync::atomic::Ordering::Release);
    });
}

// ── Build ASR engines map ─────────────────────────────────────────────

/// Build a map of ASR engine name → Box<dyn AsrProvider>.
fn build_global_asr_engines(
    cfg: &CompanionConfig,
) -> std::collections::HashMap<String, Box<dyn AsrProvider + Send + Sync>> {
    let mut engines: std::collections::HashMap<String, Box<dyn AsrProvider + Send + Sync>> =
        std::collections::HashMap::new();

    let api_token = cfg
        .api_token
        .clone()
        .or_else(|| std::env::var("COMPANION_API_TOKEN").ok())
        .unwrap_or_default();

    // Mimo ASR (Xiaomi)
    if !api_token.is_empty() {
        let base_url = cfg
            .asr_custom_url
            .clone()
            .unwrap_or_else(|| "https://token-plan-cn.xiaomimimo.com/v1/chat/completions".into());
        log::info!("[GlobalVoice] Registering Mimo ASR engine");
        engines.insert(
            "mimo".into(),
            Box::new(XiaomiAsr::with_url(&api_token, &base_url)),
        );
    }

    // OpenAI Whisper (uses asr_custom settings as well)
    if !api_token.is_empty() {
        log::info!("[GlobalVoice] Registering OpenAI Whisper engine");
        engines.insert(
            "openai".into(),
            Box::new(WhisperCloud::new(&api_token, "whisper-1")),
        );
    }

    // Aliyun ASR (separate config from asr_custom)
    if let Some(key) = &cfg.asr_custom_key {
        if let Some(model) = &cfg.asr_custom_model {
            // Use asr_custom fields as: key = "appkey:token", model = region
            if let Some((appkey, token)) = key.split_once(':') {
                log::info!("[GlobalVoice] Registering Aliyun ASR engine");
                let mut aliyun = AliyunAsr::new(appkey, token);
                if let Some(_region) = model.split_once(':').map(|p| p.1) {
                    aliyun = AliyunAsr::new(appkey, token);
                    // region override via model field: "region:cn-beijing"
                    // Re-create with region...
                }
                engines.insert("aliyun".into(), Box::new(aliyun));
            }
        }
    }

    // Whisper Local (if binary and model are configured)
    if let Some(path_str) = &cfg.asr_custom_url {
        let binary_path = std::path::PathBuf::from(path_str);
        let model_path = cfg
            .asr_custom_model
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
