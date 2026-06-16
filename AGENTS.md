# Companion

跨平台桌面智能 Agent — Tauri + Vue 3 + Bun sidecar (oh-my-pi)。

## 快速开始

```sh
cargo tauri dev
```

## 文件地图

| 文件 | 何时看 | 内容 |
|---|---|---|
| `ARCHITECTURE.md` | 理解整体架构、加新模块 | 架构图、IPC 命令表、编码规范 |
| `CONFIG.md` | 修改配置、加新配置项 | `~/.companion/config.json`、Settings 同步 |
| `TOOLS.md` | 加/改 Agent 工具 | 工具清单、沙箱模式、注册方法 |
| `DEBUGGING.md` | 排查问题 | 日志位置、常见问题、调试 |
| `LESSONS.md` | 技术选型前 | 10 条踩坑 |
| `TASKS/` | 短期任务 | 每个文件一个任务，完成即删 |
| `PLAN.md` | 看 roadmap | 长期规划 |

## 工作流

1. **新功能**：讨论因果链 → SPEC → 实现 → 自测 → 更新 TASKS
2. **修 Bug**：复现 → 定位 → 修复 → 验证
3. **任务跟踪**：`TASKS/NNN-title.md`，完成更新 PLAN.md

## AI Agent 风格

1. 新功能先讨论因果链，明确需求再落地方案
2. 维护/删除先沟通目的，评估影响范围
3. Bug 先复现再分析，有分歧反馈用户
4. SPEC 明确因果链 + 边界 + 禁止什么
5. 新功能自测通过后，询问是否需要 SPEC/harness
