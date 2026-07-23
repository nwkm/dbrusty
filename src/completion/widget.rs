use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::completion::{CompletionKind, CompletionState};

const MAX_VISIBLE_ITEMS: usize = 10;
const POPUP_WIDTH: u16 = 40;

fn kind_color(kind: &CompletionKind) -> Color {
    match kind {
        CompletionKind::Keyword => Color::Cyan,
        CompletionKind::Function => Color::LightMagenta,
        CompletionKind::Type => Color::Magenta,
        CompletionKind::Table => Color::Green,
        CompletionKind::Column => Color::Yellow,
        CompletionKind::View => Color::White,
        CompletionKind::Schema => Color::LightBlue,
        CompletionKind::Operator => Color::Red,
        CompletionKind::Command => Color::Yellow,
        CompletionKind::Snippet => Color::LightGreen,
    }
}

fn kind_label(kind: &CompletionKind) -> &'static str {
    match kind {
        CompletionKind::Keyword => "KEY",
        CompletionKind::Function => "FN",
        CompletionKind::Type => "TYPE",
        CompletionKind::Table => "TBL",
        CompletionKind::Column => "COL",
        CompletionKind::View => "VIEW",
        CompletionKind::Schema => "SCH",
        CompletionKind::Operator => "OP",
        CompletionKind::Command => "CMD",
        CompletionKind::Snippet => "SNIP",
    }
}

pub fn render_autocomplete(frame: &mut Frame, area: Rect, state: &CompletionState) {
    if !state.visible || state.filtered.is_empty() {
        return;
    }

    let popup_height = (state.filtered.len().min(MAX_VISIBLE_ITEMS) as u16 + 2)
        .min(area.height.saturating_sub(state.trigger_row + 1));

    let popup_x = (state.trigger_col + 1).min(area.width.saturating_sub(POPUP_WIDTH));
    let popup_y = state.trigger_row + 2;

    let mut popup_area = Rect::new(
        area.x + popup_x,
        area.y + popup_y,
        POPUP_WIDTH,
        popup_height + 2,
    );

    if popup_area.bottom() > area.bottom() {
        let adjusted_y = state.trigger_row.saturating_sub(popup_height + 2);
        popup_area = Rect::new(
            area.x + popup_x,
            area.y + adjusted_y,
            POPUP_WIDTH,
            popup_height + 2,
        );
    }

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let scroll = if state.selected >= MAX_VISIBLE_ITEMS {
        state.selected - MAX_VISIBLE_ITEMS + 1
    } else {
        0
    };

    let selected_style = Style::default()
        .fg(Color::Black)
        .bg(Color::Yellow)
        .add_modifier(Modifier::BOLD);

    let visible_range = scroll..(scroll + MAX_VISIBLE_ITEMS).min(state.filtered.len());

    for (display_idx, idx) in visible_range.clone().enumerate() {
        let item_idx = state.filtered[idx];
        let item = &state.items[item_idx];
        let is_selected = idx == state.selected.min(state.filtered.len() - 1);

        let kind_badge = format!(" {: <4}", kind_label(&item.kind));
        let badge_style = Style::default()
            .fg(kind_color(&item.kind))
            .add_modifier(Modifier::BOLD);

        let label = &item.label;
        let detail = if item.detail.is_empty() {
            String::new()
        } else {
            format!("  {}", item.detail)
        };

        let mut spans = vec![Span::styled(kind_badge, badge_style)];
        spans.push(Span::raw(" "));
        spans.push(Span::raw(label.clone()));
        if !detail.is_empty() {
            spans.push(Span::styled(detail, Style::default().fg(Color::DarkGray)));
        }

        let row_y = inner.y + display_idx as u16;
        let row_area = Rect::new(inner.x, row_y, inner.width, 1);

        if is_selected {
            frame.render_widget(
                Paragraph::new(Text::from(Line::from(spans))).style(selected_style),
                row_area,
            );
        } else {
            frame.render_widget(Paragraph::new(Text::from(Line::from(spans))), row_area);
        }
    }
}
