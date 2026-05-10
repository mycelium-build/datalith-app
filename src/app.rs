use gpui::*;

use crate::view::DatalithView;

pub struct AppState {
    pub view: Option<Entity<DatalithView>>,
}

impl Global for AppState {}
