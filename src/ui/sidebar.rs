use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame,
};

use crate::app::{App, Focus};
use crate::database::types::{ColumnInfo, TableInfo, ViewInfo};
use crate::widgets::tree::{render_tree, TreeItem, TreeState};

pub fn render_sidebar(frame: &mut Frame, area: Rect, app: &mut App) {
    let focused = app.focus == Focus::Sidebar;
    if !app.explorer_tree.is_empty() {
        render_tree(frame, area, &mut app.explorer_tree, focused);
    } else if app.pool.active_id.is_some() {
        render_loading(frame, area, focused);
    } else {
        render_no_connection(frame, area, focused);
    }
}

fn render_no_connection(frame: &mut Frame, area: Rect, focused: bool) {
    let border_color = if focused { Color::Cyan } else { Color::White };
    let block = Block::default()
        .title(" Explorer ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let text = Paragraph::new("No connection").block(block);
    frame.render_widget(text, area);
}

fn render_loading(frame: &mut Frame, area: Rect, focused: bool) {
    let border_color = if focused { Color::Cyan } else { Color::White };
    let block = Block::default()
        .title(" Explorer ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let text = Paragraph::new("Loading...").block(block);
    frame.render_widget(text, area);
}

pub fn build_initial_tree(
    conn_name: &str,
    databases: &[String],
    schemas: &[(String, Vec<String>)],
    tables: &[(String, Vec<TableInfo>)],
    views: &[(String, Vec<ViewInfo>)],
) -> TreeState {
    let server_children: Vec<TreeItem> = databases
        .iter()
        .map(|db| {
            let db_schemas = schemas.iter().find(|(s, _)| s == db);
            let schema_list: Vec<String> = db_schemas.cloned().unwrap_or_default().1;

            if schema_list.len() == 1 && &schema_list[0] == db {
                build_flat_database(db, tables, views)
            } else {
                build_schema_database(db, &schema_list, tables, views)
            }
        })
        .collect();

    let server = TreeItem::branch("server", conn_name, 0, server_children);
    TreeState::new(vec![server])
}

fn build_flat_database(
    db: &str,
    tables: &[(String, Vec<TableInfo>)],
    views: &[(String, Vec<ViewInfo>)],
) -> TreeItem {
    let schema_key = format!("{}.{}", db, db);
    let db_tables = tables.iter().find(|(s, _)| s == &schema_key);
    let db_views = views.iter().find(|(s, _)| s == &schema_key);

    let mut children = Vec::new();

    if let Some((_, table_list)) = db_tables {
        let table_items: Vec<TreeItem> = table_list
            .iter()
            .map(|t| {
                if t.table_type == "REDIS_TYPE" {
                    TreeItem::group(&format!("table:{}.{}", db, t.name), &t.name, 3)
                } else {
                    TreeItem::leaf(&format!("table:{}.{}", db, t.name), &t.name, 3)
                }
            })
            .collect();
        let label = format!("Tables ({})", table_list.len());
        children.push(TreeItem::branch(
            &format!("tables:{}", db),
            &label,
            2,
            table_items,
        ));
    }

    if let Some((_, view_list)) = db_views {
        let view_items: Vec<TreeItem> = view_list
            .iter()
            .map(|v| TreeItem::leaf(&format!("view:{}.{}", db, v.name), &v.name, 3))
            .collect();
        let label = format!("Views ({})", view_list.len());
        children.push(TreeItem::branch(
            &format!("views:{}", db),
            &label,
            2,
            view_items,
        ));
    }

    children.push(TreeItem::group(&format!("indexes:{}", db), "Indexes", 2));

    TreeItem::branch(&format!("db:{}", db), db, 1, children)
}

fn build_schema_database(
    db: &str,
    schema_list: &[String],
    tables: &[(String, Vec<TableInfo>)],
    views: &[(String, Vec<ViewInfo>)],
) -> TreeItem {
    let mut schema_children: Vec<TreeItem> = schema_list
        .iter()
        .map(|schema| {
            let schema_key = format!("{}.{}", db, schema);
            let db_tables = tables.iter().find(|(s, _)| s == &schema_key);
            let db_views = views.iter().find(|(s, _)| s == &schema_key);

            let mut children = Vec::new();

            if let Some((_, table_list)) = db_tables {
                let table_items: Vec<TreeItem> = table_list
                    .iter()
                    .map(|t| TreeItem::leaf(&format!("table:{}.{}", schema, t.name), &t.name, 4))
                    .collect();
                let label = format!("Tables ({})", table_list.len());
                children.push(TreeItem::branch(
                    &format!("tables:{}", schema_key),
                    &label,
                    3,
                    table_items,
                ));
            }

            if let Some((_, view_list)) = db_views {
                let view_items: Vec<TreeItem> = view_list
                    .iter()
                    .map(|v| TreeItem::leaf(&format!("view:{}.{}", schema, v.name), &v.name, 4))
                    .collect();
                let label = format!("Views ({})", view_list.len());
                children.push(TreeItem::branch(
                    &format!("views:{}", schema_key),
                    &label,
                    3,
                    view_items,
                ));
            }

            children.push(TreeItem::group(
                &format!("sequences:{}", schema_key),
                "Sequences",
                3,
            ));
            children.push(TreeItem::group(
                &format!("triggers:{}", schema_key),
                "Triggers",
                3,
            ));
            children.push(TreeItem::group(
                &format!("functions:{}", schema_key),
                "Functions",
                3,
            ));

            TreeItem {
                id: format!("schema:{}", schema_key),
                label: schema.clone(),
                depth: 2,
                expanded: schema == "public",
                has_children: !children.is_empty(),
                children,
            }
        })
        .collect();

    schema_children.push(TreeItem::group(
        &format!("extensions:{}", db),
        "Extensions",
        2,
    ));

    TreeItem::branch(&format!("db:{}", db), db, 1, schema_children)
}

pub fn render_table_detail(
    frame: &mut Frame,
    area: Rect,
    label: &str,
    columns: &[ColumnInfo],
    focused: bool,
) {
    let border_color = if focused { Color::Cyan } else { Color::White };
    let block = Block::default()
        .title(format!(" Table: {}  [{} columns] ", label, columns.len()))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let name_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let type_style = Style::default().fg(Color::Yellow);
    let pk_style = Style::default().fg(Color::Green);
    let fk_style = Style::default().fg(Color::Magenta);
    let no_style = Style::default().fg(Color::DarkGray);
    let null_style = Style::default().fg(Color::Red);
    let def_style = Style::default().fg(Color::Blue);

    let header_style = Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);
    let header = Row::new(vec![
        Cell::from(Span::styled("Name", header_style)),
        Cell::from(Span::styled("Type", header_style)),
        Cell::from(Span::styled("Nullable", header_style)),
        Cell::from(Span::styled("PK", header_style)),
        Cell::from(Span::styled("FK", header_style)),
        Cell::from(Span::styled("Default", header_style)),
    ]);

    let rows: Vec<Row> = columns
        .iter()
        .map(|col| {
            let nullable = if col.nullable {
                Span::styled("Yes", null_style)
            } else {
                Span::styled("No", no_style)
            };
            let pk = if col.is_pk {
                Span::styled("PK", pk_style)
            } else {
                Span::styled("—", no_style)
            };
            let fk = if col.is_fk {
                Span::styled("FK", fk_style)
            } else {
                Span::styled("—", no_style)
            };
            let default_str = col
                .default
                .as_deref()
                .map(|d| Span::styled(d, def_style))
                .unwrap_or_else(|| Span::styled("NULL", no_style));

            Row::new(vec![
                Cell::from(Span::styled(&col.name, name_style)),
                Cell::from(Span::styled(&col.type_name, type_style)),
                Cell::from(nullable),
                Cell::from(pk),
                Cell::from(fk),
                Cell::from(default_str),
            ])
        })
        .collect();

    let widths = vec![
        Constraint::Length(20),
        Constraint::Length(20),
        Constraint::Length(10),
        Constraint::Length(4),
        Constraint::Length(4),
        Constraint::Fill(1),
    ];

    let table = Table::new(rows, widths).header(header).column_spacing(2);

    frame.render_widget(table, inner);
}

pub fn render_column_detail(
    frame: &mut Frame,
    area: Rect,
    table_label: &str,
    col: &ColumnInfo,
    focused: bool,
) {
    let border_color = if focused { Color::Cyan } else { Color::White };
    let block = Block::default()
        .title(format!(" Column: {} ", table_label))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let bold = Style::default().add_modifier(Modifier::BOLD);
    let yes = Style::default().fg(Color::Green);
    let no = Style::default().fg(Color::Red);

    let pk = if col.is_pk { "Yes" } else { "No" };
    let fk = if col.is_fk { "Yes" } else { "No" };
    let nullable = if col.nullable { "Yes" } else { "No" };
    let auto_inc = if col.is_auto_increment { "Yes" } else { "No" };
    let default_str = col.default.as_deref().unwrap_or("NULL");

    let rows = vec![
        Row::new(vec![
            Cell::from(Span::styled("Name", bold)),
            Cell::from(col.name.as_str()),
        ]),
        Row::new(vec![
            Cell::from(Span::styled("Type", bold)),
            Cell::from(col.type_name.as_str()),
        ]),
        Row::new(vec![
            Cell::from(Span::styled("Primary Key", bold)),
            Cell::from(Span::styled(pk, if col.is_pk { yes } else { no })),
        ]),
        Row::new(vec![
            Cell::from(Span::styled("Foreign Key", bold)),
            Cell::from(Span::styled(fk, if col.is_fk { yes } else { no })),
        ]),
        Row::new(vec![
            Cell::from(Span::styled("Default", bold)),
            Cell::from(default_str),
        ]),
        Row::new(vec![
            Cell::from(Span::styled("Nullable", bold)),
            Cell::from(Span::styled(nullable, if col.nullable { yes } else { no })),
        ]),
        Row::new(vec![
            Cell::from(Span::styled("Auto Increment", bold)),
            Cell::from(Span::styled(
                auto_inc,
                if col.is_auto_increment { yes } else { no },
            )),
        ]),
    ];

    let header_style = Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);
    let header = Row::new(vec![
        Cell::from(Span::styled("Property", header_style)),
        Cell::from(Span::styled("Value", header_style)),
    ]);

    let table = Table::new(rows, vec![Constraint::Length(18), Constraint::Fill(1)])
        .header(header)
        .column_spacing(2);

    frame.render_widget(table, inner);
}

#[allow(dead_code)]
pub fn render_server_info(
    frame: &mut Frame,
    area: Rect,
    label: &str,
    info: &crate::database::types::ServerInfo,
    focused: bool,
) {
    let border_color = if focused { Color::Cyan } else { Color::White };
    let block = Block::default()
        .title(format!(" Server: {} ", label))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines = vec![
        Line::from(format!("  Vendor:     {}", info.vendor)),
        Line::from(format!("  Version:    {}", info.version)),
        Line::from(format!(
            "  Uptime:     {}",
            info.connection_uptime.as_deref().unwrap_or("N/A")
        )),
    ];
    let text = ratatui::text::Text::from(lines);
    frame.render_widget(Paragraph::new(text), inner);
}

pub fn render_generic_detail(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    message: &str,
    focused: bool,
) {
    let border_color = if focused { Color::Cyan } else { Color::White };
    let block = Block::default()
        .title(format!(" {} ", title))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let text = Paragraph::new(message);
    frame.render_widget(text, inner);
}
