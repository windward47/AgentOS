//! Avatar/Live2D IPC commands.

use tauri::{Manager, Emitter};
use companion_core::downloader::{download_model, DownloadProgress};

#[tauri::command]
pub async fn set_lip_level(voice: tauri::State<'_, crate::state::VoiceState>, level: f32) -> Result<(), String> {
    *voice.lip_level.lock().unwrap() = level.clamp(0.0, 1.0);
    Ok(())
}

#[tauri::command]
pub async fn get_lip_level(voice: tauri::State<'_, crate::state::VoiceState>) -> Result<f32, String> {
    Ok(*voice.lip_level.lock().unwrap())
}

#[tauri::command]
pub async fn get_voice_state(voice: tauri::State<'_, crate::state::VoiceState>) -> Result<String, String> {
    use std::sync::atomic::Ordering;
    let s = if voice.is_speaking.load(Ordering::Relaxed) { "speaking" }
    else if voice.is_listening.load(Ordering::Relaxed) { "listening" }
    else { "idle" };
    Ok(s.into())
}

#[tauri::command]
pub async fn list_live2d_models() -> Result<Vec<String>, String> {
    let mut models = Vec::new();
    let home = dirs::home_dir().unwrap_or_default();
    let user_dir = home.join(".companion").join("models");
    if user_dir.exists() { scan_models(&user_dir, &user_dir, &mut models); }
    let base = std::env::current_dir().unwrap_or_default();
    let root = if base.ends_with("companion-tauri") { base.parent().unwrap_or(&base).to_path_buf() } else { base };
    let web_dir = root.join("web").join("public").join("live2d").join("models");
    if web_dir.exists() { scan_models(&web_dir, &web_dir, &mut models); }
    if models.is_empty() { models.push("haru/haru.model3.json".into()); }
    models.retain(|p| { let l = p.to_lowercase(); !l.contains("epsilon") && !(l.starts_with("ren_") || l.contains("/ren.")) && !l.contains("miku_pro") });
    models.dedup();
    let mut seen = std::collections::HashSet::new();
    models.retain(|p| { let key = p.split('/').last().unwrap_or(p).to_string(); seen.insert(key) });
    models.sort();
    Ok(models)
}

fn scan_models(dir: &std::path::Path, base: &std::path::Path, out: &mut Vec<String>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name.starts_with('.') { continue; }
            if path.is_dir() { scan_models(&path, base, out); }
            else if name.ends_with(".model3.json") {
                if let Ok(rel) = path.strip_prefix(base) {
                    let r = rel.to_string_lossy().replace('\\', "/");
                    if !out.contains(&r) { out.push(r); }
                }
            }
        }
    }
}

#[tauri::command]
pub async fn set_live2d_model(app: tauri::AppHandle, model_path: String) -> Result<(), String> {
    app.emit("switch_live2d_model", model_path).map_err(|e| format!("emit: {e}"))
}

#[tauri::command]
pub async fn cmd_download_model(app: tauri::AppHandle, url: String, model_id: String) -> Result<(), String> {
    let home = dirs::home_dir().unwrap_or_default();
    let dest = home.join(".companion").join("models");
    std::fs::create_dir_all(&dest).map_err(|e| format!("mkdir: {e}"))?;
    let (tx, mut rx) = tokio::sync::mpsc::channel::<DownloadProgress>(32);
    let app_clone = app.clone();
    tokio::spawn(async move {
        if let Err(e) = download_model(&url, &model_id, &dest, tx).await {
            let _ = app_clone.emit("download_progress", DownloadProgress::Error { message: e });
        }
    });
    tokio::spawn(async move { while let Some(evt) = rx.recv().await { let _ = app.emit("download_progress", &evt); } });
    Ok(())
}

#[tauri::command]
pub async fn set_avatar_visible(app: tauri::AppHandle, visible: bool) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("avatar") {
        if visible { win.show().map_err(|e| format!("show: {e}"))?; }
        else { win.hide().map_err(|e| format!("hide: {e}"))?; }
    }
    Ok(())
}

#[tauri::command]
pub async fn get_avatar_visible(app: tauri::AppHandle) -> Result<bool, String> {
    match app.get_webview_window("avatar") {
        Some(win) => win.is_visible().map_err(|e| format!("visible: {e}")),
        None => Ok(false),
    }
}

#[tauri::command]
pub async fn set_avatar_always_on_top(app: tauri::AppHandle, on_top: bool) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("avatar") {
        win.set_always_on_top(on_top).map_err(|e| format!("on_top: {e}"))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn reset_avatar_position(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("main") {
        let main_size = win.outer_size().map_err(|e| format!("size: {e}"))?;
        if let Some(avatar) = app.get_webview_window("avatar") {
            avatar.set_position(tauri::PhysicalPosition::new(main_size.width as i32 + 100, 100)).map_err(|e| format!("pos: {e}"))?;
        }
    }
    app.emit("reset_model_position", ()).map_err(|e| format!("emit: {e}"))?;
    Ok(())
}
