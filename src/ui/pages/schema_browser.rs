use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::App;

pub fn render(frame: &mut Frame, area: Rect, _app: &App, focused: bool) {
    let border_color = if focused { Color::Cyan } else { Color::White };
    let block = Block::default()
        .title(" Schema Browser ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let text = "Select a table in the Explorer to view its schema here.";

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}
