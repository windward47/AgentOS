# Architecture

## Stack
- **Rust**: Cargo workspace — `companion-core` (pure logic) + `companion-tauri` (Tauri shell)
- **Frontend**: Vue 3 + Vite + Tailwind + Pinia
- **Sidecar**: Bun process (`services/agent-sidecar/`) via HTTP on `127.0.0.1:9876`

## Data Flow

```
ChatView (Vue) → Tauri IPC → Rust state → reqwest HTTP → Bun sidecar → LLM API
                    ↑                                              ↓
                    └── chat_token events (SSE stream) ←───────────┘
```

## Directory Map

```
companion-core/src/
  agent/omp_sidecar.rs    — HTTP client to sidecar (reqwest)
  config.rs               — CompanionConfig types

companion-tauri/src/
  main.rs                 — entry
  lib.rs                  — Tauri::Builder (plugins, states, commands)
  state/mod.rs            — all IPC commands + domain states
  voice_handler.rs        — global voice hotkey handler (debug-only logs)

services/agent-sidecar/src/
  index.ts                — Bun.serve HTTP server + SSE
  agent.ts                — AgentManager (model, tools, chat, history, memory)
  memory-manager.ts       — Mnemopi wrapper
  config.ts               — CompanionConfig + buildPiModel
  audio.ts                — ASR/TTS via Xiaomi API

web/src/
  views/ChatView.vue      — chat UI (send, stream, TTS, sandbox btn)
  views/SettingsView.vue  — settings (LLM, voice, memory, detect models)
  components/ConversationList.vue — sidebar session list
  composables/            — useCompanion, useTtsPlayer, useVoiceCore
  stores/app.ts           — Pinia store (messages, conversations, sending)
```

## IPC Commands (current, active)

| Command | Handler |
|---|---|
| `chat_stream` | Sidecar (SSE) |
| `chat` | Sidecar |
| `list_conversations/create/switch/delete/rename_conversation` | Sidecar |
| `get_current_conversation` | Sidecar |
| `get_history/clear_history` | Sidecar |
| `get_config/update_config` | Sidecar |
| `list_memories/forget_memory` | Sidecar |
| `list_models` | Rust (HTTP) |
| `open_folder` | Rust |
| `get_cursor_pos` | Rust |
| `transcribe_audio/synthesize_audio` | Sidecar → Xiaomi API |

## Conventions

- **Rust**: `thiserror` enums, `#[async_trait]`, `#[cfg(test)]` per module
- **Frontend**: `<script setup lang="ts">`, Pinia stores, camelCase
- **New feature**: trait → domain state → IPC command → composable → Vue
