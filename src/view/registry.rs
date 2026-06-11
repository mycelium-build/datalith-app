use std::collections::HashMap;
use std::path::Path;

use gpui::*;
use gpui_component::input::InputState;

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

pub(crate) type EditorFactory =
    fn(&Path, &Entity<InputState>, &mut Context<FileHandler>) -> EditorKind;

pub(crate) type ViewerFactory =
    fn(&Path, &Entity<InputState>, &mut Context<FileHandler>) -> ViewerKind;

pub(crate) struct FileRegistry {
    configs: HashMap<String, FileTypeConfig>,
    fallback: FileTypeConfig,
}

impl FileRegistry {
    pub(crate) fn new() -> Self {
        Self {
            configs: HashMap::new(),
            fallback: FileTypeConfig {
                editor_factory: Some(|_path, state, _cx| {
                    EditorKind::PlainText(PlainTextEditor::new(state.clone()))
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

    pub(crate) fn create_input_state(
        &self,
        path: &Path,
        content: String,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<InputState> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            if ext == "md" {
                state = state
                    .code_editor("markdown")
                    .line_number(false)
                    .folding(false);
            } else {
                state = state.multi_line(true).searchable(true);
            }
            state.default_value(content)
        })
    }

    pub(crate) fn create_handler(
        &self,
        path: &Path,
        state: &Entity<InputState>,
        cx: &mut Context<FileHandler>,
    ) -> FileHandler {
        let config = self.config_for(path);
        let editor = config
            .editor_factory
            .map(|factory| factory(path, state, cx));
        let viewer = config
            .viewer_factory
            .map(|factory| factory(path, state, cx));
        FileHandler::new(config.default_mode, editor, viewer)
    }
}

pub(crate) fn default_registry() -> FileRegistry {
    let mut registry = FileRegistry::new();

    // Markdown: editor + viewer
    registry.register(
        "md",
        FileTypeConfig {
            editor_factory: Some(|path, state, _cx| {
                EditorKind::Markdown(MarkdownEditor::new(state.clone(), path.to_path_buf()))
            }),
            viewer_factory: Some(|path, state, _cx| {
                ViewerKind::Markdown(MarkdownViewer::new(state.clone(), path.to_path_buf()))
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
                viewer_factory: Some(|path, _state, _cx| {
                    ViewerKind::Image(ImageViewer::new(path.to_path_buf()))
                }),
                default_mode: ViewMode::View,
            },
        );
    }

    registry
}
