use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::app::App;

pub fn render(frame: &mut Frame, area: Rect, app: &App, focused: bool) {
    let border_color = if focused { Color::Cyan } else { Color::White };

    let vertical = Layout::vertical([Constraint::Percentage(33), Constraint::Percentage(67)]);
    let [editor_area, results_area] = vertical.areas(area);

    // --- Editor pane ---
    let title = if app.editing_query {
        " Query Editor [EDITING] "
    } else {
        " Query Editor "
    };
    let editor_block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let editor_inner = editor_block.inner(editor_area);
    frame.render_widget(&editor_block, editor_area);

    if app.editing_query {
        app.query_editor.render_into(frame, editor_inner);
    } else if app.query_text.is_empty() {
        let hint = Paragraph::new(
            "Double-click a table in the sidebar\nto generate a SELECT query.\n\
             Ctrl+E  Edit query   Ctrl+R  Run query",
        )
        .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(hint, editor_inner);
    } else {
        let text = Paragraph::new(app.query_text.as_str()).wrap(Wrap { trim: false });
        frame.render_widget(text, editor_inner);
    }

    // --- Results pane ---
    let results_block = Block::default()
        .title(" Results ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let results_inner = results_block.inner(results_area);
    frame.render_widget(&results_block, results_area);

    if app.query_results.is_empty() {
        let hint = Paragraph::new("Run a query to see results here.")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(hint, results_inner);
    } else {
        let mut lines: Vec<Line> = Vec::new();
        for line_str in app.query_results.lines() {
            let style = if line_str.contains("rows affected")
                || line_str.starts_with('(')
                || line_str.starts_with("Error")
            {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else if line_str.contains("---") {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::White)
            };
            lines.push(Line::from(Span::styled(line_str, style)));
        }
        frame.render_widget(
            Paragraph::new(ratatui::text::Text::from(lines)),
            results_inner,
        );
    }
}
