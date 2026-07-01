pub(crate) mod markdown;
pub(crate) mod plain_text;
pub(crate) mod todo_txt;

use gpui::*;
use gpui_component::input::InputState;

use self::markdown::MarkdownEditor;
use self::plain_text::PlainTextEditor;
use self::todo_txt::TodoTxtEditor;

pub(crate) enum EditorKind {
    Markdown(MarkdownEditor),
    PlainText(PlainTextEditor),
    TodoTxt(TodoTxtEditor),
}

impl EditorKind {
    pub(crate) fn render(&self, cx: &mut App) -> AnyElement {
        match self {
            EditorKind::Markdown(editor) => editor.render(cx),
            EditorKind::PlainText(editor) => editor.render(cx),
            EditorKind::TodoTxt(editor) => editor.render(cx),
        }
    }

    pub(crate) fn focus_handle(&self, cx: &App) -> FocusHandle {
        match self {
            EditorKind::Markdown(editor) => editor.focus_handle(cx),
            EditorKind::PlainText(editor) => editor.focus_handle(cx),
            EditorKind::TodoTxt(editor) => editor.focus_handle(cx),
        }
    }

    pub(crate) fn input(&self) -> Option<&Entity<InputState>> {
        match self {
            EditorKind::Markdown(editor) => Some(editor.input()),
            EditorKind::PlainText(editor) => Some(editor.input()),
            EditorKind::TodoTxt(_) => None,
        }
    }

    pub(crate) fn as_markdown(&self) -> Option<&MarkdownEditor> {
        match self {
            EditorKind::Markdown(editor) => Some(editor),
            _ => None,
        }
    }
}
