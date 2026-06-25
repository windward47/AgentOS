//! Window setup helpers.

use tauri::{App, Manager, PhysicalPosition};

pub fn setup_windows(app: &App) {
    let main_win = app.get_webview_window("main").expect("main window missing");
    main_win.set_title("Companion v0.1.0").ok();

    // Close → hide to tray
    let win = main_win.clone();
    main_win.clone().on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = win.hide();
        }
    });

    // Avatar window
    if let Some(avatar) = app.get_webview_window("avatar") {
        let _ = avatar.set_position(PhysicalPosition::new(
            main_win.outer_size().unwrap().width as i32 + 100, 100,
        ));
        #[cfg(debug_assertions)]
        avatar.open_devtools();
    }
}
