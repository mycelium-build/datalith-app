use std::path::Path;

use gpui_kit::component::input::{Editor, EditorState};
use gpui_kit::{
    AnyElement, App, AppContext, Entity, FocusHandle, Focusable, IntoElement, ParentElement,
    Styled, Window, div, px,
};

use crate::ui::{BASE_FONT_SIZE, LINE_HEIGHT};

pub struct MarkdownEditor {
    input: Entity<EditorState>,
    read_only: bool,
}

impl MarkdownEditor {
    pub const fn new(input: Entity<EditorState>, read_only: bool) -> Self {
        Self { input, read_only }
    }

    pub fn new_state(path: &Path, window: &mut Window, cx: &mut App) -> Entity<EditorState> {
        let content = crate::vault::source::read_to_string(path).unwrap_or_default();
        cx.new(|cx| {
            EditorState::new(window, cx)
                .language("markdown")
                .line_number(false)
                .folding(false)
                .default_value(content)
        })
    }

    pub const fn input(&self) -> &Entity<EditorState> {
        &self.input
    }

    pub fn render(&self, _cx: &mut App) -> AnyElement {
        let base_font_size = BASE_FONT_SIZE;
        let line_height = LINE_HEIGHT;

        div()
            .size_full()
            .child(
                Editor::new(&self.input)
                    .h_full()
                    .appearance(false)
                    .readonly(self.read_only)
                    .text_size(px(base_font_size))
                    .line_height(px(base_font_size * line_height)),
            )
            .into_any_element()
    }

    pub(crate) fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.focus_handle(cx)
    }
}
