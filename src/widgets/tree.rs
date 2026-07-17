use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

#[derive(Debug, Clone)]
pub struct TreeItem {
    pub id: String,
    pub label: String,
    pub depth: usize,
    pub expanded: bool,
    pub has_children: bool,
    pub children: Vec<TreeItem>,
}

impl TreeItem {
    pub fn leaf(id: &str, label: &str, depth: usize) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            depth,
            expanded: false,
            has_children: false,
            children: Vec::new(),
        }
    }

    pub fn group(id: &str, label: &str, depth: usize) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            depth,
            expanded: false,
            has_children: true,
            children: Vec::new(),
        }
    }

    pub fn branch(id: &str, label: &str, depth: usize, children: Vec<TreeItem>) -> Self {
        let has_children = !children.is_empty();
        Self {
            id: id.to_string(),
            label: label.to_string(),
            depth,
            expanded: false,
            has_children,
            children,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TreeState {
    pub items: Vec<TreeItem>,
    pub selected: usize,
    pub offset: usize,
}

impl TreeState {
    pub fn new(items: Vec<TreeItem>) -> Self {
        Self {
            items,
            selected: 0,
            offset: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn flattened(&self) -> Vec<&TreeItem> {
        fn go<'a>(item: &'a TreeItem, out: &mut Vec<&'a TreeItem>) {
            out.push(item);
            if item.expanded {
                for child in &item.children {
                    go(child, out);
                }
            }
        }
        let mut result = Vec::new();
        for item in &self.items {
            go(item, &mut result);
        }
        result
    }

    pub fn select_up(&mut self) {
        let count = self.flattened().len();
        if count == 0 {
            return;
        }
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn select_down(&mut self) {
        let count = self.flattened().len();
        if count == 0 {
            return;
        }
        if self.selected + 1 < count {
            self.selected += 1;
        }
    }

    pub fn selected_id(&self) -> Option<String> {
        self.flattened().get(self.selected).map(|i| i.id.clone())
    }

    pub fn toggle_selected(&mut self) {
        let id = match self.selected_id() {
            Some(id) => id,
            None => return,
        };
        self.toggle_by_id(&id);
    }

    pub fn toggle_by_id(&mut self, id: &str) {
        fn toggle(items: &mut [TreeItem], id: &str) -> bool {
            for item in items.iter_mut() {
                if item.id == id {
                    item.expanded = !item.expanded;
                    if !item.expanded {
                        fn collapse_all(items: &mut [TreeItem]) {
                            for child in items.iter_mut() {
                                child.expanded = false;
                                collapse_all(&mut child.children);
                            }
                        }
                        collapse_all(&mut item.children);
                    }
                    return true;
                }
                if toggle(&mut item.children, id) {
                    return true;
                }
            }
            false
        }
        toggle(&mut self.items, id);
    }

    pub fn set_children(&mut self, id: &str, children: Vec<TreeItem>) {
        fn find<'a>(items: &'a mut [TreeItem], id: &str) -> Option<&'a mut Vec<TreeItem>> {
            for item in items.iter_mut() {
                if item.id == id {
                    return Some(&mut item.children);
                }
                if let Some(found) = find(&mut item.children, id) {
                    return Some(found);
                }
            }
            None
        }
        if let Some(target) = find(&mut self.items, id) {
            *target = children;
            fn update_has_children(items: &mut [TreeItem]) {
                for item in items.iter_mut() {
                    item.has_children = !item.children.is_empty();
                    update_has_children(&mut item.children);
                }
            }
            update_has_children(&mut self.items);
        }
    }

    pub fn adjust_offset(&mut self, visible_height: usize) {
        let len = self.flattened().len();
        if len == 0 {
            self.offset = 0;
            return;
        }
        if self.selected >= len {
            self.selected = len.saturating_sub(1);
        }
        if self.offset > len.saturating_sub(1) {
            self.offset = len.saturating_sub(1);
        }
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + visible_height {
            self.offset = self
                .selected
                .saturating_add(1)
                .saturating_sub(visible_height);
        }
        let max_offset = len.saturating_sub(visible_height);
        if self.offset > max_offset {
            self.offset = max_offset;
        }
    }

    pub fn go_to_first(&mut self) {
        self.selected = 0;
        self.offset = 0;
    }

    pub fn go_to_last(&mut self) {
        let count = self.flattened().len();
        if count > 0 {
            self.selected = count - 1;
        }
    }

    pub fn hit_test(&self, mouse_y: u16, area: Rect) -> Option<usize> {
        let content_start = area.y + 1;
        let visible_height = area.height.saturating_sub(3) as usize;
        if mouse_y < content_start {
            return None;
        }
        let row = (mouse_y - content_start) as usize;
        if row >= visible_height {
            return None;
        }
        let idx = self.offset + row;
        let items = self.flattened();
        if idx < items.len() {
            Some(idx)
        } else {
            None
        }
    }

    pub fn is_item_expanded(&self, id: &str) -> bool {
        fn check(items: &[TreeItem], id: &str) -> Option<bool> {
            for item in items {
                if item.id == id {
                    return Some(item.expanded);
                }
                if let Some(result) = check(&item.children, id) {
                    return Some(result);
                }
            }
            None
        }
        check(&self.items, id).unwrap_or(false)
    }
}

pub fn render_tree(frame: &mut Frame, area: Rect, state: &mut TreeState, focused: bool) {
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::White)
    };

    let block = Block::default()
        .title(" Explorer ")
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let visible_height = inner.height.saturating_sub(1) as usize;
    state.adjust_offset(visible_height);
    let offset = state.offset;
    let items = state.flattened();

    let mut lines: Vec<Line> = Vec::new();
    for (idx, item) in items.iter().enumerate().skip(offset).take(visible_height) {
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

        lines.push(Line::from(Span::styled(label, style)));
    }

    let text = Text::from(lines);
    frame.render_widget(Paragraph::new(text), inner);
}
