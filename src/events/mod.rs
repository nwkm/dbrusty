use crate::app::Page;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum AppEvent {
    Quit,
    Page(Page),
    FocusChanged,
    Resize(u16, u16),
    Tick,
}
