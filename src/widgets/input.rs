use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

#[derive(Debug, Clone)]
pub struct InputField {
    pub value: String,
    pub cursor_pos: usize,
    pub placeholder: String,
    pub focused: bool,
}

impl InputField {
    pub fn new(placeholder: &str) -> Self {
        Self {
            value: String::new(),
            cursor_pos: 0,
            placeholder: placeholder.to_string(),
            focused: false,
        }
    }

    pub fn with_value(placeholder: &str, value: String) -> Self {
        let cursor_pos = value.len();
        Self {
            value,
            cursor_pos,
            placeholder: placeholder.to_string(),
            focused: false,
        }
    }

    pub fn clear(&mut self) {
        self.value.clear();
        self.cursor_pos = 0;
    }

    pub fn set_value(&mut self, value: &str) {
        self.value = value.to_string();
        self.cursor_pos = self.value.len();
    }

    pub fn handle_key(&mut self, key: &KeyEvent) -> bool {
        match key.code {
            KeyCode::Char(c) => {
                self.value.insert(self.cursor_pos, c);
                self.cursor_pos += 1;
                true
            }
            KeyCode::Backspace => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                    self.value.remove(self.cursor_pos);
                }
                true
            }
            KeyCode::Delete => {
                if self.cursor_pos < self.value.len() {
                    self.value.remove(self.cursor_pos);
                }
                true
            }
            KeyCode::Left => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                }
                true
            }
            KeyCode::Right => {
                if self.cursor_pos < self.value.len() {
                    self.cursor_pos += 1;
                }
                true
            }
            KeyCode::Home => {
                self.cursor_pos = 0;
                true
            }
            KeyCode::End => {
                self.cursor_pos = self.value.len();
                true
            }
            _ => false,
        }
    }

    pub fn delete_char_before_cursor(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
            self.value.remove(self.cursor_pos);
        }
    }

    pub fn line_start(&mut self) {
        self.cursor_pos = 0;
    }

    pub fn line_end(&mut self) {
        self.cursor_pos = self.value.len();
    }

    pub fn word_right(&mut self) {
        let bytes = self.value.as_bytes();
        let len = bytes.len();
        if self.cursor_pos >= len {
            return;
        }
        // skip current word chars
        let mut i = self.cursor_pos;
        while i < len && is_word_char(bytes[i]) {
            i += 1;
        }
        // skip separator chars
        while i < len && !is_word_char(bytes[i]) {
            i += 1;
        }
        self.cursor_pos = i;
    }

    pub fn word_left(&mut self) {
        if self.cursor_pos == 0 {
            return;
        }
        let bytes = self.value.as_bytes();
        // skip separator chars before cursor
        let mut i = self.cursor_pos.saturating_sub(1);
        while i > 0 && !is_word_char(bytes[i]) {
            i -= 1;
        }
        // skip word chars
        while i > 0 && is_word_char(bytes[i]) {
            i -= 1;
        }
        if i == 0 && is_word_char(bytes[0]) {
            self.cursor_pos = 0;
        } else {
            self.cursor_pos = i + 1;
        }
    }

    pub fn delete_word_forward(&mut self) {
        let bytes = self.value.as_bytes();
        let len = bytes.len();
        if self.cursor_pos >= len {
            return;
        }
        // skip current word chars
        let mut end = self.cursor_pos;
        while end < len && is_word_char(bytes[end]) {
            end += 1;
        }
        // consume trailing whitespace
        while end < len && bytes[end] == b' ' {
            end += 1;
        }
        self.value.drain(self.cursor_pos..end);
    }

    pub fn delete_word_backward(&mut self) {
        if self.cursor_pos == 0 {
            return;
        }
        let bytes = self.value.as_bytes();
        // skip whitespace before cursor
        let mut start = self.cursor_pos.saturating_sub(1);
        while start > 0 && bytes[start] == b' ' {
            start -= 1;
        }
        // skip word chars
        while start > 0 && is_word_char(bytes[start]) {
            start -= 1;
        }
        if start == 0 && is_word_char(bytes[0]) {
            self.value.drain(0..self.cursor_pos);
            self.cursor_pos = 0;
        } else {
            let remove_start = start + 1;
            self.value.drain(remove_start..self.cursor_pos);
            self.cursor_pos = remove_start;
        }
    }

    pub fn set_cursor_to_content_end(&mut self) {
        let trimmed = self.value.trim_end();
        self.cursor_pos = trimmed.len();
    }

    pub fn set_cursor_from_click(&mut self, col: u16, area: Rect) {
        let rel_x = col.saturating_sub(area.x) as usize;
        self.cursor_pos = rel_x.min(self.value.len());
    }

    pub fn render_into(&self, frame: &mut Frame, area: Rect) {
        let display = if self.value.is_empty() {
            self.placeholder.as_str()
        } else {
            self.value.as_str()
        };
        let paragraph = Paragraph::new(display).wrap(ratatui::widgets::Wrap { trim: false });
        frame.render_widget(paragraph, area);

        let before = &self.value[..self.cursor_pos];
        let cursor_line = before.matches('\n').count();
        let col = self.cursor_pos - before.rfind('\n').map(|i| i + 1).unwrap_or(0);
        let x = area.x + col as u16;
        let y = area.y + cursor_line as u16;
        if x < area.x + area.width && y < area.y + area.height {
            frame.set_cursor_position((x, y));
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, label: &str) {
        let border_color = if self.focused {
            Color::Cyan
        } else {
            Color::DarkGray
        };
        let block = Block::default()
            .title(format!(" {} ", label))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let display = if self.value.is_empty() {
            self.placeholder.as_str()
        } else {
            self.value.as_str()
        };
        let paragraph = Paragraph::new(display).block(block);
        frame.render_widget(paragraph, area);

        if self.focused {
            let x = area.x + 1 + self.cursor_pos as u16;
            let y = area.y + 1;
            if x < area.x + area.width - 1 {
                frame.set_cursor_position((x, y));
            }
        }
    }
}

fn is_word_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}
