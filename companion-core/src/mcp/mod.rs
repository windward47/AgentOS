use async_trait::async_trait;
use serde_json::Value;
use thiserror::Error;

/// A single MCP-compatible tool that can be registered and called by LLM.
#[async_trait]
pub trait McpTool: Send + Sync {
    /// Unique tool name (snake_case, e.g. `sandbox_read`).
    fn name(&self) -> &str;

    /// Human-readable description for LLM function-calling.
    fn description(&self) -> &str;

    /// JSON Schema of the parameters.
    fn parameters(&self) -> Value;

    /// Execute the tool with the given arguments.
    async fn execute(&self, args: Value) -> Result<Value, McpError>;
}

#[derive(Debug, Error)]
pub enum McpError {
    #[error("tool not found: {0}")]
    ToolNotFound(String),
    #[error("invalid arguments: {0}")]
    InvalidArguments(String),
    #[error("execution failed: {0}")]
    ExecutionFailed(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),
}
