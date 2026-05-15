use gpui::*;

use crate::view::DatalithView;

pub(crate) struct AppState {
    pub(crate) view: Option<Entity<DatalithView>>,
}

impl Global for AppState {}
