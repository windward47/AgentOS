//! Clipboard-based text injection.
//!
//! Flow: save current clipboard → write text → simulate Ctrl+V → restore.

use arboard::Clipboard;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::thread;
use std::time::Duration;

use crate::inject::InjectError;

/// Inject text via clipboard + Ctrl+V paste.
pub fn inject_clipboard(text: &str) -> Result<(), InjectError> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| {
        InjectError::Keyboard(format!("Failed to create Enigo: {:?}", e))
    })?;

    // Save current clipboard
    let saved = {
        let mut cb = Clipboard::new().map_err(|e| {
            InjectError::Clipboard(format!("Failed to open clipboard: {}", e))
        })?;
        let saved = cb.get_text().ok();
        cb.set_text(text)
            .map_err(|e| InjectError::Clipboard(format!("Failed to set clipboard: {}", e)))?;
        saved
    };

    thread::sleep(Duration::from_millis(30));

    // Simulate Ctrl+V (Cmd+V on macOS)
    #[cfg(target_os = "macos")]
    let modifier = Key::Meta;
    #[cfg(not(target_os = "macos"))]
    let modifier = Key::Control;

    let _ = enigo.key(modifier, Direction::Press);
    let _ = enigo.key(Key::V, Direction::Click);
    let _ = enigo.key(modifier, Direction::Release);

    thread::sleep(Duration::from_millis(50));

    // Restore saved clipboard
    if let Some(saved_text) = saved {
        thread::sleep(Duration::from_millis(50));
        if let Ok(mut cb) = Clipboard::new() {
            let _ = cb.set_text(&saved_text);
        }
    }

    Ok(())
}
