use std::path::Path;

use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, ParentElement, Render,
    Styled, Window, div,
};
use gpui_component::input::InputState;

use crate::ui::editors::EditorKind;
use crate::ui::viewers::ViewerKind;
use crate::vault::VaultCatalog;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReloadOutcome {
    Reloaded,
    Unchanged,
    Unsupported,
}

pub type ReloadAdapter = fn(
    &Path,
    &mut FileHandler,
    &mut Window,
    &mut Context<FileHandler>,
) -> anyhow::Result<ReloadOutcome>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewMode {
    Edit,
    View,
}

pub enum FileHandlerEvent {
    LinkClicked(String, bool),
}

pub struct FileHandler {
    pub(crate) mode: ViewMode,
    pub(crate) editor: Option<EditorKind>,
    pub(crate) viewer: Option<ViewerKind>,
    reload_adapter: Option<ReloadAdapter>,
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
            reload_adapter: None,
        }
    }

    pub(crate) fn with_reload_adapter(mut self, reload_adapter: Option<ReloadAdapter>) -> Self {
        self.reload_adapter = reload_adapter;
        self
    }

    pub(crate) fn reload_from_disk(
        &mut self,
        path: &Path,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<ReloadOutcome> {
        let Some(reload) = self.reload_adapter else {
            return Ok(ReloadOutcome::Unsupported);
        };
        let outcome = reload(path, self, window, cx)?;
        if outcome == ReloadOutcome::Reloaded {
            cx.notify();
        }
        Ok(outcome)
    }

    pub(crate) fn is_editing(&self) -> bool {
        self.mode == ViewMode::Edit
    }

    pub(crate) const fn can_toggle_mode(&self) -> bool {
        self.editor.is_some() && self.viewer.is_some()
    }

    pub(crate) fn toggle_editing(&mut self, cx: &mut Context<Self>) {
        if !self.can_toggle_mode() {
            return;
        }
        self.mode = match self.mode {
            ViewMode::Edit => {
                if let Some(viewer) = &self.viewer {
                    viewer.refresh(cx);
                }
                ViewMode::View
            }
            ViewMode::View => ViewMode::Edit,
        };
        cx.notify();
    }

    pub(crate) fn set_vault_catalog(&self, catalog: VaultCatalog, cx: &mut Context<Self>) {
        if let Some(viewer) = &self.viewer {
            viewer.set_vault_catalog(catalog, cx);
        }
    }

    pub(crate) fn input(&self) -> Option<&Entity<InputState>> {
        self.editor.as_ref().and_then(|e| e.input())
    }

    pub(crate) fn focus_handle(&self, cx: &App) -> FocusHandle {
        match self.mode {
            ViewMode::Edit => self
                .editor
                .as_ref()
                .map_or_else(|| cx.focus_handle(), |e| e.focus_handle(cx)),
            ViewMode::View => self
                .viewer
                .as_ref()
                .map_or_else(|| cx.focus_handle(), |v| v.focus_handle(cx)),
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
