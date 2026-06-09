use async_trait::async_trait;
use serde_json::Value;
use std::process::Command;
use std::sync::Arc;
use crate::mcp::{McpError, McpTool};
use crate::sandbox::Sandbox;

/// Commands/phrases that require explicit confirmation (system mode or not).
const HIGH_RISK_CMDS: &[&str] = &[
    "rm", "del", "rd", "format", "shutdown", "reboot", "poweroff",
];

/// Path fragments that are high-risk.
const HIGH_RISK_PATHS: &[&str] = &[
    "/etc", "/boot", "/sys", "/proc",
    r"C:\Windows", r"C:\System32",
];

/// Check whether a command is high-risk.
fn is_high_risk(cmd: &str) -> bool {
    let cmd_lower = cmd.to_lowercase();
    let cmd_name = std::path::Path::new(&cmd_lower)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or(cmd_lower);

    HIGH_RISK_CMDS.iter().any(|r| cmd_name == *r)
}

/// Sandboxed command execution tool.
pub struct SandboxExecute {
    sandbox: Arc<Sandbox>,
}

impl SandboxExecute {
    pub fn new(sandbox: Arc<Sandbox>) -> Self {
        Self { sandbox }
    }
}

#[async_trait]
impl McpTool for SandboxExecute {
    fn name(&self) -> &str { "sandbox_execute" }
    fn description(&self) -> &str { "在沙盒目录下执行命令（默认拒绝危险命令）" }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": { "type": "string", "description": "要执行的命令" },
                "args": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "命令参数"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, args: Value) -> Result<Value, McpError> {
        let cmd = args.get("command").and_then(|v| v.as_str()).ok_or_else(|| {
            McpError::InvalidArguments("missing 'command' argument".into())
        })?;

        // Reject dangerous characters (injection prevention)
        let dangerous = [';', '|', '&', '$', '`', '\n', '\r'];
        if cmd.chars().any(|c| dangerous.contains(&c)) {
            return Err(McpError::ExecutionFailed("command contains dangerous characters".into()));
        }

        let cmd_args: Vec<String> = args.get("args")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        // Check for high-risk commands
        if is_high_risk(cmd) {
            return Err(McpError::PermissionDenied(
                format!("'{}' is a high-risk command. Enable system mode to use it.", cmd)
            ));
        }

        // Check high-risk paths in args
        for arg in &cmd_args {
            for risky in HIGH_RISK_PATHS {
                if arg.contains(risky) {
                    return Err(McpError::PermissionDenied(
                        format!("argument references high-risk path: {risky}")
                    ));
                }
            }
        }

        // Execute command with CWD set to sandbox root
        let output = Command::new(cmd)
            .args(&cmd_args)
            .current_dir(self.sandbox.root())
            .output()
            .map_err(|e| McpError::ExecutionFailed(format!("failed to execute command: {e}")))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok(serde_json::json!({
            "exit_code": output.status.code().unwrap_or(-1),
            "stdout": stdout,
            "stderr": stderr,
        }))
    }
}
