use async_trait::async_trait;
use serde_json::Value;
use std::fs;
use std::sync::Arc;
use crate::mcp::{McpError, McpTool};
use crate::sandbox::Sandbox;

/// Helper to turn a SandboxError into McpError.
fn mcp_err(e: crate::sandbox::SandboxError) -> McpError {
    McpError::ExecutionFailed(e.to_string())
}

// ---------------------------------------------------------------------------
// sandbox_list
// ---------------------------------------------------------------------------

pub struct SandboxList {
    sandbox: Arc<Sandbox>,
}

impl SandboxList {
    pub fn new(sandbox: Arc<Sandbox>) -> Self {
        Self { sandbox }
    }
}

#[async_trait]
impl McpTool for SandboxList {
    fn name(&self) -> &str { "sandbox_list" }
    fn description(&self) -> &str { "列出沙盒目录中的文件和子目录" }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "沙盒内的相对路径（默认 /）" }
            }
        })
    }

    async fn execute(&self, args: Value) -> Result<Value, McpError> {
        let rel_path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let resolved = self.sandbox.resolve(rel_path).map_err(mcp_err)?;

        let entries = fs::read_dir(&resolved).map_err(|e| McpError::ExecutionFailed(e.to_string()))?;
        let mut items: Vec<Value> = Vec::new();

        for entry in entries {
            let entry = entry.map_err(|e| McpError::ExecutionFailed(e.to_string()))?;
            let ft = entry.file_type().map_err(|e| McpError::ExecutionFailed(e.to_string()))?;
            items.push(serde_json::json!({
                "name": entry.file_name().to_string_lossy(),
                "type": if ft.is_dir() { "directory" } else { "file" },
                "size": entry.metadata().map(|m| m.len()).unwrap_or(0),
            }));
        }

        Ok(serde_json::json!({ "entries": items }))
    }
}

// ---------------------------------------------------------------------------
// sandbox_read
// ---------------------------------------------------------------------------

pub struct SandboxRead {
    sandbox: Arc<Sandbox>,
}

impl SandboxRead {
    pub fn new(sandbox: Arc<Sandbox>) -> Self {
        Self { sandbox }
    }
}

#[async_trait]
impl McpTool for SandboxRead {
    fn name(&self) -> &str { "sandbox_read" }
    fn description(&self) -> &str { "读取沙盒内的文件内容" }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "沙盒内的相对路径" }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, args: Value) -> Result<Value, McpError> {
        let rel_path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| {
            McpError::InvalidArguments("missing 'path' argument".into())
        })?;
        let resolved = self.sandbox.resolve(rel_path).map_err(mcp_err)?;
        let content = fs::read_to_string(&resolved)
            .map_err(|e| McpError::ExecutionFailed(e.to_string()))?;
        Ok(serde_json::json!({ "content": content }))
    }
}

// ---------------------------------------------------------------------------
// sandbox_write
// ---------------------------------------------------------------------------

pub struct SandboxWrite {
    sandbox: Arc<Sandbox>,
}

impl SandboxWrite {
    pub fn new(sandbox: Arc<Sandbox>) -> Self {
        Self { sandbox }
    }
}

#[async_trait]
impl McpTool for SandboxWrite {
    fn name(&self) -> &str { "sandbox_write" }
    fn description(&self) -> &str { "写入文件到沙盒（创建或覆盖）" }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "沙盒内的相对路径" },
                "content": { "type": "string", "description": "要写入的内容" }
            },
            "required": ["path", "content"]
        })
    }

    async fn execute(&self, args: Value) -> Result<Value, McpError> {
        let rel_path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| {
            McpError::InvalidArguments("missing 'path' argument".into())
        })?;
        let content = args.get("content").and_then(|v| v.as_str()).ok_or_else(|| {
            McpError::InvalidArguments("missing 'content' argument".into())
        })?;
        let resolved = self.sandbox.resolve(rel_path).map_err(mcp_err)?;

        // Ensure parent directory exists
        if let Some(parent) = resolved.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| McpError::ExecutionFailed(e.to_string()))?;
        }

        fs::write(&resolved, content)
            .map_err(|e| McpError::ExecutionFailed(e.to_string()))?;

        Ok(serde_json::json!({ "path": resolved.to_string_lossy(), "size": content.len() }))
    }
}

// ---------------------------------------------------------------------------
// sandbox_delete
// ---------------------------------------------------------------------------

pub struct SandboxDelete {
    sandbox: Arc<Sandbox>,
}

impl SandboxDelete {
    pub fn new(sandbox: Arc<Sandbox>) -> Self {
        Self { sandbox }
    }
}

#[async_trait]
impl McpTool for SandboxDelete {
    fn name(&self) -> &str { "sandbox_delete" }
    fn description(&self) -> &str { "删除沙盒内的文件或空目录" }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "沙盒内的相对路径" }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, args: Value) -> Result<Value, McpError> {
        let rel_path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| {
            McpError::InvalidArguments("missing 'path' argument".into())
        })?;
        let resolved = self.sandbox.resolve(rel_path).map_err(mcp_err)?;

        let path_clone = resolved.clone();
        let metadata = fs::metadata(&path_clone)
            .map_err(|e| McpError::ExecutionFailed(e.to_string()))?;

        if metadata.is_dir() {
            fs::remove_dir(&path_clone)
                .map_err(|e| McpError::ExecutionFailed(e.to_string()))?;
        } else {
            fs::remove_file(&path_clone)
                .map_err(|e| McpError::ExecutionFailed(e.to_string()))?;
        }

        Ok(serde_json::json!({ "deleted": resolved.to_string_lossy() }))
    }
}
