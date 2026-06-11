use gpui::*;
use gpui_component::input::InputState;

use super::editors::markdown::MarkdownEditor;
use super::editors::plain_text::PlainTextEditor;

pub(crate) enum EditorKind {
    Markdown(MarkdownEditor),
    PlainText(PlainTextEditor),
}

impl EditorKind {
    pub(crate) fn render(&self, cx: &mut App) -> AnyElement {
        match self {
            EditorKind::Markdown(editor) => editor.render(cx),
            EditorKind::PlainText(editor) => editor.render(cx),
        }
    }

    pub(crate) fn focus_handle(&self, cx: &App) -> FocusHandle {
        match self {
            EditorKind::Markdown(editor) => editor.focus_handle(cx),
            EditorKind::PlainText(editor) => editor.focus_handle(cx),
        }
    }

    pub(crate) fn input(&self) -> Option<&Entity<InputState>> {
        match self {
            EditorKind::Markdown(editor) => Some(editor.input()),
            EditorKind::PlainText(editor) => Some(editor.input()),
        }
    }

    pub(crate) fn as_markdown(&self) -> Option<&MarkdownEditor> {
        match self {
            EditorKind::Markdown(editor) => Some(editor),
            _ => None,
        }
    }
}
