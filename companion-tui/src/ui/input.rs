//! Text input widget + recording status icon.

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

/// The input bar state.
pub struct InputWidget {
    /// Current input text buffer.
    pub buffer: String,
    /// Cursor position in bytes (always on a char boundary).
    pub cursor: usize,
}

impl InputWidget {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            cursor: 0,
        }
    }

    /// Insert a character at cursor position.
    pub fn insert_char(&mut self, c: char) {
        self.buffer.insert(self.cursor, c);
        self.cursor += c.len_utf8();
    }

    /// Delete character before cursor.
    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            let prev = self.prev_char_boundary();
            self.buffer.drain(prev..self.cursor);
            self.cursor = prev;
        }
    }

    /// Delete character at cursor.
    pub fn delete(&mut self) {
        if self.cursor < self.buffer.len() {
            let next = self.next_char_boundary();
            self.buffer.drain(self.cursor..next);
        }
    }

    /// Move cursor left by one character.
    pub fn cursor_left(&mut self) {
        if self.cursor > 0 {
            self.cursor = self.prev_char_boundary();
        }
    }

    /// Move cursor right by one character.
    pub fn cursor_right(&mut self) {
        if self.cursor < self.buffer.len() {
            self.cursor = self.next_char_boundary();
        }
    }

    /// Move cursor to start.
    pub fn cursor_home(&mut self) {
        self.cursor = 0;
    }

    /// Move cursor to end.
    pub fn cursor_end(&mut self) {
        self.cursor = self.buffer.len();
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.cursor = 0;
    }

    /// Take the buffer content and clear.
    pub fn take(&mut self) -> String {
        let text = self.buffer.clone();
        self.clear();
        text
    }

    /// Byte offset of the previous char boundary (or 0).
    fn prev_char_boundary(&self) -> usize {
        if self.cursor == 0 {
            return 0;
        }
        // Walk back one byte at a time until we hit a char boundary
        let mut pos = self.cursor - 1;
        while pos > 0 && !self.buffer.is_char_boundary(pos) {
            pos -= 1;
        }
        pos
    }

    /// Byte offset of the next char boundary (or buffer.len()).
    fn next_char_boundary(&self) -> usize {
        let mut pos = self.cursor + 1;
        while pos < self.buffer.len() && !self.buffer.is_char_boundary(pos) {
            pos += 1;
        }
        pos
    }

    /// Render the input bar.
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let mut spans: Vec<Span> = Vec::new();

        // ">" prompt
        spans.push(Span::styled(" > ", Style::default().fg(Color::Green)));

        let text = &self.buffer;

        // Cursor at end: show cursor after text
        if self.cursor >= text.len() {
            spans.push(Span::raw(text.as_str()));
            spans.push(Span::styled("▌", Style::default().fg(Color::White)));
        } else {
            // Invariant: self.cursor is always on a char boundary
            // (maintained by insert_char/backspace/delete/cursor_left/right).
            let (before, after) = text.split_at(self.cursor);

            let char_end = after
                .char_indices()
                .nth(1)
                .map(|(i, _)| i)
                .unwrap_or(after.len());

            spans.push(Span::raw(before));
            if !after.is_empty() {
                spans.push(Span::styled(
                    &after[..char_end],
                    Style::default().fg(Color::Black).bg(Color::White),
                ));
            }
            if char_end < after.len() {
                spans.push(Span::raw(&after[char_end..]));
            }
        }

        let line = Line::from(spans);
        let para = Paragraph::new(line);
        frame.render_widget(para, area);
    }
}
