//! Chat message list widget — scrollable conversation display.

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

/// A single message in the chat.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: String, // "You" or "AI"
    pub content: String,
}

/// The chat widget state.
pub struct ChatWidget {
    /// Accumulated messages.
    pub messages: Vec<ChatMessage>,
    /// Scroll offset from bottom in lines (0 = latest view).
    pub scroll: u16,
    /// Whether user has scrolled up (disables auto-scroll).
    pub scrolled_up: bool,
}

impl ChatWidget {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            scroll: 0,
            scrolled_up: false,
        }
    }

    /// Add a user message.
    pub fn add_user(&mut self, text: &str) {
        self.messages.push(ChatMessage {
            role: "You".into(),
            content: text.to_string(),
        });
        self.reset_scroll();
    }

    /// Start an AI message (empty, will be appended to).
    pub fn start_ai(&mut self) {
        self.messages.push(ChatMessage {
            role: "AI".into(),
            content: String::new(),
        });
        self.reset_scroll();
    }

    /// Append a token to the latest AI message.
    pub fn append_token(&mut self, token: &str) {
        if let Some(last) = self.messages.last_mut() {
            if last.role == "AI" {
                last.content.push_str(token);
            }
        }
    }

    /// Scroll up by `n` lines.
    pub fn scroll_up(&mut self, n: u16) {
        self.scroll = self.scroll.saturating_add(n);
        self.scrolled_up = self.scroll > 0;
    }

    /// Scroll down by `n` lines.
    pub fn scroll_down(&mut self, n: u16) {
        self.scroll = self.scroll.saturating_sub(n);
        self.scrolled_up = self.scroll > 0;
    }

    /// Reset scroll to follow latest messages.
    fn reset_scroll(&mut self) {
        if !self.scrolled_up {
            self.scroll = 0;
        }
    }

    /// Render the chat area.
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let mut lines: Vec<Line> = Vec::new();

        for msg in &self.messages {
            // Role label
            match msg.role.as_str() {
                "You" => {
                    lines.push(Line::from(vec![
                        Span::styled(" You: ", Style::default().fg(Color::Green)),
                    ]));
                }
                _ => {
                    lines.push(Line::from(vec![
                        Span::styled(" AI:  ", Style::default().fg(Color::Cyan)),
                    ]));
                }
            }

            // Message content — wrap to fit width
            let max_width = area.width.saturating_sub(2) as usize;
            for chunk in wrap_text(&msg.content, max_width) {
                lines.push(Line::from(Span::raw(chunk)));
            }

            // Blank line between messages
            lines.push(Line::from(""));
        }

        let block = Block::default()
            .borders(Borders::NONE)
            .style(Style::default());

        let para = Paragraph::new(Text::from(lines))
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((self.scroll, 0));

        frame.render_widget(para, area);
    }
}

/// Simple text wrapping at word boundaries (or character boundaries for long words).
fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return text.lines().map(|l| l.to_string()).collect();
    }

    let mut result = Vec::new();
    for line in text.lines() {
        if line.is_empty() {
            result.push(String::new());
            continue;
        }
        let mut remaining = line;
        while !remaining.is_empty() {
            if remaining.len() <= max_width {
                result.push(remaining.to_string());
                break;
            }
            // Find a safe byte boundary at or before max_width
            let safe_end = safe_char_boundary(remaining, max_width);

            // Try to break at a space within the safe range
            let split_at = if let Some(pos) = remaining[..safe_end].rfind(' ') {
                pos + 1 // include the space
            } else {
                safe_end
            };
            result.push(remaining[..split_at].trim_end().to_string());
            remaining = remaining[split_at..].trim_start();
        }
    }
    result
}

/// Find a UTF-8 char boundary at or before `pos` bytes into `s`.
fn safe_char_boundary(s: &str, pos: usize) -> usize {
    let end = pos.min(s.len());
    if s.is_char_boundary(end) {
        return end;
    }
    // Walk back to find the nearest char boundary
    (0..end).rev().find(|&i| s.is_char_boundary(i)).unwrap_or(0)
}
