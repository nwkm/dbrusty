use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::{App, Focus, InputMode};
use crate::ui::pages;
use crate::ui::sidebar;

pub fn render_layout(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    let vertical = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ]);
    let [topbar_area, content_area, footer_area] = vertical.areas(area);

    let horizontal = Layout::horizontal([
        Constraint::Percentage(25),
        Constraint::Percentage(55),
        Constraint::Percentage(20),
    ]);
    let [sidebar_area, main_area, inspector_area] = horizontal.areas(content_area);

    app.sidebar_rect = sidebar_area;
    app.main_rect = main_area;
    app.inspector_rect = inspector_area;

    render_topbar(frame, topbar_area, app);
    sidebar::render_sidebar(frame, sidebar_area, app);
    render_main(frame, main_area, app);
    render_inspector(frame, inspector_area, app);
    render_footer(frame, footer_area, app);
}

fn render_topbar(frame: &mut Frame, area: Rect, app: &App) {
    let page_indicator = format!("[{}]", app.current_page.title());

    let text = format!(" Dbrusty  {}", page_indicator);

    let topbar = Paragraph::new(text).style(Style::default().fg(Color::White).bg(Color::DarkGray));
    frame.render_widget(topbar, area);
}

fn render_main(frame: &mut Frame, area: Rect, app: &App) {
    let focused = app.focus == Focus::Main;
    pages::render(frame, area, app.current_page, app, focused);
}

fn render_inspector(frame: &mut Frame, area: Rect, app: &App) {
    let focused = app.focus == Focus::Inspector;
    let border_color = if focused { Color::Cyan } else { Color::White };
    let block = Block::default()
        .title(" Inspector ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let text = Paragraph::new("Select an item").block(block);
    frame.render_widget(text, area);
}

fn render_footer(frame: &mut Frame, area: Rect, app: &App) {
    let mode_str = match app.input_mode {
        InputMode::Normal => " NORMAL ",
        InputMode::Insert => " INSERT ",
        InputMode::Command => " COMMAND ",
        InputMode::Search => " SEARCH ",
        InputMode::Visual => " VISUAL ",
    };

    let hints = app.key_hints.as_str();
    let status = app.status_message.as_str();

    let text = if status.is_empty() {
        format!("{}  | {}", mode_str, hints)
    } else {
        let mut s = status;
        if let Some(idx) = s.find('\n') {
            s = &s[..idx];
        }
        format!("{} {} | {}", mode_str, s, hints)
    };
    let footer = Paragraph::new(text).style(Style::default().fg(Color::Gray).bg(Color::DarkGray));
    frame.render_widget(footer, area);
}
