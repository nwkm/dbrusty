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
        .title(" Help & Keybindings ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let text = vec![
        Line::from(" Navigation"),
        Line::from("  1-8   — Switch pages"),
        Line::from("  Tab   — Cycle focus (Sidebar / Main / Inspector)"),
        Line::from("  Esc   — Normal mode"),
        Line::from("  q     — Quit"),
        Line::from(""),
        Line::from(" Query Editor (Vi Mode)"),
        Line::from("  NORMAL mode:"),
        Line::from("    i        — Enter INSERT mode"),
        Line::from("    Esc      — Exit editor"),
        Line::from("    h/l      — Move cursor left/right"),
        Line::from("    j/k      — No-op (single-line)"),
        Line::from("    H/L      — Move to start/end of line"),
        Line::from("    w/b      — Word forward/backward"),
        Line::from("    dw/db    — Delete word forward/backward"),
        Line::from("    x        — Delete char at cursor"),
        Line::from("    u        — Undo"),
        Line::from("    R        — Redo"),
        Line::from("    Y        — Copy query to clipboard"),
        Line::from("    Ctrl+R   — Run query"),
        Line::from("  INSERT mode:"),
        Line::from("    Esc/jj   — Return to NORMAL mode"),
        Line::from("    Ctrl+R   — Run query"),
        Line::from(""),
        Line::from(" Pages"),
        Line::from("  1  Dashboard"),
        Line::from("  2  Connections"),
        Line::from("  3  Database Explorer"),
        Line::from("  4  Query Editor"),
        Line::from("  5  Query History"),
        Line::from("  6  Favorites"),
        Line::from("  7  Schema Browser"),
        Line::from("  8  Logs"),
        Line::from("  ?  Help (this page)"),
    ];

    let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}
