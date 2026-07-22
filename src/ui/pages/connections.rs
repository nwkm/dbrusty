use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::app::App;
use crate::models::connection::DatabaseType;

const FORM_FIELDS: &[&str] = &[
    "Name", "Type", "Host", "Port", "Database", "Username", "Password",
];

fn db_type_color(t: &DatabaseType) -> Color {
    match t {
        DatabaseType::Postgres => Color::Blue,
        DatabaseType::MySQL | DatabaseType::MariaDB => Color::Yellow,
        DatabaseType::SQLite => Color::Cyan,
        DatabaseType::SQLServer => Color::Red,
        DatabaseType::MongoDB => Color::Green,
        DatabaseType::Redis => Color::Magenta,
    }
}

fn db_type_icon(t: &DatabaseType) -> &'static str {
    match t {
        DatabaseType::Postgres => "PG",
        DatabaseType::MySQL => "MY",
        DatabaseType::MariaDB => "MA",
        DatabaseType::SQLite => "SQ",
        DatabaseType::SQLServer => "MS",
        DatabaseType::MongoDB => "MO",
        DatabaseType::Redis => "RE",
    }
}

fn render_connection_list(frame: &mut Frame, area: Rect, app: &App, focused: bool) {
    let border_color = if focused { Color::Cyan } else { Color::White };
    let block = Block::default()
        .title(" Connections ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(area);
    frame.render_widget(&block, area);

    if app.connections.is_empty() {
        let text = Paragraph::new("No connections saved.\n\nPress 'n' to add a new connection.")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        frame.render_widget(text, inner);
        return;
    }

    let list_items: Vec<Line> = app
        .connections
        .iter()
        .enumerate()
        .map(|(i, conn)| {
            let selected = i == app.conn_list_selected;
            let prefix = if selected { " ▸ " } else { "   " };
            let icon = db_type_icon(&conn.db_type);
            let color = db_type_color(&conn.db_type);
            let status = if Some(conn.id) == app.pool.active_id {
                " ●"
            } else {
                " ○"
            };

            let style = if selected {
                Style::default().fg(color).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(color)
            };

            let host_part = if conn.db_type == DatabaseType::SQLite {
                conn.database.clone()
            } else {
                format!("{}@{}:{}", conn.username, conn.host, conn.port)
            };

            Line::from(vec![
                Span::styled(prefix, Style::default().fg(Color::White)),
                Span::styled(
                    format!("{:>2} ", icon),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(&conn.name, style),
                Span::raw("  "),
                Span::styled(host_part, Style::default().fg(Color::DarkGray)),
                Span::styled(status, Style::default().fg(Color::Green)),
            ])
        })
        .collect();

    let scroll_offset = app.conn_list_scroll;

    let visible_items: Vec<Line> = list_items
        .into_iter()
        .skip(scroll_offset)
        .take(inner.height as usize)
        .collect();

    let paragraph = Paragraph::new(visible_items);
    frame.render_widget(paragraph, inner);
}

fn render_form_modal(frame: &mut Frame, area: Rect, app: &App) {
    let form = match &app.conn_form {
        Some(f) => f,
        None => return,
    };

    let modal_width = area.width.clamp(40, 52);
    let modal_height = 16;
    let x = area.x + (area.width.saturating_sub(modal_width)) / 2;
    let y = area.y + (area.height.saturating_sub(modal_height)) / 3;

    let modal_area = Rect::new(x, y, modal_width, modal_height);

    let title = if form.is_new {
        " Add Connection "
    } else {
        " Edit Connection "
    };

    frame.render_widget(Clear, modal_area);
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(modal_area);
    frame.render_widget(&block, modal_area);

    let field_rows = Layout::vertical(vec![Constraint::Length(1); FORM_FIELDS.len()]).split(inner);

    for (i, (label, row_area)) in FORM_FIELDS.iter().zip(field_rows.iter()).enumerate() {
        let val = form.field_value(i);
        let display = if val.is_empty() {
            Span::styled(
                format!(" {}: ", label),
                Style::default().fg(Color::DarkGray),
            )
        } else {
            let val_color = if i == form.active_field {
                Color::White
            } else {
                Color::Gray
            };
            Span::styled(
                format!(" {}: {}", label, val),
                Style::default().fg(val_color),
            )
        };
        let line = if i == form.active_field {
            Line::from(vec![
                Span::styled("> ", Style::default().fg(Color::Cyan)),
                display,
            ])
        } else {
            Line::from(vec![Span::raw("  "), display])
        };
        frame.render_widget(Paragraph::new(line), *row_area);
    }

    let hint_section = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(inner);
    let hint = Paragraph::new(" Tab:Next  Shift+Tab:Prev  Enter:Save  Esc:Cancel ")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    frame.render_widget(hint, hint_section[1]);

    if let Some(err) = &form.error {
        let err_line = Paragraph::new(err.as_str()).style(Style::default().fg(Color::Red));
        frame.render_widget(err_line, hint_section[0]);
    }

    let active_field = form.active_field;
    let label = FORM_FIELDS[active_field];
    let val = form.field_value(active_field);
    let cursor_y = y + 1 + active_field as u16;
    let cursor_x = x + 6 + label.len() as u16 + val.len() as u16;
    if cursor_x < x + modal_width - 1 {
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}

fn render_confirm_modal(frame: &mut Frame, area: Rect, app: &App) {
    let confirm_idx = match app.conn_page_mode {
        crate::app::ConnPageMode::Deleting(i) => i,
        _ => return,
    };

    let conn = match app.connections.get(confirm_idx) {
        Some(c) => c,
        None => return,
    };

    let modal_width = area.width.clamp(36, 44);
    let modal_height = 7;
    let x = area.x + (area.width.saturating_sub(modal_width)) / 2;
    let y = area.y + (area.height.saturating_sub(modal_height)) / 3;

    let modal_area = Rect::new(x, y, modal_width, modal_height);

    frame.render_widget(Clear, modal_area);
    let block = Block::default()
        .title(" Confirm Delete ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));
    let inner = block.inner(modal_area);
    frame.render_widget(&block, modal_area);

    let text = format!(" Delete connection \"{}\"?", conn.name);
    let paragraph = Paragraph::new(text)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, inner);

    let hint = " Enter:Confirm  Esc:Cancel ";
    let hint_p = Paragraph::new(hint)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    frame.render_widget(hint_p, inner);
}

fn build_hints(app: &App) -> String {
    let selected_conn = app.connections.get(app.conn_list_selected);
    let is_connected = selected_conn.is_some_and(|c| Some(c.id) == app.pool.active_id);
    let disconnect_hint = if is_connected {
        "  Alt-Enter:Disconnect"
    } else {
        ""
    };
    format!(
        " n:New  e:Edit  d:Delete  Enter:Connect{}  j/k:Navigate ",
        disconnect_hint
    )
}

pub fn render(frame: &mut Frame, area: Rect, app: &App, focused: bool) {
    let vertical = Layout::vertical([Constraint::Min(1), Constraint::Length(2)]);
    let [list_area, key_hints_area] = vertical.areas(area);

    render_connection_list(frame, list_area, app, focused);

    match app.conn_page_mode {
        crate::app::ConnPageMode::Adding | crate::app::ConnPageMode::Editing(_) => {
            render_form_modal(frame, area, app);
        }
        crate::app::ConnPageMode::Deleting(_) => {
            render_confirm_modal(frame, area, app);
        }
        _ => {}
    }

    let hints = build_hints(app);
    let hint_p = Paragraph::new(hints).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(hint_p, key_hints_area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::config::AppConfig;
    use crate::models::connection::ConnectionConfig;

    fn test_app() -> App {
        App::with_config(AppConfig::default())
    }

    fn app_with_connections(count: usize) -> App {
        let mut app = test_app();
        app.connections = (0..count)
            .map(|i| ConnectionConfig {
                id: uuid::Uuid::from_u128(i as u128 + 1),
                name: format!("conn-{}", i),
                db_type: DatabaseType::Postgres,
                host: "localhost".into(),
                port: 5432,
                database: "test".into(),
                username: "user".into(),
                group: None,
                is_favorite: false,
                ssl: false,
                created_at: chrono::Utc::now(),
                last_connected: None,
                password: None,
            })
            .collect();
        app
    }

    fn connect_nth(app: &mut App, index: usize) {
        if let Some(conn) = app.connections.get(index) {
            let id = conn.id;
            app.pool
                .connect(
                    id,
                    Box::new(crate::providers::sqlite::SqliteProvider::new()),
                    conn,
                )
                .ok();
        }
    }

    #[test]
    fn hint_no_connections() {
        let app = test_app();
        let hints = build_hints(&app);
        assert!(!hints.contains("Disconnect"));
        assert!(hints.contains("n:New"));
        assert!(hints.contains("n:New"));
    }

    #[test]
    fn hint_connection_not_connected() {
        let app = app_with_connections(1);
        let hints = build_hints(&app);
        assert!(
            !hints.contains("Disconnect"),
            "should not show disconnect hint when not connected: {:?}",
            hints
        );
    }

    #[test]
    fn hint_connection_is_connected() {
        let mut app = app_with_connections(1);
        connect_nth(&mut app, 0);
        let hints = build_hints(&app);
        assert!(
            hints.contains("Disconnect"),
            "should show disconnect hint when connected: {:?}",
            hints
        );
    }

    #[test]
    fn hint_non_selected_connected() {
        let mut app = app_with_connections(2);
        app.conn_list_selected = 0;
        connect_nth(&mut app, 1);
        let hints = build_hints(&app);
        assert!(
            !hints.contains("Disconnect"),
            "should not show disconnect for non-selected connected conn: {:?}",
            hints
        );
    }

    #[test]
    fn hint_connected_then_different_selected() {
        let mut app = app_with_connections(2);
        connect_nth(&mut app, 0);
        app.conn_list_selected = 0;
        let hints = build_hints(&app);
        assert!(hints.contains("Disconnect"));

        app.conn_list_selected = 1;
        let hints = build_hints(&app);
        assert!(!hints.contains("Disconnect"));
    }
}
