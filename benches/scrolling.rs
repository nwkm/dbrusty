use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use dbrusty::app::Focus;
use dbrusty::app::{App, Page, QueryEditorFocus};
use dbrusty::config::AppConfig;

const ROW_COUNT: usize = 1000;
const COL_COUNT: usize = 8;

fn make_test_app() -> App {
    let mut app = App::with_config(AppConfig::default());

    app.current_page = Page::QueryEditor;
    app.focus = Focus::Main;
    app.query_editor_focus = QueryEditorFocus::Results;
    app.main_rect = Rect::new(0, 0, 120, 40);

    app.query_columns = (0..COL_COUNT).map(|i| format!("col_{}", i)).collect();

    app.query_data = (0..ROW_COUNT)
        .map(|i| (0..COL_COUNT).map(|j| format!("{}_{}", i, j)).collect())
        .collect();

    app.query_col_widths = (0..COL_COUNT)
        .map(|i| format!("{}_{}", ROW_COUNT - 1, i).len())
        .collect();

    app.selected_row = 0;
    app.selected_col = 0;
    app.result_scroll_v = 0;
    app.result_scroll_h = 0;

    app
}

fn bench_scroll_down(c: &mut Criterion) {
    let mut app = make_test_app();
    let data_height = app.main_rect.height.saturating_sub(4) as usize;

    let mut group = c.benchmark_group("scroll_down");
    group.throughput(Throughput::Elements(1000));
    group.measurement_time(Duration::from_secs(5));

    group.bench_function("handle_results_key::Down x1000", |b| {
        b.iter(|| {
            for _ in 0..ROW_COUNT {
                if app.selected_row + 1 < app.query_data.len() {
                    app.selected_row += 1;
                }
                if app.selected_row >= app.result_scroll_v + data_height {
                    app.result_scroll_v = app.selected_row.saturating_sub(data_height) + 1;
                }
            }
            app.selected_row = 0;
            app.result_scroll_v = 0;
        })
    });

    group.bench_function("handle_results_key::Down (single)", |b| {
        b.iter(|| {
            if app.selected_row + 1 < app.query_data.len() {
                app.selected_row += 1;
            }
            if app.selected_row >= app.result_scroll_v + data_height {
                app.result_scroll_v = app.selected_row.saturating_sub(data_height) + 1;
            }
        })
    });

    group.finish();
}

fn bench_render_data_lines(c: &mut Criterion) {
    let app = make_test_app();
    let data_height = app.main_rect.height.saturating_sub(4) as usize;

    let visible_start = app.result_scroll_v;
    let visible_end = (visible_start + data_height).min(app.query_data.len());

    let mut group = c.benchmark_group("render_data_lines");
    group.measurement_time(Duration::from_secs(5));

    group.bench_function("build_data_lines_visible_window", |b| {
        b.iter(|| {
            let _: Vec<Line> = app.query_data[visible_start..visible_end]
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
                    let is_selected = ri == app.selected_row;
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
        })
    });

    group.finish();
}

fn bench_scroll_and_render(c: &mut Criterion) {
    let mut app = make_test_app();
    let data_height = app.main_rect.height.saturating_sub(4) as usize;

    let mut group = c.benchmark_group("scroll_and_render");
    group.throughput(Throughput::Elements(1000));
    group.measurement_time(Duration::from_secs(5));

    group.bench_function("scroll_1000_rows_with_render", |b| {
        b.iter(|| {
            for _ in 0..ROW_COUNT {
                if app.selected_row + 1 < app.query_data.len() {
                    app.selected_row += 1;
                }
                if app.selected_row >= app.result_scroll_v + data_height {
                    app.result_scroll_v = app.selected_row.saturating_sub(data_height) + 1;
                }

                let visible_start = app.result_scroll_v;
                let visible_end = (visible_start + data_height).min(app.query_data.len());

                let _data_lines: Vec<Line> = app.query_data[visible_start..visible_end]
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
                        let is_selected = ri == app.selected_row;
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
            }
            app.selected_row = 0;
            app.result_scroll_v = 0;
        })
    });

    group.finish();
}

fn bench_ensure_cell_visible(c: &mut Criterion) {
    let mut app = make_test_app();
    let data_height = app.main_rect.height.saturating_sub(4) as usize;

    let mut group = c.benchmark_group("ensure_cell_visible");
    group.measurement_time(Duration::from_secs(5));

    group.bench_function("ensure_cell_visible_x1000", |b| {
        b.iter(|| {
            for i in 0..ROW_COUNT {
                app.selected_row = i;
                if app.selected_row >= app.result_scroll_v + data_height {
                    app.result_scroll_v = app.selected_row.saturating_sub(data_height) + 1;
                }
            }
            app.selected_row = 0;
            app.result_scroll_v = 0;
        })
    });

    group.finish();
}

fn bench_full_grid_render(c: &mut Criterion) {
    let app = make_test_app();
    let _data_height = app.main_rect.height.saturating_sub(4) as usize;

    let mut group = c.benchmark_group("full_grid_render");
    group.measurement_time(Duration::from_secs(5));

    // Build all 1000 rows (no virtual scrolling)
    group.bench_function("build_all_1000_lines", |b| {
        b.iter(|| {
            let _: Vec<Line> = app.query_data[..]
                .iter()
                .enumerate()
                .map(|(_ri, row)| {
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
                    Line::from(Span::styled(s, Style::default().fg(Color::White)))
                })
                .collect();
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_scroll_down,
    bench_render_data_lines,
    bench_scroll_and_render,
    bench_ensure_cell_visible,
    bench_full_grid_render,
);
criterion_main!(benches);
