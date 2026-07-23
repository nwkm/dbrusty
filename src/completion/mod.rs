pub mod mongo_provider;
pub mod provider;
pub mod redis_provider;
pub mod sql_keywords;
pub mod sql_provider;
pub mod widget;

use crate::completion::provider::CompletionProvider;
use crate::database::metadata::MetadataCache;
use crate::models::connection::DatabaseType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionKind {
    Keyword,
    Function,
    Type,
    Table,
    Column,
    View,
    Schema,
    Operator,
    Command,
    Snippet,
}

#[derive(Debug, Clone)]
pub struct CompletionItem {
    pub label: String,
    pub kind: CompletionKind,
    pub detail: String,
    pub score: i64,
}

impl CompletionItem {
    pub fn new(label: &str, kind: CompletionKind) -> Self {
        Self {
            label: label.to_string(),
            kind,
            detail: String::new(),
            score: 0,
        }
    }

    pub fn with_detail(label: &str, kind: CompletionKind, detail: &str) -> Self {
        Self {
            label: label.to_string(),
            kind,
            detail: detail.to_string(),
            score: 0,
        }
    }
}

pub struct CompletionState {
    pub items: Vec<CompletionItem>,
    pub filtered: Vec<usize>,
    pub selected: usize,
    pub visible: bool,
    pub trigger_col: u16,
    pub trigger_row: u16,
    pub prefix: String,
}

impl CompletionState {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            filtered: Vec::new(),
            selected: 0,
            visible: false,
            trigger_col: 0,
            trigger_row: 0,
            prefix: String::new(),
        }
    }

    pub fn update(
        &mut self,
        full_text: &str,
        cursor_row: u16,
        cursor_col: u16,
        provider: &dyn CompletionProvider,
        metadata: &MetadataCache,
        db_type: Option<&DatabaseType>,
    ) {
        self.items = provider.completions(metadata, db_type);

        let current_line = full_text;
        let cursor_pos = cursor_col as usize;

        let prefix = extract_prefix(current_line, cursor_pos);
        self.prefix = prefix.clone();

        if prefix.is_empty() && !cursor_after_dot(current_line, cursor_pos) {
            self.filtered.clear();
            self.visible = false;
            return;
        }

        if !prefix.is_empty() && prefix.chars().all(|c| c.is_numeric()) {
            self.filtered.clear();
            self.visible = false;
            return;
        }

        if prefix.len() < 3 && !cursor_after_dot(current_line, cursor_pos) {
            self.filtered.clear();
            self.visible = false;
            return;
        }

        self.filter(&prefix);

        if self.filtered.is_empty() {
            self.visible = false;
            return;
        }

        self.visible = true;
        self.trigger_col = cursor_col.saturating_sub(prefix.len() as u16);
        self.trigger_row = cursor_row;
    }

    fn filter(&mut self, prefix: &str) {
        let mut scored: Vec<(i64, usize)> = Vec::new();
        for (i, item) in self.items.iter().enumerate() {
            if let Some((score, _)) = fuzzy_match_completion(prefix, &item.label) {
                scored.push((score, i));
            }
        }
        scored.sort_by(|a, b| b.0.cmp(&a.0));

        self.filtered = scored.into_iter().map(|(_, i)| i).collect();
        self.selected = 0;
    }

    pub fn select_next(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = (self.selected + 1) % self.filtered.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = if self.selected == 0 {
                self.filtered.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    pub fn accept(&mut self) -> Option<CompletionItem> {
        if self.filtered.is_empty() {
            return None;
        }
        let idx = self.filtered[self.selected.min(self.filtered.len() - 1)];
        let item = self.items[idx].clone();
        self.visible = false;
        Some(item)
    }

    pub fn cancel(&mut self) {
        self.visible = false;
    }
}

fn extract_prefix(line: &str, cursor_pos: usize) -> String {
    let text = &line[..cursor_pos.min(line.len())];
    let mut end = text.len();
    while end > 0 {
        let c = text[..end].chars().last().unwrap();
        if c.is_alphanumeric() || c == '_' || c == '$' {
            end -= c.len_utf8();
        } else {
            break;
        }
    }
    text[end..].to_string()
}

fn cursor_after_dot(line: &str, cursor_pos: usize) -> bool {
    let text = &line[..cursor_pos.min(line.len())];
    text.ends_with('.')
}

fn fuzzy_match_completion(query: &str, target: &str) -> Option<(i64, Vec<usize>)> {
    if query.is_empty() {
        return Some((0, Vec::new()));
    }

    let target_lower = target.to_lowercase();
    let query_lower = query.to_lowercase();

    if target_lower == query_lower {
        return Some((1000 + target.len() as i64, (0..query.len()).collect()));
    }

    if target_lower.starts_with(&query_lower) {
        return Some((
            500 + (target.len() - query.len()) as i64,
            (0..query.len()).collect(),
        ));
    }

    None
}
