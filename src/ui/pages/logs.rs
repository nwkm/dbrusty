use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::App;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Unknown,
}

impl LogLevel {
    fn from_str(s: &str) -> Self {
        let lower = s.to_lowercase();
        if lower.starts_with("error") || lower.starts_with("failed") || lower.contains("error:") {
            LogLevel::Error
        } else if lower.starts_with("warn") || lower.contains("warning") {
            LogLevel::Warn
        } else if lower.starts_with("info")
            || lower.starts_with("connected")
            || lower.starts_with("running")
            || lower.starts_with("ready")
            || lower.starts_with("query returned")
            || lower.starts_with("loaded")
            || lower.starts_with("schema")
            || lower.starts_with("executing")
            || lower.starts_with("generating")
            || lower.starts_with("exported")
            || lower.starts_with("disconnected")
        {
            LogLevel::Info
        } else if lower.starts_with("debug") || lower.starts_with("[") {
            LogLevel::Debug
        } else {
            LogLevel::Unknown
        }
    }

    fn color(&self) -> Color {
        match self {
            LogLevel::Error => Color::Red,
            LogLevel::Warn => Color::Yellow,
            LogLevel::Info => Color::Cyan,
            LogLevel::Debug => Color::DarkGray,
            LogLevel::Unknown => Color::White,
        }
    }

    fn tag(&self) -> &'static str {
        match self {
            LogLevel::Error => "ERR ",
            LogLevel::Warn => "WARN",
            LogLevel::Info => "INFO",
            LogLevel::Debug => "DEBG",
            LogLevel::Unknown => "    ",
        }
    }
}

pub fn render(frame: &mut Frame, area: Rect, app: &mut App, focused: bool) {
    let border_color = if focused { Color::Cyan } else { Color::White };
    let block = Block::default()
        .title(" Logs ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let inner = block.inner(area);
    frame.render_widget(&block, area);

    if inner.height < 3 {
        return;
    }

    // Split: header line, content, footer hints
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(2),
        Constraint::Length(1),
    ])
    .areas(inner);
    let [header_area, content_area, footer_area] = chunks;

    // Store visible height for key handler
    app.log_visible_height = content_area.height as usize;

    let entries = &app.log_entries;
    let filtered = if app.log_filter_active && !app.log_filter_input.is_empty() {
        let filter = app.log_filter_input.to_lowercase();
        entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.to_lowercase().contains(&filter))
            .collect::<Vec<_>>()
    } else {
        entries.iter().enumerate().collect::<Vec<_>>()
    };

    // Header with count and filter status
    let filter_info = if app.log_filter_active {
        format!(
            " Filter: \"{}\" ({}/{}) ",
            app.log_filter_input,
            filtered.len(),
            entries.len()
        )
    } else {
        format!(" {} entries ", entries.len())
    };
    let header_line = Line::from(Span::styled(
        filter_info,
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    ));
    frame.render_widget(Paragraph::new(header_line), header_area);

    // Content
    if filtered.is_empty() {
        let msg = if app.log_filter_active && !app.log_filter_input.is_empty() {
            "No matching entries."
        } else {
            "No log entries yet."
        };
        let text = Paragraph::new(msg).style(Style::default().fg(Color::DarkGray));
        frame.render_widget(text, content_area);
    } else {
        let visible_height = content_area.height as usize;
        let total = filtered.len();
        let max_scroll = total.saturating_sub(visible_height);
        let scroll = app.log_scroll.min(max_scroll);

        let start = scroll;
        let end = (start + visible_height).min(total);
        let mut lines: Vec<Line> = Vec::with_capacity(visible_height);

        for &(_, entry) in &filtered[start..end] {
            let mut text: &str = entry;
            if let Some(idx) = text.find('\n') {
                text = &text[..idx];
            }

            let level = LogLevel::from_str(text);
            let level_color = level.color();
            let tag = level.tag();

            // Format: "[LEVEL] message"
            let tag_span = Span::styled(
                format!("[{}] ", tag),
                Style::default()
                    .fg(level_color)
                    .add_modifier(Modifier::BOLD),
            );
            let msg_span = Span::styled(text, Style::default().fg(level_color));

            lines.push(Line::from(vec![tag_span, msg_span]));
        }

        frame.render_widget(Paragraph::new(Text::from(lines)), content_area);
    }

    // Footer: keyboard hints + scroll position
    let total = filtered.len();
    let visible_height = content_area.height as usize;
    let scroll = app
        .log_scroll
        .min(total.saturating_sub(visible_height).max(0));
    let scroll_pct = if total > 0 {
        ((scroll + visible_height).min(total) * 100 / total).min(100)
    } else {
        100
    };
    let hints = if app.log_filter_active {
        " Esc:Clear filter  j/k:Scroll  g/G:Top/Bottom  c:Clear logs  100% "
    } else {
        " /:Filter  j/k:Scroll  g/G:Top/Bottom  c:Clear logs "
    };
    let footer_text = format!(" {}%  {} ", scroll_pct, hints);
    let footer_line = Line::from(Span::styled(
        footer_text,
        Style::default().fg(Color::DarkGray),
    ));
    frame.render_widget(Paragraph::new(footer_line), footer_area);
}

pub fn handle_key(app: &mut App, key: &crossterm::event::KeyEvent) -> bool {
    use crossterm::event::KeyCode;

    // Handle filter input mode
    if app.log_filter_active {
        match key.code {
            KeyCode::Esc => {
                app.log_filter_active = false;
                app.log_filter_input.clear();
                app.log_scroll = 0;
                return true;
            }
            KeyCode::Enter => {
                app.log_filter_active = false;
                app.log_scroll = 0;
                return true;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let entries = &app.log_entries;
                let filter = app.log_filter_input.to_lowercase();
                let total = if filter.is_empty() {
                    entries.len()
                } else {
                    entries
                        .iter()
                        .filter(|e| e.to_lowercase().contains(&filter))
                        .count()
                };
                let visible = app.log_visible_height;
                let max_scroll = total.saturating_sub(visible).max(0);
                if app.log_scroll < max_scroll {
                    app.log_scroll += 1;
                }
                return true;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if app.log_scroll > 0 {
                    app.log_scroll -= 1;
                }
                return true;
            }
            KeyCode::Backspace => {
                app.log_filter_input.pop();
                app.log_scroll = 0;
                return true;
            }
            KeyCode::Char(c) => {
                app.log_filter_input.push(c);
                app.log_scroll = 0;
                return true;
            }
            _ => return true,
        }
    }

    // Normal log navigation
    match key.code {
        KeyCode::Char('/') => {
            app.log_filter_active = true;
            app.log_filter_input.clear();
            app.log_scroll = 0;
            app.set_status("Type to filter logs, Esc to clear.");
            true
        }
        KeyCode::Char('j') | KeyCode::Down => {
            let entries = &app.log_entries;
            let total = entries.len();
            let visible = app.log_visible_height;
            let max_scroll = total.saturating_sub(visible).max(0);
            if app.log_scroll < max_scroll {
                app.log_scroll += 1;
            }
            true
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if app.log_scroll > 0 {
                app.log_scroll -= 1;
            }
            true
        }
        KeyCode::Char('g') => {
            app.log_scroll = 0;
            true
        }
        KeyCode::Char('G') => {
            let entries = &app.log_entries;
            let total = entries.len();
            let visible = app.log_visible_height;
            app.log_scroll = total.saturating_sub(visible).max(0);
            true
        }
        KeyCode::Char('c') => {
            app.log_entries.clear();
            app.log_scroll = 0;
            app.set_status("Logs cleared.");
            true
        }
        KeyCode::Home => {
            app.log_scroll = 0;
            true
        }
        KeyCode::End => {
            let entries = &app.log_entries;
            let total = entries.len();
            let visible = app.log_visible_height;
            app.log_scroll = total.saturating_sub(visible).max(0);
            true
        }
        KeyCode::PageUp => {
            app.log_scroll = app.log_scroll.saturating_sub(app.log_visible_height);
            true
        }
        KeyCode::PageDown => {
            let entries = &app.log_entries;
            let total = entries.len();
            let max_scroll = total.saturating_sub(app.log_visible_height).max(0);
            app.log_scroll = (app.log_scroll + app.log_visible_height).min(max_scroll);
            true
        }
        _ => false,
    }
}
