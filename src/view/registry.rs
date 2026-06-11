use std::collections::HashMap;
use std::path::Path;

use gpui::*;

use super::editor::EditorKind;
use super::editors::markdown::MarkdownEditor;
use super::editors::plain_text::PlainTextEditor;
use super::file_handler::{FileHandler, ViewMode};
use super::viewer::ViewerKind;
use super::viewers::image::ImageViewer;
use super::viewers::markdown::MarkdownViewer;

pub(crate) struct FileTypeConfig {
    pub(crate) editor_factory: Option<EditorFactory>,
    pub(crate) viewer_factory: Option<ViewerFactory>,
    pub(crate) default_mode: ViewMode,
}

pub(crate) type EditorFactory = fn(&Path, &mut Window, &mut Context<FileHandler>) -> EditorKind;

pub(crate) type ViewerFactory =
    fn(&Path, Option<&EditorKind>, &mut Context<FileHandler>) -> Option<ViewerKind>;

pub(crate) struct FileRegistry {
    configs: HashMap<String, FileTypeConfig>,
    fallback: FileTypeConfig,
}

impl FileRegistry {
    pub(crate) fn new() -> Self {
        Self {
            configs: HashMap::new(),
            fallback: FileTypeConfig {
                editor_factory: Some(|path, window, cx| {
                    EditorKind::PlainText(PlainTextEditor::new(PlainTextEditor::new_state(
                        path, window, cx,
                    )))
                }),
                viewer_factory: None,
                default_mode: ViewMode::Edit,
            },
        }
    }

    pub(crate) fn register(&mut self, extension: &str, config: FileTypeConfig) {
        self.configs.insert(extension.to_lowercase(), config);
    }

    pub(crate) fn config_for(&self, path: &Path) -> &FileTypeConfig {
        path.extension()
            .and_then(|e| e.to_str())
            .and_then(|ext| self.configs.get(&ext.to_lowercase()))
            .unwrap_or(&self.fallback)
    }

    pub(crate) fn is_supported(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|ext| self.configs.contains_key(&ext.to_lowercase()))
            .unwrap_or(false)
    }

    pub(crate) fn create_handler(
        &self,
        path: &Path,
        window: &mut Window,
        cx: &mut Context<FileHandler>,
    ) -> FileHandler {
        let config = self.config_for(path);
        let editor = config
            .editor_factory
            .map(|factory| factory(path, window, cx));
        let viewer = config
            .viewer_factory
            .and_then(|factory| factory(path, editor.as_ref(), cx));
        FileHandler::new(config.default_mode, editor, viewer)
    }
}

pub(crate) fn default_registry() -> FileRegistry {
    let mut registry = FileRegistry::new();

    // Markdown: editor + viewer
    registry.register(
        "md",
        FileTypeConfig {
            editor_factory: Some(|path, window, cx| {
                EditorKind::Markdown(MarkdownEditor::new(
                    MarkdownEditor::new_state(path, window, cx),
                ))
            }),
            viewer_factory: Some(|path, editor, _cx| {
                let editor = editor?;
                let state = editor.input()?.clone();
                Some(ViewerKind::Markdown(MarkdownViewer::new(
                    state,
                    path.to_path_buf(),
                )))
            }),
            default_mode: ViewMode::Edit,
        },
    );

    // Images: viewer only
    for ext in &["png", "jpg", "jpeg", "gif", "webp", "bmp", "tiff", "avif"] {
        registry.register(
            ext,
            FileTypeConfig {
                editor_factory: None,
                viewer_factory: Some(|path, _editor, _cx| {
                    Some(ViewerKind::Image(ImageViewer::new(path.to_path_buf())))
                }),
                default_mode: ViewMode::View,
            },
        );
    }

    registry
}
