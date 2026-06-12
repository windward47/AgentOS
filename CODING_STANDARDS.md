# Companion 代码编写规范

> 保证框架结构的长期稳定开发。  
> 所有新增代码应有理有据，遵循本文档。

---

## 1. 目录结构与模块职责

### 1.1 Cargo Workspace 分层

```
Cargo.toml (workspace root)
├── companion-core/   ← 纯逻辑库，零 Tauri 依赖
│   ├── Cargo.toml    ← 只含逻辑依赖（serde, tokio, cpal, reqwest, ...）
│   ├── tests/        ← 端到端测试
│   └── src/
│       ├── lib.rs    ← pub mod ...  re-export 所有模块
│       ├── agent/    ← AgentEngine trait + OmpAgentSidecar
│       ├── asr/      ← AsrProvider trait + 本地/云端实现
│       ├── ...       ← 其余 13 个模块
│
└── companion-tauri/  ← Tauri 桌面壳
    ├── Cargo.toml    ← 依赖 companion-core + tauri + 必要工具
    ├── tauri.conf.json / icons / capabilities
    └── src/
        ├── main.rs          ← 入口
        ├── lib.rs           ← Tauri::Builder (~160行)
        ├── state/mod.rs     ← 领域状态 + IPC 命令
        └── voice_handler.rs ← 全局语音逻辑
```

**规则：**
- `companion-core` **永不依赖** `tauri`、`tauri-plugin-*` 或任何与窗口/UI 相关的 crate
- `companion-tauri` **永不包含**业务逻辑实现；它负责状态管理、命令分发、窗口/托盘设置
- 跨 crate 引用统一用 `companion_core::module::...`

### 1.2 模块内部结构

每个功能模块遵循以下结构：

```
module/
├── mod.rs           ← trait 定义 + Error enum + 可选公共类型
├── impl_a.rs        ← 实现 A（如 xiaomi_asr.rs）
├── impl_b.rs        ← 实现 B（如 whisper_cloud.rs）
└── mock.rs          ← 测试用 Mock 实现
```

**规则：**
- `mod.rs` 定义 trait + `thiserror` Error enum + `Send + Sync` 约束
- 实现文件放在兄弟文件中，不得在 `mod.rs` 中写具体实现
- `mock.rs` 在每个模块中提供 Mock 实现，用于单元测试
- 测试写在每个文件内部的 `#[cfg(test)] mod tests { ... }`

---

## 2. Domain State 模式

### 2.1 禁止 God Object

❌ **禁止**：
```rust
// state/mod.rs — 所有功能塞一个 struct
pub struct AppState {
    pub agent: ..., pub config: ..., pub tools: ..., pub history: ...,
    pub audit: ..., pub is_speaking: ..., pub is_listening: ..., // 越加越多
}
```

✅ **正确**：
```rust
// state/mod.rs — 每个领域一个专注的 state
pub struct AgentState { pub agent: Arc<OmpAgentSidecar>, pub history: Arc<Mutex<Vec<...>>> }
pub struct VoiceState { pub is_speaking: AtomicBool, pub is_listening: AtomicBool, pub lip_level: Mutex<f32> }
pub struct ConfigState { pub config: Arc<Mutex<CompanionConfig>>, pub config_manager: ConfigManager, ... }
pub struct AuditState { pub audit: AuditLogger }
pub struct ToolState { pub tools: Arc<ToolRegistry> }
```

### 2.2 注册与依赖

```rust
// lib.rs — 分别注册
.manage(AgentState::new())
.manage(VoiceState::new())
.manage(ConfigState { ... })
.manage(AuditState::new(...))
.manage(ToolState::new(...))
```

```rust
// state/mod.rs — 每个 command 只取所需
#[tauri::command]
pub async fn chat(
    agent: tauri::State<'_, AgentState>,
    _config: tauri::State<'_, ConfigState>,     // 不需要的加 _ 前缀
    message: String,
) -> Result<String, String> { ... }
```

### 2.3 添加新模块的步骤

1. `companion-core/src/` 下新建目录，定义 trait + 实现 + mock
2. `companion-core/src/lib.rs` 加一行 `pub mod new_module;`
3. `companion-tauri/src/state/mod.rs` 加新 state struct
4. `companion-tauri/src/lib.rs` 加 `.manage(NewState::new(...))`
5. 新建 `#[tauri::command]`，只取需要的 state 参数
6. 在 `lib.rs` 的 `invoke_handler` 中追加

---

## 3. Rust 编写规范

### 3.1 Trait 定义

```rust
/// 一句话描述这个 trait 的职责。
#[async_trait]
pub trait MyProvider: Send + Sync {
    /// 参数说明 + 返回值说明。
    async fn do_something(&self, input: &[f32]) -> Result<String, MyError>;
}
```

**规则：**
- 每个 trait 必有 `///` doc comment
- 必有 `Send + Sync` 约束
- `#[async_trait]` 用于异步 trait

### 3.2 Error 定义

```rust
#[derive(Debug, Error)]
pub enum MyError {
    #[error("description: {0}")]
    Variant(String),
}
```

**规则：**
- 使用 `thiserror` derive
- 每个 variant 有 `#[error("...")]` 消息
- 直接 Tauri command 返回 `Result<T, String>`（自动转换）

### 3.3 测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic() {
        // Arrange / Act / Assert
    }

    #[tokio::test]
    async fn test_async() {
        // 异步测试
    }
}
```

**规则：**
- 每个模块文件内嵌 `#[cfg(test)] mod tests`
- 每个 `mock.rs` 至少一个基本测试
- 测试名用 `test_<功能>_<场景>` 格式
- `#[ignore = "reason"]` 用于需要外部依赖的集成测试

---

## 4. 前端编写规范

### 4.1 Vue 组件

- 使用 `<script setup lang="ts">`
- 使用 Pinia store，不直接在组件内管理全局状态
- Tailwind CSS 优先，少写自定义 CSS

### 4.2 IPC 调用

```typescript
import { invoke } from '@tauri-apps/api/core'

// 类型化的 IPC 调用（建议封装到 composable）
const reply = await invoke<string>('chat', { message: 'hello' })
```

**规则：**
- IPC 调用用泛型标注返回类型
- 事件类型定义在 `web/src/types/ipc.ts`

---

## 5. 添加 Agent 工具

### 5.1 内置工具

在 `companion-core/src/tools/` 下新建文件，实现 `McpTool` trait：

```rust
use crate::mcp::{McpTool, McpError};

pub struct MyTool { sandbox: Arc<Sandbox> }

#[async_trait]
impl McpTool for MyTool {
    fn name(&self) -> &str { "my_tool" }
    fn description(&self) -> &str { "做什么的" }
    fn parameters(&self) -> Value { serde_json::json!({...}) }
    async fn execute(&self, args: Value) -> Result<Value, McpError> { ... }
}
```

然后在 `tools/mod.rs` 的 `with_builtins()` 中注册：
```rust
reg.register(Box::new(MyTool::new(sandbox.clone())));
```

### 5.2 社区工具（预留）

社区工具通过 Wasm 沙盒加载（阶段三实现），流程：
1. 用户从社区商店安装 → 下载 `.wasm` 到 `~/.companion/tools/<name>/`
2. `ToolWatcher` 检测新目录 → 加载 `manifest.json` → 校验权限
3. `WasmRunner` 执行工具（`wasmtime` 沙盒）
4. 工具自动注册到 `ToolRegistry`

---

## 6. 编码风格

| 项 | 规则 |
|----|------|
| Rust 命名 | `snake_case` for 函数/变量, `PascalCase` for 类型/trait |
| TypeScript 命名 | `camelCase` for 变量/函数, `PascalCase` for 类型/接口 |
| 文件命名 | Rust: `snake_case.rs`, Vue: `PascalCase.vue` |
| Import 顺序 | 标准库 → 外部 crate → `companion_core::` → `crate::` |
| 错误处理 | 使用 `thiserror`，不加 `unwrap()` / `expect()` 在库代码中 |
| 日志 | 使用 `log::info!`/`warn!`/`error!`，不在 `companion-core` 中用 `println!` |
| 文档 | 每个 `pub` 项（trait, struct, fn）必有 `///` doc comment |

---

## 7. 开发工作流速查

### 编译与检查
```bash
cargo check                                # 全 workspace 检查（最常用）
cargo check -p companion-core              # 只检查逻辑库
cargo check -p companion-tauri             # 只检查 Tauri 壳
cargo build                                # 生产构建
cargo clippy -p companion-core             # Lint 检查
```

### 测试
```bash
cargo test -p companion-core --lib         # 单元测试 (20)
cargo test -p companion-core --test e2e_tests  # 端到端测试 (3)
cd web && npm run test:ui                  # Playwright 前端测试
```

### 运行
```powershell
# ★ 必须在 companion-tauri 目录下
cd companion-tauri
cargo tauri dev

# 如果端口 5173 被占用：
netstat -ano | findstr :5173
taskkill /PID <PID> /F
```

### 完整验收
```bash
bash scripts\verify.sh
```

### 添加新模块流程
1. `companion-core/src/` 下建目录，定义 trait + error + mock
2. `companion-core/src/lib.rs` 加 `pub mod new_module;`
3. `companion-tauri/src/state/mod.rs` 加领域状态 struct
4. `companion-tauri/src/lib.rs` 加 `.manage(NewState::new(...))`
5. 新增 `#[tauri::command]`，只取需要的 state 参数
6. 在 `invoke_handler` 中追加命令
7. `cargo check && cargo test -p companion-core --lib` 验证
