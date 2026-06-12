//! Global hotkey listener using `rdev`.
//!
//! Listens for configured hotkey combinations on a background thread
//! and sends events via a tokio channel for processing.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use rdev::{listen, EventType, Key};
use crossbeam::channel::Sender;

/// Events emitted by the hotkey listener.
#[derive(Debug, Clone)]
pub enum HotkeyEvent {
    /// Recording started (key combo pressed, push-to-talk)
    RecordStart,
    /// Recording stopped (key combo released)
    RecordStop,
    /// Trigger TTS on selected text (Alt+T by default)
    TtsTrigger,
    /// Switch inject mode (Alt+Shift+V by default)
    InjectModeSwitch,
    /// Switch ASR engine (Alt+Shift+E by default)
    EngineSwitch,
}

/// A parsed hotkey binding: modifier keys + a main key.
#[derive(Debug, Clone)]
pub struct HotkeyBinding {
    pub modifiers: HashSet<Key>,
    pub key: Key,
}

impl HotkeyBinding {
    /// Parse "Alt+`", "Ctrl+Shift+A", etc.
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('+').map(|p| p.trim()).collect();
        if parts.is_empty() {
            return None;
        }

        let mut modifiers = HashSet::new();
        let mut main_key = None;

        for part in &parts {
            match part.to_lowercase().as_str() {
                "alt" => {
                    modifiers.insert(Key::Alt);
                }
                "ctrl" | "control" => {
                    modifiers.insert(Key::ControlLeft);
                }
                "shift" => {
                    modifiers.insert(Key::ShiftLeft);
                }
                "meta" | "win" | "cmd" | "super" => {
                    modifiers.insert(Key::MetaLeft);
                }
                other => {
                    if let Some(key) = parse_key(other) {
                        main_key = Some(key);
                    } else {
                        log::warn!("Unrecognized hotkey part: {}", other);
                        return None;
                    }
                }
            }
        }

        main_key.map(|key| HotkeyBinding { modifiers, key })
    }

    /// Check if the current key state matches.
    pub fn matches(&self, pressed: &HashSet<Key>, event_key: &Key) -> bool {
        if event_key != &self.key {
            return false;
        }
        for required in &self.modifiers {
            if !pressed.contains(required) {
                let counterpart = match required {
                    Key::Alt => Key::AltGr,
                    Key::ControlLeft => Key::ControlRight,
                    Key::ShiftLeft => Key::ShiftRight,
                    Key::MetaLeft => Key::MetaRight,
                    _ => continue,
                };
                if !pressed.contains(&counterpart) {
                    return false;
                }
            }
        }
        true
    }
}

/// Start the global hotkey listener on a background thread.
///
/// Sends events to `sender` when configured hotkeys are pressed.
/// Set `stop_flag` to true to shut down the listener.
pub fn start_hotkey_listener(
    record_binding: HotkeyBinding,
    tts_binding: HotkeyBinding,
    inject_switch_binding: HotkeyBinding,
    engine_switch_binding: HotkeyBinding,
    sender: Sender<HotkeyEvent>,
    stop_flag: Arc<AtomicBool>,
) {
    std::thread::spawn(move || {
        let mut pressed: HashSet<Key> = HashSet::new();
        let mut record_held = false; // suppress key repeat for push-to-talk

        if let Err(e) = listen(move |event| {
            if stop_flag.load(Ordering::Relaxed) {
                return;
            }

            match event.event_type {
                EventType::KeyPress(key) => {
                    pressed.insert(key.clone());

                    // Push-to-talk: first press starts, ignore repeats
                    if record_binding.matches(&pressed, &key) && !record_held {
                        record_held = true;
                        let _ = sender.send(HotkeyEvent::RecordStart);
                    }

                    // TTS trigger
                    if tts_binding.matches(&pressed, &key) {
                        let _ = sender.send(HotkeyEvent::TtsTrigger);
                    }

                    // Inject mode switch
                    if inject_switch_binding.matches(&pressed, &key) {
                        let _ = sender.send(HotkeyEvent::InjectModeSwitch);
                    }

                    // Engine switch
                    if engine_switch_binding.matches(&pressed, &key) {
                        let _ = sender.send(HotkeyEvent::EngineSwitch);
                    }
                }
                EventType::KeyRelease(key) => {
                    pressed.remove(&key);

                    // Push-to-talk: stop on release, reset repeat guard
                    if key == record_binding.key
                        || record_binding.modifiers.contains(&key)
                    {
                        if record_held {
                            let _ = sender.send(HotkeyEvent::RecordStop);
                        }
                        record_held = false;
                    }
                }
                _ => {}
            }
        }) {
            log::error!("Hotkey listener error: {:?}", e);
        }
    });
}

/// Parse a single key name into rdev Key.
fn parse_key(s: &str) -> Option<Key> {
    match s.to_lowercase().as_str() {
        "`" | "backquote" => Some(Key::BackQuote),
        "-" | "minus" => Some(Key::Minus),
        "=" | "equal" => Some(Key::Equal),
        "[" => Some(Key::LeftBracket),
        "]" => Some(Key::RightBracket),
        "\\" | "backslash" => Some(Key::BackSlash),
        ";" | "semicolon" => Some(Key::SemiColon),
        "'" | "quote" => Some(Key::Quote),
        "," | "comma" => Some(Key::Comma),
        "." | "period" | "dot" => Some(Key::Dot),
        "/" | "slash" => Some(Key::Slash),
        "space" | " " => Some(Key::Space),
        "enter" | "return" => Some(Key::Return),
        "tab" => Some(Key::Tab),
        "backspace" => Some(Key::Backspace),
        "escape" | "esc" => Some(Key::Escape),
        "delete" | "del" => Some(Key::Delete),

        "a" => Some(Key::KeyA),
        "b" => Some(Key::KeyB),
        "c" => Some(Key::KeyC),
        "d" => Some(Key::KeyD),
        "e" => Some(Key::KeyE),
        "f" => Some(Key::KeyF),
        "g" => Some(Key::KeyG),
        "h" => Some(Key::KeyH),
        "i" => Some(Key::KeyI),
        "j" => Some(Key::KeyJ),
        "k" => Some(Key::KeyK),
        "l" => Some(Key::KeyL),
        "m" => Some(Key::KeyM),
        "n" => Some(Key::KeyN),
        "o" => Some(Key::KeyO),
        "p" => Some(Key::KeyP),
        "q" => Some(Key::KeyQ),
        "r" => Some(Key::KeyR),
        "s" => Some(Key::KeyS),
        "t" => Some(Key::KeyT),
        "u" => Some(Key::KeyU),
        "v" => Some(Key::KeyV),
        "w" => Some(Key::KeyW),
        "x" => Some(Key::KeyX),
        "y" => Some(Key::KeyY),
        "z" => Some(Key::KeyZ),

        "0" => Some(Key::Num0),
        "1" => Some(Key::Num1),
        "2" => Some(Key::Num2),
        "3" => Some(Key::Num3),
        "4" => Some(Key::Num4),
        "5" => Some(Key::Num5),
        "6" => Some(Key::Num6),
        "7" => Some(Key::Num7),
        "8" => Some(Key::Num8),
        "9" => Some(Key::Num9),

        "f1" => Some(Key::F1),
        "f2" => Some(Key::F2),
        "f3" => Some(Key::F3),
        "f4" => Some(Key::F4),
        "f5" => Some(Key::F5),
        "f6" => Some(Key::F6),
        "f7" => Some(Key::F7),
        "f8" => Some(Key::F8),
        "f9" => Some(Key::F9),
        "f10" => Some(Key::F10),
        "f11" => Some(Key::F11),
        "f12" => Some(Key::F12),

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_alt_backtick() {
        let binding = HotkeyBinding::parse("Alt+`").unwrap();
        assert!(binding.modifiers.contains(&Key::Alt));
        assert_eq!(binding.key, Key::BackQuote);
    }

    #[test]
    fn test_parse_ctrl_shift_a() {
        let binding = HotkeyBinding::parse("Ctrl+Shift+A").unwrap();
        assert!(binding.modifiers.contains(&Key::ControlLeft));
        assert!(binding.modifiers.contains(&Key::ShiftLeft));
        assert_eq!(binding.key, Key::KeyA);
    }

    #[test]
    fn test_parse_alt_t() {
        let binding = HotkeyBinding::parse("Alt+T").unwrap();
        assert!(binding.modifiers.contains(&Key::Alt));
        assert_eq!(binding.key, Key::KeyT);
    }

    #[test]
    fn test_matches() {
        let binding = HotkeyBinding::parse("Alt+Shift+E").unwrap();
        let mut pressed = HashSet::new();
        pressed.insert(Key::Alt);
        pressed.insert(Key::ShiftLeft);
        assert!(binding.matches(&pressed, &Key::KeyE));

        pressed.remove(&Key::Alt);
        assert!(!binding.matches(&pressed, &Key::KeyE));
    }

    #[test]
    fn test_parse_invalid() {
        assert!(HotkeyBinding::parse("").is_none());
    }
}
