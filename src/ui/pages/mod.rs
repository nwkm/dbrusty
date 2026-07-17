mod connections;
mod dashboard;
mod explorer;
mod favorites;
mod help;
mod history;
mod logs;
mod query_editor;
mod schema_browser;

pub fn render(
    frame: &mut ratatui::Frame,
    area: ratatui::layout::Rect,
    page: crate::app::Page,
    app: &crate::app::App,
    focused: bool,
) {
    match page {
        crate::app::Page::Dashboard => dashboard::render(frame, area, app, focused),
        crate::app::Page::Connections => connections::render(frame, area, app, focused),
        crate::app::Page::Explorer => explorer::render(frame, area, app, focused),
        crate::app::Page::QueryEditor => query_editor::render(frame, area, app, focused),
        crate::app::Page::History => history::render(frame, area, app, focused),
        crate::app::Page::Favorites => favorites::render(frame, area, app, focused),
        crate::app::Page::SchemaBrowser => schema_browser::render(frame, area, app, focused),
        crate::app::Page::Logs => logs::render(frame, area, app, focused),
        crate::app::Page::Help => help::render(frame, area, app, focused),
        crate::app::Page::Settings | crate::app::Page::About => {
            let _ = frame;
            let _ = area;
            let _ = app;
            let _ = focused;
        }
    }
}
