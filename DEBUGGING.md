# Debugging

## 关键日志

| 位置 | 怎么看 |
|---|---|
| Tauri 终端 | `cargo tauri dev` 输出 — `chat_stream:` 前缀是 Rust 层日志 |
| Sidecar stderr | 自动出现在终端（`Stdio::inherit()`）— `[sidecar]` 前缀 |
| Bun.serve 超时 | `[Bun.serve]: request timed out` → `idleTimeout: 120` 已设 |

## 常见问题

### `⚠️ Empty response`

1. Settings → Detect Models → 确认 ✅ models found
2. 不能用 `mimo-v2.5-pro`（reasoning 模型占满 token）
3. 检查 `custom_system_prompt` 没有不当限制

### `⚠️ Stream error` / `⚠️ HTTP error`

1. Sidecar 没有 `Listening on http://127.0.0.1:9876` → 端口被占或启动失败
2. `taskkill /f /im bun.exe` 杀残留后重启

### 文字/TTS 重复

`import()` 被多次调用导致多个 `chat_token` 监听器。已加 `_l` 守卫。

### 沙箱不切换

Settings → System Mode 改后需等 2 秒轮询。左下角状态指示应变为橙色 "Unrestricted"。

## curl 直调 Sidecar

```sh
# 测试 /rpc
curl -X POST http://127.0.0.1:9876/rpc -H 'Content-Type: application/json' \
  -d '{"id":"t1","method":"ping"}'

# 测试流式
curl -N -X POST http://127.0.0.1:9876/chat_stream \
  -H 'Content-Type: application/json' \
  -d '{"id":"t1","method":"chat_stream","params":{"message":"hi","history":[]}}'
```
