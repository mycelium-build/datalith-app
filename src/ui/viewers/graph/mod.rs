mod camera;
mod interaction;
mod model;
mod paint;
mod physics;
mod render;
mod snapshot;

use gpui::{AnyElement, App, AppContext, Context, Entity, FocusHandle, IntoElement};
use gpui_component::input::InputState;

use crate::document::handler::FileHandler;
use crate::vault::VaultCatalog;

use self::interaction::GraphViewState;

pub(crate) struct GraphViewer {
    state: Entity<GraphViewState>,
}

impl GraphViewer {
    pub(crate) fn new(
        input: Entity<InputState>,
        catalog: Option<VaultCatalog>,
        cx: &mut Context<FileHandler>,
    ) -> Self {
        let handler = cx.entity().downgrade();
        let state = cx.new(|cx| GraphViewState::new(input, catalog, handler, cx));
        state.update(cx, |state, cx| state.rebuild(cx));
        Self { state }
    }

    pub(crate) fn refresh(&self, cx: &mut App) {
        self.state.update(cx, |state, cx| state.rebuild(cx));
    }

    pub(crate) fn set_vault_catalog(&self, catalog: VaultCatalog, cx: &mut Context<FileHandler>) {
        self.state.update(cx, |state, cx| {
            state.catalog = Some(catalog);
            state.rebuild(cx);
        });
    }

    pub(crate) fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.state.read(cx).focus_handle.clone()
    }

    pub(crate) fn render(&self, _handler: Entity<FileHandler>, _cx: &mut App) -> AnyElement {
        self.state.clone().into_any_element()
    }
}
