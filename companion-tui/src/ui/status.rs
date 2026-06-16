//! Status bar widget — app title + voice state indicator.

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

/// Voice/agent state for status display.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppState {
    Idle,
    Listening,
    Processing,
}

impl AppState {
    pub fn label(&self) -> &'static str {
        match self {
            AppState::Idle => "● idle",
            AppState::Listening => "🎤 listening",
            AppState::Processing => "◉ thinking",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            AppState::Idle => Color::Gray,
            AppState::Listening => Color::Red,
            AppState::Processing => Color::Yellow,
        }
    }
}

/// Render the status bar at the top of the screen.
pub fn render(frame: &mut Frame, area: Rect, state: AppState) {
    let title = Span::styled("Companion TUI", Style::default().fg(Color::Cyan));
    let status = Span::styled(state.label(), Style::default().fg(state.color()));

    let line = Line::from(vec![
        title,
        Span::raw("  "),
        Span::styled("│", Style::default().fg(Color::DarkGray)),
        Span::raw("  "),
        status,
    ]);

    let para = Paragraph::new(line).style(Style::default());
    frame.render_widget(para, area);
}
