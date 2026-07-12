use gpui::*;

use crate::ui::DatalithView;

#[derive(Default)]
pub(crate) struct AppState {
    pub(crate) view: Option<Entity<DatalithView>>,
}

impl Global for AppState {}
