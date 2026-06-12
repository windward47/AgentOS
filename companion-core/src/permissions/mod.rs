//! Permission management — audit logging for security-sensitive operations.

/// Commands that are always considered high-risk.
pub const HIGH_RISK_CMDS: &[&str] = &[
    "rm", "del", "rd", "format", "shutdown", "reboot", "poweroff",
];

pub mod audit;
