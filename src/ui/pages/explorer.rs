use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders},
    Frame,
};

use crate::app::App;
use crate::ui::sidebar;

pub fn render(frame: &mut Frame, area: Rect, app: &App, focused: bool) {
    let border_color = if focused { Color::Cyan } else { Color::White };
    let border = || Style::default().fg(border_color);
    if app.explorer_tree.is_empty() {
        let block = Block::default()
            .title(" Database Explorer ")
            .borders(Borders::ALL)
            .border_style(border());
        let text =
            "No database connected.\n\nUse Connections (2) to add and connect to a database.";
        let paragraph = ratatui::widgets::Paragraph::new(text).block(block);
        frame.render_widget(paragraph, area);
        return;
    }

    let id = match app.explorer_tree.selected_id() {
        Some(id) => id,
        None => {
            render_placeholder(frame, area, focused);
            return;
        }
    };

    if id == "server" {
        let conn_name = app
            .pool
            .active_id
            .and_then(|id| app.connections.iter().find(|c| c.id == id))
            .map(|c| c.name.as_str())
            .unwrap_or("Connected");
        let msg = format!("Connected to \"{}\"", conn_name);
        sidebar::render_generic_detail(frame, area, "Server", &msg, focused);
        return;
    }

    if let Some((prefix, name)) = id.split_once(':') {
        match prefix {
            "table" => {
                let columns = app.metadata_cache.get_columns(name);
                if let Some(columns) = columns {
                    sidebar::render_table_detail(frame, area, name, columns, focused);
                } else {
                    sidebar::render_generic_detail(
                        frame,
                        area,
                        "Table",
                        &format!("Table \"{}\" — press Enter to load columns", name),
                        focused,
                    );
                }
            }
            "view" => {
                sidebar::render_generic_detail(frame, area, "View", name, focused);
            }
            "tables" => {
                let count = app
                    .metadata_cache
                    .tables
                    .get(name)
                    .map(|t| t.len())
                    .unwrap_or(0);
                sidebar::render_generic_detail(
                    frame,
                    area,
                    "Tables",
                    &format!("{} tables in \"{}\"", count, name),
                    focused,
                );
            }
            "views" => {
                let count = app
                    .metadata_cache
                    .views
                    .get(name)
                    .map(|v| v.len())
                    .unwrap_or(0);
                sidebar::render_generic_detail(
                    frame,
                    area,
                    "Views",
                    &format!("{} views in \"{}\"", count, name),
                    focused,
                );
            }
            "indexes" => {
                sidebar::render_generic_detail(frame, area, "Indexes", name, focused);
            }
            "column" => {
                sidebar::render_generic_detail(frame, area, "Column", name, focused);
            }
            "db" => {
                sidebar::render_generic_detail(frame, area, "Database", name, focused);
            }
            "schema" => {
                sidebar::render_generic_detail(frame, area, "Schema", name, focused);
            }
            "sequences" => {
                sidebar::render_generic_detail(frame, area, "Sequences", name, focused);
            }
            "triggers" => {
                sidebar::render_generic_detail(frame, area, "Triggers", name, focused);
            }
            "functions" => {
                sidebar::render_generic_detail(frame, area, "Functions", name, focused);
            }
            "extensions" => {
                sidebar::render_generic_detail(frame, area, "Extensions", name, focused);
            }
            _ => {
                sidebar::render_generic_detail(frame, area, "Item", name, focused);
            }
        }
        return;
    }

    render_placeholder(frame, area, focused);
}

fn render_placeholder(frame: &mut Frame, area: Rect, focused: bool) {
    let border_color = if focused { Color::Cyan } else { Color::White };
    let block = Block::default()
        .title(" Database Explorer ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let text = "Navigate the tree in the sidebar.\nPress Enter to view details.";
    let paragraph = ratatui::widgets::Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}
