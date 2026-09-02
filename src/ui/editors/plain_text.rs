use std::path::Path;

use gpui::{
    AnyElement, App, AppContext, Context, Entity, FocusHandle, Focusable, IntoElement,
    ParentElement, Styled, Window, div,
};
use gpui_component::input::{Editor, EditorState};

use crate::document::handler::{FileHandler, ReloadOutcome};

pub fn reload_text(
    path: &Path,
    handler: &mut FileHandler,
    window: &mut Window,
    cx: &mut Context<FileHandler>,
) -> anyhow::Result<ReloadOutcome> {
    let Some(input) = handler.input().cloned() else {
        return Ok(ReloadOutcome::Unsupported);
    };
    let content = std::fs::read_to_string(path)?;
    if input.read(cx).value().as_ref() == content {
        return Ok(ReloadOutcome::Unchanged);
    }
    input.update(cx, |input, cx| input.set_value(&content, window, cx));
    Ok(ReloadOutcome::Reloaded)
}

pub struct PlainTextEditor {
    input: Entity<EditorState>,
}

impl PlainTextEditor {
    pub const fn new(input: Entity<EditorState>) -> Self {
        Self { input }
    }

    pub fn new_state(path: &Path, window: &mut Window, cx: &mut App) -> Entity<EditorState> {
        let content = std::fs::read_to_string(path).unwrap_or_default();
        cx.new(|cx| {
            EditorState::new(window, cx)
                .line_number(false)
                .folding(false)
                .searchable(true)
                .default_value(content)
        })
    }

    pub const fn input(&self) -> &Entity<EditorState> {
        &self.input
    }

    pub(crate) fn render(&self, _cx: &mut App) -> AnyElement {
        div()
            .flex_1()
            .child(Editor::new(&self.input).h_full().appearance(false))
            .into_any_element()
    }

    pub(crate) fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.focus_handle(cx)
    }
}
