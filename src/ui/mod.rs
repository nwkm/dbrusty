mod layout;
pub mod pages;
pub mod search;
pub mod sidebar;

pub fn render(frame: &mut ratatui::Frame, app: &mut crate::app::App) {
    layout::render_layout(frame, app);
}
