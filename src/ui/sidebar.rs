use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph},
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
            .map(|t| TreeItem::leaf(&format!("table:{}.{}", db, t.name), &t.name, 3))
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
        .title(format!(" Table: {} ", label))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = vec![
        Line::from(format!(" Columns ({})", columns.len())),
        Line::from(String::new()),
    ];

    for col in columns {
        let pk = if col.is_pk { " PK" } else { "" };
        let null = if col.nullable { "" } else { " NOT NULL" };
        let def = col
            .default
            .as_ref()
            .map(|d| format!(" DEFAULT {}", d))
            .unwrap_or_default();
        lines.push(Line::from(format!(
            "  {}  {}  {}{}{}",
            col.name, col.type_name, pk, null, def
        )));
    }

    let text = ratatui::text::Text::from(lines);
    frame.render_widget(Paragraph::new(text), inner);
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
