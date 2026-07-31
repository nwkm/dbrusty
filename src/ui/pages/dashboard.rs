use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::app::App;

pub fn render(frame: &mut Frame, area: Rect, _app: &mut App, focused: bool) {
    let border_color = if focused { Color::Cyan } else { Color::White };
    let block = Block::default()
        .title(" Dashboard ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let text = vec![
        Line::from(""),
        Line::from(" Welcome to Dbrusty v0.1.0"),
        Line::from(""),
        Line::from(" A cross-platform terminal database client"),
        Line::from(""),
        Line::from(" Quick actions:"),
        Line::from("  2  — Open Connections to add a database"),
        Line::from("  3  — Open Explorer to browse schema"),
        Line::from("  4  — Open Query Editor to run SQL"),
        Line::from("  ?  — View all keyboard shortcuts"),
    ];

    let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}
