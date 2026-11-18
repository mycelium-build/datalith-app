use std::collections::HashMap;
use std::path::Path;

use gpui_kit::{Context, Window};

use datalith_core::file_types::FileKind;

use super::handler::{FileHandler, ReloadAdapter, ViewMode};
use crate::editors::EditorKind;
use crate::editors::base::BaseEditor;
use crate::editors::markdown::MarkdownEditor;
use crate::editors::plain_text::{PlainTextEditor, reload_text};
use crate::editors::todo_txt::{TodoTxtEditor, reload_todo_txt};
use crate::icons::DatalithIcon;
use crate::viewers::ViewerKind;
use crate::viewers::base::BaseViewer;
use crate::viewers::image::ImageViewer;
use crate::viewers::markdown::MarkdownViewer;
use datalith_vault::VaultCatalog;

pub struct FileTypeConfig {
    pub icon: DatalithIcon,
    pub(crate) editor_factory: Option<EditorFactory>,
    pub(crate) viewer_factory: Option<ViewerFactory>,
    pub(crate) reload_adapter: Option<ReloadAdapter>,
    pub(crate) default_mode: ViewMode,
}

pub type EditorFactory = fn(&Path, &mut Window, &mut Context<FileHandler>) -> EditorKind;

pub struct ViewerDependencies {
    vault_catalog: Option<VaultCatalog>,
}

impl ViewerDependencies {
    #[must_use]
    pub const fn new(vault_catalog: Option<VaultCatalog>) -> Self {
        Self { vault_catalog }
    }
}

/// Builds the viewer for a file type, if any.
///
/// `editor` is the already-built editor for the same file.
/// Viewers that render text (markdown, base) read `editor.input()`, the shared editor buffer,
/// so view mode shows exactly what the editor holds without re-reading the file.
/// It is `None` for editor-less types.
pub type ViewerFactory = fn(
    &Path,
    Option<&EditorKind>,
    &ViewerDependencies,
    &mut Context<FileHandler>,
) -> Option<ViewerKind>;

pub struct FileRegistry {
    configs: HashMap<FileKind, FileTypeConfig>,
    fallback: FileTypeConfig,
}

impl FileRegistry {
    pub(crate) fn new() -> Self {
        Self {
            configs: HashMap::new(),
            fallback: FileTypeConfig {
                icon: DatalithIcon::File,
                editor_factory: Some(|path, window, cx| {
                    EditorKind::PlainText(PlainTextEditor::new(
                        PlainTextEditor::new_state(path, window, cx),
                        datalith_vault::source::is_read_only(path),
                    ))
                }),
                viewer_factory: None,
                reload_adapter: None,
                default_mode: ViewMode::Edit,
            },
        }
    }

    fn register(&mut self, kind: FileKind, config: FileTypeConfig) {
        self.configs.insert(kind, config);
    }

    pub fn config_for(&self, path: Option<&Path>) -> &FileTypeConfig {
        path.and_then(FileKind::for_path)
            .and_then(|kind| self.configs.get(&kind))
            .unwrap_or(&self.fallback)
    }

    #[must_use]
    pub fn is_supported(&self, path: &Path) -> bool {
        FileKind::for_path(path).is_some()
    }

    pub fn create_handler(
        &self,
        path: &Path,
        dependencies: &ViewerDependencies,
        window: &mut Window,
        cx: &mut Context<FileHandler>,
    ) -> FileHandler {
        let config = self.config_for(Some(path));
        let read_only = datalith_vault::source::is_read_only(path);
        let mut editor = config
            .editor_factory
            .map(|factory| factory(path, window, cx));
        if let (Some(catalog), Some(editor)) = (dependencies.vault_catalog.clone(), editor.as_mut())
        {
            editor.set_vault_catalog(catalog, cx);
        }
        let viewer = config
            .viewer_factory
            .and_then(|factory| factory(path, editor.as_ref(), dependencies, cx));
        FileHandler::new(config.default_mode, editor, viewer)
            .with_read_only(read_only)
            .with_reload_adapter(config.reload_adapter)
    }
}

#[allow(clippy::too_many_lines)]
pub fn default_registry() -> FileRegistry {
    let mut registry = FileRegistry::new();

    // Base Definition: YAML editor + derived list/table/cards/graph views
    registry.register(
        FileKind::Base,
        FileTypeConfig {
            icon: DatalithIcon::Base,
            editor_factory: Some(|path, window, cx| {
                EditorKind::Base(BaseEditor::new(
                    BaseEditor::new_state(path, window, cx),
                    datalith_vault::source::is_read_only(path),
                ))
            }),
            viewer_factory: Some(|_path, editor, dependencies, cx| {
                let input = editor?.input()?.clone();
                Some(ViewerKind::Base(BaseViewer::new(
                    input,
                    dependencies.vault_catalog.clone(),
                    cx,
                )))
            }),
            reload_adapter: Some(reload_text),
            default_mode: ViewMode::View,
        },
    );

    // Markdown: editor + viewer
    registry.register(
        FileKind::Markdown,
        FileTypeConfig {
            icon: DatalithIcon::Note,
            editor_factory: Some(|path, window, cx| {
                EditorKind::Markdown(MarkdownEditor::new(
                    MarkdownEditor::new_state(path, window, cx),
                    path,
                    datalith_vault::source::is_read_only(path),
                ))
            }),
            viewer_factory: Some(|path, editor, _dependencies, cx| {
                let editor = editor?;
                let state = editor.input()?.clone();
                Some(ViewerKind::Markdown(
                    MarkdownViewer::new(state, path.to_path_buf()).with_keyboard(cx),
                ))
            }),
            reload_adapter: Some(reload_text),
            default_mode: ViewMode::Edit,
        },
    );

    // Images: viewer only
    registry.register(
        FileKind::Image,
        FileTypeConfig {
            icon: DatalithIcon::Image,
            editor_factory: None,
            viewer_factory: Some(|path, _editor, _dependencies, cx| {
                Some(ViewerKind::Image(ImageViewer::new(path.to_path_buf(), cx)))
            }),
            reload_adapter: None,
            default_mode: ViewMode::View,
        },
    );

    // Todo.txt: editor only
    registry.register(
        FileKind::TodoTxt,
        FileTypeConfig {
            icon: DatalithIcon::Todo,
            editor_factory: Some(|path, window, cx| {
                EditorKind::TodoTxt(TodoTxtEditor::new(TodoTxtEditor::new_state(
                    path, window, cx,
                )))
            }),
            viewer_factory: None,
            reload_adapter: Some(reload_todo_txt),
            default_mode: ViewMode::Edit,
        },
    );

    registry
}
