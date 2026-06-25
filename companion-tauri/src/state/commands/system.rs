//! System IPC commands (cursor, folders, screenshots, audit, models).

#[tauri::command]
pub async fn open_folder(path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    { std::process::Command::new("explorer").arg(&path).spawn().map_err(|e| format!("{e}"))?; }
    #[cfg(target_os = "macos")]
    { std::process::Command::new("open").arg(&path).spawn().map_err(|e| format!("{e}"))?; }
    #[cfg(target_os = "linux")]
    { std::process::Command::new("xdg-open").arg(&path).spawn().map_err(|e| format!("{e}"))?; }
    Ok(())
}

#[tauri::command]
pub async fn get_cursor_pos() -> Result<(i32, i32), String> {
    use enigo::{Enigo, Mouse, Settings};
    let enigo = Enigo::new(&Settings::default()).map_err(|e| format!("enigo: {:?}", e))?;
    enigo.location().map_err(|e| format!("location: {:?}", e))
}

#[tauri::command]
pub async fn browse_screenshot(url: String) -> Result<String, String> {
    let lower = url.to_lowercase();
    if !lower.starts_with("http://") && !lower.starts_with("https://") {
        return Err("Only http:// and https:// URLs are allowed".into());
    }
    let cwd = std::env::current_dir().unwrap_or_default();
    let script = if cwd.ends_with("companion-tauri") { cwd.parent().unwrap_or(&cwd).join("scripts") } else { cwd.join("scripts") }.join("browser-screenshot.mjs");
    let tmp = std::env::temp_dir().join(format!("companion_browse_{}_{}.png", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0)));
    let output = std::process::Command::new("node").arg(&script).arg(&url).arg(&tmp).arg("15000").output().map_err(|e| format!("Browser script: {e}"))?;
    if !output.status.success() { return Err(format!("Browser failed: {}", String::from_utf8_lossy(&output.stderr))); }
    let bytes = std::fs::read(&tmp).map_err(|e| format!("Read: {e}"))?;
    std::fs::remove_file(&tmp).ok();
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    Ok(format!("data:image/png;base64,{}", STANDARD.encode(&bytes)))
}

#[tauri::command]
pub async fn get_audit_log() -> Result<String, String> {
    let path = dirs::home_dir().unwrap_or_default().join(".companion").join("logs").join("command.log");
    if !path.exists() { return Ok("(no logs yet)".into()); }
    std::fs::read_to_string(&path).map_err(|e| format!("read log: {e}"))
}

#[tauri::command]
pub async fn list_models(base_url: String, api_key: String) -> Result<Vec<String>, String> {
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let client = reqwest::Client::new();
    let resp = client.get(&url).header("Authorization", format!("Bearer {api_key}")).header("Content-Type", "application/json").timeout(std::time::Duration::from_secs(10)).send().await.map_err(|e| format!("Request: {e}"))?;
    if !resp.status().is_success() { return Err(format!("HTTP {}: {}", resp.status().as_u16(), resp.text().await.unwrap_or_default())); }
    let body: serde_json::Value = resp.json().await.map_err(|e| format!("Parse: {e}"))?;
    Ok(body["data"].as_array().unwrap_or(&vec![]).iter().filter_map(|v| v["id"].as_str().map(String::from)).collect())
}
