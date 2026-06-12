# Companion

> 跨平台、模块化、可扩展的智能桌面 Agent — 支持纯语音、Live2D 桌面和 VR 三种呈现模式。

## 核心理念

Companion 是一个**解耦**的 Agent 生态系统：

```
┌─────────────────────────────────────┐
│       表达层（可替换）               │
│  纯语音 │ Live2D 桌面 │ Godot VR   │
├─────────────────────────────────────┤
│       感知层（可替换）               │
│  ASR │ TTS │ VAD │ 情绪识别         │
├─────────────────────────────────────┤
│       Agent 核心（Bun sidecar）      │
│  @oh-my-pi/pi-agent-core SDK          │
│  对话管理 │ LLM 编排 │ 工具调用     │
├─────────────────────────────────────┤
│       工具层（社区贡献，MCP 协议）   │
│  文件操作 │ 命令执行 │ 浏览器控制   │
└─────────────────────────────────────┘
```

## 三种交互模式

| 模式 | 技术栈 | 适用场景 |
|------|--------|----------|
| **纯语音** | Tauri + cpal + Whisper + TTS | 后台驻守、低功耗设备 |
| **Live2D 桌面** | Tauri 独立透明窗口 + PixiJS 7 + pixi-live2d-display + Cubism Core | 日常桌面交互，多窗口架构 |
| **VR 3D** | Godot 4 + OpenXR + VRM | 沉浸式 VR 体验 |

## Agent 核心策略

Companion **不自研 LLM 编排**，而是通过 Bun sidecar 进程接入 [oh-my-pi](https://github.com/can1357/oh-my-pi) 的 `@oh-my-pi/pi-agent-core` SDK。
SDK 提供 40+ LLM 提供商、多模型路由、原生工具调用、流式事件等能力。
Companion 专注于语音交互体验和虚拟形象呈现。

## 开发阶段

| 阶段 | 内容 | 状态 |
|------|------|------|
| 前置约定 | Git 仓库 + 分支 + .gitignore + LICENSE + README | ✅ 已完成 |
| 阶段零 | 项目初始化 + trait 定义 + 配置系统 | ✅ 已完成 |
| **阶段一** | **MVP — 核心对话 + 沙盒工具 + Live2D 形象** | ✅ **已完成** |
| Sprint 1.1 | 文字对话 + oh-my-pi SDK Agent 集成 | ✅ |
| Sprint 1.2 | 沙盒路径检查 + 文件/命令工具 | ✅ |
| Sprint 1.3 | 麦克风捕获 + VAD 状态机 + ASR (本地/云端) | ✅ |
| Sprint 1.4 | Live2D 形象 (PixiJS + pixi-live2d-display) | ✅ |
| Sprint 1.5 | oh-my-pi SDK Agent 核心集成 | ✅ |
| Sprint 1.6 | 设置面板 + 状态栏 | ✅ |
| **阶段二** | **实时打断 + 全局语音热键 + 系统托盘 + 浏览器控制 + Live2D 动画联动** | ✅ **已完成** |
| Sprint 2.1 | 实时打断 (bg VAD → stop TTS → ASR → auto-send) | ✅ |
| Sprint 2.2-2.5 | 全局语音热键 ASR/TTS (Alt+\` / Alt+T) + 系统托盘 + Live2D animation | ✅ |
| **重构 R1** | **Cargo workspace + Domain State 拆分 + 死代码清理** | ✅ **已完成** |
| 阶段三 | 情绪识别 + 风格系统 + MCP 插件 + 社区商店 | 📋 待开始 |
| 阶段四 | VR 模式 + 跨平台打包 + 性能优化 | 📋 待开始 |

## 项目结构

```
AgentOS/
├── Cargo.toml                          ← workspace 根
├── companion-core/                     ← 纯逻辑库（零 Tauri 依赖）
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                      ← 15 个模块 re-export
│       ├── agent/                      ← AgentEngine trait + OmpAgentSidecar
│       ├── asr/                        ← AsrProvider + Xiaomi/Whisper/Aliyun
│       ├── audio/                      ← AudioCapture + VAD + utils
│       ├── config.rs                   ← CompanionConfig + ConfigManager
│       ├── capture_mgr.rs              ← 全局录音管理器
│       ├── emotion/                    ← EmotionEngine trait (预留)
│       ├── hotkey/                     ← HotkeyBinding + rdev 监听
│       ├── inject/                     ← InjectMode + keyboard/clipboard/text_reader
│       ├── llm/                        ← ChatLlm trait (降级，预留)
│       ├── mcp/                        ← McpTool trait
│       ├── permissions/                ← AuditLogger + HIGH_RISK_CMDS
│       ├── sandbox/                    ← Sandbox::resolve() 路径校验
│       ├── tools/                      ← ToolRegistry + file_tools + command_tools
│       ├── tts/                        ← TtsProvider + XiaomiTts + playback
│       └── websocket/                  ← WebSocket 服务 (VR 预留，stub)
├── companion-tauri/                    ← Tauri 桌面壳
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── build.rs
│   ├── icons/
│   ├── capabilities/
│   └── src/
│       ├── main.rs                     ← 入口
│       ├── lib.rs                      ← Tauri::Builder (~160行)
│       ├── state/mod.rs                ← 5 个领域状态 + 15 个 IPC 命令
│       └── voice_handler.rs            ← 全局语音命令处理
├── web/                                ← Vue 3 前端
│   ├── src/
│   │   ├── main.ts                     ← 入口 (Vue + Pinia + Router)
│   │   ├── App.vue                     ← 根组件 + 状态栏
│   │   ├── router/index.ts             ← 路由 (Chat + Settings + Avatar)
│   │   ├── stores/app.ts               ← Pinia store
│   │   ├── views/                      ← ChatView, SettingsView, AvatarView
│   │   ├── components/                 ← Live2DCanvas
│   │   ├── avatar/main.ts              ← Live2D 窗口入口 (pixi-live2d-display)
│   │   ├── live2d/                     ← Cubism 4 Framework TS 移植版
│   │   └── types/ipc.ts                ← IPC 事件类型定义
│   └── vite.config.ts
├── services/agent-sidecar/             ← Bun sidecar TS 项目
├── docs/api/README.md
├── scripts/verify.sh / verify-tauri.sh
├── .gitignore / LICENSE / README.md
└── PLAN.md / SPEC.md / AGENTS.md
```

## Tauri IPC 命令

| 命令 | 参数 | 返回 | 说明 |
|------|------|------|------|
| `chat` | `message: string` | `string` | 发送消息给 Agent |
| `chat_with_tools` | `message: string` | `string` | 发送消息（含工具调用） |
| `get_history` | 无 | `ConversationMessage[]` | 获取对话历史 |
| `clear_history` | 无 | `()` | 清空对话历史 |
| `transcribe_audio` | `audio: number[]` (16kHz PCM) | `string` | 语音转文字 (Xiaomi ASR) |
| `synthesize_audio` | `text: string, voice?: string` | `number[]` (PCM) | 文字转语音 (Xiaomi TTS) |
| `get_config` | 无 | `CompanionConfig` | 获取当前配置 |
| `update_config` | `new_config: CompanionConfig` | `()` | 更新配置 |
| `set_lip_level` | `level: number` (0-1) | `()` | TTS 嘴型同步 |
| `get_lip_level` | 无 | `number` | 读取嘴型值 |
| `get_voice_state` | 无 | `"idle"\|"listening"\|"speaking"` | Live2D 动画状态 |
| `get_cursor_pos` | 无 | `[number, number]` | 全局光标坐标 |
| `browse_screenshot` | `url: string` | `string` (base64 PNG) | 浏览器截图 |
| `get_audit_log` | 无 | `string` | 读取操作日志 |
| `list_models` | `base_url: string, api_key: string` | `string[]` | 获取可用模型列表 |

## 技术栈

- **前端框架**: Tauri 2.0 + Vue 3 + Vite + Tailwind CSS
- **后端语言**: Rust (Cargo workspace: companion-core + companion-tauri)
- **Agent 后端**: oh-my-pi (Bun sidecar NDJSON-RPC 持久进程)
- **LLM 模型**: SiliconFlow Nex-N2-Pro (免费) / Xiaomi MiMo V2.5 Pro
- **Live2D 渲染**: 独立透明 Tauri 窗口 + PixiJS 7 + pixi-live2d-display + Cubism Core
- **Live2D 交互**: 滚轮缩放、中键拖拽移动、左键双击还原、全局鼠标追踪转头、右键 Pin/关闭
- **ASR**: Xiaomi MiMo V2.5 ASR / WhisperCloud / Aliyun / WhisperLocal (Chat Completions API)
- **TTS**: Xiaomi MiMo V2.5 TTS (Chat Completions API, 9种声音)
- **VAD**: 基于 RMS 能量的四态状态机 + 浏览器 MediaRecorder
- **工具协议**: omp 内置工具 + MCP 扩展预留
- **许可协议**: MIT

## 快速开始

### 前置条件

```bash
# 1. 安装 oh-my-pi (Agent 核心)
curl -fsSL https://omp.sh/install | sh

# 2. 设置 API Token
set COMPANION_API_TOKEN="your-token-here"   # Windows PowerShell
# 或 $env:COMPANION_API_TOKEN = "your-token-here"
```

### 一键验证（推荐先跑这个确认环境正常）
```bash
bash scripts\verify.sh
```

### 编译
```bash
cargo check                        # 全 workspace 检查（最常用）
cargo check -p companion-core      # 只检查逻辑库
cargo check -p companion-tauri     # 只检查 Tauri 壳
cargo build                        # 生产构建
```

### 测试
```bash
cargo test -p companion-core --lib              # 单元测试 20 个
cargo test -p companion-core --test e2e_tests    # 端到端测试 3 个
cd web && npm run test:ui                        # 前端 Playwright 测试
```

### 运行 Tauri 桌面应用
```powershell
# ★ 必须在 companion-tauri 目录下运行（tauri.conf.json 在那里）
cd companion-tauri

# 启动（自动启动前端 dev server + Rust 后端）
cargo tauri dev

# 如果端口 5173 被占用：
netstat -ano | findstr :5173
taskkill /PID <PID> /F

# 或者分两个终端调试：
# 终端 1：
cd web && npm run dev
# 终端 2（等终端 1 启动后再运行）：
cd companion-tauri && cargo tauri dev
```

### 常见问题

| 问题 | 解决 |
|------|------|
| `Port 5173 is already in use` | `taskkill /F /IM node.exe` 或 `netstat -ano \| findstr :5173` 杀进程 |
| `omp` 找不到 | `curl -fsSL https://omp.sh/install \| sh` |
| `COMPANION_API_TOKEN` 未设置 | `set COMPANION_API_TOKEN=xxx` |
| `cargo tauri dev` 报错 "no tauri.conf.json" | 确保 `cd companion-tauri` 后再运行 |
| 杀不掉 node 进程 | 任务管理器 → 结束 Node.js 或重启电脑 |

---

*Companion 不是一个应用，而是一个 Agent 生态的底座。*

## Testing with playwright-cli

```bash
# Install (one-time)
npm install -g @playwright/cli@latest
playwright-cli install --skills

# Visual debugging (headed mode)
playwright-cli open http://localhost:5173/avatar.html --headed
playwright-cli screenshot

# Check console
playwright-cli console

# Run JS in page
playwright-cli eval "document.title"
```
