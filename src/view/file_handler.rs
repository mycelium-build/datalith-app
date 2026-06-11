use gpui::*;

use super::editor::EditorKind;
use super::viewer::{MarkdownViewerEvent, ViewerKind};

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

    pub(crate) fn supports_editing(&self) -> bool {
        self.editor.is_some()
            && self
                .viewer
                .as_ref()
                .map_or(true, |v| v.supports_editing())
    }

    pub(crate) fn toggle_editing(&mut self, cx: &mut Context<Self>) {
        if !self.supports_editing() {
            return;
        }
        self.mode = match self.mode {
            ViewMode::Edit => ViewMode::View,
            ViewMode::View => ViewMode::Edit,
        };
        cx.notify();
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

    fn drain_viewer_events(&self, cx: &mut Context<Self>) {
        if let Some(ref viewer) = self.viewer {
            for event in viewer.drain_events() {
                match event {
                    MarkdownViewerEvent::LinkClicked(url, new_tab) => {
                        cx.emit(FileHandlerEvent::LinkClicked(url, new_tab));
                    }
                }
            }
        }
    }
}

impl Focusable for FileHandler {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.focus_handle(cx)
    }
}

impl Render for FileHandler {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match self.mode {
            ViewMode::Edit => {
                if let Some(ref editor) = self.editor {
                    editor.render(cx)
                } else if let Some(ref mut viewer) = self.viewer {
                    viewer.render(cx)
                } else {
                    div().flex_1().into_any_element()
                }
            }
            ViewMode::View => {
                if let Some(ref mut viewer) = self.viewer {
                    viewer.render(cx)
                } else if let Some(ref editor) = self.editor {
                    editor.render(cx)
                } else {
                    div().flex_1().into_any_element()
                }
            }
        };

        self.drain_viewer_events(cx);

        div()
            .size_full()
            .overflow_hidden()
            .child(content)
    }
}
