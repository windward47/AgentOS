#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// ---------------------------------------------------------------------------
// Companion Core — modular desktop agent backend
// ---------------------------------------------------------------------------

pub mod agent;
pub mod audio;
pub mod asr;
pub mod config;
pub mod emotion;
pub mod llm;
pub mod mcp;
pub mod permissions;
pub mod sandbox;
pub mod state;
pub mod tools;
pub mod tts;
pub mod websocket;

use state::AppState;
use tauri::Manager;

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
            state::get_history,
            state::clear_history,
            state::transcribe_audio,
            state::synthesize_audio,
            state::get_config,
            state::update_config,
            state::set_lip_level,
            state::get_lip_level,
            state::browse_screenshot,
            state::get_audit_log,
            state::list_models,
        ])
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            window.set_title("Companion v0.1.0").ok();
            log::info!("Companion v{} started", env!("CARGO_PKG_VERSION"));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running companion application");
}
