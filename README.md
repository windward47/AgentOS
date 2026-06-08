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
│       Agent 核心（通过 omp RPC）     │
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
| **Live2D 桌面** | Tauri + PixiJS + pixi-live2d-display | 日常桌面交互 |
| **VR 3D** | Godot 4 + OpenXR + VRM | 沉浸式 VR 体验 |

## Agent 核心策略

Companion **不自研 LLM 编排**，而是通过子进程接入 [oh-my-pi](https://github.com/can1357/oh-my-pi) (`omp --mode rpc`)。
oh-my-pi 提供 40+ LLM 提供商、32 个内置工具、多模型路由、流干预规则等能力。
Companion 专注于语音交互体验和虚拟形象呈现。

## 开发阶段

| 阶段 | 内容 | 状态 |
|------|------|------|
| 前置约定 | Git 仓库 + 分支 + .gitignore + LICENSE + README | ✅ 已完成 |
| 阶段零 | 项目初始化 + trait 定义 + 配置系统 | ✅ 已完成 |
| **阶段一** | **MVP — 核心对话 + 沙盒工具 + Live2D 形象** | ✅ **已完成** |
| Sprint 1.1 | 文字对话 + oh-my-pi RPC Agent 集成 | ✅ |
| Sprint 1.2 | 沙盒路径检查 + 文件/命令工具 | ✅ |
| Sprint 1.3 | 麦克风捕获 + VAD 状态机 + ASR (本地/云端) | ✅ |
| Sprint 1.4 | Live2D 形象 (PixiJS + pixi-live2d-display) | ✅ |
| Sprint 1.5 | oh-my-pi RPC Agent 核心集成 | ✅ |
| Sprint 1.6 | 设置面板 + 状态栏 | ✅ |
| 阶段二 | 实时打断 + 本地 TTS + 浏览器控制 + 系统模式 | 📋 待开始 |
| 阶段三 | 情绪识别 + 风格系统 + MCP 插件 + 社区商店 | 📋 待开始 |
| 阶段四 | VR 模式 + 跨平台打包 + 性能优化 | 📋 待开始 |

## 项目结构

```
AgentOS/
├── src-tauri/                      # Rust 后端 (companion-core)
│   ├── src/
│   │   ├── main.rs                 # 入口
│   │   ├── lib.rs                  # Tauri Builder + 模块声明
│   │   ├── config.rs               # 配置系统 (~/.companion/config.json)
│   │   ├── agent/
│   │   │   ├── mod.rs              # AgentEngine trait + AgentStreamEvent
│   │   │   └── omp_rpc.rs          # oh-my-pi RPC 子进程客户端
│   │   ├── audio/
│   │   │   ├── mod.rs              # AudioError
│   │   │   ├── capture.rs          # cpal 麦克风 + 环形缓冲区
│   │   │   └── vad.rs              # VAD 状态机 (EnergyVad + 四态)
│   │   ├── asr/
│   │   │   ├── mod.rs              # AsrProvider trait + VoiceInputService
│   │   │   ├── mock.rs             # MockAsr (测试用)
│   │   │   ├── whisper_local.rs    # Whisper.cpp 子进程
│   │   │   └── whisper_cloud.rs    # OpenAI Whisper API
│   │   ├── tts/
│   │   │   ├── mod.rs              # TtsProvider trait
│   │   │   └── mock.rs             # MockTts (测试用)
│   │   ├── llm/mod.rs              # ChatLlm trait (降级方案)
│   │   ├── emotion/mod.rs          # EmotionEngine trait (预留)
│   │   ├── mcp/mod.rs              # McpTool trait
│   │   ├── tools/
│   │   │   ├── mod.rs              # ToolRegistry
│   │   │   ├── file_tools.rs       # sandbox_list/read/write/delete
│   │   │   └── command_tools.rs    # sandbox_execute (含黑名单)
│   │   ├── sandbox/mod.rs          # 沙盒路径校验 (resolve)
│   │   ├── permissions/mod.rs      # 权限管理 (预留)
│   │   ├── state/mod.rs            # AppState + 8 个 Tauri IPC 命令
│   │   └── websocket/mod.rs        # WebSocket 服务 (VR 预留)
│   └── tauri.conf.json
├── web/                            # Vue 3 前端
│   ├── src/
│   │   ├── main.ts                 # 入口 (Vue + Pinia + Router)
│   │   ├── App.vue                 # 根组件 + 状态栏
│   │   ├── router/index.ts         # 路由 (Chat + Settings)
│   │   ├── stores/app.ts           # 全局状态 Pinia store
│   │   ├── views/
│   │   │   ├── ChatView.vue        # 对话界面 (含 Live2D 侧栏)
│   │   │   └── SettingsView.vue    # 设置面板
│   │   ├── components/
│   │   │   └── Live2DCanvas.vue    # PixiJS + Live2D 渲染
│   │   └── types/ipc.ts            # IPC 事件类型定义
│   └── vite.config.ts              # Vite + Tailwind CSS
├── docs/api/README.md              # API 接口文档
├── .gitignore / LICENSE / README.md
└── PLAN.md / SPEC.md
```

## Tauri IPC 命令

| 命令 | 参数 | 返回 | 说明 |
|------|------|------|------|
| `chat` | `message: string` | `string` | 发送消息给 Agent |
| `get_history` | 无 | `ConversationMessage[]` | 获取对话历史 |
| `clear_history` | 无 | `()` | 清空对话历史 |
| `transcribe_audio` | `audio: number[]` (f32 PCM) | `string` | 语音转文字 |
| `get_config` | 无 | `CompanionConfig` | 获取当前配置 |
| `update_config` | `config: CompanionConfig` | `()` | 更新配置 |

## 技术栈

- **前端框架**: Tauri 2.0 + Vue 3 + Vite + Tailwind CSS
- **后端语言**: Rust
- **Agent 后端**: oh-my-pi (`omp --mode rpc`)
- **Live2D 渲染**: PixiJS 7 + pixi-live2d-display
- **ASR**: Whisper.cpp (本地) / OpenAI Whisper API (云端)
- **麦克风捕获**: cpal
- **VAD**: 基于 RMS 能量的四态状态机
- **工具协议**: MCP (JSON-RPC over stdio/WebSocket) / omp 内置
- **许可协议**: MIT

## 快速开始

```bash
# 安装 oh-my-pi (Agent 核心)
curl -fsSL https://omp.sh/install | sh

# 设置环境变量
export OPENAI_API_KEY="sk-..."

# 启动开发模式
cargo tauri dev
```

---

*Companion 不是一个应用，而是一个 Agent 生态的底座。*
