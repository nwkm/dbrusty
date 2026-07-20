use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, CellEditFocus, CellEditMode, Focus, QueryEditorFocus};
use crate::widgets::mongo_doc_list::render_mongo_doc_list;

fn render_cell_edit_popup(frame: &mut Frame, area: Rect, app: &App) {
    let popup_width = area.width.saturating_sub(10).min(60);
    let popup_height = 9;
    let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = area.y + (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" Edit Cell ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(popup_area);
    frame.render_widget(&block, popup_area);

    let input_area = Rect::new(inner.x, inner.y, inner.width, 3);
    let input_border = if app.cell_edit_focus == CellEditFocus::Editor {
        Color::Cyan
    } else {
        Color::DarkGray
    };
    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(input_border));
    let input_inner = input_block.inner(input_area);
    frame.render_widget(&input_block, input_area);

    if !app.cell_edit_input.value.is_empty() || app.cell_edit_focus == CellEditFocus::Editor {
        frame.render_widget(
            Paragraph::new(app.cell_edit_input.value.as_str()),
            input_inner,
        );
    } else {
        frame.render_widget(
            Paragraph::new("Value").style(Style::default().fg(Color::DarkGray)),
            input_inner,
        );
    }

    let btn_area = Rect::new(inner.x, inner.y + 5, inner.width, 1);
    let btn_gap = btn_area.width.saturating_sub(22) / 2;

    let cancel_bg = if app.cell_edit_focus == CellEditFocus::Cancel {
        Color::Red
    } else {
        Color::Reset
    };
    let confirm_bg = if app.cell_edit_focus == CellEditFocus::Confirm {
        Color::Green
    } else {
        Color::Reset
    };

    let cancel_span = Span::styled(
        " [ Cancel ] ",
        Style::default().fg(Color::White).bg(cancel_bg),
    );
    let confirm_span = Span::styled(
        " [ Confirm ] ",
        Style::default().fg(Color::White).bg(confirm_bg),
    );

    let btn_line = Line::from(vec![
        Span::raw(" ".repeat(btn_gap as usize)),
        cancel_span,
        Span::raw("  "),
        confirm_span,
    ]);
    frame.render_widget(Paragraph::new(Text::from(btn_line)), btn_area);

    if app.cell_edit_focus == CellEditFocus::Editor {
        let cursor_x = input_inner.x + app.cell_edit_input.cursor_pos as u16;
        let cursor_y = input_inner.y;
        if cursor_x < input_inner.x + input_inner.width.saturating_sub(1) {
            frame.set_cursor_position((cursor_x, cursor_y));
        }
    }
}

pub fn render(frame: &mut Frame, area: Rect, app: &App, focused: bool) {
    let vertical = Layout::vertical([Constraint::Percentage(25), Constraint::Percentage(75)]);
    let [top_area, results_area] = vertical.areas(area);

    let editor_focused =
        focused && app.focus == Focus::Main && app.query_editor_focus == QueryEditorFocus::Editor;
    let editor_border = if editor_focused {
        Color::Cyan
    } else {
        Color::White
    };
    let title = if app.editing_query {
        " Query Editor [EDITING] "
    } else {
        " Query Editor "
    };
    let editor_block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(editor_border));
    let editor_inner = editor_block.inner(top_area);
    frame.render_widget(&editor_block, top_area);

    if app.editing_query {
        app.query_editor.render_into(frame, editor_inner);
    } else if app.query_text.is_empty() {
        let hint = Paragraph::new(
            "Double-click a table in the sidebar\nto generate a SELECT query.\n\
             Ctrl+E  Edit query   Ctrl+R  Run query",
        )
        .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(hint, editor_inner);
    } else {
        let text = Paragraph::new(app.query_text.as_str()).wrap(Wrap { trim: false });
        frame.render_widget(text, editor_inner);
    }

    let results_focused =
        focused && app.focus == Focus::Main && app.query_editor_focus == QueryEditorFocus::Results;
    let results_border = if results_focused && app.cell_edit_mode == CellEditMode::Inactive {
        Color::Green
    } else if app.cell_edit_mode == CellEditMode::Editing {
        Color::Cyan
    } else {
        Color::DarkGray
    };
    let results_block = Block::default()
        .title(" Results ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(results_border));
    let results_inner = results_block.inner(results_area);
    frame.render_widget(&results_block, results_area);

    if app.is_mongo_query() {
        if let Some(documents) = app.mongo_raw_documents.as_deref() {
            render_mongo_doc_list(
                frame,
                results_inner,
                documents,
                &app.mongo_doc_state,
                results_focused,
            );
        }
    } else if !app.query_data.is_empty() && !app.query_columns.is_empty() {
        let header_line = app
            .query_columns
            .iter()
            .enumerate()
            .map(|(ci, name)| {
                let s = if ci > 0 {
                    format!(" | {:width$}", name, width = app.query_col_widths[ci])
                } else {
                    format!("{:width$}", name, width = app.query_col_widths[ci])
                };
                Span::styled(
                    s,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
            })
            .collect::<Vec<_>>();

        let sep_line = app
            .query_col_widths
            .iter()
            .enumerate()
            .map(|(ci, &w)| {
                let s = if ci > 0 {
                    format!("-+-{}", "-".repeat(w))
                } else {
                    "-".repeat(w)
                };
                Span::styled(s, Style::default().fg(Color::DarkGray))
            })
            .collect::<Vec<_>>();

        let header_count = 2.min(results_inner.height as usize);
        let header_area = Rect::new(
            results_inner.x,
            results_inner.y,
            results_inner.width,
            header_count as u16,
        );

        frame.render_widget(
            Paragraph::new(Text::from(vec![
                Line::from(header_line),
                Line::from(sep_line),
            ]))
            .scroll((0, app.result_scroll_h)),
            header_area,
        );

        let data_top = header_count as u16;
        let data_height = results_inner.height.saturating_sub(data_top);
        if data_height > 0 {
            let data_area = Rect::new(
                results_inner.x,
                results_inner.y + data_top,
                results_inner.width,
                data_height,
            );

            // Virtual scrolling: only render visible rows
            let visible_start = app.result_scroll_v;
            let visible_end = (visible_start + data_height as usize).min(app.query_data.len());

            let data_lines: Vec<Line> = app.query_data[visible_start..visible_end]
                .iter()
                .enumerate()
                .map(|(i, row)| {
                    let ri = visible_start + i;
                    let s = row
                        .iter()
                        .enumerate()
                        .map(|(ci, val)| {
                            if ci > 0 {
                                format!(" | {:width$}", val, width = app.query_col_widths[ci])
                            } else {
                                format!("{:width$}", val, width = app.query_col_widths[ci])
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("");
                    let is_selected = ri == app.selected_row && results_focused;
                    let style = if is_selected {
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    Line::from(Span::styled(s, style))
                })
                .collect();

            frame.render_widget(
                Paragraph::new(Text::from(data_lines)).scroll((0, app.result_scroll_h)),
                data_area,
            );
        }
    } else if !app.query_results.is_empty() {
        let all_lines: Vec<&str> = app.query_results.lines().collect();
        let header_end = all_lines
            .iter()
            .position(|l| l.contains("---"))
            .map(|i| i + 1)
            .unwrap_or(1)
            .min(all_lines.len());

        let header_lines = &all_lines[..header_end];
        let data_lines = &all_lines[header_end..];

        let header_count = header_end.min(results_inner.height as usize);
        let header_area = Rect::new(
            results_inner.x,
            results_inner.y,
            results_inner.width,
            header_count as u16,
        );

        let header_text: Vec<Line> = header_lines
            .iter()
            .map(|s| {
                let style = if s.contains("---") {
                    Style::default().fg(Color::DarkGray)
                } else {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                };
                Line::from(Span::styled(*s, style))
            })
            .collect();
        frame.render_widget(
            Paragraph::new(ratatui::text::Text::from(header_text)).scroll((0, app.result_scroll_h)),
            header_area,
        );

        let data_top = header_count as u16;
        let data_height = results_inner.height.saturating_sub(data_top);
        if data_height > 0 && !data_lines.is_empty() {
            let data_area = Rect::new(
                results_inner.x,
                results_inner.y + data_top,
                results_inner.width,
                data_height,
            );

            let data_text: Vec<Line> = data_lines
                .iter()
                .map(|s| Line::from(Span::styled(*s, Style::default().fg(Color::White))))
                .collect();
            frame.render_widget(
                Paragraph::new(ratatui::text::Text::from(data_text))
                    .scroll((app.result_scroll_v as u16, app.result_scroll_h)),
                data_area,
            );
        }

        if data_lines.is_empty() && results_inner.height > header_count as u16 {
            let msg_area = Rect::new(
                results_inner.x,
                results_inner.y + header_count as u16,
                results_inner.width,
                results_inner.height - header_count as u16,
            );
            let msg = Paragraph::new(header_lines.last().copied().unwrap_or("")).style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            );
            frame.render_widget(msg, msg_area);
        }
    } else {
        let hint = Paragraph::new("Run a query to see results here.")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(hint, results_inner);
    }

    if app.cell_edit_mode == CellEditMode::Editing {
        render_cell_edit_popup(frame, frame.area(), app);
    }
}
