//! Tool layer — MCP-compatible tool registry and built-in tools.
//!
//! Contains [`ToolRegistry`] for registering/lookup/execution of tools,
//! plus built-in implementations (file I/O, command execution).

pub mod command_tools;
pub mod file_tools;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use serde_json::Value;
use crate::mcp::{McpTool, McpError};
use crate::sandbox::Sandbox;

/// Central registry of all available tools.
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn McpTool>>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: HashMap::new() }
    }

    /// Register a single tool.
    pub fn register(&mut self, tool: Box<dyn McpTool>) {
        let name = tool.name().to_string();
        self.tools.insert(name, tool);
    }

    /// Build the registry with all built-in tools.
    pub fn with_builtins(sandbox_path: PathBuf) -> Self {
        let sandbox = Arc::new(Sandbox::new(sandbox_path));
        let mut reg = Self::new();

        // File tools
        reg.register(Box::new(file_tools::SandboxList::new(sandbox.clone())));
        reg.register(Box::new(file_tools::SandboxRead::new(sandbox.clone())));
        reg.register(Box::new(file_tools::SandboxWrite::new(sandbox.clone())));
        reg.register(Box::new(file_tools::SandboxDelete::new(sandbox.clone())));

        // Command tools
        reg.register(Box::new(command_tools::SandboxExecute::new(sandbox)));

        reg
    }

    /// Return the list of tool definitions for LLM function-calling.
    pub fn definitions(&self) -> Vec<serde_json::Value> {
        self.tools
            .values()
            .map(|t| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.name(),
                        "description": t.description(),
                        "parameters": t.parameters(),
                    }
                })
            })
            .collect()
    }

    /// Execute a tool by name.
    pub async fn execute(&self, name: &str, args: Value) -> Result<Value, McpError> {
        match self.tools.get(name) {
            Some(tool) => tool.execute(args).await,
            None => Err(McpError::ToolNotFound(name.to_string())),
        }
    }
}
