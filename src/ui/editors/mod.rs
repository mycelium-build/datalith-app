pub mod base;
pub mod graph;
pub mod markdown;
pub mod plain_text;
pub mod todo_txt;

use gpui::{AnyElement, App, Entity, FocusHandle};
use gpui_component::input::EditorState;

use self::base::BaseEditor;
use self::graph::GraphEditor;
use self::markdown::MarkdownEditor;
use self::plain_text::PlainTextEditor;
use self::todo_txt::TodoTxtEditor;

pub enum EditorKind {
    Base(BaseEditor),
    Graph(GraphEditor),
    Markdown(MarkdownEditor),
    PlainText(PlainTextEditor),
    TodoTxt(TodoTxtEditor),
}

impl EditorKind {
    pub fn render(&self, cx: &mut App) -> AnyElement {
        match self {
            Self::Base(editor) | Self::Graph(editor) => editor.render(cx),
            Self::Markdown(editor) => editor.render(cx),
            Self::PlainText(editor) => editor.render(cx),
            Self::TodoTxt(editor) => editor.render(cx),
        }
    }

    pub fn focus_handle(&self, cx: &App) -> FocusHandle {
        match self {
            Self::Base(editor) | Self::Graph(editor) => editor.focus_handle(cx),
            Self::Markdown(editor) => editor.focus_handle(cx),
            Self::PlainText(editor) => editor.focus_handle(cx),
            Self::TodoTxt(editor) => editor.focus_handle(cx),
        }
    }

    pub const fn input(&self) -> Option<&Entity<EditorState>> {
        match self {
            Self::Base(editor) | Self::Graph(editor) => Some(editor.input()),
            Self::Markdown(editor) => Some(editor.input()),
            Self::PlainText(editor) => Some(editor.input()),
            Self::TodoTxt(_) => None,
        }
    }

    pub const fn as_markdown(&self) -> Option<&MarkdownEditor> {
        match self {
            Self::Markdown(editor) => Some(editor),
            _ => None,
        }
    }
}
