use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use serde_json;

use crate::app::{App, Focus, InputMode, Page};
use crate::ui::pages;
use crate::ui::sidebar;
use crate::widgets::tree::{TreeItem, TreeState};

pub fn render_layout(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    let vertical = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ]);
    let [topbar_area, content_area, footer_area] = vertical.areas(area);

    let sidebar_w = content_area.width / 4;
    let inspector_w = if app.show_inspector { sidebar_w } else { 0 };
    let main_w = content_area
        .width
        .saturating_sub(sidebar_w)
        .saturating_sub(inspector_w);

    let horizontal = Layout::horizontal([
        Constraint::Length(sidebar_w),
        Constraint::Length(main_w),
        Constraint::Length(inspector_w),
    ]);
    let [sidebar_area, main_area, inspector_area] = horizontal.areas(content_area);

    app.sidebar_rect = sidebar_area;
    app.main_rect = main_area;
    app.inspector_rect = inspector_area;

    render_topbar(frame, topbar_area, app);
    sidebar::render_sidebar(frame, sidebar_area, app);

    let focused = app.focus == Focus::Main || app.focus == Focus::Inspector;
    pages::render(frame, main_area, app.current_page, app, focused);

    if app.show_inspector {
        render_inspector(frame, inspector_area, app);
    }
    render_footer(frame, footer_area, app);

    if app.global_search.is_some() && app.global_search.as_ref().unwrap().is_open {
        crate::ui::search::render_global_search(frame, area, app);
    }
}

fn render_topbar(frame: &mut Frame, area: Rect, app: &App) {
    let page_indicator = format!("[{}]", app.current_page.title());

    let text = format!(" Dbrusty  {}", page_indicator);

    let topbar = Paragraph::new(text).style(Style::default().fg(Color::White).bg(Color::DarkGray));
    frame.render_widget(topbar, area);
}

pub fn parse_json_tree(json_str: &str) -> Vec<TreeItem> {
    match serde_json::from_str::<serde_json::Value>(json_str) {
        Ok(val) => {
            let mut items = json_to_items(&val, 0);
            for item in &mut items {
                item.expanded = true;
            }
            items
        }
        Err(_) => vec![TreeItem::leaf("json:error", json_str, 0)],
    }
}

fn json_to_items(val: &serde_json::Value, depth: usize) -> Vec<TreeItem> {
    match val {
        serde_json::Value::Object(map) => map
            .iter()
            .map(|(k, v)| bson_json_to_item(k, v, depth))
            .collect(),
        serde_json::Value::Array(arr) => arr
            .iter()
            .enumerate()
            .map(|(i, v)| bson_json_to_item(&format!("[{}]", i), v, depth))
            .collect(),
        _ => vec![TreeItem::leaf("json:root", &format_json_val(val), depth)],
    }
}

fn format_json_val(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => format!("\"{}\"", s),
        serde_json::Value::Object(m) if m.is_empty() => "{}".to_string(),
        serde_json::Value::Object(_) => "{…}".to_string(),
        serde_json::Value::Array(a) if a.is_empty() => "[]".to_string(),
        serde_json::Value::Array(_) => "[…]".to_string(),
    }
}

fn bson_json_to_item(key: &str, val: &serde_json::Value, depth: usize) -> TreeItem {
    let id = format!("json:{}:{}", depth, key);
    match val {
        serde_json::Value::Object(map) => {
            if map.is_empty() {
                return TreeItem::leaf(&id, &format!("{}: {{}}", key), depth);
            }
            let children: Vec<TreeItem> = map
                .iter()
                .map(|(k, v)| bson_json_to_item(k, v, depth + 1))
                .collect();
            let label = format!("{}: {{{}}}", key, children.len());
            TreeItem::branch(&id, &label, depth, children)
        }
        serde_json::Value::Array(arr) => {
            if arr.is_empty() {
                return TreeItem::leaf(&id, &format!("{}: []", key), depth);
            }
            let children: Vec<TreeItem> = arr
                .iter()
                .enumerate()
                .map(|(i, v)| bson_json_to_item(&format!("[{}]", i), v, depth + 1))
                .collect();
            let label = format!("{}: [{}]", key, children.len());
            TreeItem::branch(&id, &label, depth, children)
        }
        serde_json::Value::String(s) => TreeItem::leaf(&id, &format!("{}: \"{}\"", key, s), depth),
        serde_json::Value::Number(n) => TreeItem::leaf(&id, &format!("{}: {}", key, n), depth),
        serde_json::Value::Bool(b) => TreeItem::leaf(&id, &format!("{}: {}", key, b), depth),
        serde_json::Value::Null => TreeItem::leaf(&id, &format!("{}: null", key), depth),
    }
}

fn render_json_tree_content(frame: &mut Frame, area: Rect, state: &TreeState, focused: bool) {
    let visible_height = area.height as usize;
    let items = state.flattened();
    let max_offset = items.len().saturating_sub(visible_height);
    let offset = state.offset.min(max_offset);
    let sel = state.selected.min(items.len().saturating_sub(1));
    // Ensure selected is in view (read-only approximation of adjust_offset)
    let offset = if sel < offset {
        sel
    } else if sel >= offset + visible_height && sel >= visible_height {
        sel.saturating_add(1).saturating_sub(visible_height)
    } else {
        offset
    };
    let offset = offset.min(max_offset);

    let lines: Vec<Line> = items
        .iter()
        .enumerate()
        .skip(offset)
        .take(visible_height)
        .map(|(idx, item)| {
            let is_selected = idx == state.selected;
            let indent = "  ".repeat(item.depth);
            let icon = match (item.has_children, item.expanded) {
                (true, true) => "\u{25bc} ",
                (true, false) => "\u{25b6} ",
                (false, _) => "  ",
            };
            let label = format!("{}{}{}", indent, icon, item.label);
            let style = if is_selected && focused {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else if is_selected {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            Line::from(Span::styled(label, style))
        })
        .collect();

    frame.render_widget(Paragraph::new(Text::from(lines)), area);
}

fn render_inspector(frame: &mut Frame, area: Rect, app: &App) {
    let focused = app.focus == Focus::Inspector;
    let border_color = if focused { Color::Cyan } else { Color::White };
    let block = Block::default()
        .title(" Inspector ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let inner = block.inner(area);
    frame.render_widget(&block, area);

    if app.current_page != Page::QueryEditor || app.query_data.is_empty() {
        let text = Paragraph::new("Select an item").style(Style::default().fg(Color::DarkGray));
        frame.render_widget(text, inner);
        return;
    }

    if app.selected_row >= app.query_data.len() {
        return;
    }

    // Check if the selected cell contains JSON — show tree viewer in inspector
    let is_json_cell = app
        .query_data_typed
        .get(app.selected_row)
        .and_then(|r| r.get(app.selected_col))
        .and_then(|v| v.json_str())
        .map(|s| s.starts_with('{') || s.starts_with('['))
        .unwrap_or(false);

    if is_json_cell && focused {
        let col_name = app
            .query_columns
            .get(app.selected_col)
            .map(|s| s.as_str())
            .unwrap_or("");
        let type_badge = app
            .query_data_typed
            .get(app.selected_row)
            .and_then(|r| r.get(app.selected_col))
            .map(|v| v.type_name())
            .unwrap_or("");

        let header_span = Line::from(vec![
            Span::styled(
                format!(" {} ", col_name),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" {} ", type_badge),
                Style::default().fg(Color::DarkGray),
            ),
        ]);
        let header_area = Rect::new(inner.x, inner.y, inner.width, 1);
        frame.render_widget(Paragraph::new(Text::from(vec![header_span])), header_area);

        let tree_area = Rect::new(
            inner.x,
            inner.y + 1,
            inner.width,
            inner.height.saturating_sub(1),
        );
        render_json_tree_content(frame, tree_area, &app.json_tree_state, true);
        return;
    }

    if is_json_cell {
        // Show tree in unfocused state too
        let tree_area = Rect::new(inner.x, inner.y, inner.width, inner.height);
        render_json_tree_content(frame, tree_area, &app.json_tree_state, false);
        return;
    }

    let visible_height = inner.height.saturating_sub(1) as usize;
    if visible_height == 0 {
        return;
    }

    let col_count = app.query_columns.len();
    let max_scroll = col_count.saturating_sub(visible_height / 3).max(0);
    let scroll = app.inspector_scroll.min(max_scroll);

    let mut lines: Vec<Line> = Vec::new();
    let items_per_page = (visible_height / 3).max(1);
    let end = (scroll + items_per_page).min(col_count);

    for i in scroll..end {
        let col_name = app.query_columns[i].as_str();
        let is_editing = app.inspector_edit_col == Some(i);

        let value = if is_editing {
            app.inspector_edit_value.as_str()
        } else {
            app.query_data[app.selected_row]
                .get(i)
                .map(|s| s.as_str())
                .unwrap_or("")
        };

        let type_badge: &str = app
            .query_data_typed
            .get(app.selected_row)
            .and_then(|r| r.get(i))
            .map(|v| v.type_name())
            .unwrap_or("");

        let display_width = inner.width.saturating_sub(2) as usize;
        let label = format!(" {} ", col_name);
        let badge = format!(" {} ", type_badge);
        let padding = display_width.saturating_sub(label.len() + badge.len());
        let dots: String = if padding > 0 {
            " ".repeat(padding)
        } else {
            String::new()
        };

        lines.push(Line::from(vec![
            Span::styled(
                label,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(dots),
            Span::styled(badge, Style::default().fg(Color::DarkGray)),
        ]));

        if is_editing {
            let scroll_h = app.inspector_edit_scroll_h as usize;
            let visible = if scroll_h < value.len() {
                let end = (scroll_h + display_width).min(value.len());
                value[scroll_h..end].to_string()
            } else {
                String::new()
            };
            lines.push(Line::from(Span::styled(
                visible,
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )));
        } else {
            let display_val = if value.len() > display_width {
                format!("{}…", &value[..display_width.saturating_sub(1)])
            } else {
                value.to_string()
            };
            let is_json = value.starts_with('{') || value.starts_with('[');
            if is_json && display_val.len() > display_width / 2 {
                let json_lines = split_text(&display_val, display_width);
                for jl in json_lines {
                    lines.push(Line::from(Span::styled(
                        jl,
                        Style::default().fg(Color::Green),
                    )));
                }
            } else {
                lines.push(Line::from(Span::styled(
                    display_val,
                    Style::default().fg(Color::White),
                )));
            }
        }

        lines.push(Line::raw(""));
    }

    let text = Paragraph::new(Text::from(lines));
    frame.render_widget(text, inner);

    if let Some(edit_col) = app.inspector_edit_col {
        let visible_idx = edit_col.saturating_sub(scroll);
        if visible_idx < items_per_page {
            let cursor_visual = app
                .inspector_edit_cursor
                .saturating_sub(app.inspector_edit_scroll_h as usize);
            let cursor_x = inner.x + cursor_visual as u16;
            let cursor_y = inner.y + (visible_idx * 3 + 1) as u16;
            if cursor_x < inner.x + inner.width {
                frame.set_cursor_position((cursor_x, cursor_y));
            }
        }
    }
}

fn split_text(s: &str, max_width: usize) -> Vec<String> {
    let mut result = Vec::new();
    let mut start = 0;
    while start < s.len() {
        let end = (start + max_width).min(s.len());
        result.push(s[start..end].to_string());
        start = end;
    }
    result
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
