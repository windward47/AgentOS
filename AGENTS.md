# Companion

跨平台、模块化、可扩展的桌面智能 Agent — 支持纯语音 / Live2D / VR 三模式。
Agent 核心通过 RPC 子进程接入 oh-my-pi，不自研 LLM 编排。

## Project

- **Stack**: Tauri 2.0 (Rust backend) + Vue 3 + Vite + Tailwind CSS (frontend)
- **Agent core**: oh-my-pi via Bun sidecar (`services/agent-sidecar/`) using `@oh-my-pi/pi-agent-core` SDK
  - Persistent Bun subprocess with JSON-RPC over stdin/stdout
  - Full event streaming (tokens, tool calls)
  - `@oh-my-pi/pi-ai` for LLM provider routing
  - Reads `~/.omp/agent/models.yml` and `~/.omp/agent/config.yml` for provider config
- **Live2D**: Pre-built avatar-agent bundle (`web/public/avatar-agent/`) with Cubism Core (live2dcubismcore.min.js) + shimmed Cubism2 globals — loaded in a separate transparent Tauri window
- **LLM API**: SiliconFlow API (`api.siliconflow.cn`) via omp → `sensenova` provider alias in `~/.omp/agent/models.yml`. Default model: `nex-agi/Nex-N2-Pro` (free tier)
- **Entry points**:
  - Rust: `src-tauri/src/main.rs` → `lib.rs` (Tauri Builder)
  - Frontend: `web/src/main.ts` → `App.vue` → `views/ChatView.vue`
- **Data root**: `~/.companion/` (config.json, sandbox/, logs/, tools/, models/)
- **Sidecar**: `services/agent-sidecar/` — Bun process powered by `@oh-my-pi/pi-agent-core`

## Commands

```sh
# Build / check / test (Rust backend)
cd src-tauri && cargo check
cd src-tauri && cargo test --lib          # unit tests (10)
cd src-tauri && cargo test --test e2e_tests  # IPC e2e (3)
cd src-tauri && cargo fix --lib -p companion-core --allow-dirty

# Frontend dev / build / test
cd web && npm run dev          # Vite dev server: http://localhost:5173
cd web && npm run build        # vue-tsc check + vite build
cd web && npm run test:ui      # Playwright 4 smoke tests (headless)
cd web && npm run test:ui:headed  # Playwright with visible browser
cd web && npm run verify       # build + test:ui

# Full Tauri dev (runs both)
cargo tauri dev                # requires omp installed + $OPENAI_API_KEY

# One-command full verification
bash scripts/verify.sh         # 7 steps: cargo check/test/e2e + npm build + playwright + omp
bash scripts/verify-tauri.sh   # Also tries CDP connection to real Tauri window

# Install omp (Agent core)
curl -fsSL https://omp.sh/install | sh
```

## Architecture

```
src-tauri/src/
  agent/          AgentEngine trait + OmpAgentSidecar (Bun sidecar via JSON-RPC)
  audio/          AudioCapture (cpal mic + ring buffer) + VAD (4-state)
  asr/            AsrProvider + WhisperLocal/WhisperCloud/XiaomiAsr
  config.rs       ConfigManager: ~/.companion/config.json
  emotion/        EmotionEngine trait (预留)
  llm/            ChatLlm trait (direct API fallback, 预留)
  mcp/            McpTool trait
  permissions/    (预留)
  sandbox/        Sandbox::resolve() — path canonicalization + escape
  state/          AppState + 8 Tauri IPC commands
  tools/          ToolRegistry + 5 built-in tools
  tts/            TtsProvider + XiaomiTts + mock
  websocket/      WebSocket server (VR 预留)

web/src/
  views/          ChatView.vue, SettingsView.vue, AvatarView.vue
  components/     Live2DCanvas.vue
  router/         / (chat), /settings, /avatar
  stores/         Pinia app store (messages, sending state)
  types/          IPC event type definitions (AudioLevelEvent, etc.)
```

**Key design**: All ML/AI modules (ASR, TTS, Emotion) define Rust traits in their `mod.rs`.
Local and cloud implementations live in sibling files. Runtime switching via config.

## IPC Commands (Tauri → Rust)

| Command | Args | Returns |
|---------|------|---------|
| `chat` | `message: string` | `string` |
| `get_history` | — | `Vec<ConversationMessage>` |
| `clear_history` | — | `()` |
| `transcribe_audio` | `audio: Vec<f32>` (16kHz mono PCM) | `string` |
| `synthesize_audio` | `text: string, voice?: string` | `Vec<f32>` (PCM f32 mono) |
| `get_config` | — | `CompanionConfig` |
| `update_config` | `config: CompanionConfig` | `()` |
| `set_lip_level` | `level: f32` (0-1) | `()` |
| `get_lip_level` | — | `f32` |

## Conventions

- **Rust errors**: `thiserror` derive enums in each module. Tauri commands return `Result<T, String>`.
- **Rust tests**: `#[cfg(test)] mod tests` inside each module file. Use `#[tokio::test]` for async tests.
- **Traits**: `#[async_trait]`, `Send + Sync` bounds, doc comments with `///`.
- **Serde**: derive `Serialize + Deserialize` for all data types crossing the FFI boundary.
- **State**: `AppState` holds `Arc<dyn AgentEngine>`, `Arc<Mutex<CompanionConfig>>`, `Arc<Mutex<Vec<ConversationMessage>>>`.
- **Frontend**: Vue 3 `<script setup lang="ts">`, Pinia stores, Tailwind CSS (`@import "tailwindcss"` in style.css).
- **Naming**: snake_case for Rust, camelCase for TypeScript/JSON. IPC event types in `web/src/types/ipc.ts`.
- **Module pattern**: Each Rust module has `mod.rs` defining the trait + error enum; implementations in sibling files (e.g. `whisper_local.rs`, `whisper_cloud.rs`).
- **Testing pattern**: Mock implementations in `mock.rs` within each module for unit testing.

## Verification Pipeline

Three-layer verification, runnable via `bash scripts/verify.sh`:

| Layer | Tool | Tests | What it verifies |
|-------|------|-------|-----------------|
| **Rust unit** | `cargo test --lib` | 10 | traits, config, sandbox, VAD, mocks |
| **Rust e2e** | `cargo test --test e2e_tests` | 3 | AgentEngine chat flow, history truncation |
| **UI smoke** | Playwright + real Chrome | 4 | layout, settings nav, message send, sidebar |

Playwright opens actual Chrome, navigates the app, clicks through Settings, types messages.
Screenshots saved to `web/tests/screenshots/` and `web/test-results/`.

To verify against the real Tauri desktop app (not just browser):
1. `cargo tauri dev` (opens Window)
2. WebView2 exposes a CDP endpoint (Tauri ≥2 supports `internal_toggle_devtools`)
3. Playwright connects via `browserType.connectOverCDP()` → tests real Rust IPC

## Lessons Learned (重要教训)

这些是从实际踩坑中总结的，每次做技术选型时先过一遍。

### 1. 第三方 SDK 选型：先验证，再集成

**Live2D 是最典型的反面案例**。pixi-live2d-display npm 包已经和当前 pixi.js 生态断裂：

| 版本 | 问题 |
|------|------|
| v0.4.0 | 对 .moc3 模型强制要求 Cubism 2 运行时 (`live2d.min.js`)，该文件是专有的且不在任何 CDN |
| v0.5.0-beta | `import "@pixi/core"` 等裸 specifier 在 Vite/Rolldown 中无法解析 |

**正确做法**：在任何第三方库上投入超过 30 分钟之前：
1. 去官网/GitHub 确认最新版、支持的运行时、Breaking Changes
2. 找一个**已确认能工作的参考实现**（我们的 avatar-agent）
3. `npm install && npm run build` 立即验证打包是否通过

### 2. 协议假设必须手工验证

`omp --mode rpc` 的 RPC 帧只返回 `{"success":true}`，**消息内容不走帧**。我们在没验证之前就写了一整套 `OmpRpcClient`（NDJSON 解析、帧循环、auto-spawn），全部白费。最后换成了 `omp -p` 同步模式才工作。

**正确做法**：
```bash
echo '{"id":"r1","type":"prompt","message":"hello"}' | omp --mode rpc --no-session
# 确认响应格式后，再写 Rust 代码
```

### 3. Windows 上 npm 全局包需要 `.cmd` 后缀

`%APPDATA%/npm/omp` 是 POSIX shell script，Windows `Command::new("omp")` 不会自动执行它。必须用 `omp.cmd`。

### 4. Tauri 多窗口架构是 Live2D 的正确容器

Live2D 不应该和聊天 UI 挤在同一个 Vue 组件里。正确做法：
- `tauri.conf.json` 定义第二个窗口：`transparent: true, decorations: false`
- 指向独立的 HTML 文件
- 主窗口通过 Tauri `emit('audio_level')` 驱动嘴型

### 5. Playwright 是必须的，不是可选的

本会话中 Playwright 多次在实际用户反馈之前发现 Bug：
- Settings 页面无 Tauri 环境时永久 Loading
- Live2D Canvas 存在但加载失败的具体错误
- 404 资源路径

每次提交前跑 `npm run test:ui`，每次怀疑 UI 问题时先截图。

### 6. 非标准 API 端点必须实测

小米 Token Plan 的 ASR/TTS 模型列在 `/v1/models` 中，但 `/v1/audio/transcriptions` 和 `/v1/audio/speech`（OpenAI 标准端点）都是 404。
实际的 ASR/TTS 接口是 `/v1/chat/completions`——通过 `input_audio` 和 `audio` modality 参数实现。
**不会 curl 实测就直接开始写代码，必然走弯路。**

## Current Sprint Status

| Sprint | Status | Core Deliverable |
|--------|--------|-----------------|
| 1.1 | ✅ | Chat + omp -p + Nex-N2-Pro |
| 1.2 | ✅ | Sandbox (5 tools, path escape) |
| 1.3 | ✅ | ASR/TTS (Xiaomi chat API) + voice UI |
| 1.4 | ✅ | Live2D: PixiJS + Cubism4 CDN + transparent window + lip-sync + eye tracking + scroll zoom |
| 1.5 | ✅ | omp config (Windows .cmd, SiliconFlow) |
| 1.6 | ✅ | Settings UI + model/tool bridges to omp |
| **2.1** | **✅** | **Interrupt: bg VAD → stop TTS → ASR → auto-send** |
| 2.2 | ✅ | Local TTS (ChatTTS) — replaced by global voice hotkey ASR/TTS with Xiaomi API |
| 2.3 | ✅ | Browser screenshot (Playwright headless Chrome) |
| 2.4 | ✅ | Audit logging + system mode switch |
| **2.5** | **✅** | **Global voice hotkey system (Alt+` ASR, Alt+T TTS) + system tray menu + Live2D animation** |

## Notes

## Al Agent风格
1. 用户如果需要新增功能，先与用户详细讨论，明确用户需求，了解用户为何要添加该功能，想要实现什么效果，直到你最终明确因果链后，将你的理解
回馈给用户，最后再讨论落地方案;
2. 用户如果需要维护某些功能或者删除某些功能，先与用户沟通清楚用户希望删除或者维护后实现什么目的，然后将你的理解反馈给用户，最后评估实现该目的
会牵扯哪些模块，逻辑，再输出具体的落地方案;
3. 用户如果明确指明具体的bug或者问题，先与用户明确详细的复现路径，确认复现后，分析可能涉及的模块功能点以及相关文档，然后进行深层分析，如果分析期复现路径期
间遇到与当前项目决策点有分歧，先反馈给用户，与用户讨论完明确因果链后，最后再输出具体的方案;
4. spec的风格主要是明确因果链，以及明确边界，和禁止什么;
5. harness的落地原则:不包含纯产品意图(功能意图)、探索性功能、文案、UI、一次性设计;
6. 对于新功能，如果最终落地且自测通过，需要反馈给用户是否需要spec、harness;
7. 对于维护或者bug修复，根据spec和harness落地原则，自行评估是否需要spec和harness。
除非用户明确任务全自动化，交给你完全自主，否则按照以上AI Agent风格来。

<!-- Quick-add space for future agent notes -->

## Testing

### playwright-cli (CLI-based browser testing)
- **Install**: `npm install -g @playwright/cli@latest && playwright-cli install --skills`
- **Usage**: `playwright-cli open http://localhost:5173/avatar.html --headed` for live visual debugging
- **Key commands**: `snapshot`, `screenshot`, `eval "document.title"`, `console`
- **Skills**: installed to `.claude/skills/playwright-cli/`
- **Why CLI not MCP**: Token-efficient — no large tool schemas loaded into agent context
