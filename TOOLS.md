# Tools

Agent 通过 pi-agent-core 的 Tool 接口调用外部能力。

## 沙箱模式（system_mode: false）

| 工具 | 说明 | 范围 |
|---|---|---|
| `sandbox_list/read/write/delete/execute` | 文件操作 | `~/.companion/sandbox/` |
| `web_search` | DuckDuckGo + Bing fallback | 互联网 |
| `web_fetch` | 读取 URL 内容 | 互联网 |
| `memory_retain` | 记住重要信息 | 本地 Mnemopi |
| `get_user_paths` | 获取桌面/文档等路径 | 本地 |

## 非沙箱模式（system_mode: true）

沙箱模式所有工具 + 以下全盘工具：

| 工具 | 说明 |
|---|---|
| `read` | 读取任意文件 |
| `write` | 写入任意文件 |
| `search` | 文本搜索 (grep) |
| `find` | glob 文件查找 |
| `bash` | 执行任意命令 |
| `get_user_paths` | 获取 home/desktop/documents 路径 |

## 如何加新工具

1. 在 `services/agent-sidecar/src/agent.ts` 定义 `AgentTool` 对象
2. 在沙箱/非沙箱对应分支 push 到 `tools` 数组
3. 如需 LLM 感知，更新 `custom_system_prompt` 描述
