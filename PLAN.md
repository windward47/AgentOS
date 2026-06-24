# Plan

## Sprint Status

| Sprint | Status | Deliverable |
|--------|--------|------------|
| S6 | ✅ | HTTP+SSE architecture, conversation context fix, deadlock fix, config sync, TTS, sandbox tools |
| S7 | 📋 | Voice input + streaming TTS chunking + UI polish |

## Short-term Tasks

See `TASKS/` directory for details.

| Task | Status |
|---|---|
| 001 — Voice Input Integration | pending |
| 002 — TTS Streaming Chunking | done |
| 003 — Sandbox UI State Sync | pending |

## Code Audit (2026-06-16)

### High Priority

| # | Issue | Location | Status |
|---|---|---|---|
| A1 | `state/mod.rs` 643-line God Module | 30+ commands mixed | 📋 TASKS/004 |
| A2 | `lib.rs` setup closure 240 lines + 11 unwraps | tray menu builder | 📋 TASKS/004 |
| A3 | `capture_mgr.rs` mutex poison panic risk | L99, L122 | ✅ fixed |
| A4 | `voice_handler.rs:145` blind audio chunking | local TTS | ✅ fixed |

### Medium Priority

| # | Issue | Location | Status |
|---|---|---|---|
| B1 | `provider_to_model()` dead code | `agent/mod.rs:94` | ✅ deleted |
| B2 | `AgentEngine::chat_stream` permanent stub | `omp_sidecar.rs:207` | ✅ deleted |
| B3 | `AgentResponse` fields always empty | `agent/mod.rs:43` | ✅ deleted |
| B4 | `switch_model` / `voice_name` never called | trait dead interfaces | ✅ deleted |
| B5 | `audio/utils.rs` uses `Box<dyn Error>` | inconsistent with thiserror | ✅ fixed |

### Low Priority

| # | Issue |
|---|---|
| C1 | `capture_mgr.rs` should be under `audio/` |
| C2 | `tts/playback.rs` naming imprecise |
| C3 | `ConfigState::new()` dead code |

## Long-term Backlog

- Emotion recognition + Live2D expression sync
- MCP plugins
- VR mode
- Cross-platform packaging
- pi-natives Shell (replace execSync)
- hashline AI file editing protocol
