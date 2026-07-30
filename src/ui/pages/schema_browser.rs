use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::{App, Focus, SchemaBrowserCategory, SchemaBrowserMode};
use crate::database::types::ColumnInfo;

pub fn render(frame: &mut Frame, area: Rect, app: &App, focused: bool) {
    let border_color = if focused { Color::Cyan } else { Color::White };
    let block = Block::default()
        .title(" Schema Browser ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let inner = block.inner(area);
    frame.render_widget(&block, area);

    if app.pool.active_id.is_none() {
        let text =
            Paragraph::new("No active connection.").style(Style::default().fg(Color::DarkGray));
        frame.render_widget(text, inner);
        return;
    }

    let show_create = app.schema_browser.mode == SchemaBrowserMode::CreatingTable;
    let show_drop = matches!(
        &app.schema_browser.mode,
        SchemaBrowserMode::DroppingTable(_)
    );
    let show_rename = matches!(
        &app.schema_browser.mode,
        SchemaBrowserMode::RenamingTable(_)
    );
    let show_alter = matches!(
        &app.schema_browser.mode,
        SchemaBrowserMode::AlteringTable(_)
    );
    let show_import = app.schema_browser.mode == SchemaBrowserMode::ImportingSchema;
    let show_export = !app.schema_browser.export_ddl.is_empty();
    let show_diff = app.schema_browser.mode == SchemaBrowserMode::SchemaDiff;

    let horizontal = Layout::horizontal([Constraint::Ratio(1, 4), Constraint::Ratio(3, 4)]);
    let [cat_area, detail_area] = horizontal.areas(inner);

    render_categories(frame, cat_area, app, focused);
    render_detail(frame, detail_area, app, focused);

    if show_create {
        render_create_table_modal(frame, area, app);
    } else if show_drop {
        if let SchemaBrowserMode::DroppingTable(ref name) = app.schema_browser.mode {
            render_drop_confirm_modal(frame, area, name, focused);
        }
    } else if show_rename {
        if let SchemaBrowserMode::RenamingTable(ref name) = app.schema_browser.mode {
            render_rename_modal(frame, area, name, app);
        }
    } else if show_alter {
        if let SchemaBrowserMode::AlteringTable(ref name) = app.schema_browser.mode {
            render_alter_table_modal(frame, area, name, app);
        }
    } else if show_import {
        render_import_schema_modal(frame, area, app);
    } else if show_export {
        render_export_modal(frame, area, app);
    } else if show_diff {
        render_schema_diff_modal(frame, area, app);
    }
}

fn render_categories(frame: &mut Frame, area: Rect, app: &App, focused: bool) {
    let side_focused = focused && app.focus == Focus::Main;
    let side_color = if side_focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(side_color));
    let inner = block.inner(area);
    frame.render_widget(&block, area);

    let schema_key = app
        .pool
        .active_id
        .and_then(|id| app.connections.iter().find(|c| c.id == id))
        .map(|c| c.database.as_str())
        .unwrap_or("main");
    let header = format!(" Schema: {}", schema_key);
    let header_line = Line::from(Span::styled(
        header,
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    ));
    let mut lines = vec![header_line, Line::from("")];
    for (i, cat) in SchemaBrowserCategory::all().iter().enumerate() {
        let is_sel = i == app.schema_browser.selected_category && side_focused;
        let prefix = if is_sel { "> " } else { "  " };
        let count = get_category_count(app, cat);
        let label = format!("{} {} ({})", prefix, cat.label(), count);
        let style = if is_sel {
            Style::default()
                .fg(Color::Black)
                .bg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        lines.push(Line::from(Span::styled(label, style)));
    }
    frame.render_widget(Paragraph::new(Text::from(lines)), inner);
}

fn get_category_count(app: &App, cat: &SchemaBrowserCategory) -> usize {
    let db_name = app
        .pool
        .active_id
        .and_then(|id| {
            app.connections
                .iter()
                .find(|c| c.id == id)
                .map(|c| c.database.as_str())
        })
        .unwrap_or("main");
    match cat {
        SchemaBrowserCategory::Tables => app.metadata_cache.tables.values().map(|v| v.len()).sum(),
        SchemaBrowserCategory::Views => app.metadata_cache.views.values().map(|v| v.len()).sum(),
        SchemaBrowserCategory::Indexes => {
            // Count indexes from cache
            let mut total = 0;
            for tables in app.metadata_cache.tables.values() {
                for t in tables {
                    let key = format!("{}.{}", db_name, t.name);
                    if let Some(idxs) = app.metadata_cache.indexes.get(&key) {
                        total += idxs.len();
                    }
                }
            }
            total
        }
        _ => 0,
    }
}

fn render_detail(frame: &mut Frame, area: Rect, app: &App, focused: bool) {
    let detail_focused = focused && app.focus == Focus::Main;
    let detail_color = if detail_focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(detail_color));
    let inner = block.inner(area);
    frame.render_widget(&block, area);

    let cat_list = SchemaBrowserCategory::all();
    if app.schema_browser.selected_category >= cat_list.len() {
        return;
    }
    let cat = &cat_list[app.schema_browser.selected_category];
    let db_name = app
        .pool
        .active_id
        .and_then(|id| {
            app.connections
                .iter()
                .find(|c| c.id == id)
                .map(|c| c.database.as_str())
        })
        .unwrap_or("main");

    let items = match cat {
        SchemaBrowserCategory::Tables => {
            let mut v: Vec<String> = app
                .metadata_cache
                .tables
                .values()
                .flat_map(|tables| tables.iter().map(|t| t.name.clone()))
                .collect();
            v.sort();
            v
        }
        SchemaBrowserCategory::Views => {
            let mut v: Vec<String> = app
                .metadata_cache
                .views
                .values()
                .flat_map(|views| views.iter().map(|v| v.name.clone()))
                .collect();
            v.sort();
            v
        }
        SchemaBrowserCategory::Indexes => {
            let mut v: Vec<String> = Vec::new();
            for tables in app.metadata_cache.tables.values() {
                for t in tables {
                    let key = format!("{}.{}", db_name, t.name);
                    if let Some(idxs) = app.metadata_cache.indexes.get(&key) {
                        for idx in idxs {
                            v.push(format!("{}.{}", t.name, idx.name));
                        }
                    }
                }
            }
            v.sort();
            v
        }
        _ => Vec::new(),
    };

    if items.is_empty() {
        let text = Paragraph::new(format!("No {} found.", cat.label()))
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(text, inner);
        return;
    }

    let item_count = items.len();
    let title = format!(" {} ({}) ", cat.label(), item_count);
    let title_span = Line::from(Span::styled(
        title,
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    ));

    // Calculate visible item count based on available height
    let visible_items = (inner.height.saturating_sub(4) as usize).min(12).max(3);

    // Split detail area: top has item list, bottom has DDL
    let list_rows = visible_items.min(item_count) as u16 + 1;
    let chunks = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(list_rows),
        Constraint::Min(0),
    ])
    .areas(inner);
    let [_title_area, item_area, ddl_area] = chunks;

    // Render title
    frame.render_widget(Paragraph::new(title_span), _title_area);

    // Render item list with scroll
    let scroll = app
        .schema_browser
        .item_scroll
        .min(item_count.saturating_sub(visible_items));
    let mut item_lines: Vec<Line> = items
        .iter()
        .enumerate()
        .skip(scroll)
        .take(visible_items)
        .map(|(i, name)| {
            let is_sel = i == app.schema_browser.selected_item && detail_focused;
            let prefix = if is_sel { "> " } else { "  " };
            let label = format!("{}{}", prefix, name);
            let style = if is_sel {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            Line::from(Span::styled(label, style))
        })
        .collect();

    let remaining = item_count.saturating_sub(scroll + visible_items);
    if remaining > 0 {
        item_lines.push(Line::from(Span::styled(
            format!("  ... {} more (scroll with h/l)", remaining),
            Style::default().fg(Color::DarkGray),
        )));
    }
    frame.render_widget(Paragraph::new(Text::from(item_lines)), item_area);

    // Render DDL / columns for selected item
    if app.schema_browser.selected_item < items.len() {
        let item_name = &items[app.schema_browser.selected_item];
        let ddl = get_item_ddl(app, cat, item_name);

        let (ddl_lines, base_style): (Vec<Line<'static>>, Style) = if let Some(text) = ddl {
            // DDL from database — plain green text
            let lines: Vec<Line> = text
                .lines()
                .map(|l| {
                    Line::from(Span::styled(
                        l.to_string(),
                        Style::default().fg(Color::Green),
                    ))
                })
                .collect();
            (lines, Style::default().fg(Color::Green))
        } else {
            let cols = get_item_columns(app, item_name);
            if cols.is_empty() {
                let msg = "Columns not loaded — navigate with h/l to load.";
                let lines = vec![Line::from(Span::styled(
                    msg,
                    Style::default().fg(Color::DarkGray),
                ))];
                (lines, Style::default().fg(Color::DarkGray))
            } else {
                (
                    format_colored_columns(&cols),
                    Style::default().fg(Color::Cyan),
                )
            }
        };

        let scroll = app.schema_browser.scroll.min(
            ddl_lines
                .len()
                .saturating_sub(ddl_area.height as usize)
                .max(0),
        );
        let visible: Vec<Line> = ddl_lines
            .into_iter()
            .skip(scroll)
            .take(ddl_area.height as usize)
            .collect();
        let ddl_para = Paragraph::new(Text::from(visible)).style(base_style);
        frame.render_widget(ddl_para, ddl_area);
    }
}

fn get_item_columns(app: &App, table_name: &str) -> Vec<ColumnInfo> {
    for (_key, tables) in &app.metadata_cache.tables {
        for t in tables {
            if t.name == table_name {
                let schema_key = format!("{}.{}", t.schema, t.name);
                if let Some(cols) = app.metadata_cache.get_columns(&schema_key) {
                    return cols.to_vec();
                }
                let direct_key = format!("main.{}", table_name);
                if let Some(cols) = app.metadata_cache.get_columns(&direct_key) {
                    return cols.to_vec();
                }
            }
        }
    }
    // Try direct lookup
    if let Some(cols) = app.metadata_cache.get_columns(table_name) {
        return cols.to_vec();
    }
    Vec::new()
}

fn get_item_ddl(_app: &App, _cat: &SchemaBrowserCategory, _item_name: &str) -> Option<String> {
    // DDL is stored in the schema_browser ddl_text if it was loaded
    // For now, return None to show column-based ddl
    None
}

fn format_colored_columns(cols: &[ColumnInfo]) -> Vec<Line<'static>> {
    let cols = cols.to_vec();
    if cols.is_empty() {
        return vec![Line::from(Span::styled(
            "  No columns",
            Style::default().fg(Color::DarkGray),
        ))];
    }
    let mut out: Vec<_> = Vec::new();
    out.push(Line::from(Span::styled(
        "-- Columns",
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
    )));
    // Header
    let header_style = Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);
    let header = Line::from(vec![
        Span::styled("  Name                ", header_style.clone()),
        Span::styled("Type                ", header_style.clone()),
        Span::styled("Nullable ", header_style.clone()),
        Span::styled("PK  ", header_style.clone()),
        Span::styled("Default", header_style),
    ]);
    out.push(header);
    out.push(Line::from(Span::styled(
        "  ---------------------------------------------------------------------",
        Style::default().fg(Color::DarkGray),
    )));

    let name_style = Style::default().fg(Color::Cyan);
    let type_style = Style::default().fg(Color::Yellow);
    let pk_style = Style::default().fg(Color::Green);
    let no_style = Style::default().fg(Color::DarkGray);
    let null_style = Style::default().fg(Color::Red);
    let def_style = Style::default().fg(Color::Blue);

    for col in cols {
        let nullable = if col.nullable { "YES" } else { "NO" };
        let nullable_style = if col.nullable { null_style } else { no_style };
        let pk = if col.is_pk { "PK" } else { "" };
        let pk_style_col = if col.is_pk { pk_style } else { no_style };
        let def = col.default.clone().unwrap_or_default();
        let def_span = if col.default.is_some() {
            Span::styled(def, def_style)
        } else {
            Span::styled(String::new(), no_style)
        };
        out.push(Line::from(vec![
            Span::styled(format!("  {:<20} ", col.name), name_style),
            Span::styled(format!("{:<20} ", col.type_name), type_style),
            Span::styled(format!("{:<9} ", nullable), nullable_style),
            Span::styled(format!("{:<4} ", pk), pk_style_col),
            def_span,
        ]));
    }
    out
}

pub fn render_create_table_modal(frame: &mut Frame, area: Rect, app: &App) {
    let modal_w = area.width.min(50);
    let modal_h = 12;
    let x = area.x + (area.width.saturating_sub(modal_w)) / 2;
    let y = area.y + (area.height.saturating_sub(modal_h)) / 3;
    let modal_area = Rect::new(x, y, modal_w, modal_h);

    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .title(" Create Table ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(modal_area);
    frame.render_widget(&block, modal_area);

    let name_label = "Table Name:";
    let prefix = if app.schema_browser.create_field == 0 {
        "> "
    } else {
        "  "
    };
    let name_line = Line::from(vec![
        Span::styled(
            format!("{} {}", prefix, name_label),
            Style::default().fg(Color::White),
        ),
        Span::styled(
            app.schema_browser.create_name.as_str(),
            if app.schema_browser.create_field == 0 {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            },
        ),
    ]);

    let fields = vec![name_line];

    let mut render_lines: Vec<Line> = Vec::new();
    for line in &fields {
        render_lines.push(line.clone());
    }

    if let Some(ref err) = app.schema_browser.create_error {
        render_lines.push(Line::from(Span::styled(
            err.as_str(),
            Style::default().fg(Color::Red),
        )));
    }

    let hint = " Tab:Next  Enter:Create  Esc:Cancel ";
    render_lines.push(Line::from(Span::styled(
        hint,
        Style::default().fg(Color::DarkGray),
    )));

    frame.render_widget(Paragraph::new(Text::from(render_lines)), inner);
}

fn render_drop_confirm_modal(frame: &mut Frame, area: Rect, table_name: &str, _focused: bool) {
    let modal_w = area.width.min(46);
    let modal_h = 8;
    let x = area.x + (area.width.saturating_sub(modal_w)) / 2;
    let y = area.y + (area.height.saturating_sub(modal_h)) / 3;
    let modal_area = Rect::new(x, y, modal_w, modal_h);

    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .title(" Confirm Drop ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));
    let inner = block.inner(modal_area);
    frame.render_widget(&block, modal_area);

    let lines = vec![
        Line::from(Span::styled(
            format!("Drop table \"{}\"?", table_name),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "This cannot be undone.",
            Style::default().fg(Color::Red),
        )),
        Line::from(""),
        Line::from(Span::styled(
            " y:Confirm  n:Esc:Cancel ",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    frame.render_widget(Paragraph::new(Text::from(lines)), inner);
}

fn render_rename_modal(frame: &mut Frame, area: Rect, table_name: &str, app: &App) {
    let modal_w = area.width.min(46);
    let modal_h = 8;
    let x = area.x + (area.width.saturating_sub(modal_w)) / 2;
    let y = area.y + (area.height.saturating_sub(modal_h)) / 3;
    let modal_area = Rect::new(x, y, modal_w, modal_h);

    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .title(" Rename Table ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(modal_area);
    frame.render_widget(&block, modal_area);

    let input = &app.schema_browser.create_name;
    let lines = vec![
        Line::from(Span::styled(
            format!("Rename \"{}\" to:", table_name),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!("> {}", input),
            Style::default().fg(Color::Cyan),
        )),
        Line::from(""),
        Line::from(Span::styled(
            " Enter:Confirm  Esc:Cancel ",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    frame.render_widget(Paragraph::new(Text::from(lines)), inner);
}

pub fn handle_key(app: &mut App, key: &crossterm::event::KeyEvent) -> bool {
    use crossterm::event::KeyCode;

    // Handle modal modes first
    // If export DDL is displayed, handle Esc/j/k/c in export mode
    if !app.schema_browser.export_ddl.is_empty()
        && app.schema_browser.mode == SchemaBrowserMode::Browse
    {
        match key.code {
            KeyCode::Esc => {
                app.schema_browser.export_ddl.clear();
                app.schema_browser.ddl_text.clear();
                return true;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                let max = app
                    .schema_browser
                    .export_ddl
                    .lines()
                    .count()
                    .saturating_sub(1);
                if app.schema_browser.scroll < max {
                    app.schema_browser.scroll += 1;
                }
                return true;
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if app.schema_browser.scroll > 0 {
                    app.schema_browser.scroll -= 1;
                }
                return true;
            }
            KeyCode::Char('c') => {
                if let Ok(mut cb) = arboard::Clipboard::new() {
                    let _ = cb.set_text(&app.schema_browser.export_ddl);
                }
                app.set_status("Schema DDL copied to clipboard.");
                return true;
            }
            _ => {}
        }
    }

    // Auto-load columns for the currently selected item if needed
    cache_selected_item_columns(app);

    match app.schema_browser.mode {
        SchemaBrowserMode::CreatingTable => {
            return handle_create_key(app, key);
        }
        SchemaBrowserMode::DroppingTable(_) => {
            return handle_drop_confirm_key(app, key);
        }
        SchemaBrowserMode::RenamingTable(_) => {
            return handle_rename_key(app, key);
        }
        SchemaBrowserMode::AlteringTable(_) => {
            return handle_alter_key(app, key);
        }
        SchemaBrowserMode::ImportingSchema => {
            return handle_import_key(app, key);
        }
        SchemaBrowserMode::SchemaDiff => {
            return handle_diff_key(app, key);
        }
        SchemaBrowserMode::Browse => {}
    }

    match key.code {
        // Navigate categories
        KeyCode::Char('j') | KeyCode::Down => {
            let cat_count = SchemaBrowserCategory::all().len();
            if app.schema_browser.selected_category + 1 < cat_count {
                app.schema_browser.selected_category += 1;
                app.schema_browser.selected_item = 0;
                app.schema_browser.item_scroll = 0;
                app.schema_browser.ddl_text.clear();
            }
            true
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if app.schema_browser.selected_category > 0 {
                app.schema_browser.selected_category -= 1;
                app.schema_browser.selected_item = 0;
                app.schema_browser.item_scroll = 0;
                app.schema_browser.ddl_text.clear();
            }
            true
        }

        // Navigate items
        KeyCode::Tab => {
            // Toggle between category list and item list by cycling focus
            app.focus = match app.focus {
                Focus::Main => Focus::Sidebar,
                _ => Focus::Main,
            };
            true
        }

        // Jump to first/last item
        KeyCode::Char('g') => {
            app.schema_browser.selected_item = 0;
            app.schema_browser.item_scroll = 0;
            app.schema_browser.ddl_text.clear();
            cache_selected_item_columns(app);
            true
        }
        KeyCode::Char('G') => {
            let items = get_current_items(app);
            if !items.is_empty() {
                app.schema_browser.selected_item = items.len() - 1;
                app.schema_browser.item_scroll = items.len().saturating_sub(12);
                app.schema_browser.ddl_text.clear();
                cache_selected_item_columns(app);
            }
            true
        }

        // Navigate items with l/h
        KeyCode::Char('l') | KeyCode::Right => {
            let items = get_current_items(app);
            if !items.is_empty() && app.schema_browser.selected_item + 1 < items.len() {
                app.schema_browser.selected_item += 1;
                app.schema_browser.ddl_text.clear();
                // Auto-scroll item list
                let visible = 12usize;
                if app.schema_browser.selected_item >= app.schema_browser.item_scroll + visible {
                    app.schema_browser.item_scroll =
                        app.schema_browser.selected_item.saturating_sub(visible) + 1;
                }
                cache_selected_item_columns(app);
            }
            true
        }
        KeyCode::Char('h') | KeyCode::Left => {
            if app.schema_browser.selected_item > 0 {
                app.schema_browser.selected_item -= 1;
                app.schema_browser.ddl_text.clear();
                // Auto-scroll item list
                if app.schema_browser.selected_item < app.schema_browser.item_scroll {
                    app.schema_browser.item_scroll = app.schema_browser.selected_item;
                    cache_selected_item_columns(app);
                }
            }
            true
        }

        // Actions
        KeyCode::Char('n') => {
            app.schema_browser.mode = SchemaBrowserMode::CreatingTable;
            app.schema_browser.create_name.clear();
            app.schema_browser.create_field = 0;
            app.schema_browser.create_error = None;
            app.set_status("Enter table name and press Enter to create.");
            true
        }
        KeyCode::Char('d') => {
            let items = get_current_items(app);
            if !items.is_empty() && app.schema_browser.selected_item < items.len() {
                let name = items[app.schema_browser.selected_item].clone();
                app.schema_browser.mode = SchemaBrowserMode::DroppingTable(name);
            }
            true
        }
        KeyCode::Char('r') => {
            let items = get_current_items(app);
            if !items.is_empty() && app.schema_browser.selected_item < items.len() {
                let name = items[app.schema_browser.selected_item].clone();
                app.schema_browser.mode = SchemaBrowserMode::RenamingTable(name.clone());
                app.schema_browser.create_name = name;
            }
            true
        }
        KeyCode::Char('c') => {
            // Copy DDL to clipboard
            let items = get_current_items(app);
            if !items.is_empty() && app.schema_browser.selected_item < items.len() {
                let item_name = &items[app.schema_browser.selected_item];
                let ddl = get_item_ddl_text(app, item_name);
                if !ddl.is_empty() {
                    if let Ok(mut cb) = arboard::Clipboard::new() {
                        let _ = cb.set_text(&ddl);
                    }
                    app.set_status(format!("Copied DDL for \"{}\" to clipboard.", item_name));
                } else {
                    app.set_status("No DDL to copy.");
                }
            }
            true
        }
        KeyCode::Char('e') => {
            // Edit in Query Editor
            let items = get_current_items(app);
            if !items.is_empty() && app.schema_browser.selected_item < items.len() {
                let item_name = &items[app.schema_browser.selected_item];
                let ddl = get_item_ddl_text(app, item_name);
                let _cat = &SchemaBrowserCategory::all()[app.schema_browser.selected_category];
                if !ddl.is_empty() {
                    app.export_sql = ddl;
                } else {
                    app.export_sql = format!("SELECT * FROM \"{}\" LIMIT 100;", item_name);
                }
                app.pending_action = crate::app::PendingAction::OpenQuery(app.export_sql.clone());
            }
            true
        }
        KeyCode::Char('f') => {
            // Fetch DDL from database
            let items = get_current_items(app);
            if !items.is_empty() && app.schema_browser.selected_item < items.len() {
                let item_name = &items[app.schema_browser.selected_item];
                let cat = &SchemaBrowserCategory::all()[app.schema_browser.selected_category];
                let cat_label = cat.label().to_lowercase();
                if app.pool.active_id.is_some() {
                    let db_schema = app
                        .connections
                        .iter()
                        .find(|c| Some(c.id) == app.pool.active_id)
                        .map(|c| c.database.as_str())
                        .unwrap_or("main")
                        .to_string();
                    if cat_label == "tables" {
                        app.pending_action = crate::app::PendingAction::GenerateDdl(db_schema);
                    } else if let Some(_provider) = app.pool.get_active() {
                        // We'll need to handle this async
                    }
                }
                app.schema_browser.ddl_text = format!("Fetching DDL for {}...", item_name);
            }
            true
        }

        KeyCode::Char('a') => {
            // Alter table
            let items = get_current_items(app);
            if !items.is_empty() && app.schema_browser.selected_item < items.len() {
                let name = items[app.schema_browser.selected_item].clone();
                app.schema_browser.mode = SchemaBrowserMode::AlteringTable(name);
                app.schema_browser.alter_action = 0;
                app.schema_browser.alter_column_name.clear();
                app.schema_browser.alter_column_type = String::from("TEXT");
                app.schema_browser.alter_focus = 0;
                app.set_status("Alter table: select action and fill fields.");
            }
            true
        }
        KeyCode::Char('i') => {
            // Import schema
            app.schema_browser.mode = SchemaBrowserMode::ImportingSchema;
            app.schema_browser.import_sql.clear();
            app.schema_browser.import_cursor = 0;
            app.set_status("Paste SQL and press Enter to execute.");
            true
        }
        // Note: char('I') is a different key from char('i')
        KeyCode::Char('E') => {
            // Export (Reverse engineer) full schema
            if app.pool.active_id.is_some() {
                let db_schema = app
                    .connections
                    .iter()
                    .find(|c| Some(c.id) == app.pool.active_id)
                    .map(|c| c.database.as_str())
                    .unwrap_or("main")
                    .to_string();
                app.pending_action = crate::app::PendingAction::GenerateDdl(db_schema);
                app.set_status("Generating schema DDL...");
            }
            true
        }
        KeyCode::Char('I') => {
            // Import from file (CSV/JSON/SQL)
            app.import_dialog = Some(crate::app::ImportFileDialog::new());
            app.set_status("Enter file path (.csv, .json, .sql):");
            true
        }

        KeyCode::Char('s') => {
            // Schema diff: capture first snapshot
            if app.schema_browser.diff_snapshot_a.is_empty() {
                let snap = build_schema_snapshot(app);
                app.schema_browser.diff_snapshot_a = snap;
                app.set_status("Snapshot A captured. Navigate to another state and press 's' again for Snapshot B.");
            } else if app.schema_browser.diff_snapshot_b.is_empty() {
                let snap = build_schema_snapshot(app);
                app.schema_browser.diff_snapshot_b = snap;
                app.schema_browser.diff_result = compute_schema_diff(app);
                app.schema_browser.mode = SchemaBrowserMode::SchemaDiff;
                app.set_status("Schema diff computed.");
            } else {
                // Reset and start over
                app.schema_browser.diff_snapshot_a.clear();
                app.schema_browser.diff_snapshot_b.clear();
                app.schema_browser.diff_result.clear();
                app.set_status("Diff reset. Press 's' to capture Snapshot A.");
            }
            true
        }

        _ => false,
    }
}

fn get_current_items(app: &App) -> Vec<String> {
    let cat_list = SchemaBrowserCategory::all();
    if app.schema_browser.selected_category >= cat_list.len() {
        return Vec::new();
    }
    let cat = &cat_list[app.schema_browser.selected_category];
    let db_name = app
        .pool
        .active_id
        .and_then(|id| {
            app.connections
                .iter()
                .find(|c| c.id == id)
                .map(|c| c.database.as_str())
        })
        .unwrap_or("main");

    match cat {
        SchemaBrowserCategory::Tables => {
            let mut v: Vec<String> = app
                .metadata_cache
                .tables
                .values()
                .flat_map(|tables| tables.iter().map(|t| t.name.clone()))
                .collect();
            v.sort();
            v
        }
        SchemaBrowserCategory::Views => {
            let mut v: Vec<String> = app
                .metadata_cache
                .views
                .values()
                .flat_map(|views| views.iter().map(|v| v.name.clone()))
                .collect();
            v.sort();
            v
        }
        SchemaBrowserCategory::Indexes => {
            let mut v: Vec<String> = Vec::new();
            for tables in app.metadata_cache.tables.values() {
                for t in tables {
                    let key = format!("{}.{}", db_name, t.name);
                    if let Some(idxs) = app.metadata_cache.indexes.get(&key) {
                        for idx in idxs {
                            v.push(format!("{}.{}", t.name, idx.name));
                        }
                    }
                }
            }
            v.sort();
            v
        }
        _ => Vec::new(),
    }
}

fn cache_selected_item_columns(app: &mut App) {
    let cat_list = SchemaBrowserCategory::all();
    if app.schema_browser.selected_category >= cat_list.len() {
        return;
    }
    let cat = &cat_list[app.schema_browser.selected_category];
    if *cat != SchemaBrowserCategory::Tables {
        return;
    }
    let items = get_current_items(app);
    if app.schema_browser.selected_item >= items.len() {
        return;
    }
    let item_name = &items[app.schema_browser.selected_item];
    // Check if columns already cached
    if !get_item_columns(app, item_name).is_empty() {
        return;
    }
    // Find the qualified key and enqueue a cache load
    for (_key, tables) in &app.metadata_cache.tables {
        for t in tables {
            if t.name == *item_name {
                let schema_key = format!("{}.{}", t.schema, t.name);
                app.pending_action = crate::app::PendingAction::CacheColumns { table: schema_key };
                return;
            }
        }
    }
    // Fallback: use bare name
    app.pending_action = crate::app::PendingAction::CacheColumns {
        table: item_name.clone(),
    };
}

fn get_item_ddl_text(app: &App, item_name: &str) -> String {
    if !app.schema_browser.ddl_text.is_empty() {
        return app.schema_browser.ddl_text.clone();
    }
    let cols = get_item_columns(app, item_name);
    if !cols.is_empty() {
        let mut out = String::from("-- Columns\n");
        for col in &cols {
            let nullable = if col.nullable { "YES" } else { "NO" };
            let pk = if col.is_pk { "PK" } else { "" };
            let def = col.default.as_deref().unwrap_or("");
            out.push_str(&format!(
                "  {:<20} {:<20} {:<8} {:<4} {}\n",
                col.name, col.type_name, nullable, pk, def
            ));
        }
        return out;
    }
    String::new()
}

fn handle_create_key(app: &mut App, key: &crossterm::event::KeyEvent) -> bool {
    use crossterm::event::KeyCode;
    match key.code {
        KeyCode::Esc => {
            app.schema_browser.mode = SchemaBrowserMode::Browse;
            true
        }
        KeyCode::Enter => {
            let name = app.schema_browser.create_name.trim().to_string();
            if name.is_empty() {
                app.schema_browser.create_error = Some("Table name cannot be empty.".into());
                return true;
            }
            let _db_name = app
                .pool
                .active_id
                .and_then(|id| app.connections.iter().find(|c| c.id == id))
                .map(|c| c.database.as_str())
                .unwrap_or("main")
                .to_string();
            let sql = format!(
                "CREATE TABLE \"{}\" ();\n-- Add columns and constraints manually",
                name
            );
            app.pending_action = crate::app::PendingAction::OpenQuery(sql);
            app.schema_browser.mode = SchemaBrowserMode::Browse;
            app.set_status(format!("Creating table \"{}\"...", name));
            true
        }
        KeyCode::Tab => {
            app.schema_browser.create_field = (app.schema_browser.create_field + 1) % 2;
            true
        }
        KeyCode::Char(c) => {
            if app.schema_browser.create_field == 0 {
                app.schema_browser.create_name.push(c);
            }
            true
        }
        KeyCode::Backspace => {
            if app.schema_browser.create_field == 0 {
                app.schema_browser.create_name.pop();
            }
            true
        }
        _ => true,
    }
}

fn handle_drop_confirm_key(app: &mut App, key: &crossterm::event::KeyEvent) -> bool {
    use crossterm::event::KeyCode;
    match key.code {
        KeyCode::Char('y') | KeyCode::Enter => {
            if let SchemaBrowserMode::DroppingTable(ref name) = app.schema_browser.mode.clone() {
                let _db_name = app
                    .pool
                    .active_id
                    .and_then(|id| app.connections.iter().find(|c| c.id == id))
                    .map(|c| c.database.as_str())
                    .unwrap_or("main")
                    .to_string();
                let sql = format!("DROP TABLE IF EXISTS \"{}\" CASCADE;", name);
                app.pending_action = crate::app::PendingAction::RunQuery(sql);
                app.set_status(format!("Dropping table \"{}\"...", name));
            }
            app.schema_browser.mode = SchemaBrowserMode::Browse;
            true
        }
        KeyCode::Char('n') | KeyCode::Esc => {
            app.schema_browser.mode = SchemaBrowserMode::Browse;
            true
        }
        _ => true,
    }
}

fn handle_rename_key(app: &mut App, key: &crossterm::event::KeyEvent) -> bool {
    use crossterm::event::KeyCode;
    match key.code {
        KeyCode::Esc => {
            app.schema_browser.mode = SchemaBrowserMode::Browse;
            true
        }
        KeyCode::Enter => {
            let old_name = match &app.schema_browser.mode {
                SchemaBrowserMode::RenamingTable(name) => name.clone(),
                _ => return true,
            };
            let new_name = app.schema_browser.create_name.trim().to_string();
            if new_name.is_empty() || new_name == *old_name {
                app.schema_browser.mode = SchemaBrowserMode::Browse;
                return true;
            }
            let sql = format!("ALTER TABLE \"{}\" RENAME TO \"{}\";", old_name, new_name);
            app.pending_action = crate::app::PendingAction::RunQuery(sql);
            app.set_status(format!("Renaming \"{}\" to \"{}\"...", old_name, new_name));
            app.schema_browser.mode = SchemaBrowserMode::Browse;
            true
        }
        KeyCode::Char(c) => {
            app.schema_browser.create_name.push(c);
            true
        }
        KeyCode::Backspace => {
            app.schema_browser.create_name.pop();
            true
        }
        _ => true,
    }
}

// ── Alter table modal ──

pub fn render_alter_table_modal(frame: &mut Frame, area: Rect, table_name: &str, app: &App) {
    let modal_w = area.width.min(56);
    let modal_h = 14;
    let x = area.x + (area.width.saturating_sub(modal_w)) / 2;
    let y = area.y + (area.height.saturating_sub(modal_h)) / 3;
    let modal_area = Rect::new(x, y, modal_w, modal_h);

    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .title(format!(" Alter Table: {} ", table_name))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(modal_area);
    frame.render_widget(&block, modal_area);

    let actions = ["Add Column", "Drop Column", "Modify Column"];
    let action_str = actions[app.schema_browser.alter_action.min(2)];
    let action_prefix = if app.schema_browser.alter_focus == 0 {
        ">"
    } else {
        " "
    };
    let col_prefix = if app.schema_browser.alter_focus == 1 {
        ">"
    } else {
        " "
    };
    let type_prefix = if app.schema_browser.alter_focus == 2 {
        ">"
    } else {
        " "
    };

    let lines = vec![
        Line::from(Span::styled(
            format!("{} Action: {}", action_prefix, action_str),
            if app.schema_browser.alter_focus == 0 {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            },
        )),
        Line::from(Span::styled(
            format!(
                "{} Col Name: {}",
                col_prefix, app.schema_browser.alter_column_name
            ),
            if app.schema_browser.alter_focus == 1 {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            },
        )),
        Line::from(Span::styled(
            format!(
                "{} Col Type: {}",
                type_prefix, app.schema_browser.alter_column_type
            ),
            if app.schema_browser.alter_focus == 2 {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            },
        )),
        Line::from(""),
        Line::from(Span::styled(
            " Tab:Next  Enter:Confirm  Esc:Cancel  h/l:Change action ",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    frame.render_widget(Paragraph::new(Text::from(lines)), inner);
}

fn handle_alter_key(app: &mut App, key: &crossterm::event::KeyEvent) -> bool {
    use crossterm::event::KeyCode;
    match key.code {
        KeyCode::Esc => {
            app.schema_browser.mode = SchemaBrowserMode::Browse;
            true
        }
        KeyCode::Tab => {
            app.schema_browser.alter_focus = (app.schema_browser.alter_focus + 1) % 3;
            true
        }
        KeyCode::Char('h') | KeyCode::Left => {
            if app.schema_browser.alter_focus == 0 && app.schema_browser.alter_action > 0 {
                app.schema_browser.alter_action -= 1;
            }
            true
        }
        KeyCode::Char('l') | KeyCode::Right => {
            if app.schema_browser.alter_focus == 0 && app.schema_browser.alter_action < 2 {
                app.schema_browser.alter_action += 1;
            }
            true
        }
        KeyCode::Enter => {
            let table = match &app.schema_browser.mode {
                SchemaBrowserMode::AlteringTable(name) => name.clone(),
                _ => return true,
            };
            let col_name = app.schema_browser.alter_column_name.trim().to_string();
            if col_name.is_empty() && app.schema_browser.alter_action != 0 {
                // For drop/modify, column name is required
                app.set_status("Column name required.");
                return true;
            }
            let col_type = app.schema_browser.alter_column_type.trim().to_string();
            let sql = match app.schema_browser.alter_action {
                0 => {
                    // Add column
                    let type_str = if col_type.is_empty() {
                        "TEXT"
                    } else {
                        &col_type
                    };
                    if col_name.is_empty() {
                        return true;
                    }
                    format!(
                        "ALTER TABLE \"{}\" ADD COLUMN \"{}\" {};",
                        table, col_name, type_str
                    )
                }
                1 => {
                    // Drop column
                    format!("ALTER TABLE \"{}\" DROP COLUMN \"{}\";", table, col_name)
                }
                2 => {
                    // Modify column
                    let type_str = if col_type.is_empty() {
                        "TEXT"
                    } else {
                        &col_type
                    };
                    format!(
                        "ALTER TABLE \"{}\" ALTER COLUMN \"{}\" TYPE {};",
                        table, col_name, type_str
                    )
                }
                _ => return true,
            };
            app.pending_action = crate::app::PendingAction::RunQuery(sql);
            app.set_status(format!("Altering table \"{}\"...", table));
            app.schema_browser.mode = SchemaBrowserMode::Browse;
            true
        }
        KeyCode::Char(c) => {
            match app.schema_browser.alter_focus {
                1 => app.schema_browser.alter_column_name.push(c),
                2 => app.schema_browser.alter_column_type.push(c),
                _ => {}
            }
            true
        }
        KeyCode::Backspace => {
            match app.schema_browser.alter_focus {
                1 => {
                    app.schema_browser.alter_column_name.pop();
                }
                2 => {
                    app.schema_browser.alter_column_type.pop();
                }
                _ => {}
            }
            true
        }
        _ => true,
    }
}

// ── Import schema modal ──

pub fn render_import_schema_modal(frame: &mut Frame, area: Rect, app: &App) {
    let modal_w = area.width.min(60);
    let modal_h = 14;
    let x = area.x + (area.width.saturating_sub(modal_w)) / 2;
    let y = area.y + (area.height.saturating_sub(modal_h)) / 3;
    let modal_area = Rect::new(x, y, modal_w, modal_h);

    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .title(" Import Schema SQL ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(modal_area);
    frame.render_widget(&block, modal_area);

    let display = &app.schema_browser.import_sql;
    let cursor = app.schema_browser.import_cursor.min(display.len());
    let before = &display[..cursor];
    let after = &display[cursor..];
    let text = format!("{}|{}", before, after);

    let lines = vec![
        Line::from(Span::styled(
            "SQL:",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            text.as_str(),
            Style::default().fg(Color::White),
        )),
        Line::from(""),
        Line::from(Span::styled(
            " Enter:Execute  Esc:Cancel  Backspace:Delete ",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    frame.render_widget(Paragraph::new(Text::from(lines)), inner);
}

fn handle_import_key(app: &mut App, key: &crossterm::event::KeyEvent) -> bool {
    use crossterm::event::KeyCode;
    match key.code {
        KeyCode::Esc => {
            app.schema_browser.mode = SchemaBrowserMode::Browse;
            true
        }
        KeyCode::Enter => {
            let sql = app.schema_browser.import_sql.trim().to_string();
            if sql.is_empty() {
                app.set_status("No SQL to execute.");
                app.schema_browser.mode = SchemaBrowserMode::Browse;
                return true;
            }
            app.pending_action = crate::app::PendingAction::RunQuery(sql);
            app.set_status("Executing import SQL...");
            app.schema_browser.mode = SchemaBrowserMode::Browse;
            true
        }
        KeyCode::Char(c) => {
            let cursor = app.schema_browser.import_cursor;
            app.schema_browser.import_sql.insert(cursor, c);
            app.schema_browser.import_cursor += 1;
            true
        }
        KeyCode::Backspace => {
            let cursor = app.schema_browser.import_cursor;
            if cursor > 0 {
                let new_cursor = cursor - 1;
                app.schema_browser.import_sql.remove(new_cursor);
                app.schema_browser.import_cursor = new_cursor;
            }
            true
        }
        KeyCode::Left => {
            if app.schema_browser.import_cursor > 0 {
                app.schema_browser.import_cursor -= 1;
            }
            true
        }
        KeyCode::Right => {
            if app.schema_browser.import_cursor < app.schema_browser.import_sql.len() {
                app.schema_browser.import_cursor += 1;
            }
            true
        }
        KeyCode::Home => {
            app.schema_browser.import_cursor = 0;
            true
        }
        KeyCode::End => {
            app.schema_browser.import_cursor = app.schema_browser.import_sql.len();
            true
        }
        _ => true,
    }
}

// ── Export (Reverse Engineer) modal ──

pub fn render_export_modal(frame: &mut Frame, area: Rect, app: &App) {
    let modal_w = area.width.min(70);
    let modal_h = area.height.min(24);
    let x = area.x + (area.width.saturating_sub(modal_w)) / 2;
    let y = area.y + (area.height.saturating_sub(modal_h)) / 3;
    let modal_area = Rect::new(x, y, modal_w, modal_h);

    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .title(" Exported Schema DDL ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));
    let inner = block.inner(modal_area);
    frame.render_widget(&block, modal_area);

    let ddl = &app.schema_browser.export_ddl;
    let scroll = app.schema_browser.scroll.min(
        ddl.lines()
            .count()
            .saturating_sub(inner.height as usize)
            .max(0),
    );
    let visible: String = ddl
        .lines()
        .skip(scroll)
        .take(inner.height as usize)
        .collect::<Vec<&str>>()
        .join("\n");

    let para = Paragraph::new(visible.as_str())
        .style(Style::default().fg(Color::Green))
        .wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(para, inner);

    let hint = Line::from(Span::styled(
        " j/k:Scroll  c:Copyd to clipboard  Esc:Close ",
        Style::default().fg(Color::DarkGray),
    ));
    let hint_y = modal_area.y + modal_area.height;
    if hint_y < area.height {
        frame.render_widget(
            Paragraph::new(hint),
            Rect::new(modal_area.x, hint_y, modal_w, 1),
        );
    }
}

// ── Schema diff modal ──

pub fn render_schema_diff_modal(frame: &mut Frame, area: Rect, app: &App) {
    let modal_w = area.width.min(70);
    let modal_h = area.height.min(24);
    let x = area.x + (area.width.saturating_sub(modal_w)) / 2;
    let y = area.y + (area.height.saturating_sub(modal_h)) / 3;
    let modal_area = Rect::new(x, y, modal_w, modal_h);

    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .title(" Schema Diff ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let inner = block.inner(modal_area);
    frame.render_widget(&block, modal_area);

    let diff = &app.schema_browser.diff_result;
    let scroll = app.schema_browser.scroll.min(
        diff.lines()
            .count()
            .saturating_sub(inner.height as usize)
            .max(0),
    );
    let visible: String = diff
        .lines()
        .skip(scroll)
        .take(inner.height as usize)
        .collect::<Vec<&str>>()
        .join("\n");

    let para = Paragraph::new(visible.as_str())
        .style(Style::default().fg(Color::White))
        .wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(para, inner);

    let hint = Line::from(Span::styled(
        " j/k:Scroll  Esc:Close  s:Reset ",
        Style::default().fg(Color::DarkGray),
    ));
    let hint_y = modal_area.y + modal_area.height;
    if hint_y < area.height {
        frame.render_widget(
            Paragraph::new(hint),
            Rect::new(modal_area.x, hint_y, modal_w, 1),
        );
    }
}

fn handle_diff_key(app: &mut App, key: &crossterm::event::KeyEvent) -> bool {
    use crossterm::event::KeyCode;
    match key.code {
        KeyCode::Esc => {
            app.schema_browser.mode = SchemaBrowserMode::Browse;
            true
        }
        KeyCode::Char('j') | KeyCode::Down => {
            let max = app
                .schema_browser
                .diff_result
                .lines()
                .count()
                .saturating_sub(1);
            if app.schema_browser.scroll < max {
                app.schema_browser.scroll += 1;
            }
            true
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if app.schema_browser.scroll > 0 {
                app.schema_browser.scroll -= 1;
            }
            true
        }
        KeyCode::Char('s') => {
            app.schema_browser.diff_snapshot_a.clear();
            app.schema_browser.diff_snapshot_b.clear();
            app.schema_browser.diff_result.clear();
            app.schema_browser.scroll = 0;
            app.schema_browser.mode = SchemaBrowserMode::Browse;
            app.set_status("Diff reset. Press 's' to capture Snapshot A.");
            true
        }
        _ => true,
    }
}

// ── Schema snapshot and diff helpers ──

fn build_schema_snapshot(app: &App) -> String {
    let mut lines: Vec<String> = Vec::new();
    for (_key, tables) in &app.metadata_cache.tables {
        for t in tables {
            lines.push(format!("TABLE:{}", t.name));
            let schema_key = format!("{}.{}", t.schema, t.name);
            let cols = app
                .metadata_cache
                .get_columns(&schema_key)
                .or_else(|| app.metadata_cache.get_columns(&format!("main.{}", t.name)))
                .map(|s| s.to_vec())
                .unwrap_or_default();
            for c in &cols {
                let nullable = if c.nullable { "Y" } else { "N" };
                let pk = if c.is_pk { "PK" } else { "" };
                let def = c.default.as_deref().unwrap_or("");
                lines.push(format!(
                    "  COL:{}|{}|{}|{}|{}",
                    c.name, c.type_name, nullable, pk, def
                ));
            }
        }
    }
    lines.sort();
    lines.join("\n")
}

fn compute_schema_diff(app: &App) -> String {
    let a_lines: std::collections::BTreeSet<&str> =
        app.schema_browser.diff_snapshot_a.lines().collect();
    let b_lines: std::collections::BTreeSet<&str> =
        app.schema_browser.diff_snapshot_b.lines().collect();

    let mut out: Vec<String> = Vec::new();
    out.push(format!("=== Schema Diff ==="));
    out.push(format!("--- Snapshot A"));
    out.push(format!("+++ Snapshot B"));
    out.push(String::new());

    // Tables in A but not in B
    let a_tables: std::collections::BTreeSet<&str> = a_lines
        .iter()
        .filter(|l| l.starts_with("TABLE:"))
        .copied()
        .collect();
    let b_tables: std::collections::BTreeSet<&str> = b_lines
        .iter()
        .filter(|l| l.starts_with("TABLE:"))
        .copied()
        .collect();

    for t in a_tables.difference(&b_tables) {
        out.push(format!("- {}", t));
        for l in &a_lines {
            if *l != *t && l.starts_with("  ") && a_lines.contains(t) {
                // Check if this column belongs to the removed table
                // Simple heuristic: col lines after a table line belong to it
            }
        }
    }
    for t in b_tables.difference(&a_tables) {
        out.push(format!("+ {}", t));
    }

    // Tables in both - compare columns
    for t in a_tables.intersection(&b_tables) {
        // Get columns for this table by finding lines between this TABLE line and next
        let table_label = *t;
        let a_table_cols: std::collections::BTreeSet<&str> = a_lines
            .iter()
            .skip_while(|l| **l != table_label)
            .skip(1)
            .take_while(|l| !l.starts_with("TABLE:"))
            .copied()
            .collect();
        let b_table_cols: std::collections::BTreeSet<&str> = b_lines
            .iter()
            .skip_while(|l| **l != table_label)
            .skip(1)
            .take_while(|l| !l.starts_with("TABLE:"))
            .copied()
            .collect();

        let mut has_diff = false;
        for c in a_table_cols.difference(&b_table_cols) {
            if !has_diff {
                out.push(format!(" {}:", t.strip_prefix("TABLE:").unwrap_or(t)));
                has_diff = true;
            }
            out.push(format!("  - {}", c));
        }
        for c in b_table_cols.difference(&a_table_cols) {
            if !has_diff {
                out.push(format!(" {}:", t.strip_prefix("TABLE:").unwrap_or(t)));
                has_diff = true;
            }
            out.push(format!("  + {}", c));
        }
    }

    if out.len() == 3 {
        out.push("  (no differences)".to_string());
    }

    out.join("\n")
}
