//! TUI application event loop.
//!
//! Orchestrates:
//! - ratatui rendering (status + chat + input)
//! - Keyboard input (type text, Enter to send, Ctrl+C/q to quit)
//! - Voice events (hotkey PTT → ASR → auto-send)
//! - Streaming sidecar tokens (display tokens as they arrive)

use std::io;
use std::sync::Arc;
use std::time::Duration;

use companion_core::agent::omp_sidecar::OmpAgentSidecar;
use companion_core::config::CompanionConfig;

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    Frame,
};

use crate::ui::chat::ChatWidget;
use crate::ui::input::InputWidget;
use crate::ui::status::{self, AppState};
use crate::voice::{VoiceController, VoiceEvent};

/// Stream control signal — replaces fragile `\0` string sentinel.
#[derive(Debug)]
enum StreamSignal {
    /// A text token to append to the AI message.
    Token(String),
    /// Stream completed successfully.
    Done,
    /// Stream errored with a message.
    Error(String),
}

/// Main TUI entry point.
pub async fn run(config: CompanionConfig) {
    // ── Init terminal ──
    enable_raw_mode().expect("enable raw mode");
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).expect("enter alt screen");

    let mut terminal = ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(stdout))
        .expect("create terminal");

    // ── Spawn sidecar agent ──
    let agent = Arc::new(OmpAgentSidecar::new());
    if let Err(e) = agent.spawn().await {
        log::error!("Failed to spawn agent: {e}");
        cleanup();
        eprintln!("Error: {e}");
        return;
    }
    log::info!("Agent sidecar ready");

    // ── Spawn voice controller ──
    let voice_rx = VoiceController::spawn(&config);

    // ── App state ──
    let mut state = AppState::Idle;
    let mut chat = ChatWidget::new();
    let mut input = InputWidget::new();

    // Agent streaming state
    let mut stream_rx: Option<tokio::sync::mpsc::Receiver<StreamSignal>> = None;
    let mut stream_handle: Option<tokio::task::JoinHandle<()>> = None;

    // ── Set up panic hook to restore terminal on crash ──
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        cleanup();
        default_hook(info);
    }));

    // ── Main event loop ──
    loop {
        // ── Poll keyboard events (non-blocking) ──
        let should_quit = match event::poll(Duration::from_millis(16)) {
            Ok(true) => {
                match event::read() {
                    Ok(Event::Key(key)) if key.kind != KeyEventKind::Release => {
                        handle_key(key.code, &mut input, &mut chat, agent.clone(), &mut stream_rx, &mut stream_handle, &mut state)
                    }
                    Err(e) => {
                        log::error!("event::read error: {e}");
                        false
                    }
                    _ => false,
                }
            }
            Ok(false) => false,
            Err(e) => {
                log::error!("event::poll error: {e}");
                false
            }
        };

        if should_quit {
            break;
        }

        // ── Poll voice events (non-blocking) ──
        while let Ok(evt) = voice_rx.try_recv() {
            match evt {
                VoiceEvent::ListeningStarted => {
                    state = AppState::Listening;
                }
                VoiceEvent::ListeningStopped => {
                    state = AppState::Processing;
                }
                VoiceEvent::TextReady(text) => {
                    // Auto-send via agent
                    chat.add_user(&text);
                    start_stream(
                        agent.clone(),
                        text,
                        &mut stream_rx,
                        &mut stream_handle,
                        &mut chat,
                    );
                    state = AppState::Processing;
                }
                VoiceEvent::Error(err) => {
                    log::error!("Voice error: {err}");
                    // Silently ignore — don't disrupt the UI
                }
            }
        }

        // ── Poll streaming tokens ──
        let stream_finished = stream_rx.is_none();
        if !stream_finished {
            let mut done = false;
            if let Some(ref mut rx) = stream_rx {
                loop {
                    match rx.try_recv() {
                        Ok(StreamSignal::Token(token)) => {
                            chat.append_token(&token);
                        }
                        Ok(StreamSignal::Done) => {
                            done = true;
                            break;
                        }
                        Ok(StreamSignal::Error(msg)) => {
                            done = true;
                            chat.append_token(&format!(" [⚠️ {msg}]"));
                            break;
                        }
                        Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
                        Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                            done = true;
                            break;
                        }
                    }
                }
            }
            if done {
                stream_rx = None;
                // Check if the spawned task finished (panics are logged by tokio)
                if let Some(ref handle) = stream_handle {
                    if handle.is_finished() {
                        log::debug!("Stream task completed");
                    }
                }
                state = AppState::Idle;
            }
        }

        // ── Render ──
        terminal
            .draw(|frame| render_ui(frame, &chat, &input, state))
            .expect("draw frame");
    }

    // ── Cleanup ──
    cleanup();
}

/// Handle a single key event. Returns true if the app should quit.
fn handle_key(
    code: KeyCode,
    input: &mut InputWidget,
    chat: &mut ChatWidget,
    agent: Arc<OmpAgentSidecar>,
    stream_rx: &mut Option<tokio::sync::mpsc::Receiver<StreamSignal>>,
    stream_handle: &mut Option<tokio::task::JoinHandle<()>>,
    state: &mut AppState,
) -> bool {
    match code {
        KeyCode::Char('q') if cfg!(debug_assertions) => return true,
        KeyCode::Esc => return true,
        KeyCode::Char(c) => {
            input.insert_char(c);
        }
        KeyCode::Backspace => {
            input.backspace();
        }
        KeyCode::Delete => {
            input.delete();
        }
        KeyCode::Left => {
            input.cursor_left();
        }
        KeyCode::Right => {
            input.cursor_right();
        }
        KeyCode::Home => {
            input.cursor_home();
        }
        KeyCode::End => {
            input.cursor_end();
        }
        KeyCode::PageUp => {
            chat.scroll_up(5);
        }
        KeyCode::PageDown => {
            chat.scroll_down(5);
            if chat.scroll == 0 {
                chat.scrolled_up = false;
            }
        }
        KeyCode::Enter => {
            let text = input.take();
            if !text.is_empty() {
                chat.add_user(&text);
                start_stream(agent.clone(), text, stream_rx, stream_handle, chat);
                *state = AppState::Processing;
            }
        }
        _ => {}
    }
    false
}

/// Start a streaming agent request.
fn start_stream(
    agent: Arc<OmpAgentSidecar>,
    message: String,
    stream_rx: &mut Option<tokio::sync::mpsc::Receiver<StreamSignal>>,
    stream_handle: &mut Option<tokio::task::JoinHandle<()>>,
    chat: &mut ChatWidget,
) {
    chat.start_ai();

    let agent = agent.clone();
    let msg = message;

    let handle = tokio::runtime::Handle::current();
    let history_json = Vec::new(); // TODO: pass actual conversation history

    let (tx, rx) = tokio::sync::mpsc::channel::<StreamSignal>(256);
    *stream_rx = Some(rx);

    let jh = handle.spawn(async move {
        match agent.chat_stream_tokens(&msg, &history_json).await {
            Ok(mut token_rx) => {
                while let Some(token) = token_rx.recv().await {
                    if token.starts_with('\0') {
                        // Sidecar sentinel: \0 = done, \0⚠️ = error
                        let msg = &token[1..];
                        if msg.starts_with("⚠️") {
                            let _ = tx.send(StreamSignal::Error(msg.to_string())).await;
                        } else {
                            // Normal completion — msg may contain final text
                            if !msg.is_empty() {
                                let _ = tx.send(StreamSignal::Token(msg.to_string())).await;
                            }
                            let _ = tx.send(StreamSignal::Done).await;
                        }
                        break;
                    } else {
                        if tx.send(StreamSignal::Token(token)).await.is_err() {
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                let _ = tx.send(StreamSignal::Error(format!("Agent error: {e}"))).await;
            }
        }
    });
    *stream_handle = Some(jh);
}

/// Render the full TUI layout.
fn render_ui(frame: &mut Frame, chat: &ChatWidget, input: &InputWidget, state: AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),  // status bar
            Constraint::Min(1),     // chat area (fills rest)
            Constraint::Length(1),  // input bar
        ])
        .split(frame.area());

    status::render(frame, chunks[0], state);
    chat.render(frame, chunks[1]);
    input.render(frame, chunks[2]);
}

/// Restore terminal to normal.
fn cleanup() {
    disable_raw_mode().ok();
    execute!(io::stdout(), LeaveAlternateScreen).ok();
    log::info!("TUI exited, terminal restored");
}
