use std::cell::{Cell, RefCell};
use std::collections::HashSet;

use bson::{Bson, Document};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::Paragraph,
    Frame,
};

#[derive(Debug, Clone)]
pub struct MongoDocListState {
    pub expanded_fields: HashSet<String>,
    pub selected_line: usize,
    pub scroll_offset: usize,
    cached_lines: RefCell<Vec<DocListLine>>,
    cached_hash: Cell<u64>,
}

fn cache_hash(expanded: &HashSet<String>, doc_count: usize) -> u64 {
    let mut h: u64 = doc_count as u64;
    for key in expanded {
        h = h.wrapping_mul(31).wrapping_add(key.len() as u64);
    }
    h
}

impl Default for MongoDocListState {
    fn default() -> Self {
        Self {
            expanded_fields: HashSet::new(),
            selected_line: 0,
            scroll_offset: 0,
            cached_lines: RefCell::new(Vec::new()),
            cached_hash: Cell::new(0),
        }
    }
}

impl MongoDocListState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn ensure_cache(&self, documents: &[Document]) {
        let h = cache_hash(&self.expanded_fields, documents.len());
        if h != self.cached_hash.get() {
            let lines = build_flat_list(documents, &self.expanded_fields);
            *self.cached_lines.borrow_mut() = lines;
            self.cached_hash.set(h);
        }
    }

    pub fn get_cached_lines(
        &self,
        documents: &[Document],
    ) -> impl std::ops::Deref<Target = Vec<DocListLine>> + '_ {
        self.ensure_cache(documents);
        self.cached_lines.borrow()
    }

    pub fn cached_line_count(&self, documents: &[Document]) -> usize {
        self.ensure_cache(documents);
        self.cached_lines.borrow().len()
    }

    fn invalidate_cache(&self) {
        self.cached_hash.set(0);
    }

    pub fn toggle_selected(&mut self, documents: &[Document]) {
        let path_key = {
            let lines = self.get_cached_lines(documents);
            lines.get(self.selected_line).and_then(|line| {
                if !line.is_header && line.is_expandable {
                    Some(line.path_key.clone())
                } else {
                    None
                }
            })
        };
        if let Some(pk) = path_key {
            if self.expanded_fields.contains(&pk) {
                self.expanded_fields.remove(&pk);
            } else {
                self.expanded_fields.insert(pk);
            }
            self.invalidate_cache();
        }
    }

    pub fn expand_all(&mut self, documents: &[Document]) {
        let new_expanded = {
            let lines = build_flat_list(documents, &HashSet::new());
            let current_doc_idx = lines
                .get(self.selected_line)
                .map(|l| l.doc_index)
                .unwrap_or(0);
            let mut new_expanded = HashSet::new();
            for line in &lines {
                if line.doc_index == current_doc_idx && line.is_expandable {
                    new_expanded.insert(line.path_key.clone());
                }
            }
            new_expanded
        };
        self.expanded_fields = new_expanded;
        self.invalidate_cache();
    }

    pub fn collapse_all(&mut self, documents: &[Document]) {
        let current_doc_idx = {
            let lines = build_flat_list(documents, &self.expanded_fields);
            lines
                .get(self.selected_line)
                .map(|l| l.doc_index)
                .unwrap_or(0)
        };
        let to_remove: Vec<String> = self
            .expanded_fields
            .iter()
            .filter(|k| k.starts_with(&format!("{}.", current_doc_idx)))
            .cloned()
            .collect();
        for k in to_remove {
            self.expanded_fields.remove(&k);
        }
        self.invalidate_cache();
    }

    pub fn selected_value(&self, documents: &[Document]) -> Option<String> {
        let lines = self.get_cached_lines(documents);
        lines.get(self.selected_line).and_then(|l| {
            if l.is_separator {
                None
            } else if l.is_header {
                Some(format!("Document {}", l.doc_index))
            } else {
                Some(l.value_display.clone())
            }
        })
    }

    pub fn adjust_offset(&mut self, visible_height: usize, total_lines: usize) {
        if total_lines == 0 {
            self.selected_line = 0;
            self.scroll_offset = 0;
            return;
        }
        if self.selected_line >= total_lines {
            self.selected_line = total_lines.saturating_sub(1);
        }
        if self.scroll_offset > total_lines.saturating_sub(1) {
            self.scroll_offset = total_lines.saturating_sub(1);
        }
        if self.selected_line < self.scroll_offset {
            self.scroll_offset = self.selected_line;
        } else if self.selected_line >= self.scroll_offset + visible_height {
            self.scroll_offset = self
                .selected_line
                .saturating_add(1)
                .saturating_sub(visible_height);
        }
        let max_offset = total_lines.saturating_sub(visible_height);
        if self.scroll_offset > max_offset {
            self.scroll_offset = max_offset;
        }
    }

    pub fn select_up(&mut self) {
        if self.selected_line > 0 {
            self.selected_line -= 1;
        }
        if self.selected_line < self.scroll_offset {
            self.scroll_offset = self.selected_line;
        }
    }

    pub fn select_down(&mut self, total_lines: usize, visible_height: usize) {
        if total_lines > 0 && self.selected_line + 1 < total_lines {
            self.selected_line += 1;
        }
        if self.selected_line >= self.scroll_offset + visible_height {
            self.scroll_offset = self
                .selected_line
                .saturating_add(1)
                .saturating_sub(visible_height);
        }
    }

    pub fn go_to_first(&mut self) {
        self.selected_line = 0;
        self.scroll_offset = 0;
    }

    pub fn go_to_last(&mut self, total_lines: usize) {
        if total_lines > 0 {
            self.selected_line = total_lines - 1;
        }
    }

    pub fn page_up(&mut self, visible_height: usize) {
        self.selected_line = self.selected_line.saturating_sub(visible_height);
        if self.selected_line < self.scroll_offset {
            self.scroll_offset = self.selected_line;
        }
    }

    pub fn page_down(&mut self, visible_height: usize, total_lines: usize) {
        self.selected_line = self
            .selected_line
            .saturating_add(visible_height)
            .min(total_lines.saturating_sub(1));
        self.scroll_offset = self.scroll_offset.saturating_add(visible_height);
        let max_offset = total_lines.saturating_sub(visible_height);
        if self.scroll_offset > max_offset {
            self.scroll_offset = max_offset;
        }
    }
}

#[derive(Debug, Clone)]
pub struct DocListLine {
    pub doc_index: usize,
    pub indent: String,
    pub icon: String,
    pub field_name: String,
    pub value_display: String,
    pub value_color: Color,
    pub is_header: bool,
    pub is_separator: bool,
    pub is_expandable: bool,
    pub path_key: String,
    pub type_name: &'static str,
}

fn bson_display_value(bson: &Bson) -> String {
    match bson {
        Bson::Null => "null".to_string(),
        Bson::Boolean(b) => b.to_string(),
        Bson::Int32(i) => i.to_string(),
        Bson::Int64(i) => i.to_string(),
        Bson::Double(f) => {
            if *f == f.floor() && f.is_finite() {
                format!("{:.1}", f)
            } else {
                f.to_string()
            }
        }
        Bson::String(s) => {
            let truncated: String = s.chars().take(500).collect();
            if truncated.len() < s.len() {
                format!("\"{}…\"", truncated)
            } else {
                format!("\"{}\"", s)
            }
        }
        Bson::ObjectId(oid) => format!("ObjectId(\"{}\")", oid.to_hex()),
        Bson::DateTime(dt) => format!("ISODate(\"{}\")", dt),
        Bson::Timestamp(ts) => format!("Timestamp({}, {})", ts.time, ts.increment),
        Bson::Binary(bin) => format!("Binary({}, {:?})", bin.bytes.len(), bin.subtype),
        Bson::Decimal128(d) => d.to_string(),
        Bson::RegularExpression(r) => format!("/{}/{}", r.pattern, r.options),
        Bson::JavaScriptCode(code) => {
            let truncated: String = code.chars().take(80).collect();
            if truncated.len() < code.len() {
                format!("JavaScript(\"{}…\")", truncated)
            } else {
                format!("JavaScript(\"{}\")", code)
            }
        }
        Bson::JavaScriptCodeWithScope(cws) => {
            let truncated: String = cws.code.chars().take(80).collect();
            if truncated.len() < cws.code.len() {
                format!("JavaScript(\"{}…\")", truncated)
            } else {
                format!("JavaScript(\"{}\")", cws.code)
            }
        }
        Bson::Symbol(s) => format!("Symbol(\"{}\")", s),
        Bson::DbPointer(ptr) => format!("DbPointer({:?})", ptr),
        Bson::Undefined => "undefined".to_string(),
        Bson::MaxKey => "MaxKey".to_string(),
        Bson::MinKey => "MinKey".to_string(),
        Bson::Document(d) => {
            if d.is_empty() {
                "{}".to_string()
            } else {
                format!("{{{}}}", d.len())
            }
        }
        Bson::Array(a) => {
            if a.is_empty() {
                "[]".to_string()
            } else {
                let count = a.len();
                if count == 1 {
                    "[1 item]".to_string()
                } else {
                    format!("[{} items]", count)
                }
            }
        }
    }
}

fn bson_value_color(bson: &Bson) -> Color {
    match bson {
        Bson::Null => Color::DarkGray,
        Bson::Boolean(_) => Color::White,
        Bson::Int32(_) => Color::Blue,
        Bson::Int64(_) => Color::Blue,
        Bson::Double(_) => Color::Blue,
        Bson::Decimal128(_) => Color::Blue,
        Bson::String(_) => Color::Green,
        Bson::ObjectId(_) => Color::Red,
        Bson::DateTime(_) => Color::Blue,
        Bson::Timestamp(_) => Color::Blue,
        Bson::Symbol(_) => Color::Green,
        Bson::JavaScriptCode(_) => Color::Green,
        Bson::JavaScriptCodeWithScope(_) => Color::Green,
        Bson::RegularExpression(_) => Color::Green,
        Bson::Binary(_) => Color::White,
        Bson::DbPointer(_) => Color::White,
        Bson::Undefined => Color::DarkGray,
        Bson::MaxKey => Color::White,
        Bson::MinKey => Color::White,
        Bson::Document(_) => Color::White,
        Bson::Array(_) => Color::White,
    }
}

fn is_bson_expandable(bson: &Bson) -> bool {
    match bson {
        Bson::Document(d) => !d.is_empty(),
        Bson::Array(a) => !a.is_empty(),
        _ => false,
    }
}

fn bson_type_name(bson: &Bson) -> &'static str {
    match bson {
        Bson::Null => "null",
        Bson::Boolean(_) => "bool",
        Bson::Int32(_) => "int32",
        Bson::Int64(_) => "int64",
        Bson::Double(_) => "double",
        Bson::String(_) => "string",
        Bson::ObjectId(_) => "objectId",
        Bson::DateTime(_) => "date",
        Bson::Timestamp(_) => "timestamp",
        Bson::Binary(_) => "binary",
        Bson::Decimal128(_) => "decimal",
        Bson::RegularExpression(_) => "regex",
        Bson::JavaScriptCode(_) => "javascript",
        Bson::JavaScriptCodeWithScope(_) => "javascript",
        Bson::Symbol(_) => "symbol",
        Bson::DbPointer(_) => "dbPointer",
        Bson::Undefined => "undefined",
        Bson::MaxKey => "maxKey",
        Bson::MinKey => "minKey",
        Bson::Document(_) => "object",
        Bson::Array(_) => "array",
    }
}

fn build_flat_list(documents: &[Document], expanded_fields: &HashSet<String>) -> Vec<DocListLine> {
    let mut lines = Vec::new();

    for (doc_idx, doc) in documents.iter().enumerate() {
        if doc_idx > 0 {
            lines.push(DocListLine {
                doc_index: doc_idx,
                indent: String::new(),
                icon: String::new(),
                field_name: String::new(),
                value_display: String::new(),
                value_color: Color::DarkGray,
                is_header: false,
                is_separator: true,
                is_expandable: false,
                path_key: String::new(),
                type_name: "",
            });
        }

        lines.push(DocListLine {
            doc_index: doc_idx,
            indent: String::new(),
            icon: String::new(),
            field_name: format!("Document {}", doc_idx),
            value_display: String::new(),
            value_color: Color::White,
            is_header: true,
            is_separator: false,
            is_expandable: false,
            path_key: String::new(),
            type_name: "",
        });

        for (field_name, bson_value) in doc {
            collect_field_lines(
                doc_idx,
                field_name,
                bson_value,
                1,
                String::new(),
                expanded_fields,
                &mut lines,
            );
        }
    }

    lines
}

fn collect_field_lines(
    doc_idx: usize,
    field_name: &str,
    bson_value: &Bson,
    depth: usize,
    parent_path: String,
    expanded_fields: &HashSet<String>,
    lines: &mut Vec<DocListLine>,
) {
    let path_key = if parent_path.is_empty() {
        format!("{}.{}", doc_idx, field_name)
    } else {
        format!("{}.{}.{}", doc_idx, parent_path, field_name)
    };

    let expandable = is_bson_expandable(bson_value);
    let expanded = expandable && expanded_fields.contains(&path_key);
    let display = bson_display_value(bson_value);
    let color = bson_value_color(bson_value);
    let type_name = bson_type_name(bson_value);
    let indent_str = "  ".repeat(depth);

    let icon = if expandable {
        if expanded {
            "▼ "
        } else {
            "▶ "
        }
    } else {
        ""
    };

    lines.push(DocListLine {
        doc_index: doc_idx,
        indent: indent_str.clone(),
        icon: icon.to_string(),
        field_name: field_name.to_string(),
        value_display: display,
        value_color: if expandable { Color::White } else { color },
        is_header: false,
        is_separator: false,
        is_expandable: expandable,
        path_key: path_key.clone(),
        type_name,
    });

    if expanded {
        match bson_value {
            Bson::Document(subdoc) => {
                for (k, v) in subdoc {
                    collect_field_lines(
                        doc_idx,
                        k,
                        v,
                        depth + 1,
                        path_key.clone(),
                        expanded_fields,
                        lines,
                    );
                }
            }
            Bson::Array(arr) => {
                for (i, item) in arr.iter().enumerate() {
                    let item_name = format!("[{}]", i);
                    collect_field_lines(
                        doc_idx,
                        &item_name,
                        item,
                        depth + 1,
                        path_key.clone(),
                        expanded_fields,
                        lines,
                    );
                }
            }
            _ => {}
        }
    }
}

pub fn flat_line_count(documents: &[Document], expanded_fields: &HashSet<String>) -> usize {
    build_flat_list(documents, expanded_fields).len()
}

pub fn build_flat_list_ref(
    documents: &[Document],
    expanded_fields: &HashSet<String>,
) -> Vec<DocListLine> {
    build_flat_list(documents, expanded_fields)
}

pub fn render_mongo_doc_list(
    frame: &mut Frame,
    area: Rect,
    documents: &[Document],
    state: &MongoDocListState,
    focused: bool,
) {
    let lines = state.get_cached_lines(documents);
    let total_lines = lines.len();
    let visible_height = area.height as usize;

    if total_lines == 0 {
        let text = Paragraph::new("No documents found").style(Style::default().fg(Color::DarkGray));
        frame.render_widget(text, area);
        return;
    }

    // Compute view offset without mutating state (read-only)
    let mut view_offset = state
        .scroll_offset
        .min(total_lines.saturating_sub(visible_height));
    let sel = state.selected_line.min(total_lines.saturating_sub(1));
    if sel < view_offset {
        view_offset = sel;
    } else if sel >= view_offset + visible_height && sel >= visible_height {
        view_offset = sel.saturating_add(1).saturating_sub(visible_height);
    }
    view_offset = view_offset.min(total_lines.saturating_sub(visible_height));

    let offset = view_offset;
    let end = (offset + visible_height).min(total_lines);

    // Pre-compute separator rule (avoids per-line allocation)
    let sep_rule = "─".repeat(area.width as usize);

    // Pre-compute selection styles (avoids re-constructing per span)
    let sel_focused_style = Style::default()
        .fg(Color::Black)
        .bg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let sel_unfocused_style = Style::default()
        .bg(Color::DarkGray)
        .add_modifier(Modifier::BOLD);

    let rendered_lines: Vec<Line> = (offset..end)
        .map(|i| {
            let line = &lines[i];
            let is_selected = i == state.selected_line;

            if line.is_header {
                let style = if is_selected && focused {
                    sel_focused_style
                } else if is_selected {
                    sel_unfocused_style
                } else {
                    Style::default().add_modifier(Modifier::BOLD)
                };
                return Line::from(Span::styled(&line.field_name, style));
            }

            if line.is_separator {
                let style = if is_selected && focused {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else if is_selected {
                    Style::default().fg(Color::DarkGray).bg(Color::DarkGray)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                return Line::from(Span::styled(&sep_rule, style));
            }

            let indent_span = Span::raw(&line.indent);
            let icon_span = Span::styled(&line.icon, Style::default().fg(Color::White));
            let name_span = Span::styled(&line.field_name, Style::default().fg(Color::White));
            let sep_span = Span::raw("  ");
            let value_span =
                Span::styled(&line.value_display, Style::default().fg(line.value_color));

            let mut spans: Vec<Span> = Vec::with_capacity(7);
            spans.push(indent_span);
            spans.push(icon_span);
            spans.push(name_span);
            spans.push(sep_span);
            spans.push(value_span);

            if !line.is_expandable && !line.type_name.is_empty() {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(
                    line.type_name,
                    Style::default().fg(Color::DarkGray),
                ));
            }

            if is_selected && focused {
                let styled: Vec<Span> = spans
                    .into_iter()
                    .map(|s| Span::styled(s.content, sel_focused_style))
                    .collect();
                Line::from(styled)
            } else if is_selected {
                let styled: Vec<Span> = spans
                    .into_iter()
                    .map(|s| Span::styled(s.content, sel_unfocused_style))
                    .collect();
                Line::from(styled)
            } else {
                Line::from(spans)
            }
        })
        .collect();

    frame.render_widget(Paragraph::new(Text::from(rendered_lines)), area);
}
