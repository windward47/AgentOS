# Lessons Learned

每一条都是用时间换的。做技术选型前先过一遍。

1. **第三方 SDK：先验证再集成**。Live2D 的 pixi-live2d-display 和当前 pixi.js 生态断裂，白费数小时。正确做法：找已确认能工作的参考，npm build 立即验证。

2. **协议假设必须手工验证**。omp RPC 帧只返回 `{"success":true}`，消息不走帧。没验证就写了整套 NDJSON 解析，全废。

3. **Windows npm 全局包需要 `.cmd` 后缀**。`omp` 是 POSIX sh，Windows 必须 `omp.cmd`。

4. **Tauri 多窗口是 Live2D 正确容器**。Live2D 不和聊天挤同一组件，独立 transparent 窗口。

5. **Playwright 必须**。在实际用户反馈前多次发现 Bug。每次提交前跑 `npm run test:ui`。

6. **非标准 API 端点必须 curl 实测**。小米 ASR/TTS 不是 `/v1/audio/*`，是 `/v1/chat/completions`。

7. **Domain State 非 God Object**。按领域拆分 state，Tauri command 只取需要的。

8. **单 crate → workspace**。核心逻辑独立，零 Tauri 依赖，可独立测试。

9. **Agent 工具优先复用 omp**。pi-coding-agent 有 30+ 现成工具，不重复造。

10. **管道不如 HTTP**。stdin/stdout 死锁、WS 连接管理复杂，最终 HTTP+SSE 最稳定。
