use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, CellEditFocus, CellEditMode, Focus, InputMode, QueryEditorFocus};
use crate::models::connection::DatabaseType;
use crate::syntax;
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

fn render_result_filter(frame: &mut Frame, area: Rect, app: &App) {
    let input_display = format!("Result > {}", app.result_filter_input);
    let input_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let input_line = Line::from(Span::styled(input_display, input_style));

    let count = app.result_filtered_indices.len();
    let total = app.query_data.len();
    let count_text = if app.result_filter_input.is_empty() {
        format!(" {} results total", total)
    } else {
        format!(" {} / {} results", count, total)
    };
    let count_line = Line::from(Span::styled(
        count_text,
        Style::default().fg(Color::DarkGray),
    ));

    frame.render_widget(
        Paragraph::new(Text::from(vec![input_line, count_line])),
        area,
    );

    if area.width > 0 {
        let cursor_x = area.x + "Result > ".len() as u16 + app.result_filter_cursor as u16;
        let cursor_y = area.y;
        if cursor_x < area.x + area.width {
            frame.set_cursor_position((cursor_x, cursor_y));
        }
    }
}

fn build_cell_spans<'a>(
    val: &str,
    positions: &[u32],
    width: usize,
    base_style: Style,
    hl_style: Style,
) -> Vec<Span<'a>> {
    if positions.is_empty() {
        return vec![Span::styled(
            format!("{:width$}", val, width = width),
            base_style,
        )];
    }

    let mut spans = Vec::new();
    let chars: Vec<char> = val.chars().collect();
    let mut prev = 0;
    for &pos in positions {
        let p = pos as usize;
        if p >= chars.len() {
            continue;
        }
        if p > prev {
            let s: String = chars[prev..p].iter().collect();
            spans.push(Span::styled(s, base_style));
        }
        let s: String = chars[p..=p].iter().collect();
        spans.push(Span::styled(s, hl_style));
        prev = p + 1;
    }
    if prev < chars.len() {
        let s: String = chars[prev..].iter().collect();
        spans.push(Span::styled(s, base_style));
    }

    let total: usize = spans.iter().map(|s| s.content.len()).sum();
    if total < width {
        spans.push(Span::raw(" ".repeat(width - total)));
    }

    spans
}

fn render_editor(frame: &mut Frame, area: Rect, app: &App, focused: bool) {
    let editor_focused =
        focused && app.focus == Focus::Main && app.query_editor_focus == QueryEditorFocus::Editor;
    let editor_border = if editor_focused {
        Color::Cyan
    } else {
        Color::White
    };
    let title = if app.editing_query {
        if app.input_mode == InputMode::Insert {
            " Query Editor [INSERT] "
        } else {
            " Query Editor [NORMAL] "
        }
    } else {
        " Query Editor "
    };
    let editor_block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(editor_border));
    let editor_inner = editor_block.inner(area);
    frame.render_widget(&editor_block, area);

    if app.editing_query {
        let lines = app.query_editor.lines();
        let (cursor_row, cursor_col) = app.query_editor.cursor();
        let cursor_row = cursor_row as usize;
        let cursor_col = cursor_col as usize;

        let db_type = app.pool.active_id.and_then(|id| {
            app.connections
                .iter()
                .find(|c| c.id == id)
                .map(|c| &c.db_type)
        });

        let sql_fn: fn(&str) -> Vec<Span<'_>> = syntax::highlight_sql_line;
        let highlighter: fn(&str) -> Vec<Span<'_>> = match db_type {
            Some(DatabaseType::MongoDB) => {
                syntax::highlight_mongo_line as fn(&str) -> Vec<Span<'_>>
            }
            Some(DatabaseType::Redis) => syntax::highlight_redis_line as fn(&str) -> Vec<Span<'_>>,
            _ => sql_fn,
        };

        let highlighted_lines: Vec<Line<'_>> = lines
            .iter()
            .enumerate()
            .map(|(row, line)| {
                let spans = if row == cursor_row {
                    let spans = highlighter(line);
                    let mut col = 0;
                    let mut modified = Vec::new();
                    for span in &spans {
                        let span_s = span.content.to_string();
                        let span_w = span_s.len();
                        let span_end = col + span_w;
                        if col <= cursor_col && cursor_col < span_end {
                            let offset = cursor_col - col;
                            let before: String = span_s.chars().take(offset).collect();
                            let cchar: String = span_s.chars().skip(offset).take(1).collect();
                            let after: String = span_s.chars().skip(offset + 1).collect();
                            if !before.is_empty() {
                                modified.push(Span::styled(before, span.style));
                            }
                            if !cchar.is_empty() {
                                modified.push(Span::styled(
                                    cchar,
                                    span.style.add_modifier(Modifier::REVERSED),
                                ));
                            }
                            if !after.is_empty() {
                                modified.push(Span::styled(after, span.style));
                            }
                        } else {
                            modified.push(span.clone());
                        }
                        col = span_end;
                    }
                    modified
                } else {
                    highlighter(line)
                };
                let line_style = if row == cursor_row {
                    Style::default().bg(Color::DarkGray)
                } else {
                    Style::default()
                };
                Line::from(spans).style(line_style)
            })
            .collect();

        frame.render_widget(Paragraph::new(Text::from(highlighted_lines)), editor_inner);

        if editor_focused {
            let cy = editor_inner.y + cursor_row as u16;
            let cx = editor_inner.x + cursor_col as u16;
            if cy < editor_inner.y + editor_inner.height {
                frame.set_cursor_position((cx, cy));
            }
        }
    } else if app.query_text.is_empty() {
        let hint = Paragraph::new(
            "Double-click a table in the sidebar\nto generate a SELECT query.\n\
             Ctrl+E  Edit query   Ctrl+R  Run query",
        )
        .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(hint, editor_inner);
    } else {
        let db_type = app.pool.active_id.and_then(|id| {
            app.connections
                .iter()
                .find(|c| c.id == id)
                .map(|c| &c.db_type)
        });
        let sql_fn: fn(&str) -> Vec<Span<'_>> = syntax::highlight_sql_line;
        let highlighter: fn(&str) -> Vec<Span<'_>> = match db_type {
            Some(DatabaseType::MongoDB) => {
                syntax::highlight_mongo_line as fn(&str) -> Vec<Span<'_>>
            }
            Some(DatabaseType::Redis) => syntax::highlight_redis_line as fn(&str) -> Vec<Span<'_>>,
            _ => sql_fn,
        };
        let text_lines: Vec<Line<'_>> = app
            .query_text
            .lines()
            .map(|line| Line::from(highlighter(line)))
            .collect();
        let text = Paragraph::new(Text::from(text_lines)).wrap(Wrap { trim: false });
        frame.render_widget(text, editor_inner);
    }
}

pub fn render(frame: &mut Frame, area: Rect, app: &mut App, focused: bool) {
    let (top_area, results_area) = if app.show_query_editor {
        let vertical = Layout::vertical([Constraint::Percentage(25), Constraint::Min(0)]);
        let [editor_area, res_area] = vertical.areas(area);
        (Some(editor_area), res_area)
    } else {
        (None, area)
    };

    if let Some(editor_area) = top_area {
        render_editor(frame, editor_area, app, focused);
    }

    let results_focused =
        focused && app.focus == Focus::Main && app.query_editor_focus == QueryEditorFocus::Results;
    let results_title = if app.result_filter_active {
        " Results [FILTER] "
    } else {
        " Results "
    };
    let results_border = if results_focused && app.cell_edit_mode == CellEditMode::Inactive {
        Color::Green
    } else if app.cell_edit_mode == CellEditMode::Editing {
        Color::Cyan
    } else {
        Color::DarkGray
    };
    let results_block = Block::default()
        .title(results_title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(results_border));
    let results_inner = results_block.inner(results_area);
    frame.render_widget(&results_block, results_area);

    let filter_active = app.result_filter_active;
    let (filter_area, data_area) = if filter_active {
        let fl = Layout::vertical([Constraint::Length(2), Constraint::Min(0)]);
        let [f, d] = fl.areas(results_inner);
        (Some(f), d)
    } else {
        (None, results_inner)
    };

    if let Some(fa) = filter_area {
        render_result_filter(frame, fa, app);
    }

    if app.is_mongo_query() {
        if let Some(documents) = app.mongo_raw_documents.as_deref() {
            render_mongo_doc_list(
                frame,
                data_area,
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

        let header_count = 2.min(data_area.height as usize);
        let header_area = Rect::new(
            data_area.x,
            data_area.y,
            data_area.width,
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
        let data_height = data_area.height.saturating_sub(data_top);
        if data_height > 0 {
            let body_area = Rect::new(
                data_area.x,
                data_area.y + data_top,
                data_area.width,
                data_height,
            );

            let using_filter = !app.result_filter_input.is_empty();

            let (visible_start, visible_end) = if using_filter {
                let fi = &app.result_filtered_indices;
                let start = app.result_scroll_v.min(fi.len().saturating_sub(1));
                let end = (start + data_height as usize).min(fi.len());
                (start, end)
            } else {
                let start = app
                    .result_scroll_v
                    .min(app.query_data.len().saturating_sub(1));
                let end = (start + data_height as usize).min(app.query_data.len());
                (start, end)
            };

            let data_lines: Vec<Line> = if using_filter {
                let fi = &app.result_filtered_indices;
                fi[visible_start..visible_end]
                    .iter()
                    .enumerate()
                    .map(|(vi, &ri)| {
                        let row = &app.query_data[ri];
                        let is_selected = ri == app.selected_row && results_focused;
                        let base_style = if is_selected {
                            Style::default()
                                .fg(Color::Black)
                                .bg(Color::Yellow)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::White)
                        };
                        let hl_style = if is_selected {
                            Style::default()
                                .fg(Color::Cyan)
                                .bg(Color::Yellow)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD)
                        };
                        let row_highlights = app
                            .result_filter_highlights
                            .get(visible_start + vi)
                            .map(|h| h.as_slice())
                            .unwrap_or(&[]);
                        let mut row_spans = Vec::new();
                        for (ci, val) in row.iter().enumerate() {
                            if ci > 0 {
                                row_spans.push(Span::styled(" | ", base_style));
                            }
                            let cell_positions =
                                row_highlights.get(ci).map(|v| v.as_slice()).unwrap_or(&[]);
                            let cell_spans = build_cell_spans(
                                val,
                                cell_positions,
                                app.query_col_widths[ci],
                                base_style,
                                hl_style,
                            );
                            row_spans.extend(cell_spans);
                        }
                        Line::from(row_spans)
                    })
                    .collect()
            } else {
                app.query_data[visible_start..visible_end]
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
                    .collect()
            };

            frame.render_widget(
                Paragraph::new(Text::from(data_lines)).scroll((0, app.result_scroll_h)),
                body_area,
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
        let data_body = &all_lines[header_end..];

        let header_count = header_end.min(data_area.height as usize);
        let header_area = Rect::new(
            data_area.x,
            data_area.y,
            data_area.width,
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
        let data_height = data_area.height.saturating_sub(data_top);
        if data_height > 0 && !data_body.is_empty() {
            let body_area = Rect::new(
                data_area.x,
                data_area.y + data_top,
                data_area.width,
                data_height,
            );

            let raw_text: Vec<Line> = data_body
                .iter()
                .map(|s| Line::from(Span::styled(*s, Style::default().fg(Color::White))))
                .collect();
            frame.render_widget(
                Paragraph::new(ratatui::text::Text::from(raw_text))
                    .scroll((app.result_scroll_v as u16, app.result_scroll_h)),
                body_area,
            );
        }

        if data_body.is_empty() && data_area.height > header_count as u16 {
            let msg_area = Rect::new(
                data_area.x,
                data_area.y + header_count as u16,
                data_area.width,
                data_area.height - header_count as u16,
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
        frame.render_widget(hint, data_area);
    }

    if app.cell_edit_mode == CellEditMode::Editing {
        render_cell_edit_popup(frame, frame.area(), app);
    }
}
