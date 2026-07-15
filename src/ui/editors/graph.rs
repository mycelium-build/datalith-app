use std::path::Path;

use gpui::*;
use gpui_component::input::{Input, InputState};

use crate::ui::{BASE_FONT_SIZE, LINE_HEIGHT};

pub(crate) struct GraphEditor {
    input: Entity<InputState>,
}

impl GraphEditor {
    pub(crate) fn new(input: Entity<InputState>) -> Self {
        Self { input }
    }

    pub(crate) fn new_state(path: &Path, window: &mut Window, cx: &mut App) -> Entity<InputState> {
        let content = std::fs::read_to_string(path).unwrap_or_default();
        cx.new(|cx| {
            InputState::new(window, cx)
                .code_editor("yaml")
                .line_number(false)
                .folding(false)
                .default_value(content)
        })
    }

    pub(crate) fn input(&self) -> &Entity<InputState> {
        &self.input
    }

    pub(crate) fn render(&self, _cx: &mut App) -> AnyElement {
        div()
            .size_full()
            .child(
                Input::new(&self.input)
                    .h_full()
                    .appearance(false)
                    .text_size(px(BASE_FONT_SIZE as f32))
                    .line_height(px(BASE_FONT_SIZE as f32 * LINE_HEIGHT)),
            )
            .into_any_element()
    }

    pub(crate) fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.focus_handle(cx)
    }
}
