//! Audit logging for security-sensitive operations.
//!
//! All system-mode switches, high-risk command executions, and tool invocations
//! are recorded to ~/.companion/logs/command.log with timestamps.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

/// Audit log entry types.
#[derive(Debug, Clone, PartialEq)]
pub enum AuditEvent {
    ModeSwitch { from: bool, to: bool },
    ToolExec { tool: String, params: String, success: bool },
    CommandExec { cmd: String, args: String, exit_code: i32 },
}

impl AuditEvent {
    fn to_line(&self) -> String {
        let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        match self {
            AuditEvent::ModeSwitch { from, to } => {
                format!("[{ts}] MODE_SWITCH: {} -> {}", if *from { "UNRESTRICTED" } else { "SANDBOX" }, if *to { "UNRESTRICTED" } else { "SANDBOX" })
            }
            AuditEvent::ToolExec { tool, params, success } => {
                let status = if *success { "OK" } else { "FAIL" };
                format!("[{ts}] TOOL: {tool} PARAMS={params} STATUS={status}")
            }
            AuditEvent::CommandExec { cmd, args, exit_code } => {
                let dangerous = is_high_risk(cmd, args);
                let risk = if dangerous { "HIGH_RISK" } else { "NORMAL" };
                format!("[{ts}] CMD: {cmd} {args} RISK={risk} EXIT={exit_code}")
            }
        }
    }
}

fn is_high_risk(cmd: &str, _args: &str) -> bool {
    let cmd_lower = cmd.to_lowercase();
    let name = std::path::Path::new(&cmd_lower)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or(cmd_lower);
    super::HIGH_RISK_CMDS.iter().any(|r| name == *r)
}

/// Thread-safe audit logger.
pub struct AuditLogger {
    path: PathBuf,
    file: Mutex<Option<std::fs::File>>,
}

impl AuditLogger {
    pub fn new(data_root: &PathBuf) -> Result<Self, String> {
        let log_dir = data_root.join("logs");
        fs::create_dir_all(&log_dir).map_err(|e| format!("log dir: {e}"))?;
        let path = log_dir.join("command.log");
        Ok(Self { path, file: Mutex::new(None) })
    }

    /// Log an event.
    pub fn log(&self, event: AuditEvent) {
        let line = event.to_line();
        self.append(&line);
    }

    /// Return the path to the log file.
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    fn append(&self, line: &str) {
        let mut guard = self.file.lock().unwrap();
        // Try to open on every call — don't permanently cache failures
        match OpenOptions::new().create(true).append(true).open(&self.path) {
            Ok(mut f) => {
                let _ = writeln!(f, "{line}");
                let _ = f.flush();
                *guard = Some(f);
            }
            Err(e) => {
                // Preserve previous handle for one-more-try semantics
                if let Some(ref mut f) = *guard {
                    let _ = writeln!(f, "{line}");
                    let _ = f.flush();
                } else {
                    eprintln!("[AuditLogger] cannot open log: {e}");
                }
            }
        }
    }
}
