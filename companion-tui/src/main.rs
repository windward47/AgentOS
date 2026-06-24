//! Companion TUI — terminal user interface with voice interaction.
//!
//! Three modes:
//! - Interactive TUI (default): ratatui chat flow with hotkey PTT
//! - Pipe mode (`--stdin --stdout`): one-shot text in, reply out
//! - Background mode: hotkey-only voice, no TUI

use std::path::PathBuf;

use clap::Parser;

use companion_core::agent::AgentEngine;
use companion_core::config::{CompanionConfig, ConfigManager};

mod app;
mod voice;
mod ui;

/// Companion — terminal-native AI companion with voice.
#[derive(Parser, Debug)]
#[command(name = "companion-tui", version, about)]
struct Cli {
    /// Pipe mode: read user message from stdin, write reply to stdout, then exit.
    #[arg(long)]
    stdin: bool,

    /// Pipe mode: output only the reply text (no TUI, no decorations).
    #[arg(long)]
    stdout: bool,

    /// Path to config file (default: ~/.companion/config.json).
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Log level (trace, debug, info, warn, error).
    #[arg(long, default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Init logging
    let level = cli.log_level.parse::<log::LevelFilter>().unwrap_or(log::LevelFilter::Info);
    env_logger::Builder::new().filter_level(level).format_timestamp_millis().init();

    log::info!("companion-tui v{} starting", env!("CARGO_PKG_VERSION"));

    // Load shared config
    let config = match load_config(cli.config.as_deref()) {
        Ok(c) => c,
        Err(e) => {
            log::error!("Failed to load config: {e}");
            eprintln!("Error: failed to load config — {e}");
            std::process::exit(1);
        }
    };

    // Pipe mode requires both --stdin and --stdout; reject partial flags
    let want_pipe = cli.stdin || cli.stdout;
    if want_pipe && !(cli.stdin && cli.stdout) {
        eprintln!("Error: pipe mode requires both --stdin and --stdout.");
        eprintln!("Usage: echo \"message\" | companion-tui --stdin --stdout");
        std::process::exit(2);
    }

    if cli.stdin && cli.stdout {
        // ── Pipe mode ──
        run_pipe_mode(&config).await;
    } else {
        // ── Interactive TUI mode ──
        app::run(config).await;
    }
}

/// Load config from custom path or default location.
fn load_config(custom_path: Option<&std::path::Path>) -> Result<CompanionConfig, String> {
    if let Some(path) = custom_path {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("invalid config JSON: {e}"))
    } else {
        ConfigManager::new()
            .map_err(|e| format!("{e}"))
            .and_then(|cm| cm.load().map_err(|e| format!("{e}")))
    }
}

/// One-shot stdin → Agent → stdout.
async fn run_pipe_mode(config: &CompanionConfig) {
    use std::io::Read;

    let mut input = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut input) {
        log::error!("Failed to read stdin: {e}");
        std::process::exit(1);
    }
    let input = input.trim();
    if input.is_empty() {
        eprintln!("(no input)");
        return;
    }

    log::info!("Pipe mode: sending {} chars to agent", input.len());

    let agent = companion_core::agent::omp_sidecar::OmpAgentSidecar::new();
    if let Err(e) = agent.spawn().await {
        log::error!("Failed to spawn agent: {e}");
        eprintln!("Error: {e}");
        std::process::exit(1);
    }

    // TODO: pass conversation history (&[]) — pipe mode is stateless one-shot.
    //       Future: optionally load/save session from ~/.companion/sessions/
    match agent.chat(input, &[], Some(config.custom_system_prompt.as_str())).await {
        Ok(resp) => {
            println!("{}", resp.text);
        }
        Err(e) => {
            log::error!("Agent error: {e}");
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}
