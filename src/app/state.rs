use gpui_kit::{App, Entity, Global};

use crate::ui::DatalithView;

#[derive(Default)]
pub struct AppState {
    pub view: Option<Entity<DatalithView>>,
}

impl Global for AppState {}

impl AppState {
    /// The main view, once a window has been opened.
    pub fn view(cx: &App) -> Option<Entity<DatalithView>> {
        cx.try_global::<Self>().and_then(|state| state.view.clone())
    }
}
