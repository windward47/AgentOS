# Configuration

一切配置在 `~/.companion/config.json`。

## 关键字段

```json
{
  "llm": {
    "provider": "xiaomi",
    "model": "mimo-v2.5",
    "url": "https://token-plan-cn.xiaomimimo.com/v1",
    "key": "sk-..."
  },
  "default_api_key": "",
  "custom_system_prompt": "You are Companion...",
  "system_mode": false,
  "sandbox_path": "~/.companion/sandbox",
  "tts_auto_play": false,
  "tts_voice": "茉莉",
  "tts_speed": 1.0
}
```

## Settings 同步

1. Settings UI 改 → `updateConfig()` → Rust IPC → sidecar RPC → 写文件 + 重建 Agent
2. LLM 配置变更：sidecar `updateCompanionConfig` 检测到 `partial.llm` 后重建 Model + recreate Agent
3. System Prompt/Sandbox/System Mode 变更同样触发重建

## Model 构建

AgentManager 构造函数从 `llm.*` 字段构建 `Model<Api>`：
- `provider` + `model` → identifier
- `url` → base URL（默认 `https://api.siliconflow.cn/v1`）
- `key` → API key（fallback: `default_api_key`）
- 统一使用 `api: "openai-completions"`

任何 OpenAI 兼容 API 均可接入。Settings 改即生效，无需重启。
