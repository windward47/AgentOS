#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub mod state;
pub mod voice_handler;
pub mod tray;
pub mod window_setup;

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use companion_core::config::ConfigManager;
use companion_core::hotkey::{start_hotkey_listener, HotkeyBinding, HotkeyEvent};
use companion_core::inject::InjectMode;

use tauri::Manager;
use state::{AgentState, VoiceState, ConfigState};
use voice_handler::{VoiceCommand, handle_voice_command};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::default().level(log::LevelFilter::Info).build())
        .plugin(tauri_plugin_shell::init())
        .manage(AgentState::new())
        .manage(VoiceState::new())
        .manage({
            let cm = ConfigManager::new().expect("config manager");
            let cfg = cm.load().expect("load config");
            ConfigState {
                config: Arc::new(tokio::sync::Mutex::new(cfg.clone())),
                config_manager: cm,
                system_mode: AtomicBool::new(cfg.system_mode),
            }
        })
        .invoke_handler(tauri::generate_handler![
            state::chat, state::chat_stream, state::get_history, state::clear_history,
            state::transcribe_audio, state::synthesize_audio,
            state::get_config, state::update_config,
            state::set_lip_level, state::get_lip_level, state::get_voice_state,
            state::get_cursor_pos, state::open_folder,
            state::set_avatar_visible, state::list_live2d_models, state::set_live2d_model,
            state::cmd_download_model, state::get_avatar_visible, state::set_avatar_always_on_top,
            state::reset_avatar_position, state::browse_screenshot, state::get_audit_log,
            state::list_models,
            state::list_conversations, state::get_current_conversation,
            state::create_conversation, state::switch_conversation,
            state::delete_conversation, state::rename_conversation,
            state::list_memories, state::forget_memory,
        ])
        .setup(|app| {
            window_setup::setup_windows(app);

            let cfg_snapshot = app.handle().state::<ConfigState>().inner().config.blocking_lock().clone();

            let capture_handle = Arc::new(companion_core::capture_mgr::spawn_capture_manager());
            let inject_mode = Arc::new(std::sync::Mutex::new(InjectMode::from_config(&cfg_snapshot)));
            let asr_engine = Arc::new(std::sync::Mutex::new(cfg_snapshot.global_voice.asr_engine.clone()));
            let asr_engines = voice_handler::build_global_asr_engines(&cfg_snapshot);

            // Tray + voice channel
            let (voice_tx, voice_rx) = {
                let (tx, rx) = crossbeam::channel::unbounded::<VoiceCommand>();
                let tx2 = tx.clone();
                tray::build_tray(app, &cfg_snapshot, tx2);
                (tx, rx)
            };

            // Hotkeys
            let (hotkey_tx, hotkey_rx) = crossbeam::channel::unbounded::<HotkeyEvent>();
            let parse = |s: &str, fallback: &str| HotkeyBinding::parse(s).unwrap_or_else(|| HotkeyBinding::parse(fallback).unwrap());
            let stop_flag = Arc::new(AtomicBool::new(false));
            start_hotkey_listener(
                parse(&cfg_snapshot.global_voice.record_hotkey, "Alt+`"),
                parse(&cfg_snapshot.global_voice.tts_hotkey, "Alt+T"),
                parse(&cfg_snapshot.global_voice.inject_mode_switch_hotkey, "Alt+Shift+V"),
                parse(&cfg_snapshot.global_voice.engine_switch_hotkey, "Alt+Shift+E"),
                hotkey_tx, stop_flag,
            );

            // Bridge hotkey → voice commands
            let voice_tx2 = voice_tx.clone();
            std::thread::spawn(move || {
                while let Ok(evt) = hotkey_rx.recv() {
                    let cmd = match evt {
                        HotkeyEvent::RecordStart => VoiceCommand::RecordStart,
                        HotkeyEvent::RecordStop => VoiceCommand::RecordStop,
                        HotkeyEvent::TtsTrigger => VoiceCommand::TtsTrigger,
                        HotkeyEvent::InjectModeSwitch => VoiceCommand::ToggleInjectMode,
                        HotkeyEvent::EngineSwitch => VoiceCommand::CycleAsrEngine,
                    };
                    if voice_tx2.send(cmd).is_err() { break; }
                }
            });

            // Voice handler thread
            let app_handle = app.handle().clone();
            let (capture, inject, asr, tts) = (capture_handle, inject_mode, asr_engine, cfg_snapshot.global_voice.tts_engine.clone());
            let engines = Arc::new(asr_engines);
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().expect("voice rt");
                loop {
                    let cmd = match voice_rx.recv() { Ok(c) => c, Err(_) => break };
                    rt.block_on(handle_voice_command(&app_handle, cmd, &capture, &inject, &asr, &engines, &tts));
                }
            });

            log::info!("Companion v{} started", env!("CARGO_PKG_VERSION"));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error running companion");
}
