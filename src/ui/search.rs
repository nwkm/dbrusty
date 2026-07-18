use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::App;
use crate::database::metadata::MetadataCache;
use crate::widgets::input::InputField;
use crate::widgets::tree::TreeState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchSource {
    Explorer,
    Metadata,
    Query,
    Results,
    QueryHistory,
}

impl SearchSource {
    fn label(&self) -> &str {
        match self {
            SearchSource::Explorer => "Explorer",
            SearchSource::Metadata => "Metadata",
            SearchSource::Query => "Query",
            SearchSource::Results => "Results",
            SearchSource::QueryHistory => "History",
        }
    }

    fn color(&self) -> Color {
        match self {
            SearchSource::Explorer => Color::Cyan,
            SearchSource::Metadata => Color::Yellow,
            SearchSource::Query => Color::Green,
            SearchSource::Results => Color::Magenta,
            SearchSource::QueryHistory => Color::Blue,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub source: SearchSource,
    pub label: String,
    pub line_index: usize,
    pub match_positions: Vec<usize>,
    pub context: Option<String>,
}

pub struct GlobalSearchState {
    pub input: InputField,
    pub results: Vec<SearchResult>,
    pub all_results: Vec<SearchResult>,
    pub selected: usize,
    pub scroll_offset: usize,
    pub is_open: bool,
}

impl GlobalSearchState {
    pub fn new() -> Self {
        Self {
            input: InputField::new("Search..."),
            results: Vec::new(),
            all_results: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            is_open: false,
        }
    }

    pub fn filter(&mut self) {
        let query = self.input.value.trim();
        if query.is_empty() {
            self.results = self.all_results.clone();
            self.selected = 0;
            self.scroll_offset = 0;
            return;
        }

        let mut scored: Vec<(i64, usize)> = Vec::new();
        for (i, result) in self.all_results.iter().enumerate() {
            if let Some((score, _)) = fuzzy_match(query, &result.label) {
                scored.push((score, i));
            }
        }

        scored.sort_by(|a, b| b.0.cmp(&a.0));

        let mut new_results = Vec::with_capacity(scored.len());
        for (_, idx) in scored {
            let result = self.all_results[idx].clone();
            new_results.push(result);
        }

        self.results = new_results;
        self.selected = if self.results.is_empty() { 0 } else { 0 };
        self.scroll_offset = 0;
    }

    pub fn update_search_state(&mut self) {
        for (i, result) in self.results.iter_mut().enumerate() {
            let query = self.input.value.trim();
            if !query.is_empty() {
                if let Some((_, positions)) = fuzzy_match(query, &result.label) {
                    result.match_positions = positions;
                }
            } else {
                result.match_positions = Vec::new();
            }
            if i != self.selected {
                continue;
            }
        }
    }
}

pub fn fuzzy_match(query: &str, target: &str) -> Option<(i64, Vec<usize>)> {
    if query.is_empty() {
        return Some((0, Vec::new()));
    }

    let query_lower: Vec<char> = query.chars().map(|c| c.to_ascii_lowercase()).collect();
    let target_lower: Vec<char> = target.chars().map(|c| c.to_ascii_lowercase()).collect();

    let mut ti = 0;
    let mut positions = Vec::new();
    let mut consecutive = 0;
    let mut total_consecutive_bonus = 0i64;

    for &qc in &query_lower {
        while ti < target_lower.len() && target_lower[ti] != qc {
            consecutive = 0;
            ti += 1;
        }
        if ti >= target_lower.len() {
            return None;
        }
        positions.push(ti);
        consecutive += 1;
        if consecutive > 1 {
            total_consecutive_bonus += 10;
        }
        ti += 1;
    }

    let first_pos = positions[0];
    let start_bonus = if first_pos == 0 {
        20
    } else if first_pos > 0
        && (target_lower[first_pos - 1] == ' '
            || target_lower[first_pos - 1] == '.'
            || target_lower[first_pos - 1] == '_'
            || target_lower[first_pos - 1] == '-')
    {
        15
    } else {
        0
    };

    let length_penalty = (target.len() as i64).saturating_sub(query.len() as i64) * 2;
    let score = (query.len() as i64 * 10) + total_consecutive_bonus + start_bonus - length_penalty;

    Some((score.max(0), positions))
}

fn index_explorer_tree(tree: &TreeState) -> Vec<SearchResult> {
    let mut results = Vec::new();
    for item in &tree.items {
        flatten_tree_item(item, 0, &mut results);
    }
    results
}

fn flatten_tree_item(
    item: &crate::widgets::tree::TreeItem,
    idx: usize,
    results: &mut Vec<SearchResult>,
) {
    let prefix = "  ".repeat(item.depth);
    let icon = if item.expanded { "▼" } else { "▶" };
    let context = Some(format!("depth {}", item.depth));
    results.push(SearchResult {
        source: SearchSource::Explorer,
        label: format!("{}{} {}", prefix, icon, item.label),
        line_index: idx,
        match_positions: Vec::new(),
        context,
    });
    for child in &item.children {
        flatten_tree_item(child, idx + 1, results);
    }
}

fn index_metadata(cache: &MetadataCache) -> Vec<SearchResult> {
    let mut results = Vec::new();

    for (schema_key, tables) in &cache.tables {
        for table in tables {
            results.push(SearchResult {
                source: SearchSource::Metadata,
                label: format!("{}.{} (table)", schema_key, table.name),
                line_index: 0,
                match_positions: Vec::new(),
                context: None,
            });
        }
    }

    for (table_key, columns) in &cache.columns {
        for col in columns {
            let pk = if col.is_pk { " PK" } else { "" };
            let fk = if col.is_fk { " FK" } else { "" };
            results.push(SearchResult {
                source: SearchSource::Metadata,
                label: format!(
                    "{}.{} ({}:{}{})",
                    table_key, col.name, col.type_name, pk, fk
                ),
                line_index: 0,
                match_positions: Vec::new(),
                context: None,
            });
        }
    }

    for (schema_key, views) in &cache.views {
        for view in views {
            results.push(SearchResult {
                source: SearchSource::Metadata,
                label: format!("{}.{} (view)", schema_key, view.name),
                line_index: 0,
                match_positions: Vec::new(),
                context: None,
            });
        }
    }

    results
}

fn index_query(query_text: &str) -> Vec<SearchResult> {
    if query_text.is_empty() {
        return Vec::new();
    }
    query_text
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(i, line)| SearchResult {
            source: SearchSource::Query,
            label: line.trim().to_string(),
            line_index: i,
            match_positions: Vec::new(),
            context: None,
        })
        .collect()
}

fn index_results(columns: &[String], data: &[Vec<String>]) -> Vec<SearchResult> {
    let mut results = Vec::new();
    for (row_idx, row) in data.iter().enumerate() {
        for (col_idx, cell) in row.iter().enumerate() {
            if cell.is_empty() || cell == "NULL" {
                continue;
            }
            let col_name = columns.get(col_idx).map(|s| s.as_str()).unwrap_or("?");
            results.push(SearchResult {
                source: SearchSource::Results,
                label: format!("{}: {}", col_name, cell),
                line_index: row_idx,
                match_positions: Vec::new(),
                context: None,
            });
        }
    }
    results
}

pub fn render_global_search(frame: &mut Frame, area: Rect, app: &App) {
    let state = match &app.global_search {
        Some(s) if s.is_open => s,
        _ => return,
    };

    let popup_width = area.width.saturating_sub(4).min(80);
    let popup_height = area.height.saturating_sub(4).min(24);
    let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = area.y + (area.height.saturating_sub(popup_height)) / 3;
    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" Global Search ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let search_text = if state.input.value.is_empty() {
        Text::from(Line::from(vec![
            Span::raw("  "),
            Span::styled("?", Style::default().fg(Color::DarkGray)),
            Span::raw("  "),
            Span::styled("Search...", Style::default().fg(Color::DarkGray)),
        ]))
    } else {
        Text::from(Line::from(vec![
            Span::raw("  "),
            Span::styled(&state.input.value, Style::default().fg(Color::White)),
        ]))
    };
    frame.render_widget(
        Paragraph::new(search_text),
        Rect::new(inner.x, inner.y, inner.width, 1),
    );

    let list_area = Rect::new(
        inner.x,
        inner.y + 1,
        inner.width,
        inner.height.saturating_sub(2),
    );
    if list_area.height == 0 {
        return;
    }

    let visible = list_area.height as usize;
    let scroll = state.scroll_offset;
    let selected = state.selected;

    let highlighted = Style::default().bg(Color::White).fg(Color::Black);
    let dim = Style::default().fg(Color::DarkGray);
    let match_hl = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);

    for i in 0..visible {
        let result_idx = scroll + i;
        if result_idx >= state.results.len() {
            break;
        }

        let result = &state.results[result_idx];
        let is_selected = result_idx == selected;

        let mut spans = Vec::new();

        let badge_style = Style::default().fg(result.source.color());
        let badge = format!("[{:8}] ", result.source.label());
        spans.push(Span::styled(
            badge,
            if is_selected {
                highlighted
            } else {
                badge_style
            },
        ));

        let label_chars: Vec<char> = result.label.chars().collect();
        let query = state.input.value.trim();
        let query_lower: Vec<char> = query.chars().map(|c| c.to_ascii_lowercase()).collect();
        let label_lower: Vec<char> = result
            .label
            .chars()
            .map(|c| c.to_ascii_lowercase())
            .collect();

        if !query.is_empty() {
            let mut qi = 0;
            let mut pi = 0;
            let mut chars = String::new();
            for (ci, lc) in label_lower.iter().enumerate() {
                let is_match = if qi < query_lower.len() && *lc == query_lower[qi] {
                    if pi >= result.match_positions.len() || ci == result.match_positions[pi] {
                        pi += 1;
                        qi += 1;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                };

                let ch = label_chars[ci];
                if is_match {
                    if !chars.is_empty() {
                        spans.push(Span::raw(chars.clone()));
                        chars.clear();
                    }
                    spans.push(Span::styled(ch.to_string(), match_hl));
                } else {
                    chars.push(ch);
                }
            }
            if !chars.is_empty() {
                spans.push(Span::raw(chars));
            }
        } else {
            spans.push(Span::raw(&result.label));
        }

        let line_info = result.context.as_deref().unwrap_or("");
        let info_style = if is_selected { highlighted } else { dim };
        spans.push(Span::styled(format!("  {}", line_info), info_style));

        let row_line = Line::from(spans);
        let text_vec = Text::from(row_line);
        let row_y = list_area.y + i as u16;
        let row_area = Rect::new(list_area.x, row_y, list_area.width, 1);

        if is_selected {
            frame.render_widget(
                Paragraph::new(text_vec).style(Style::default().bg(Color::White).fg(Color::Black)),
                row_area,
            );
        } else {
            frame.render_widget(Paragraph::new(text_vec), row_area);
        }
    }

    let footer_text = format!("  {} results  ", state.results.len());
    let footer_y = inner.y + inner.height - 1;
    let footer_area = Rect::new(inner.x, footer_y, inner.width, 1);
    let footer_style = Style::default().fg(Color::DarkGray);
    frame.render_widget(
        Paragraph::new(Text::from(Line::from(Span::styled(
            footer_text,
            footer_style,
        )))),
        footer_area,
    );
}

pub fn rebuild_search_index(app: &mut App) {
    let mut all = Vec::new();

    all.extend(index_explorer_tree(&app.explorer_tree));
    all.extend(index_metadata(&app.metadata_cache));
    all.extend(index_query(&app.query_text));
    all.extend(index_results(&app.query_columns, &app.query_data));

    let state = app.global_search.as_mut().unwrap();
    state.all_results = all;
    state.filter();
}

pub fn handle_global_search_key(app: &mut App, key: &crossterm::event::KeyEvent) -> bool {
    let state = match app.global_search.as_mut() {
        Some(s) if s.is_open => s,
        _ => return false,
    };

    use crossterm::event::{KeyCode, KeyModifiers};

    match key.code {
        KeyCode::Esc => {
            state.is_open = false;
            true
        }
        KeyCode::Enter => {
            if state.results.is_empty() {
                return true;
            }
            let idx = state.selected.min(state.results.len().saturating_sub(1));
            let result = state.results[idx].clone();
            state.is_open = false;
            jump_to_result(app, &result);
            true
        }
        KeyCode::Up | KeyCode::Char('k') if key.modifiers == KeyModifiers::NONE => {
            if state.selected > 0 {
                state.selected -= 1;
                if state.selected < state.scroll_offset {
                    state.scroll_offset = state.selected;
                }
            }
            true
        }
        KeyCode::Down | KeyCode::Char('j') if key.modifiers == KeyModifiers::NONE => {
            if state.selected + 1 < state.results.len() {
                state.selected += 1;
                let visible = (app
                    .main_rect
                    .height
                    .saturating_sub(4)
                    .min(24)
                    .saturating_sub(4)) as usize;
                if state.selected >= state.scroll_offset + visible {
                    state.scroll_offset = state.selected.saturating_sub(visible).saturating_add(1);
                }
            }
            true
        }
        KeyCode::PageUp => {
            let visible = (app
                .main_rect
                .height
                .saturating_sub(4)
                .min(24)
                .saturating_sub(4)) as usize;
            state.selected = state.selected.saturating_sub(visible);
            if state.selected < state.scroll_offset {
                state.scroll_offset = state.selected;
            }
            true
        }
        KeyCode::PageDown => {
            let visible = (app
                .main_rect
                .height
                .saturating_sub(4)
                .min(24)
                .saturating_sub(4)) as usize;
            state.selected = (state.selected + visible).min(state.results.len().saturating_sub(1));
            let max_scroll = state.results.len().saturating_sub(visible);
            if state.selected >= state.scroll_offset + visible {
                state.scroll_offset = state
                    .selected
                    .saturating_sub(visible)
                    .saturating_add(1)
                    .min(max_scroll);
            }
            true
        }
        KeyCode::Char('g') if key.modifiers == KeyModifiers::NONE => {
            state.selected = 0;
            state.scroll_offset = 0;
            true
        }
        KeyCode::Char('G') if key.modifiers == KeyModifiers::NONE => {
            if !state.results.is_empty() {
                state.selected = state.results.len() - 1;
                let visible = (app
                    .main_rect
                    .height
                    .saturating_sub(4)
                    .min(24)
                    .saturating_sub(4)) as usize;
                state.scroll_offset = state.selected.saturating_sub(visible).saturating_add(1);
            }
            true
        }
        _ => {
            if state.input.handle_key(key) {
                state.filter();
                true
            } else {
                false
            }
        }
    }
}

fn jump_to_result(app: &mut App, result: &SearchResult) {
    match result.source {
        SearchSource::Explorer => {
            app.current_page = crate::app::Page::Explorer;
            app.focus = crate::app::Focus::Sidebar;
            app.set_status(format!("Navigated to: {}", result.label));
        }
        SearchSource::Metadata => {
            app.current_page = crate::app::Page::Explorer;
            app.focus = crate::app::Focus::Sidebar;
            app.set_status(format!("Navigated to metadata: {}", result.label));
        }
        SearchSource::Query => {
            app.current_page = crate::app::Page::QueryEditor;
            app.query_editor_focus = crate::app::QueryEditorFocus::Editor;
            if !result.label.is_empty() {
                app.query_text = result.label.clone();
                app.query_editor.set_value(&result.label);
            }
            app.set_status(format!("Jumped to query line {}", result.line_index + 1));
        }
        SearchSource::Results => {
            app.current_page = crate::app::Page::QueryEditor;
            app.query_editor_focus = crate::app::QueryEditorFocus::Results;
            app.selected_row = result.line_index;
            app.set_status(format!("Jumped to result row {}", result.line_index + 1));
        }
        SearchSource::QueryHistory => {
            if !result.label.is_empty() {
                app.query_text = result.label.clone();
                app.query_editor.set_value(&result.label);
            }
            app.current_page = crate::app::Page::QueryEditor;
            app.query_editor_focus = crate::app::QueryEditorFocus::Editor;
            app.set_status("Loaded query from history");
        }
    }
}
