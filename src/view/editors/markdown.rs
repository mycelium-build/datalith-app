use std::path::PathBuf;

use gpui::*;
use gpui_component::input::{Input, InputState};

use crate::consts::{BASE_FONT_SIZE, MD_LINE_HEIGHT};
use crate::markdown::find_link_at_offset;

pub(crate) struct MarkdownEditor {
    input: Entity<InputState>,
    file_path: PathBuf,
}

impl MarkdownEditor {
    pub(crate) fn new(input: Entity<InputState>, file_path: PathBuf) -> Self {
        Self { input, file_path }
    }

    pub(crate) fn new_state(
        content: String,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<InputState> {
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

    pub(crate) fn file_path(&self) -> &PathBuf {
        &self.file_path
    }

    pub(crate) fn open_link_at_cursor(&self, cx: &mut App) -> Option<(String, bool)> {
        let offset = self.input.read(cx).cursor();
        let text = self.input.read(cx).value().to_string();
        find_link_at_offset(&text, offset).map(|url| (url, true))
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
