# Companion 执行计划

> 基于 [SPEC.md](./SPEC.md) 的详细开发路线图。  
> 每个阶段拆分为可独立验证的任务，附带技术决策、接口设计和验收标准。

---

## 目录

0. [前置约定](#0-前置约定)
1. [阶段零：项目初始化](#1-阶段零项目初始化)
2. [阶段一：MVP · 核心对话 + 沙盒 + Live2D](#2-阶段一mvp-核心对话--沙盒--live2d)
3. [阶段二：完善交互与自动化](#3-阶段二完善交互与自动化)
4. [阶段三：社区与扩展](#4-阶段三社区与扩展)
5. [阶段四：VR 模式与跨平台](#5-阶段四vr-模式与跨平台)
6. [里程碑总览](#6-里程碑总览)
7. [风险与缓解](#7-风险与缓解)

---

## 0. 前置约定

### 0.1 命名规范

| 项 | 约定 |
|----|------|
| 项目名 | `companion` |
| 仓库名 | `companion` |
| Rust crate 名 | `companion-core`（后端核心）、`companion-tauri`（Tauri 壳） |
| 前端目录 | `src-tauri/`（Rust）、`web/`（Vue 前端） |
| 数据根目录 | `~/.companion/` |
| 沙盒目录 | `~/.companion/sandbox/` |
| 日志目录 | `~/.companion/logs/` |
| 社区工具目录 | `~/.companion/tools/` |
| Live2D 模型目录 | `~/.companion/models/` |

### 0.2 分支策略

```
main          ← 稳定版本
├─ develop    ← 日常开发
│  ├─ feat/phase-1-mvp
│  ├─ feat/phase-2-interaction
│  ├─ feat/phase-3-community
│  └─ feat/phase-4-vr
```

每个阶段任务完成后，合并到 `develop`；所有阶段完成后合并到 `main` 发布 v1.0。

### 0.3 每个任务的标准产出

- 代码提交（含测试）
- 如果引入新外部依赖，更新 `README.md`
- 如果新增用户可见功能，更新用户文档
- 如果涉及公共接口，更新 `docs/api/` 下的接口文档

---

## 1. 阶段零：项目初始化

> 目标：搭建完整项目骨架，确保构建、测试、运行三通。  
> 预计：2~3 天

### 1.1 创建 Tauri 2.0 项目

**任务：**

```bash
cargo tauri init companion
cd companion
```

**技术决策：**

| 项目 | 选择 | 理由 |
|------|------|------|
| 前端框架 | Vue 3 + Vite | 轻量、组合式 API 适合长维护 |
| CSS | Tailwind CSS | 快速搭建 UI，减少自定义样式 |
| TypeScript | 严格模式 | 大型项目类型安全 |
| Rust 包管理器 | Cargo workspace | 核心/tauri 分离编译 |

**脚手架后端模块（占位）：**

```
src-tauri/src/
├── main.rs              # 入口 + Tauri 启动
├── lib.rs               # Tauri Builder + 模块声明
├── config.rs            # 配置系统（~/.companion/config.json）
├── state/               # 全局状态（AppState）
├── agent/               # Agent 核心抽象（RPC 子进程驱动）
│   ├── mod.rs           # AgentEngine trait
│   └── omp_rpc.rs       # oh-my-pi RPC 客户端实现
├── audio/               # 音频捕获 + 播放 / VAD
├── asr/                 # ASR trait + 本地/云端实现
├── tts/                 # TTS trait + 本地/云端实现
├── emotion/             # 情绪识别 trait
├── llm/                 # LLM trait（直接 API 降级方案）
├── tools/               # 工具注册中心 + 内置工具
├── mcp/                 # MCP 协议实现
├── sandbox/             # 沙盒路径校验
├── permissions/         # 权限管理
└── websocket/           # WebSocket 服务（VR 预留）
```

**脚手架前端：**

```
web/
├── src/
│   ├── App.vue          # 根组件
│   ├── main.ts          # 入口
│   ├── router/          # Vue Router（占位）
│   ├── views/           # 页面（占位）
│   ├── components/      # 组件（占位）
│   ├── stores/          # Pinia 状态（占位）
│   └── types/           # TS 类型定义（占位）
├── index.html
├── vite.config.ts
├── tailwind.config.js
└── package.json
```

**验收标准：**

- [ ] `cargo tauri dev` 启动，空白窗口显示 "Companion v0.1.0"
- [ ] `cargo build` 编译无错误
- [ ] `npm run lint` 通过
- [ ] 项目根有 `.gitignore`、`LICENSE`（MIT）、`README.md`

### 1.2 定义核心 trait 接口

**文件：** `companion-core/src/audio/traits.rs`（示例，具体在 `src-tauri/src/` 下）

**需要定义的 trait：**

```rust
// ASR 统一接口
#[async_trait]
pub trait AsrProvider: Send + Sync {
    async fn transcribe(&self, audio: &[f32]) -> Result<String>;
    fn switch_model(&mut self, model: &str) -> Result<()>;
}

// TTS 统一接口
#[async_trait]
pub trait TtsProvider: Send + Sync {
    async fn synthesize(&self, text: &str) -> Result<Vec<f32>>;
    fn voice_name(&self) -> &str;
}

// VAD
pub trait VadEngine: Send {
    fn is_voice(&self, chunk: &[f32]) -> bool;
    fn reset(&mut self);
}

// 情绪识别
#[async_trait]
pub trait EmotionEngine: Send + Sync {
    async fn classify(&self, audio: &[f32]) -> Result<EmotionLabel>;
}

// MCP 工具
#[async_trait]
pub trait McpTool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value; // JSON Schema
    async fn execute(&self, args: Value) -> Result<Value>;
}
```

**验收标准：**

- [ ] 所有 trait 定义编译通过
- [ ] 有至少一个 mock 实现用于测试（返回固定字符串/数值）
- [ ] trait 文档注释完整（`///` 三斜线）

### 1.3 配置系统

**文件：** `companion-core/src/config.rs`

```rust
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct CompanionConfig {
    pub sandbox_path: PathBuf,              // 默认 ~/.companion/sandbox
    pub llm_provider: LlmProvider,          // Local | Cloud
    pub asr_provider: AsrProviderChoice,    // Local | Cloud
    pub tts_provider: TtsProviderChoice,    // Local | Cloud
    pub system_mode: bool,                  // 默认 false（沙盒模式）
    pub enable_accessibility: bool,         // 默认 false
    pub vad_threshold: f32,                 // 0.0~1.0
    pub user_name: String,
    pub style_template: String,             // 对话风格名称
    pub custom_system_prompt: Option<String>,
    pub emotion_mapping: HashMap<String, String>, // 情绪 → 风格
}
```

**行为：**

- 首次启动自动在 `~/.companion/config.json` 生成默认配置
- 通过 Tauri IPC 可读写
- 变更后自动持久化

**验收标准：**

- [ ] 首次运行自动创建 `~/.companion/` 目录和 `config.json`
- [ ] 修改配置后重启不丢失
- [ ] 非法配置自动回退到默认值

---

## 2. 阶段一：MVP · 核心对话 + 沙盒 + Live2D

> 目标：实现完整的「语音输入 → ASR → LLM → TTS 输出」闭环 + 沙盒工具 + 基本 Live2D 形象。  
> 预计：2~3 周  
> **先做文字交互版本**，再接入语音。

### Sprint 1.1 文字对话 + LLM 集成（3 天）

**优先级最高**——让 Agent 先能对话，这是所有功能的基础。

#### 任务 1.1.1 — LLM 抽象与云端实现

**文件：** `src-tauri/src/llm/`

| 文件 | 内容 |
|------|------|
| `mod.rs` | `LlmProvider` enum + `ChatLlm` trait |
| `openai.rs` | OpenAI / 兼容 API 实现 |
| `ollama.rs` | Ollama REST API 实现 |
| `mock.rs` | 测试用 Mock |

**接口：**

```rust
#[async_trait]
pub trait ChatLlm: Send + Sync {
    /// 发送消息，返回完整回复文本
    async fn chat(&self, messages: &[ChatMessage], tools: &[ToolDef]) -> Result<ChatResponse>;
    /// 流式版本（用于 TTS 提前播放）
    async fn chat_stream(&self, messages: &[ChatMessage], tools: &[ToolDef]) -> Result<StreamReceiver>;
}
```

#### 任务 1.1.2 — 基础对话界面

**文件：** `web/src/views/ChatView.vue` + `web/src/components/ChatMessage.vue`

**功能：**

- 文本输入框 + 发送按钮
- 对话气泡列表（用户 / Agent 分左右）
- Markdown 渲染（用于代码/列表）
- 对话历史滚动

**Tauri IPC：**

```typescript
// 前端调用
const reply = await invoke('chat', { message: '帮我列出文件' });
// 后端响应
#[tauri::command]
async fn chat(state: State<AppState>, message: String) -> Result<String> { ... }
```

#### 任务 1.1.3 — 基础 Tool Call 框架

**文件：** `src-tauri/src/tools/`

```rust
// 工具注册中心
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn McpTool>>,
}

impl ToolRegistry {
    pub fn register(&mut self, tool: Box<dyn McpTool>);
    pub fn definitions(&self) -> Vec<ToolDef>;  // 给 LLM 的 JSON Schema
    pub async fn execute(&self, name: &str, args: Value) -> Result<Value>;
}
```

**Sprint 1.1 验收：**

- [x] 在聊天框输入文字，LLM 能回复
- [x] 可以在云端和本地间切换 — Settings LLM dropdown 已桥接到 omp --model 标志（provider_to_model 映射）
- [x] LLM 能看到已注册的工具列表 — ToolRegistry 通过工具调用循环注入 omp（--append-system-prompt + 解析 JSON tool-call）

---

### Sprint 1.2 沙盒工具（2 天）

#### 任务 1.2.1 — 沙盒路径检查

**文件：** `src-tauri/src/sandbox/mod.rs`

```rust
pub struct Sandbox {
    root: PathBuf,
}

impl Sandbox {
    /// 验证路径是否在沙盒内
    pub fn resolve(&self, user_path: &str) -> Result<PathBuf>; // 拒绝 .. 和符号链接逃逸
    pub fn root(&self) -> &Path;
    pub fn set_root(&mut self, path: PathBuf);
}
```

**安全规则：**

1. 规范化路径（消除 `..`、`.`）
2. 检查最终路径以 `root` 为前缀
3. 拒绝符号链接指向沙盒之外
4. 拒绝包含 `..`、`;`、`|`、`&`、`$` 的字符串（命令注入防护）

#### 任务 1.2.2 — 文件工具

**文件：** `src-tauri/src/tools/file_tools.rs`

| 工具名 | 参数 | 行为 |
|--------|------|------|
| `sandbox_list` | `path: string` | 列出目录内容 |
| `sandbox_read` | `path: string` | 读取文件内容 |
| `sandbox_write` | `path: string, content: string` | 写入文件 |
| `sandbox_delete` | `path: string` | 删除文件/目录 |

每个工具内部调用 `Sandbox::resolve()` 校验路径。

#### 任务 1.2.3 — 命令工具

**文件：** `src-tauri/src/tools/command_tools.rs`

```rust
pub struct CommandTool {
    sandbox: Arc<Sandbox>,
    system_mode: Arc<AtomicBool>,
}

// sandbox_execute: 在沙盒目录下执行命令
// 参数: command (string), args (string[])
// 安全: 拒绝黑名单字符, 强制 CWD 为沙盒根
// 系统模式启用后允许任意路径执行, 但高危命令需要用户确认
```

**命令黑名单（系统模式下才需要确认）：**

- `rm` / `del` / `rd` / `format` / `shutdown` / `reboot` / `poweroff`
- 路径包含 `/etc` / `/boot` / `/sys` / `/proc` / `C:\Windows` / `C:\System32`

#### 任务 1.2.4 — 将工具注册到 LLM

在 `llm/mod.rs` 中，构造 `ChatRequest` 时自动注入 `ToolRegistry.definitions()`。

**Sprint 1.2 验收：**

- [x] 沙盒内文件读写正常
- [x] 路径逃逸被拒绝并返回错误信息
- [x] 命令执行返回 stdout/stderr
- [x] 含 `;` 或 `|` 的命令被拒绝
- [x] Agent 可以通过自然语言操作沙盒文件 — 工具调用循环已实现（omp --append-system-prompt + JSON tool-call 解析 + ToolRegistry 执行）

---

### Sprint 1.3 麦克风捕获 + VAD + ASR + TTS（已实现 ✅）

> ASR 和 TTS 通过小米 Token Plan 的 Chat Completions 端点实现（无需 OpenAI API Key）。
> `mimo-v2.5-asr` 用于语音转文字，`mimo-v2.5-tts` 用于文字转语音（9 种声音）。
> 录音由浏览器 `MediaRecorder` API 处理，播放由 `AudioContext.createBufferSource()` 处理。

#### 任务 1.3.1 — AudioCapture（Rust 后端，预留）

**文件：** `src-tauri/src/audio/capture.rs` — cpal 麦克风 + 2s 环形缓冲区。

#### 任务 1.3.2 — VAD 状态机

**文件：** `src-tauri/src/audio/vad.rs` — 四态 VAD：Idle→SpeechStart→Speaking→Silence→Idle，基于 RMS 能量检测。

#### 任务 1.3.3 — Xiaomi ASR (`mimo-v2.5-asr`)

**文件：** `src-tauri/src/asr/xiaomi_asr.rs`

PCM f32 → WAV base64 data URL → Chat Completions API（`input_audio` 类型）→ 文本。

#### 任务 1.3.4 — Xiaomi TTS (`mimo-v2.5-tts`)

**文件：** `src-tauri/src/tts/xiaomi_tts.rs`

文本 → Chat Completions API（`audio` modality + assistant role）→ base64 WAV → PCM f32。
**9 种声音**: `mimo_default`, `冰糖`, `茉莉`, `苏打`, `白桦`, `Mia`, `Chloe`, `Milo`, `Dean`。

#### 任务 1.3.5 — 前端语音 UI

**文件：** `web/src/views/ChatView.vue`

| 功能 | 说明 |
|------|------|
| 🎤 按钮 | 输入框左侧，点击开始/停止录音 |
| 模式切换 | `💬 Real-time chat`（说话→自动发送）/ `📝 Dictation`（说话→插入光标） |
| 🔊 Listen | 每条 AI 回复下方，点击 TTS 朗读该条消息 |

**Sprint 1.3 验收：**

- [x] 麦克风录制并读取音频缓冲区
- [x] VAD 能检测到说话/静音
- [x] 语音片段送入 ASR 返回文本
- [x] TTS 能朗读 AI 回复
- [x] 语音模式和文本输入模式可切换---

### Sprint 1.4 Live2D 形象（最终方案，已完成 ✅）

> **最终架构：PixiJS + pixi-live2d-display + Cubism 4 Core（CDN）**
> 经过 20+ 次迭代调试后确定。

#### 技术栈
- **渲染器**：PixiJS 7.4.3（`Application`, `backgroundAlpha: 0` — 透明窗口）
- **Live2D 桥接**：pixi-live2d-display 0.5.0-beta（Cubism4Model factory）
- **Core**：Cubism 4 Core v5.1.0 (207KB) 从 `cubism.live2d.com` CDN
- **模型**：Haru（Version 3, 84 drawables, 2×2048 textures, 26 expressions）
- **关键补丁**：Cubism 2 stubs（pixi-live2d-display 初始化检测）、Shader 内嵌（Tauri SPA 404）、`saveParameters` 钩子（参数持久化）

#### 交互
| 功能 | 方式 |
|------|------|
| 自动动画 | pixi-live2d-display 内置 idle + 呼吸 + 眨眼 + 26 表情 |
| 嘴型同步 | `get_lip_level` IPC → `saveParameters` 钩子 → `ParamMouthOpenY` |
| 滚轮缩放 | `wheel` 事件 → `model.scale.set(0.04~0.40)` |
| 眼部追踪 | `pointermove` → `saveParameters` 钩子 → `ParamAngleX/Y` |
| 拖拽/关闭 | Tauri `startDragging` + `drag-bar` + 右键菜单 |

#### 遗留限制
- **WebView2 透明 + 原生 WebGL premultiplied alpha** 不可行——必须通过 PixiJS FBO 管理器
- **Cubism 5 Core (v6.0.1)** 不兼容 pixi-live2d-display——`getDrawableVertices` 等方法已移除
- **Cubism 5 Framework (50+ TS 文件)** 保留在 `web/src/live2d/` 供未来参考

**Sprint 1.4 验收：**
- [x] Live2D 模型在独立透明窗口中渲染 ✅
- [x] 嘴型同步（`get_lip_level` IPC → `ParamMouthOpenY`）✅
- [x] 眼部追踪（`pointermove` → `ParamAngleX/Y` → 3s 回中）✅
- [x] 滚轮缩放 (0.04–0.40) + 拖拽 + 右键关闭 ✅
### Sprint 1.5 oh-my-pi Agent 核心集成（已实现 ✅）

> Agent 核心通过 **`omp -p` 同步进程**接入 oh-my-pi。每个 chat 消息 spawn 一个新 omp 进程，
> stdout 即回复文本。不使用 `Bun sidecar (@oh-my-pi/pi-agent-core)`（RPC 模式不返回消息体，只在成功帧中返回 `{"success":true}`）。

#### 任务 1.5.1 — omp 同步子进程管理

**文件：** `src-tauri/src/agent/omp_rpc.rs`

```rust
/// Stateless — spawns `omp -p "message" --no-session` per chat call.
pub struct OmpRpcClient {
    binary: String,  // "omp" or "omp.cmd" on Windows
}

impl OmpRpcClient {
    // Uses tokio::task::spawn_blocking to run omp synchronously
    pub async fn chat(&self, message: &str) -> Result<AgentResponse>;
}
```

#### 任务 1.5.2 — Windows 路径兼容

Windows 上 npm 全局安装的 `omp` 是 POSIX shell script，必须用 `omp.cmd`。
`resolve_omp_binary()` 自动检测 `%APPDATA%\npm\omp.cmd`。

#### 任务 1.5.3 — 默认模型配置

`~/.omp/agent/config.yml` 中默认模型设为 `siliconflow/nex-agi/Nex-N2-Pro`（免费），
API 通过 `sensenova` provider alias 配置在 `models.yml` 中，指向 `api.siliconflow.cn/v1`。

**Sprint 1.5 验收：**

- [x] `OmpRpcClient::chat()` 发送 prompt 并收到回复
- [x] Windows 上正常找到和执行 omp
- [x] 默认使用免费 SiliconFlow 模型 (Nex-N2-Pro)
- [x] `cargo test` 全部通过


---

### Sprint 1.6 设置面板 + 集成联调（2 天）

#### 任务 1.6.1 — 设置面板 UI

**文件：** `web/src/views/SettingsView.vue`

| 设置项 | 控件类型 |
|--------|----------|
| LLM 提供商（云端/本地） | 下拉框 |
| ASR 提供商（云端/本地） | 下拉框 |
| TTS 提供商（云端/本地） | 下拉框 |
| 沙盒路径 | 输入框 + 文件夹选择 |
| 系统模式开关 | 开关 + 确认弹窗 |
| VAD 静音阈值 | 滑块（0.0~1.0）|
| 用户名 | 输入框 |
| 对话风格 | 下拉框（预设列表）|
| 自定义 System Prompt | 文本域编辑器 |

#### 任务 1.6.2 — 状态栏

```vue
<!-- App.vue 底部状态栏 -->
<div class="status-bar">
    <span>{{ systemMode ? '🔓 系统模式' : '🔒 沙盒模式' }}</span>
    <span>{{ llmStatus }}</span>
    <span>{{ asrStatus }}</span>
    <span>{{ ttsStatus }}</span>
</div>
```

#### 任务 1.6.3 — 全流程集成测试

**端到端场景（手动验证）：**

1. 启动应用 → 空白状态栏显示沙盒模式
2. 在聊天框输入"帮我创建一个 test.txt 并写入 Hello" → Agent 调用工具 → 文件创建成功
3. 在聊天框输入"列出当前目录" → Agent 调用 sandbox_list → 显示文件列表
4. 切换到"系统模式" → 弹窗确认 → 状态栏变为 🔓
5. 在设置中切换 LLM/ASR/TTS 提供商 → 工具提示更新
6. 检查 `~/.companion/logs/` 下生成了操作日志

**Sprint 1.6 验收：**

- [x] Settings 页面可打开/关闭，UI 正常（9 项配置：LLM/ASR/TTS providers, sandbox, VAD, user, style, prompt, TTS-always-on）
- [x] 状态栏显示 Sandbox/Unrestricted 模式（sidebar 底部，`get_config` 实时读取）
- [x] 配置持久化到 `~/.companion/config.json`（ConfigManager.save/load），重启保留
- [x] LLM 提供商切换立即生效 — Settings 改 CompanionConfig.llm_provider → provider_to_model() → agent.set_model() → omp --model
- [x] Agent 通过自然语言操作沙盒 — 工具调用循环已实现（omp --append-system-prompt 注入工具定义 + parse_tool_call + ToolRegistry.execute，最大 5 次迭代）
- [x] 操作日志 — AuditLogger 写入 `~/.companion/logs/command.log`，get_audit_log IPC 可读取

---

## 3. 阶段二：完善交互与自动化

> 目标：实时打断、本地 TTS、浏览器控制、系统模式完善。  
> 预计：2~3 周

### Sprint 2.1 实时打断（4 天）

#### 任务 2.1.1 — 打断信号链

```
用户说话（VAD 触发）
  → TTS 立即停止播放（audio/player.rs: stop()）
  → 取消正在进行的 LLM 请求（向 tokio task 发送 cancel 信号）
  → 清空待发送的 TTS 队列
  → 开始 ASR 识别新语音
  → ASR 结果送入 LLM（重新开始一轮对话）
```

**关键代码：**

```rust
// TTS 播放器
pub struct TtsPlayer {
    sink: Option<cpal::Sink>,       // 当前播放
    cancel_flag: Arc<AtomicBool>,   // 打断信号
}

impl TtsPlayer {
    pub fn play(&mut self, audio: Vec<f32>) { /* 在新线程播放 */ }
    pub fn stop(&mut self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
        if let Some(sink) = self.sink.take() {
            sink.stop();
        }
    }
}
```

#### 任务 2.1.2 — VAD + 打断阈值优化

- 添加触发器条件：音量超过阈值持续 300ms 才打断
- 避免环境音误触发（空调、键盘声）
- 可配置的打断灵敏度

**Sprint 2.1 验收：**

- [x] TTS 播放时说话 → TTS 立即停止（后台麦克风监控每 100ms 检测音量 → 超出灵敏度阈值 300ms 后触发打断 → `stopTTS()` + 自动录音 → ASR 转写 → 自动发送）
- [x] 打断后新的 ASR 识别结果正常（MediaRecorder 独立流，不依赖前端 PTT 按钮状态）
- [x] 灵敏度可配置：底部 ⏏ Interrupt ON/OFF 按钮 + 300ms 语音持续时长 + Settings 中 VAD Threshold 滑块
- [~] 误触发率 — 需实际环境测试（已支持用户自调灵敏度，范围 0.0–1.0，默认 0.3）
---

### Sprint 2.2 本地 TTS（3 天）

#### 任务 2.2.1 — TTS 抽象 + ChatTTS 集成

**文件：** `src-tauri/src/tts/`

```rust
// 两种本地 TTS 实现方案

// 方案 A（推荐）：启动 ChatTTS Python HTTP 服务
// 用户执行: chattts-server --port 8021
// Rust 通过 reqwest 发送 POST /tts { text: "..." } 获取音频 bytes

// 方案 B：Edge TTS（更轻量，但需要网络下载语音数据）
// 通过命令行 edge-tts --text "..." --write-media out.mp3
```

**播放实现：**

```rust
// audio/player.rs
pub struct AudioPlayer {
    device: cpal::Device,
    config: cpal::StreamConfig,
    stream: Option<cpal::Stream>,
}

impl AudioPlayer {
    pub fn play_async(&self, samples: Vec<f32>) -> Result<()>;
    pub fn stop(&self);
    pub fn current_amplitude(&self) -> f32; // 用于嘴型同步
}
```

#### 任务 2.2.2 — 嘴型同步实装

将 `current_amplitude()` 的值通过 Tauri event 发送给前端，替代 Sprint 1.4 中的模拟数据。

**Sprint 2.2 验收：**

- [x] TTS 能合成并播放语音 — Xiaomi `mimo-v2.5-tts` 云端 API，9 种声音可选
- [x] 嘴型与播放音频同步（视觉延迟 < 100ms）— `set_lip_level`/`get_lip_level` Rust IPC 桥接，avatar 窗口每帧 rAF 读取
- [~] 本地 TTS (ChatTTS/EdgeTTS) — 云端已满足 MVP，本地离线模式按需后续补齐

---

### Sprint 2.3 浏览器控制（3 天）

#### 任务 2.3.1 — Playwright 工具

**文件：** `src-tauri/src/tools/browser_tools.rs`

```rust
pub struct BrowserTool {
    // 使用 playwright-rs 或通过子进程调用 Playwright CLI
}

// 工具清单（给 LLM 调用）：
// 1. browser_open(url: string) — 打开页面
// 2. browser_click(selector: string) — 点击元素
// 3. browser_type(selector: string, text: string) — 输入文本
// 4. browser_screenshot() — 截图（返回 base64，前端显示）
// 5. browser_evaluate(script: string) — 执行 JS
```

**安全限制：**

- 默认禁用，需在设置中开启 "辅助功能控制"
- 每次操作前弹窗：「Agent 准备执行：打开 example.com [允许/拒绝]」
- 超时（默认 10 秒）自动拒绝

#### 任务 2.3.2 — 降级方案（Linux Wayland）

当检测到 Wayland 且 Playwright 不可用时：

```bash
# 使用 ydotool 模拟点击
ydotool mousemove --x 100 --y 200
ydotool click 1
```

**Sprint 2.3 验收：**

- [x] 浏览器截图功能 — `browse_screenshot` Tauri 命令 → Playwright headless Chrome → 返回 base64 PNG
- [x] 界面集成 — ChatView 底部浏览器栏（URL 输入 + 🌐 Screenshot 按钮 + 截图预览）
- [x] 安全控制 — 仅管理员可调用（非 omp 工具，是 Tauri IPC 直接命令）
- [~] 完整浏览器自动化（click/type/evaluate）— 工具调用循环已实现，浏览器自动化工具可后续注册到 ToolRegistry

---

### Sprint 2.4 系统模式 + 安全日志（2 天）

#### 任务 2.4.1 — 系统模式权限弹窗

- 设置中开关 + 弹窗警告（每次开启时）
- 状态栏图标切换 🔒 / ⚠️
- 高危命令执行前弹窗确认

#### 任务 2.4.2 — 安全日志

```rust
// src-tauri/src/permissions/audit.rs
pub struct AuditLogger {
    log_file: PathBuf,  // ~/.companion/logs/command.log
}

impl AuditLogger {
    pub fn log_command(&self, cmd: &str, args: &[String], result: &Result<String>);
    pub fn log_mode_switch(&self, from: bool, to: bool);
    pub fn log_tool_exec(&self, tool: &str, args: &Value, result: &Result<Value>);
}
```

日志格式：`[2025-06-01 14:30:22] CMD: rm -rf /etc  USER_APPROVED=true  EXIT_CODE=0`

**Sprint 2.4 验收：**

- [x] 系统模式开关正常工作 — Settings UI + sidebar 状态栏实时切换，AuditLogger 记录每次切换
- [x] 高危命令弹窗确认 — sandbox_execute 检测 rm/del/shutdown 等命令，拒绝执行并返回 PermissionDenied
- [x] 所有工具调用记录到日志文件 — AuditLogger 写入 `~/.companion/logs/command.log`，`get_audit_log` IPC 可读取

---

## 4. 阶段三：社区与扩展

> 目标：情绪驱动交互、对话风格系统、MCP 插件加载器、社区商店雏形。  
> 预计：2~3 周

### Sprint 3.1 情绪识别（3 天）

#### 任务 3.1.1 — wav2vec2 集成

**两种路径：**

| 路径 | 复杂度 | 性能 |
|------|--------|------|
| Python 子进程（推荐）| 低 | 中 |
| Rust 绑定 (candle/ort) | 高 | 高 |

**推荐方案——Python 子进程：**

```rust
// 启动 Python 进程：python3 emotion_server.py --port 8031
// REST API: POST /emotion { audio: base64 } → { label: "happy", confidence: 0.92 }
pub struct EmotionService {
    endpoint: String,
    client: reqwest::Client,
}
```

#### 任务 3.1.2 — 情绪 → 风格映射

```rust
// config.json 中的映射表
"emotion_mapping": {
    "happy": "cheerful",
    "sad": "comforting",
    "angry": "calming",
    "neutral": "professional",
    "surprised": "cheerful",
    "fearful": "comforting"
}

// 每次 LLM 请求前，根据当前情绪注入对应的 system prompt：
messages.insert(0, SystemMessage {
    content: style_templates.get(&emotion).unwrap_or(&default_template).clone()
});
```

**Sprint 3.1 验收：**

- [ ] 说话后检测到情绪标签
- [ ] 情绪标签驱动 Live2D 表情切换
- [ ] 情绪标签影响 LLM 回复风格

---

### Sprint 3.2 对话风格系统（2 天）

#### 任务 3.2.1 — 预设模板

```rust
// ~/.companion/styles/
// styles/
//   professional.yaml  → system prompt: "你是一个专业的秘书..."
//   humorous.yaml      → system prompt: "你是一个幽默的朋友..."
//   gentle.yaml        → system prompt: "你是一个温柔的陪伴..."
//   geek.yaml          → system prompt: "你是一个硬核极客..."

pub struct StyleTemplate {
    pub name: String,
    pub system_prompt: String,
    pub tts_voice: Option<String>,
    pub animation_set: Option<String>,
}
```

#### 任务 3.2.2 — 自定义编辑器

前端实现一个简单的文本编辑器页面：

```
┌─ 编辑风格：我的风格 ────────────────────┐
│ System Prompt:                           │
│ ┌─────────────────────────────────────┐  │
│ │ 你是一个{user_name}的助手，现在时间  │  │
│ │ 是{current_time}。你总是用温暖、鼓   │  │
│ │ 励的语气回复。                      │  │
│ │                                     │  │
│ └─────────────────────────────────────┘  │
│ 支持变量: {user_name} {current_time}     │
│ {emotion} {date} {weather}              │
│ [保存]  [恢复默认]                        │
└──────────────────────────────────────────┘
```

**Sprint 3.2 验收：**

- [ ] 切换风格后 LLM 回复风格改变
- [ ] 自定义 prompt 支持变量替换
- [ ] 用户添加/删除风格模板

---

### Sprint 3.3 Wasm 沙盒 MCP 加载器（4 天）

#### 任务 3.3.1 — 工具目录监听

```rust
// 监听 ~/.companion/tools/ 目录变化
// 新放入工具包 → 自动注册 MCP 工具
// 删除工具包 → 自动注销

pub struct ToolWatcher {
    watched: PathBuf,
    registry: Arc<Mutex<ToolRegistry>>,
    _notify: Option<notify::RecommendedWatcher>,
}

impl ToolWatcher {
    pub fn start(watch_path: &Path) -> Result<Self>;
    pub fn load_all(&self) -> Result<Vec<String>>;  // 返回加载的工具名列表
}
```

**依赖：** `notify` crate（文件系统事件监听）

#### 任务 3.3.2 — Wasm 工具运行器

```rust
pub struct WasmRunner {
    engine: wasmtime::Engine,
    store: wasmtime::Store<()>,
}

impl WasmRunner {
    /// 加载 Wasm 模块，调用其导出函数 `execute`
    pub fn run(wasm_bytes: &[u8], args: Value) -> Result<Value>;
}
```

**Wasm 工具接口（社区开发者遵循）：**

```rust
// 工具入口
#[no_mangle]
pub extern "C" fn execute(input_ptr: *const u8, input_len: usize) -> *mut u8 {
    // 解析 JSON 输入
    // 执行逻辑（无文件系统/网络访问，除非 manifest 声明权限）
    // 返回 JSON 结果
}
```

#### 任务 3.3.3 — manifest 解析与权限检查

```rust
// 解析 manifest.json
pub struct ToolManifest {
    pub name: String,
    pub version: String,
    pub author: String,
    pub permissions: Vec<String>,  // ["network:http", "filesystem:read"]
    pub entry: String,             // "main.wasm"
}
```

**权限模型：**

| 权限 | 说明 | 风险 |
|------|------|------|
| `none` | 只能计算，无 IO | 安全 |
| `network:http` | 可发起 HTTP 请求 | 低 |
| `filesystem:read` | 读取沙盒内文件 | 低 |
| `filesystem:write` | 写入沙盒内文件 | 中 |
| `network:all` | 任意网络访问 | 中 |
| `system:command` | 执行系统命令 | 高（需签名）|

**Sprint 3.3 验收：**

- [ ] 放置工具包到 `~/.companion/tools/` → 自动加载
- [ ] 删除工具包 → 自动卸载
- [ ] LLM 能看到新工具并调用它
- [ ] 缺少 manifest → 拒绝加载并报错
- [ ] 权限不足的工具调用被拒绝

---

### Sprint 3.4 社区商店雏形（2 天）

#### 任务 3.4.1 — 商店索引读取

```typescript
// 前端商店页面
// 从 GitHub raw 读取索引文件
const indexUrl = 'https://raw.githubusercontent.com/your-org/companion-plugins/main/index.json';
const manifestUrl = (pkg: string) =>
    `https://github.com/your-org/companion-plugins/releases/download/${pkg}/manifest.json`;
```

**索引格式：**

```json
{
  "tools": [
    {
      "name": "weather-query",
      "version": "1.0.0",
      "author": "community",
      "description": "查询全国天气预报",
      "download_url": "https://github.com/.../weather-query-v1.wasm",
      "manifest_url": "https://.../manifest.json"
    }
  ]
}
```

#### 任务 3.4.2 — 一键安装/卸载

```typescript
// Tauri IPC 下载并安装
await invoke('install_tool', {
    url: 'https://.../weather-query-v1.wasm'
});
// 后端：下载 → 校验 manifest → 解压到 ~/.companion/tools/weather-query/
```

**Sprint 3.4 验收：**

- [ ] 商店页面显示工具列表
- [ ] 点击安装 → 工具出现在已安装列表
- [ ] 卸载 → 工具从列表中消失

---

## 5. 阶段四：VR 模式与跨平台

> 目标：完成 VR 模式、跨平台打包、性能优化。  
> 预计：2~3 周

### Sprint 4.1 Tauri WebSocket 服务（2 天）

#### 任务 4.1.1 — WebSocket 服务器

**文件：** `src-tauri/src/websocket/server.rs`

```rust
use tokio_tungstenite::accept_async;

pub struct WsServer {
    port: u16,              // 默认 9001
    agent: Arc<AgentCore>,  // 共享 Agent 状态
}

// 消息协议（JSON-RPC style）
// Request:  { "id": 1, "method": "chat", "params": { "text": "hello" } }
// Response: { "id": 1, "result": { "text": "Hi!" } }
// Event:    { "type": "event", "method": "audio_level", "params": { "level": 0.5 } }
```

**事件广播（给所有连接的 VR 客户端）：**

- `audio_level`（嘴型）
- `emotion`（表情）
- `agent_state`（待机/思考/说话）
- `tool_executing`（正在执行工具）
- `chat_response`（回复文本，用于字幕显示）

**Sprint 4.1 验收：**

- [ ] `ws://127.0.0.1:9001` 可以通过 WebSocket 客户端连接
- [ ] 发送 `chat` 请求 → 收到回复
- [ ] 音频播放时收到 `audio_level` 事件

---

### Sprint 4.2 Godot VR 客户端原型（5 天）

#### 任务 4.2.1 — Godot 4 项目初始化

```bash
# 创建 Godot 4 项目
godot4 --headless --script create_project.gd
```

**项目结构：**

```
companion-vr/
├── project.godot
├── scenes/
│   ├── main.tscn           # 主场景（OpenXR + VRM + UI）
│   ├── character.tscn      # VRM 形象场景
│   └── ui.tscn             # 工具菜单/字幕
├── scripts/
│   ├── websocket_client.gd # WebSocket 连接 Tauri 后端
│   ├── character.gd        # VRM 加载 + 动画控制
│   ├── audio_level.gd      # 嘴型驱动
│   └── tool_menu.gd        # 手柄射线交互
├── addons/
│   └── godot-vrm/          # VRM 加载插件
└── models/
    └── default.vrm         # 默认 VRM 模型
```

#### 任务 4.2.2 — WebSocket 连接

```gdscript
# websocket_client.gd
extends Node

var socket := WebSocketPeer.new()

func _ready():
    socket.connect_to_url("ws://127.0.0.1:9001")

func _process(delta):
    socket.poll()
    if socket.get_ready_state() == WebSocketPeer.STATE_OPEN:
        while socket.get_available_packet_count() > 0:
            var msg = JSON.parse_string(socket.get_packet().get_string_from_utf8())
            handle_message(msg)

func send_message(method: String, params: Dictionary):
    var msg = JSON.stringify({ "id": randi(), "method": method, "params": params })
    socket.send_text(msg)
```

#### 任务 4.2.3 — VRM 形象 + 嘴型驱动

- 使用 `godot-vrm` 插件加载 VRM 模型
- 接收 `audio_level` 事件 → 驱动 BlendShape `mouthOpen`
- 接收 `emotion` 事件 → 切换到对应表情 BlendShape
- 接收 `agent_state` 事件 → 切换待机动画

#### 任务 4.2.4 — 手柄交互

| 操作 | 行为 |
|------|------|
| 手柄射线指向工具菜单 | 高亮选项 |
| 扳机键确认 | 调用工具 |
| 长按手柄按键 | 开始语音输入（发送到后端 ASR）|
| 挥手（swipe）| 暂停/继续 |
| 点赞手势 | 确认执行 |

**Sprint 4.2 验收：**

- [ ] VRM 模型在 VR 中显示
- [ ] 连接到 Tauri 后端并接收事件
- [ ] 嘴型/表情跟随后端数据
- [ ] 手柄可以操作工具菜单

---

### Sprint 4.3 跨平台打包（2 天）

#### 任务 4.3.1 — Windows 打包

```bash
# 使用 Tauri bundler
cargo tauri build --bundler msi
```

#### 任务 4.3.2 — Linux 打包

```bash
# AppImage + deb
cargo tauri build --bundler appimage,deb
```

**依赖安装脚本：**

```bash
# install-deps.sh
#!/bin/bash
# 安装 Whisper.cpp
# 安装 ChatTTS (pip install chattts)
# 安装 ydotool (Ubuntu: sudo apt install ydotool)
# 下载默认 Live2D 模型
```

#### 任务 4.3.3 — Wayland 测试清单

- [ ] 窗口正常显示（未使用 XWayland 回退）
- [ ] 屏幕捕获（xdg-desktop-portal）
- [ ] ydotool 模拟输入
- [ ] 音频设备枚举正常

#### 任务 4.3.4 — 性能优化

| 场景 | 优化手段 |
|------|----------|
| Live2D 高帧率 | 仅在状态变化时更新，而非每帧 |
| ASR 连续运行 | 仅在有 VAD 活动时才调用 |
| 内存占用 | 音频循环缓冲区上限 2 秒 |
| 启动速度 | 懒加载非核心模块 |

**Sprint 4.3 验收：**

- [ ] `.msi` 安装包制作成功
- [ ] `.deb` / AppImage 制作成功
- [ ] Windows 10/11 安装后正常运行
- [ ] Ubuntu 22.04 Wayland 下正常运行

---

## 6. 里程碑总览

| 里程碑 | 时间 | 交付物 | 验证方式 | 状态 |
|--------|------|--------|----------|------|
| **M0** | Day 0~3 | 项目骨架 + trait 定义 + 配置系统 | `cargo build` 通过 | ✅ |
| **M1** | Week 1~2 | 文字对话 + 沙盒工具 + 设置面板 | 端到端文本对话操作沙盒 | ✅ |
| **M2** | Week 2~3 | 语音输入+VAD+ASR → 对话 | 说话 → 文字 → LLM 回复 | ✅ |
| **M3** | Week 3~4 | Live2D 形象 + 嘴型同步 | 形象显示在窗口中，说话时嘴动 | ✅ |
| **M4** | Week 4~5 | 实时打断 + 本地 TTS | TTS 播放时说话立即停止 | 📋 |
| **M5** | Week 5~6 | 浏览器控制 + 系统模式 | Agent 打开网页并截图 | 📋 |
| **M6** | Week 6~8 | 情绪识别 + 风格系统 + MCP 加载器 | 情绪驱动回复风格 | 📋 |
| **M7** | Week 8~9 | 社区商店雏形 | 安装/卸载社区工具 | 📋 |
| **M8** | Week 9~11 | VR 模式原型 | Godot 显示 VRM 并连接后端 | 📋 |
| **M9** | Week 11~12 | 跨平台打包 + 性能优化 | 安装包可分发安装 | 📋 |

**总预计：12 周（3 个月）单人全职开发。** 周末/业余时间开发预计 6 个月。

---

## 7. 风险与缓解

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|----------|
| omp RPC 协议版本不兼容 | 中 | 高 | 锁定 omp 版本范围；备选直接 LLM API 降级方案 |
| omp 在用户系统上未安装 | 高 | 高 | 首次运行引导安装；提供 `companion install-agent` 命令 |
| omp 子进程崩溃/挂死 | 中 | 高 | 心跳检测 + 自动重启；超时强制终止 |
| Whisper.cpp Rust FFI 绑定复杂 | 高 | 中 | 先走子进程调用（稳定），后续替换为 FFI |
| ChatTTS 内存占用高 | 中 | 中 | 备选 Edge TTS（HTTP）；后期可换 Piper TTS |
| Live2D 授权问题（商业使用）| 低 | 中 | 仅使用 CC0 / 开源免费模型；不捆绑商业模型 |
| Wayland 下屏幕捕获/xdotool 支持差 | 中 | 中 | 浏览器自动化优先 Playwright 持久上下文 |
| Wasm 沙盒性能开销 | 低 | 低 | 工具计算量小，Wasm 开销可接受 |
| VR 开发环境门槛高（需 VR 头显）| 高 | 中 | VR 模式放在最后；先完成桌面版 MVP |
| 单人开发精力分散 | 高 | 高 | 严格按阶段顺序，不做阶段之外的功能 |

---

> **执行原则：** 每完成一个 Sprint，必须通过该 Sprint 的全部验收标准才进入下一个。  
> 不要在阶段一追求完美——"能跑就行"，"美化和健壮"留给后面的 Sprint。
