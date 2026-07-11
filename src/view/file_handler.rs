use gpui::*;
use gpui_component::input::InputState;

use super::editors::EditorKind;
use super::viewers::ViewerKind;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum ViewMode {
    Edit,
    View,
}

pub(crate) enum FileHandlerEvent {
    LinkClicked(String, bool),
}

pub(crate) struct FileHandler {
    pub(crate) mode: ViewMode,
    pub(crate) editor: Option<EditorKind>,
    pub(crate) viewer: Option<ViewerKind>,
}

impl EventEmitter<FileHandlerEvent> for FileHandler {}

impl FileHandler {
    pub(crate) fn new(
        mode: ViewMode,
        editor: Option<EditorKind>,
        viewer: Option<ViewerKind>,
    ) -> Self {
        Self {
            mode,
            editor,
            viewer,
        }
    }

    pub(crate) fn is_editing(&self) -> bool {
        self.mode == ViewMode::Edit
    }

    pub(crate) fn can_toggle_mode(&self) -> bool {
        self.editor.is_some() && self.viewer.is_some()
    }

    pub(crate) fn toggle_editing(&mut self, cx: &mut Context<Self>) {
        if !self.can_toggle_mode() {
            return;
        }
        self.mode = match self.mode {
            ViewMode::Edit => ViewMode::View,
            ViewMode::View => ViewMode::Edit,
        };
        cx.notify();
    }

    pub(crate) fn input(&self) -> Option<&Entity<InputState>> {
        self.editor.as_ref().and_then(|e| e.input())
    }

    pub(crate) fn focus_handle(&self, cx: &App) -> FocusHandle {
        match self.mode {
            ViewMode::Edit => self
                .editor
                .as_ref()
                .map(|e| e.focus_handle(cx))
                .unwrap_or_else(|| cx.focus_handle()),
            ViewMode::View => self
                .viewer
                .as_ref()
                .map(|v| v.focus_handle(cx))
                .unwrap_or_else(|| cx.focus_handle()),
        }
    }
}

impl Focusable for FileHandler {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        // Delegates to the inherent method above (inherent methods shadow trait methods)
        self.focus_handle(cx)
    }
}

impl Render for FileHandler {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let content = match self.mode {
            ViewMode::Edit => {
                if let Some(ref editor) = self.editor {
                    editor.render(cx)
                } else if let Some(ref viewer) = self.viewer {
                    viewer.render(entity, cx)
                } else {
                    div().flex_1().into_any_element()
                }
            }
            ViewMode::View => {
                if let Some(ref viewer) = self.viewer {
                    viewer.render(entity, cx)
                } else if let Some(ref editor) = self.editor {
                    editor.render(cx)
                } else {
                    div().flex_1().into_any_element()
                }
            }
        };

        div().size_full().overflow_hidden().child(content)
    }
}
