use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::App;

pub fn render(frame: &mut Frame, area: Rect, app: &App, focused: bool) {
    let border_color = if focused { Color::Cyan } else { Color::White };
    let block = Block::default()
        .title(" Logs ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let inner = block.inner(area);
    frame.render_widget(&block, area);

    let visible_height = inner.height as usize;
    let entries = &app.log_entries;
    if entries.is_empty() {
        let text = Paragraph::new("No messages").style(Style::default().fg(Color::DarkGray));
        frame.render_widget(text, inner);
        return;
    }

    let mut lines: Vec<Line> = Vec::with_capacity(visible_height);
    let start = entries.len().saturating_sub(visible_height);
    for entry in entries.iter().skip(start) {
        let mut text: &str = entry;
        if let Some(idx) = text.find('\n') {
            text = &text[..idx];
        }
        lines.push(Line::from(Span::styled(
            text,
            Style::default().fg(Color::DarkGray),
        )));
    }
    frame.render_widget(Paragraph::new(ratatui::text::Text::from(lines)), inner);
}
