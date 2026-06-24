# 004 — Split state/mod.rs God Module

Status: pending

## Problem
`state/mod.rs` is 643 lines with 30+ IPC commands mixing chat, conversation, memory, ASR/TTS, avatar, config, cursor, screenshot.

## Plan
Split into:
- `commands/chat.rs` — chat, chat_stream, get_history, clear_history
- `commands/conversation.rs` — list/create/switch/delete/rename, get_current
- `commands/media.rs` — transcribe_audio, synthesize_audio, list_models
- `commands/memory.rs` — list_memories, forget_memory
- `commands/avatar.rs` — avatar visibility, always-on-top, position, live2d models
- `commands/system.rs` — get_cursor_pos, open_folder, browse_screenshot, audit_log
- `commands/config.rs` — get_config, update_config

Each file: `#[tauri::command]` functions + any helper structs.
`state/mod.rs` becomes a barrel: `pub mod commands::*` + state struct definitions.

## Also
Extract `lib.rs` 240-line setup closure into:
- `tray.rs` — tray menu builder
- `window.rs` — window setup
- `hotkey_setup.rs` — global hotkey listener
