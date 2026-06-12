//! Read text from current selection or clipboard.
//!
//! Uses universal approach: save clipboard → simulate Ctrl+C → read → restore.

use arboard::Clipboard;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::thread;
use std::time::Duration;

use crate::inject::InjectError;

/// Read selected text by simulating Ctrl+C (Cmd+C on macOS) and reading clipboard.
/// Retries a few times for timing issues.
pub fn read_selected_text() -> Result<String, InjectError> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| {
        InjectError::Keyboard(format!("Failed to create Enigo: {:?}", e))
    })?;

    // Save current clipboard
    let mut cb = Clipboard::new().map_err(|e| {
        InjectError::Clipboard(format!("Failed to open clipboard: {}", e))
    })?;
    let saved = cb.get_text().ok();

    // Clear clipboard so we can detect if copy succeeded
    cb.clear().ok();
    drop(cb);

    thread::sleep(Duration::from_millis(80));

    // Simulate Ctrl+C (Cmd+C on macOS)
    #[cfg(target_os = "macos")]
    let modifier = Key::Meta;
    #[cfg(not(target_os = "macos"))]
    let modifier = Key::Control;

    let _ = enigo.key(modifier, Direction::Press);
    let _ = enigo.key(Key::C, Direction::Click);
    let _ = enigo.key(modifier, Direction::Release);

    // Retry up to 10 times to get clipboard content
    let mut selected = String::new();
    for i in 0..10 {
        thread::sleep(Duration::from_millis(100 + i * 30));
        if let Ok(mut cb) = Clipboard::new() {
            if let Ok(text) = cb.get_text() {
                if !text.is_empty() {
                    selected = text;
                    break;
                }
            }
        }
    }

    // Restore saved clipboard
    if let Some(ref saved_text) = saved {
        thread::sleep(Duration::from_millis(50));
        if let Ok(mut cb) = Clipboard::new() {
            let _ = cb.set_text(saved_text);
        }
    }

    Ok(selected)
}

/// Read clipboard text directly (no simulation).
pub fn read_clipboard_text() -> Result<String, InjectError> {
    let mut cb = Clipboard::new().map_err(|e| {
        InjectError::Clipboard(format!("Failed to open clipboard: {}", e))
    })?;
    Ok(cb.get_text().ok().unwrap_or_default())
}
