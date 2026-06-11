use std::path::Path;

use gpui::*;
use gpui_component::input::{Input, InputState};

use crate::consts::{BASE_FONT_SIZE, MD_LINE_HEIGHT};

pub(crate) struct MarkdownEditor {
    input: Entity<InputState>,
}

impl MarkdownEditor {
    pub(crate) fn new(input: Entity<InputState>) -> Self {
        Self { input }
    }

    pub(crate) fn new_state(
        path: &Path,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<InputState> {
        let content = std::fs::read_to_string(path).unwrap_or_default();
        cx.new(|cx| {
            InputState::new(window, cx)
                .code_editor("markdown")
                .line_number(false)
                .folding(false)
                .default_value(content)
        })
    }

    pub(crate) fn input(&self) -> &Entity<InputState> {
        &self.input
    }

    pub(crate) fn render(&self, _cx: &mut App) -> AnyElement {
        let base_font_size = BASE_FONT_SIZE as f32;
        let line_height = MD_LINE_HEIGHT;

        div()
            .size_full()
            .child(
                Input::new(&self.input)
                    .h_full()
                    .appearance(false)
                    .text_size(px(base_font_size))
                    .line_height(px(base_font_size * line_height)),
            )
            .into_any_element()
    }

    pub(crate) fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.focus_handle(cx)
    }
}
