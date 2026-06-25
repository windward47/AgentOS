//! System tray menu builder + event handler.

use tauri::{
    image::Image,
    menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, Manager,
};

use crate::voice_handler::{VoiceCommand, build_global_asr_engines};
use companion_core::config::CompanionConfig;

pub fn build_tray(app: &App, cfg: &CompanionConfig, voice_tx: crossbeam::channel::Sender<VoiceCommand>) {
    let asr_engines = build_global_asr_engines(cfg);

    let icon_img = image::load_from_memory(include_bytes!("../icons/icon.png"))
        .expect("decode tray icon").into_rgba8();
    let (w, h) = icon_img.dimensions();
    let tray_icon = Image::new_owned(icon_img.into_raw(), w, h);

    let asr_names: Vec<&str> = asr_engines.keys().map(|s| s.as_str()).collect();
    let asr_submenu = {
        let mut b = SubmenuBuilder::new(app, "ASR Engine");
        for name in &asr_names { b = b.item(&MenuItemBuilder::with_id(format!("asr_{}", name), name).build(app).unwrap()); }
        b.build().unwrap()
    };
    let inject_submenu = SubmenuBuilder::new(app, "Inject Mode")
        .item(&MenuItemBuilder::with_id("inject_keyboard", "Keyboard").build(app).unwrap())
        .item(&MenuItemBuilder::with_id("inject_clipboard", "Clipboard").build(app).unwrap())
        .build().unwrap();

    let sep = || PredefinedMenuItem::separator(app).unwrap();
    let menu = MenuBuilder::new(app)
        .item(&asr_submenu).item(&inject_submenu).item(&sep())
        .item(&MenuItemBuilder::with_id("record", "🎤 Record (Alt+`)").build(app).unwrap())
        .item(&MenuItemBuilder::with_id("tts_selection", "🔊 TTS (Alt+T)").build(app).unwrap())
        .item(&sep())
        .item(&MenuItemBuilder::with_id("show_hide", "Show/Hide").build(app).unwrap())
        .item(&sep())
        .item(&MenuItemBuilder::with_id("settings", "Settings").build(app).unwrap())
        .item(&sep())
        .item(&MenuItemBuilder::with_id("quit", "Quit").accelerator("CmdOrCtrl+Q").build(app).unwrap())
        .build().unwrap();

    let _tray = TrayIconBuilder::new()
        .icon(tray_icon).tooltip("Companion").menu(&menu)
        .on_menu_event(move |app, event| {
            let id = event.id.as_ref();
            match id {
                "show_hide" => toggle_main_window(app),
                "settings" => show_settings(app),
                "quit" => app.exit(0),
                "record" => { let _ = voice_tx.send(VoiceCommand::RecordStart); }
                "tts_selection" => { let _ = voice_tx.send(VoiceCommand::TtsTrigger); }
                n if n.starts_with("asr_") => { let _ = voice_tx.send(VoiceCommand::SetAsrEngine(n.strip_prefix("asr_").unwrap_or("mimo").into())); }
                "inject_keyboard" => { let _ = voice_tx.send(VoiceCommand::SetInjectMode("keyboard".into())); }
                "inject_clipboard" => { let _ = voice_tx.send(VoiceCommand::SetInjectMode("clipboard".into())); }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                toggle_main_window(tray.app_handle());
            }
        })
        .build(app).unwrap();

}

fn toggle_main_window(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        if win.is_visible().unwrap_or(false) { let _ = win.hide(); }
        else { let _ = win.show(); let _ = win.set_focus(); }
    }
}

fn show_settings(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show(); let _ = win.set_focus();
        let _ = win.eval("window.location.hash = '#/settings'");
    }
}
