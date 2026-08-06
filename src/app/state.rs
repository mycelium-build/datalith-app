use gpui::{Entity, Global};

use crate::ui::DatalithView;

#[derive(Default)]
pub struct AppState {
    pub view: Option<Entity<DatalithView>>,
}

impl Global for AppState {}
