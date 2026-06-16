# Companion TUI — 实现计划

> 基于 SPEC.md §2.1 纯语音模式规格  
> 设计日期：2026-06-16

## 设计决策摘要

| 决策 | 选择 | 理由 |
|------|------|------|
| 运行模式 | 独立二进制 `companion-tui` | 零 Tauri/WebView 依赖，最轻量 |
| 参考项目 | [ostt](https://github.com/kristoferlund/ostt) (MIT) | 最成熟的 Rust TUI+语音项目 |
| TUI 框架 | ratatui + crossterm | 最主流，ostt 同款 |
| 界面布局 | 聊天流模式（状态栏+消息列表+输入栏） | 消息可滚动，打字机效果 |
| 语音触发 | 全局热键 PTT | 复用现有 hotkey 模块 |
| Agent 通信 | 复用 Bun sidecar NDJSON-RPC | 和 Tauri 版一致 |
| TTS | v1 纯文本，不加语音播放 | 先跑通对话 |
| 配置 | 共享 `~/.companion/config.json` | 和 Tauri 版互通 |

## 项目结构

```
Cargo workspace (已有 Cargo.toml 加新成员)
├── companion-core/          (已有，不变)
├── companion-tauri/         (已有，不变)
└── companion-tui/           ★ 新增
    ├── Cargo.toml
    └── src/
        ├── main.rs           → CLI 解析 (clap) + 启动分流
        ├── app.rs            → TUI 事件循环 (ratatui + tokio)
        ├── ui/
        │   ├── mod.rs
        │   ├── chat.rs       → 消息列表渲染 + 滚动
        │   ├── input.rs      → 文本输入框 + 录音状态图标
        │   └── status.rs     → 状态栏 (idle/listening/thinking)
        └── voice.rs          → 热键监听 → CaptureHandle.start/stop → ASR
```

## TUI 布局

```
┌───────────────────────────────────────────┐
│  Companion TUI v0.1.0       [● idle     ] │  ← 状态栏
│───────────────────────────────────────────│
│                                           │
│  You: 今天天气怎么样？                     │  ← 聊天流 (可滚动)
│  AI:  今天北京晴，25°C，适合出门...       │
│                                           │
│───────────────────────────────────────────│
│  > ▌                              [🎤]   │  ← 输入栏 + 录音状态
└───────────────────────────────────────────┘
```

## 数据流

```
热键按下  → CaptureHandle::start()     [状态: listening]
热键松开  → CaptureHandle::stop()      [状态: processing]
          → i16→f32 → ASR trait
          → sidecar agent.prompt()
          → 流式 token → UI 实时追加  [状态: thinking→idle]
```

## CLI 接口

```
companion-tui [OPTIONS]

OPTIONS:
  --stdin            从 stdin 读取文本输入（管道模式）
  --stdout           回复直接输出到 stdout（管道模式）
  -c, --config PATH  配置文件路径
  --log-level LEVEL  日志级别
```

三种模式：
- **交互 TUI**: `companion-tui`（默认）
- **管道**: `echo "你好" | companion-tui --stdin --stdout`
- **后台**: `companion-tui &`（纯热键语音）

## 实施步骤

1. 创建 `companion-tui/` crate，注册到 workspace
2. `main.rs`：clap CLI 解析 + 模式分流
3. `voice.rs`：热键监听 → 麦克风采集 → ASR
4. 搭建 sidecar 通信（复用 OmpAgentSidecar）
5. `ui/`：ratatui 聊天界面
6. `app.rs`：事件循环串联（键盘 + 热键 + sidecar 流式）
7. 集成测试 + 手动验证
