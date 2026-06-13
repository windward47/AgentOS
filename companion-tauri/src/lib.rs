#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// ---------------------------------------------------------------------------
// Companion Tauri — desktop application shell
// ---------------------------------------------------------------------------

pub mod state;
pub mod voice_handler;

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use companion_core::config::ConfigManager;
use companion_core::hotkey::{start_hotkey_listener, HotkeyBinding, HotkeyEvent};
use companion_core::inject::InjectMode;

use tauri::{
    image::Image,
    menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

use state::{
    AgentState, VoiceState, ConfigState,
};
use voice_handler::{
    VoiceCommand, handle_voice_command, build_global_asr_engines,
};

// ── Entry point ─────────────────────────────────────────────────────────

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
        // ── Register domain states ──
        .manage(AgentState::new())
        .manage(VoiceState::new())
        // ConfigState — constructed with loaded config first
        .manage({
            let config_manager =
                ConfigManager::new().expect("failed to init config manager");
            let config = config_manager.load().expect("failed to load config");
            let system_mode = config.system_mode;
            ConfigState {
                config: Arc::new(tokio::sync::Mutex::new(config)),
                config_manager,
                system_mode: AtomicBool::new(system_mode),
            }
        })
        .invoke_handler(tauri::generate_handler![
            state::chat,
            state::agent_action,
            state::get_history,
            state::clear_history,
            state::transcribe_audio,
            state::synthesize_audio,
            state::get_config,
            state::update_config,
            state::set_lip_level,
            state::get_lip_level,
            state::get_voice_state,
            state::get_cursor_pos,
            state::set_avatar_visible,
            state::list_live2d_models,
            state::set_live2d_model,
            state::cmd_download_model,
            state::get_avatar_visible,
            state::set_avatar_always_on_top,
            state::reset_avatar_position,
            state::browse_screenshot,
            state::get_audit_log,
            state::list_models,
        ])
        .setup(|app: &mut tauri::App| {
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
                #[cfg(debug_assertions)]
                avatar.open_devtools();
            }

            // ---- Global voice state ----
            let cfg_snapshot = {
                let cfg = app.state::<ConfigState>();
                let guard = cfg.config.blocking_lock();
                guard.clone()
            };

            let capture_handle = Arc::new(companion_core::capture_mgr::spawn_capture_manager());
            let global_inject_mode: std::sync::Mutex<InjectMode> =
                std::sync::Mutex::new(InjectMode::from_config(&cfg_snapshot));
            let global_inject_mode = Arc::new(global_inject_mode);
            let global_asr_engine: std::sync::Mutex<String> =
                std::sync::Mutex::new(cfg_snapshot.global_voice.asr_engine.clone());
            let global_asr_engine = Arc::new(global_asr_engine);
            let asr_engines = build_global_asr_engines(&cfg_snapshot);

            // ---- System tray icon & menu ----
            let icon_img = image::load_from_memory(include_bytes!("../icons/icon.png"))
                .expect("failed to decode tray icon")
                .into_rgba8();
            let (w, h) = icon_img.dimensions();
            let tray_icon = Image::new_owned(icon_img.into_raw(), w, h);

            let asr_engines_names: Vec<&str> = asr_engines.keys().map(|s| s.as_str()).collect();
            let asr_submenu = {
                let mut builder = SubmenuBuilder::new(app, "ASR Engine");
                for name in &asr_engines_names {
                    builder =
                        builder.item(&MenuItemBuilder::with_id(format!("asr_{}", name), name).build(app).unwrap());
                }
                builder.build().unwrap()
            };

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

            // ── Global voice channel ──
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
                        "quit" => app.exit(0),
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

            // ── Start global hotkey listener ──
            let (hotkey_tx, hotkey_rx) =
                crossbeam::channel::unbounded::<HotkeyEvent>();

            let record_binding = HotkeyBinding::parse(&cfg_snapshot.global_voice.record_hotkey)
                .unwrap_or_else(|| HotkeyBinding::parse("Alt+`").unwrap());
            let tts_binding = HotkeyBinding::parse(&cfg_snapshot.global_voice.tts_hotkey)
                .unwrap_or_else(|| HotkeyBinding::parse("Alt+T").unwrap());
            let inject_switch = HotkeyBinding::parse(&cfg_snapshot.global_voice.inject_mode_switch_hotkey)
                .unwrap_or_else(|| HotkeyBinding::parse("Alt+Shift+V").unwrap());
            let engine_switch = HotkeyBinding::parse(&cfg_snapshot.global_voice.engine_switch_hotkey)
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
                        HotkeyEvent::RecordStart => VoiceCommand::RecordStart,
                        HotkeyEvent::RecordStop => VoiceCommand::RecordStop,
                        HotkeyEvent::TtsTrigger => VoiceCommand::TtsTrigger,
                        HotkeyEvent::InjectModeSwitch => VoiceCommand::ToggleInjectMode,
                        HotkeyEvent::EngineSwitch => VoiceCommand::CycleAsrEngine,
                    };
                    if voice_tx2.send(cmd).is_err() {
                        break;
                    }
                }
            });

            // ── Spawn event handler (std::thread for !Send AudioCapture) ──
            let app_handle = app.handle().clone();
            let the_capture = capture_handle.clone();
            let the_inject_mode = global_inject_mode.clone();
            let the_asr_engine = global_asr_engine.clone();
            let the_engines = Arc::new(asr_engines);
            let tts_cfg = cfg_snapshot.global_voice.tts_engine.clone();

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
