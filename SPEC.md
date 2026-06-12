# 项目规格书：Companion — 可定制形象的智能 Agent

> 版本：v1.0  
> 基于多轮对话整理，涵盖纯语音 / Live2D 桌面 / VR 三种模式，以及社区插件生态设计。

---

## 目录

1. [项目定位与核心理念](#1-项目定位与核心理念)
2. [三种交互模式](#2-三种交互模式)
3. [架构分层详解](#3-架构分层详解)
4. [与现有开源项目的关系](#4-与现有开源项目的关系)
5. [社区与创意工坊机制](#5-社区与创意工坊机制)
6. [硬件与边缘设备策略](#6-硬件与边缘设备策略)
7. [对话风格与情绪系统](#7-对话风格与情绪系统)
8. [安全与隐私设计](#8-安全与隐私设计)
9. [开发阶段](#9-开发阶段)
10. [技术栈总结](#10-技术栈总结)
11. [接口开放清单](#11-接口开放清单)

---

## 1. 项目定位与核心理念

**Companion** 是一个**跨平台、模块化、可扩展**的智能桌面 Agent，具备：

- **多形态呈现**：纯语音（无界面）、桌面 2D（Live2D）、VR 3D（Godot + VRM）
- **多源智能**：可切换本地 / 云端 ASR、TTS、LLM
- **可插拔工具**：通过 MCP 协议动态加载任意能力
- **社区驱动**：借鉴 Steam 创意工坊思路，接口开放、组件透明、沙盒安全

**核心理念：解耦。**

```
┌─────────────────────────────────────────────────────────────┐
│                    表达层（可替换）                          │
│  纯语音 TUI  │  Live2D 桌面  │  Godot VR  │  自定义前端     │
└──────────┬──────────────────┬──────────────┬───────────────┘
           │      统一接口 (WebSocket / JSON-RPC / IPC)        │
┌──────────▼──────────────────▼──────────────▼───────────────┐
│                    感知层（可替换）                          │
│  ASR (Whisper/Azure) │  TTS (ChatTTS/Edge) │  情绪识别     │
│  VAD  │  音频捕获/播放  │  视觉输入（预留）                  │
└──────────┬──────────────────┬──────────────┬───────────────┘
           │      统一接口 (MCP + 感知抽象)                    │
┌──────────▼──────────────────▼──────────────▼───────────────┐
│                  Agent 核心（轻量，无 UI）                    │
│  对话管理 │  LLM 编排 │  工具调用 │  安全策略 │  状态管理   │
└──────────┬──────────────────┬──────────────┬───────────────┘
           │                  MCP 协议                         │
┌──────────▼──────────────────▼──────────────▼───────────────┐
│                    工具层（社区贡献）                        │
│  文件操作 │  命令执行 │  浏览器控制 │  家电控制 │  社交组件 │  ...
└─────────────────────────────────────────────────────────────┘
```

Agent 核心只依赖标准接口，不绑定任何具体 ASR / TTS / 图形库。核心可运行在树莓派 Zero 上，表达层在 PC 上。

**Agent 核心的实现：** 通过 RPC 子进程 (`Bun sidecar (NDJSON JSON-RPC over stdin/stdout)`) 接入 [oh-my-pi](https://github.com/can1357/oh-my-pi)，复用其 LLM 编排、工具调用、对话管理能力。Companion 不重复实现 Agent 逻辑，而是做 omp 的"前端"——提供语音交互和虚拟形象呈现。

---

## 2. 三种交互模式

### 2.1 纯语音模式（Headless / TUI）

| 项目 | 说明 |
|------|------|
| 启动方式 | `companion --no-gui --stdin --stdout` 或系统托盘静默运行 |
| 交互方式 | 语音对话（麦克风 + 扬声器），或命令行文本交互 |
| 适用场景 | 后台驻守、SSH 连接、低功耗设备（树莓派）、无障碍辅助 |
| 资源占用 | 极低，仅 Agent 核心 + 音频模块 |

### 2.2 桌面 2D 模式（Live2D）

| 项目 | 说明 |
|------|------|
| 渲染引擎 | Tauri 独立透明 WebView 窗口 + PixiJS 7 + pixi-live2d-display |
| 形象格式 | Live2D Cubism 3 (.model3.json / .moc3) |
| Cubism 运行时 | live2dcubismcore.min.js (Emscripten WASM, 207KB) |
| 形象驱动 | 嘴型（音频振幅 → ParamMouthOpenY）+ 情绪（wav2vec2 → 表情切换） |
| 状态动画 | idle（眨眼 + 呼吸）+ speaking + listening + thinking + error |
| 默认模型 | Haru Greeter Pro（Cubism 3） |
| 窗口管理 | `transparent: true, decorations: false, alwaysOnTop: true` — 主窗口独立于聊天窗口 |
| 交互 | 顶部拖拽条移动窗口 + 右键菜单关闭 |

> **注意**: pixi-live2d-display npm 包已与当前 pixi.js 生态断裂（v0.4.0 要求已消失的 Cubism 2 运行时；
> v0.5.0-beta 无法通过 Vite/Rolldown 打包 `@pixi/core` 裸导入）。
> 最终方案：预构建 avatar-agent 的完整 PixiJS bundle + Cubism 2 全局变量 shim。
> 长期方案：移植到官方 Cubism 4 SDK for Web 的 Native 渲染路径（`web/src/live2d/renderer.ts` 已写好了框架，只差 Framework 编译产物）。

### 2.3 VR 模式（Godot + VRM）

| 项目 | 说明 |
|------|------|
| 渲染引擎 | Godot 4 + OpenXR |
| 形象格式 | VRM (.vrm) |
| 通信方式 | 独立进程，通过 WebSocket (ws://127.0.0.1:9001) 与 Tauri 后端通信 |
| 交互方式 | 手柄射线、语音按键、手势识别 |
| 空间定位 | 形象固定于面前 2 米，可手柄拖拽，始终面朝用户 |
| 为什么不选 WebXR | Linux WebKitGTK 对 WebXR 支持极差；Godot OpenXR 成熟稳定 |

---

## 3. 架构分层详解

### 3.1 Agent 核心（Core）

这是整个系统的大脑，被设计为**无 UI、轻量、可独立运行**的二进制。

**职责：**

- 对话管理与状态维护
- LLM 调用编排（本地 Ollama / 云端 OpenAI / Claude）
- 工具选择与参数生成
- 安全策略执行（沙盒路径校验、命令黑名单）
- 系统模式切换（沙盒模式 ↔ 系统模式）

**技术方案：**

- **主要方案（推荐）：** 通过子进程接入 **oh-my-pi** (`omp -p`, print mode)。  
  oh-my-pi 提供完整的 LLM 编排、32+ 内置工具、多模型路由、流干预规则。  
  Companion 通过同步子进程（stdout 即回复文本）与 omp 通信，专注于语音/感知/呈现。
  > **协议教训**：`Bun sidecar (NDJSON JSON-RPC over stdin/stdout)` 的 NDJSON-RPC 不返回消息体（只在 success 帧中返回 `{"success":true}`），
  > 不能用于接收 LLM 回复。`omp -p` 是唯一可行的同步模式。
- **降级方案：** 直接 LLM API 调用（OpenAI / Ollama），适用于无 omp 环境。
- 最小运行环境：4 核 1GHz CPU, 512MB RAM（树莓派 Zero 2W）

**接口暴露：**

- MCP 服务器（stdio / WebSocket）
- JSON-RPC over WebSocket（自定义扩展接口）

### 3.2 感知层（Perception）

**模块清单：**

| 模块 | 本地方案 | 云端方案 | 接口标准 |
|------|----------|----------|----------|
| ASR (语音→文本) | Xiaomi MiMo V2.5 ASR / Whisper.cpp | Chat Completions API / 子进程 | Rust trait: `fn transcribe(audio: &[f32]) -> Result<String>` |
| TTS (文本→语音) | Xiaomi MiMo V2.5 TTS / ChatTTS / Azure TTS | Chat Completions API / Python HTTP | Rust trait: `fn synthesize(text: &str) -> Vec<f32>` |
| VAD (语音活动检测) | webrtcvad / silero-vad | — | `fn is_voice(chunk: &[f32]) -> bool` |
| 情绪识别 | wav2vec2-base 微调模型 | — | `fn classify_emotion(audio: &[f32]) -> EmotionLabel` |
| 音频捕获 | cpal (麦克风输入) | — | 循环缓冲区，20ms 帧 |
| 音频播放 | cpal + hound (扬声器输出) | — | 支持打断，瞬时幅度回读 |

**设计原则：**

- 每个模块定义一个 Rust `trait`，本地 / 云端各实现该 trait
- 运行时通过配置选择具体实现
- 可热插拔：更换 ASR 不涉及任何其他模块的修改

### 3.3 表达层（Presentation）

**三种实现共享同一套后端接口：**

```json
{
  "type": "event",
  "method": "audio_level",
  "params": { "level": 0.75 }
}
{
  "type": "event",
  "method": "emotion",
  "params": { "label": "happy", "intensity": 0.8 }
}
{
  "type": "event",
  "method": "agent_state",
  "params": { "state": "thinking" }
}
{
  "type": "response",
  "id": 1,
  "result": { "text": "已打开浏览器" }
}
```

**表达层无需理解 LLM、工具或文件系统——它只负责呈现。**

### 3.4 工具层（Tools / MCP）

**标准 MCP 协议，所有的工具都是插件：**

| 内置工具（阶段一） | 社区工具（示例） |
|--------------------|-----------------|
| sandbox_read / write / delete | turn_on_light (智能家居) |
| sandbox_execute（命令） | send_message (社交) |
| web_browse（浏览器操作） | robot_control (机器人) |
| file_list（目录浏览） | calendar_query (日历) |
| | weather_query (天气) |
| | git_operations (开发) |

**工具的生命周期：**

1. 用户从社区商店安装工具包
2. 工具包放入 `~/.companion/tools/` 目录
3. Agent 核心自动检测 → 注册 MCP 工具
4. LLM 在对话中按需调用
5. 卸载时删除目录，自动注销

---

## 4. 与现有开源项目的关系

### 4.1 主要策略：复用 oh-my-pi 作为 Agent 核心

**oh-my-pi** (omp) 是 [Pi](https://github.com/badlogic/pi-mono) 的扩展分支，由 Can Bölük 维护，
是一个"电池全包"的编码 Agent。Companion 将其作为 Agent 核心引擎。

| 项目 | 优势 | 集成方式 |
|------|------|----------|
| **oh-my-pi** ✅ | 32+ 内置工具、40+ LLM 提供商、RPC 模式、MIT 协议 | `Bun sidecar (NDJSON JSON-RPC over stdin/stdout)` （NDJSON over stdio 子进程） |
| **OpenCode** | 免费模型策略、MCP 服务模式 | MCP 客户端调用（备选） |

**架构示意（以 oh-my-pi 为例）：**

```
┌─ Companion (Tauri) ───────────────────────┐
│  Live2D 界面 │ 语音模块 │ AgentEngine trait│
│  ┌─ agent::OmpAgentSidecar ───────────┐   │
│  │  Bun sidecar NDJSON-RPC            │   │
│  └──────────────────┬─────────────────┘   │
└─────────────────────┬─────────────────────┘
                      │ NDJSON-RPC over stdio
┌─────────────────────▼─────────────────────┐
│  Bun sidecar (NDJSON JSON-RPC over stdin/stdout) (独立进程)                  │
│  LLM 编排 │ 32+ 工具 │ 子任务 │ 流干预      │
└───────────────────────────────────────────┘
```

### 4.2 你的核心竞争力

不重复造 Agent 的轮子，专注在：

1. **语音交互体验**：低延迟打断、自然 TTS、情绪感知
2. **虚拟形象**：Live2D + VR 沉浸式呈现
3. **社区生态**：插件系统、创意工坊、接口标准
4. **硬件适配**：IoT 控制、机器人接口、边缘部署

---

## 5. 社区与创意工坊机制

### 5.1 组件包结构

每个社区组件（工具 / 插件 / 形象 / 语音包）标准化结构：

```
my_component/
├── manifest.json        # 元数据（名称/版本/作者/依赖/权限声明）
├── main.wasm            # 或 .py / .rs → 编译为 Wasm / 动态库
├── README.md
└── assets/              # 图标、示例音频、预览图等
```

**manifest.json 示例：**

```json
{
  "name": "home-assistant-control",
  "version": "1.0.0",
  "author": "community-devs",
  "description": "通过自然语言控制 Home Assistant 设备",
  "type": "tool",
  "permissions": ["network:http"],
  "entry": "main.wasm",
  "config_ui": "config.html"
}
```

### 5.2 社区商店

- **索引仓库**：GitHub 上维护 `awesome-companion-plugins`，存放审核通过的 manifest
- **浏览与安装**：用户在应用内浏览 → 一键下载（GitHub Releases）→ 沙盒安装
- **评分与评论**：利用 GitHub Issues / Discussions
- **自动更新**：定期检查 manifest 版本

### 5.3 安全沙盒

| 安全层级 | 措施 |
|----------|------|
| WASM 沙盒 | 社区工具默认以 Wasm 运行，无文件系统/网络访问 |
| 权限声明 | manifest 必须声明所需权限，安装时由用户批准 |
| 高危签名 | 系统级操作工具需开发者签名，安装时显示高危警告 |
| 运行隔离 | 工具运行在独立的子进程/容器中，超时自动终止 |
| 操作确认 | 所有自动化操作执行前，界面上显示 [允许/拒绝] 弹窗 |

---

## 6. 硬件与边缘设备策略

### 6.1 Agent 核心在资源受限硬件上运行

| 硬件 | 角色 | 配置 |
|------|------|------|
| 树莓派 Zero 2W | Agent 核心 + 小模型 LLM | 4核1GHz, 512MB, Ollama TinyLlama |
| PC / 笔记本 | 表达层（TTS + Live2D/VR） | 标准配置 |
| 网络 | MQTT / WebSocket 通信 | 可选断网重连 |

**工作流：**

```
用户对着 PC 麦克风说话
→ PC 完成 ASR（本地/云端）
→ 文本通过 WebSocket 发送到树莓派上的 Agent 核心
→ 核心调用 LLM + 工具，生成回复文本
→ 回复文本发回 PC
→ PC 的 TTS 播放 + Live2D 嘴型驱动
```

### 6.2 预留硬件接口

- **串口 / 蓝牙 MCP 工具**：控制外接传感器、机器人
- **GPIO（树莓派）**：直接控制物理设备
- **MQTT 桥接**：与智能家居中枢互通

---

## 7. 对话风格与情绪系统

### 7.1 预设风格模板

| 风格 | 说明 |
|------|------|
| 专业秘书 | 正式、简洁、高效 |
| 幽默朋友 | 俏皮、表情词、偶尔开玩笑 |
| 温柔陪伴 | 共情、鼓励、轻言细语 |
| 硬核极客 | 直接、技术术语、命令式 |

### 7.2 用户自定义

提供文本编辑器，让用户直接修改 system prompt，支持变量：
`{user_name}`、`{current_time}`、`{emotion}`

### 7.3 动态情绪匹配

```
用户语音 → wav2vec2 情绪分类 → 情绪标签 (happy/sad/angry/neutral)
  → ① 修改 system prompt（切换风格）
  → ② 驱动 Live2D / VRM 表情 BlendShape
  → ③ 影响 TTS 音色/语速（如有支持）
```

用户可自定义「情绪 → 风格」映射表。

---

## 8. 安全与隐私设计

| 项目 | 措施 |
|------|------|
| 录音数据 | 不离开本地（除非使用云端 API，会有明确提示） |
| 沙盒模式 | 文件操作限制在 `~/.companion/sandbox/` |
| 系统模式 | 开关 + 弹窗警告 + 高危命令二次确认 |
| 操作日志 | 所有系统级操作记录到 `~/.companion/logs/command.log` |
| 遥测 | 零遥测，不收集任何数据 |
| 社区工具 | Wasm 沙盒 + 权限声明 + 签名验证 |

---

## 9. 开发阶段

### 阶段一：MVP（核心对话 + 沙盒 + Live2D 基本形象）

- Tauri 项目骨架 + 窗口 + 系统托盘
- MCP 客户端模块（对接 OpenCode / Oh My Pi）
- 麦克风捕获 + VAD + 本地 Whisper ASR
- 集成云端 LLM API（本地 Ollama 可选）
- 沙盒文件读写工具 + 命令执行工具
- Live2D 加载 + 嘴型驱动（音频振幅）
- 对话历史界面
- 基础设置面板（模型选择、沙盒路径）

### 阶段二：完善交互与自动化

- 实时打断逻辑（TTS 与 ASR 并发管理）
- 本地 TTS 集成（ChatTTS）+ 音频播放
- 浏览器控制工具（Playwright）
- 系统模式开关 + 高危命令确认
- 安全日志

### 阶段三：社区与扩展

- 情绪识别（wav2vec2）
- 对话风格预设 + 自定义编辑器
- MCP 社区工具加载器（Wasm 沙盒）
- 插件商店浏览 / 安装
- 工具开发文档 + SDK 示例

### 阶段四：VR 模式与跨平台

- Tauri 后端增加 WebSocket 服务模块
- Godot 4 VR 客户端原型 + VRM 加载
- 手柄交互 + 空间定位
- Ubuntu Wayland 完整测试
- Windows 测试
- 性能优化 + 打包（.msi / .deb / AppImage）

---

## 10. 技术栈总结

| 层级 | 技术 | 说明 |
|------|------|------|
| 前端框架 | Tauri 2.0 + Vue 3 / React | WebView 与 Rust 后端 |
| 2D 渲染 | PixiJS + pixi-live2d-display | Canvas 绘制的 Live2D |
| 3D/VR 渲染 | Godot 4 + OpenXR | 独立进程，WebSocket 通信 |
| Agent 后端 | **oh-my-pi** (`Bun sidecar (NDJSON JSON-RPC over stdin/stdout)`) | RPC 子进程，NDJSON 通信 |
| Agent 降级 | 直接 LLM API（OpenAI / Ollama） | 无 omp 环境的备用方案 |
| ASR | Whisper.cpp / OpenAI Whisper API | 可切换 |
| TTS | ChatTTS / Azure TTS / ElevenLabs | 可切换 |
| LLM | 由 omp 管理（40+ 提供商） | 通过 omp 路由 |
| VAD | webrtcvad / silero-vad | — |
| 情绪识别 | wav2vec2-base 微调 | 本地运行 |
| 工具协议 | omp 内置 + MCP 扩展 | omp 管理工具调用，MCP 用于社区扩展 |
| 插件扩展 | Wasm 沙盒 | 社区贡献 |
| 通信 | WebSocket / NDJSON stdio / MQTT | 跨进程/跨设备 |

---

## 11. 接口开放清单

| 接口 | 类型 | 说明 |
|------|------|------|
| `companion.tool.v1` | MCP | 开发一个工具，Agent 自动调用 |
| `companion.asr.v1` | Rust trait | 实现自定义 ASR（本地/云端） |
| `companion.tts.v1` | Rust trait | 实现自定义 TTS |
| `companion.emotion.v1` | Rust trait | 实现自定义情绪识别 |
| `companion.character.v1` | WebSocket Events | 制作自定义形象前端（任何语言/框架） |
| `companion.prompt.v1` | Config | 自定义对话风格模板 |

---

> **Companion 不是一个应用，而是一个 Agent 生态的底座。**  
> 核心轻到能跑在树莓派上，接口开放到社区能随意扩展，  
> 呈现方式多样到同时支持命令行、Live2D 和 VR，  
> 而这一切，都建立在「解耦 + 标准化接口」之上。
