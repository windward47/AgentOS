//! Keyboard simulation injection via `enigo`.

use enigo::{Enigo, Keyboard, Settings};
use std::thread;
use std::time::Duration;

use crate::inject::InjectError;

/// Inject text by simulating keystrokes at current cursor position.
pub fn inject_keyboard(text: &str) -> Result<(), InjectError> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| {
        InjectError::Keyboard(format!("Failed to create Enigo: {:?}", e))
    })?;

    thread::sleep(Duration::from_millis(50));

    // Longer text: use clipboard fallback to avoid slowness
    if text.len() > 100 {
        return super::clipboard::inject_clipboard(text);
    }

    enigo.text(text).map_err(|e| {
        InjectError::Keyboard(format!("enigo.text() failed: {:?}", e))
    })?;

    Ok(())
}
