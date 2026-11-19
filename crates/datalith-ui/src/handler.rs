use std::path::Path;

use gpui_kit::component::input::EditorState;
use gpui_kit::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement as _,
    IntoElement, ParentElement, Render, Styled, Window, div, prelude::FluentBuilder as _,
};
use serde::{Deserialize, Serialize};

use crate::contexts::{BASE_CONTEXT, NOTE_CONTEXT, TODO_LIST_CONTEXT};
use crate::editors::EditorKind;
use crate::viewers::ViewerKind;
use datalith_vault::VaultCatalog;

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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewMode {
    Edit,
    #[default]
    View,
}

pub enum FileHandlerEvent {
    LinkClicked(String, bool),
    VaultFileRequested(std::path::PathBuf, bool),
}

pub struct FileHandler {
    pub(crate) mode: ViewMode,
    pub(crate) editor: Option<EditorKind>,
    pub(crate) viewer: Option<ViewerKind>,
    read_only: bool,
    reload_adapter: Option<ReloadAdapter>,
}

impl EventEmitter<FileHandlerEvent> for FileHandler {}

impl FileHandler {
    pub fn markdown_link_at_cursor(&self, cx: &App) -> Option<String> {
        let input = self.editor.as_ref()?.as_markdown()?.input().read(cx);
        datalith_core::markdown::find_link_at_offset(&input.value(), input.cursor())
    }

    #[must_use]
    pub fn new(mode: ViewMode, editor: Option<EditorKind>, viewer: Option<ViewerKind>) -> Self {
        Self {
            mode,
            editor,
            viewer,
            read_only: false,
            reload_adapter: None,
        }
    }

    #[must_use]
    pub const fn with_read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        if read_only {
            self.mode = ViewMode::View;
        }
        self
    }

    pub(crate) fn with_reload_adapter(mut self, reload_adapter: Option<ReloadAdapter>) -> Self {
        self.reload_adapter = reload_adapter;
        self
    }

    /// Reloads the open document using its registered adapter.
    ///
    /// # Errors
    /// Returns an error if the adapter cannot read or parse the updated file.
    pub fn reload_from_disk(
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
            if let Some(viewer) = &self.viewer {
                viewer.refresh(cx);
            }
            cx.notify();
        }
        Ok(outcome)
    }

    pub fn dismiss_view_control(&self, cx: &mut App) -> bool {
        self.mode == ViewMode::View
            && matches!(&self.viewer, Some(ViewerKind::Base(base)) if base.dismiss_control(cx))
    }

    #[must_use]
    pub fn is_editing(&self) -> bool {
        self.mode == ViewMode::Edit
    }

    #[must_use]
    pub const fn mode(&self) -> ViewMode {
        self.mode
    }

    #[must_use]
    pub const fn can_toggle_mode(&self) -> bool {
        !self.read_only && self.editor.is_some() && self.viewer.is_some()
    }

    #[must_use]
    pub const fn is_read_only(&self) -> bool {
        self.read_only
    }

    pub fn set_mode(&mut self, mode: ViewMode, cx: &mut Context<Self>) {
        let mode = if self.read_only {
            ViewMode::View
        } else {
            match (self.editor.is_some(), self.viewer.is_some()) {
                (true, true) | (false, false) => mode,
                (true, false) => ViewMode::Edit,
                (false, true) => ViewMode::View,
            }
        };
        if self.mode == mode {
            return;
        }
        if mode == ViewMode::View
            && let Some(viewer) = &self.viewer
        {
            viewer.refresh(cx);
        }
        self.mode = mode;
        cx.notify();
    }

    pub fn set_vault_catalog(&mut self, catalog: VaultCatalog, cx: &mut Context<Self>) {
        if let Some(editor) = &mut self.editor {
            editor.set_vault_catalog(catalog.clone(), cx);
        }
        if let Some(viewer) = &self.viewer {
            viewer.set_vault_catalog(catalog, cx);
        }
    }

    /// The shortcut zone of this document, in both modes.
    const fn key_context(&self) -> Option<&'static str> {
        match (&self.editor, &self.viewer) {
            (Some(EditorKind::TodoTxt(_)), _) => Some(TODO_LIST_CONTEXT),
            (Some(EditorKind::Base(_)), _) | (_, Some(ViewerKind::Base(_))) => Some(BASE_CONTEXT),
            (Some(EditorKind::Markdown(_) | EditorKind::PlainText(_)), _)
            | (_, Some(ViewerKind::Markdown(_))) => Some(NOTE_CONTEXT),
            _ => None,
        }
    }

    #[must_use]
    pub fn input(&self) -> Option<&Entity<EditorState>> {
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
                .map(|viewer| viewer.focus_handle(cx))
                .or_else(|| self.editor.as_ref().map(|editor| editor.focus_handle(cx)))
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

        div()
            .when_some(self.key_context(), |element, context| {
                element.key_context(context)
            })
            .size_full()
            .min_h_0()
            .overflow_hidden()
            .child(content)
    }
}
