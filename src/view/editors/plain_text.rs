use gpui::*;
use gpui_component::input::{Input, InputState};

pub(crate) struct PlainTextEditor {
    input: Entity<InputState>,
}

impl PlainTextEditor {
    pub(crate) fn new(input: Entity<InputState>) -> Self {
        Self { input }
    }

    pub(crate) fn input(&self) -> &Entity<InputState> {
        &self.input
    }

    pub(crate) fn render(&self, _cx: &mut App) -> AnyElement {
        div()
            .flex_1()
            .child(Input::new(&self.input).h_full().appearance(false))
            .into_any_element()
    }

    pub(crate) fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.focus_handle(cx)
    }
}
