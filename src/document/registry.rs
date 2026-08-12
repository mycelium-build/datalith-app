use std::collections::HashMap;
use std::path::Path;

use gpui::{Context, Window};

use crate::document::file_types::{FileTypeCapabilities, RegisteredFileTypes};

use super::handler::{FileHandler, ReloadAdapter, ViewMode};
use crate::ui::editors::EditorKind;
use crate::ui::editors::graph::GraphEditor;
use crate::ui::editors::markdown::MarkdownEditor;
use crate::ui::editors::plain_text::{PlainTextEditor, reload_text};
use crate::ui::editors::todo_txt::{TodoTxtEditor, reload_todo_txt};
use crate::ui::icons::DatalithIcon;
use crate::ui::viewers::ViewerKind;
use crate::ui::viewers::graph::GraphViewer;
use crate::ui::viewers::image::ImageViewer;
use crate::ui::viewers::markdown::MarkdownViewer;
use crate::vault::VaultCatalog;

pub struct FileTypeConfig {
    pub(crate) capabilities: FileTypeCapabilities,
    pub(crate) icon: DatalithIcon,
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
    pub(crate) const fn new(vault_catalog: Option<VaultCatalog>) -> Self {
        Self { vault_catalog }
    }
}

pub type ViewerFactory = fn(
    &Path,
    Option<&EditorKind>,
    &ViewerDependencies,
    &mut Context<FileHandler>,
) -> Option<ViewerKind>;

pub struct FileRegistry {
    configs: HashMap<String, FileTypeConfig>,
    fallback: FileTypeConfig,
}

impl FileRegistry {
    pub(crate) fn new() -> Self {
        Self {
            configs: HashMap::new(),
            fallback: FileTypeConfig {
                capabilities: FileTypeCapabilities {
                    text_search: false,
                    wiki_links: false,
                    yaml_frontmatter: false,
                },
                icon: DatalithIcon::File,
                editor_factory: Some(|path, window, cx| {
                    EditorKind::PlainText(PlainTextEditor::new(PlainTextEditor::new_state(
                        path, window, cx,
                    )))
                }),
                viewer_factory: None,
                reload_adapter: None,
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
            .is_some_and(|ext| self.configs.contains_key(&ext.to_lowercase()))
    }

    #[must_use]
    pub(crate) fn registered_file_types(&self) -> RegisteredFileTypes {
        RegisteredFileTypes::new(
            self.configs
                .iter()
                .map(|(extension, config)| (extension.clone(), config.capabilities)),
        )
    }

    pub(crate) fn create_handler(
        &self,
        path: &Path,
        dependencies: &ViewerDependencies,
        window: &mut Window,
        cx: &mut Context<FileHandler>,
    ) -> FileHandler {
        let config = self.config_for(path);
        let editor = config
            .editor_factory
            .map(|factory| factory(path, window, cx));
        let viewer = config
            .viewer_factory
            .and_then(|factory| factory(path, editor.as_ref(), dependencies, cx));
        FileHandler::new(config.default_mode, editor, viewer)
            .with_reload_adapter(config.reload_adapter)
    }
}

pub fn default_registry() -> FileRegistry {
    let mut registry = FileRegistry::new();

    // Graph Definition: YAML editor + derived Graph View
    registry.register(
        "graph",
        FileTypeConfig {
            capabilities: FileTypeCapabilities {
                text_search: false,
                wiki_links: false,
                yaml_frontmatter: false,
            },
            icon: DatalithIcon::Graph,
            editor_factory: Some(|path, window, cx| {
                EditorKind::Graph(GraphEditor::new(GraphEditor::new_state(path, window, cx)))
            }),
            viewer_factory: Some(|_path, editor, dependencies, cx| {
                let input = editor?.input()?.clone();
                Some(ViewerKind::Graph(GraphViewer::new(
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
        "md",
        FileTypeConfig {
            capabilities: FileTypeCapabilities {
                text_search: true,
                wiki_links: true,
                yaml_frontmatter: true,
            },
            icon: DatalithIcon::Note,
            editor_factory: Some(|path, window, cx| {
                EditorKind::Markdown(MarkdownEditor::new(MarkdownEditor::new_state(
                    path, window, cx,
                )))
            }),
            viewer_factory: Some(|path, editor, _dependencies, _cx| {
                let editor = editor?;
                let state = editor.input()?.clone();
                Some(ViewerKind::Markdown(MarkdownViewer::new(
                    state,
                    path.to_path_buf(),
                )))
            }),
            reload_adapter: Some(reload_text),
            default_mode: ViewMode::Edit,
        },
    );

    // Images: viewer only
    for ext in &["png", "jpg", "jpeg", "gif", "webp", "bmp", "tiff", "avif"] {
        registry.register(
            ext,
            FileTypeConfig {
                capabilities: FileTypeCapabilities {
                    text_search: false,
                    wiki_links: false,
                    yaml_frontmatter: false,
                },
                icon: DatalithIcon::Image,
                editor_factory: None,
                viewer_factory: Some(|path, _editor, _dependencies, _cx| {
                    Some(ViewerKind::Image(ImageViewer::new(path.to_path_buf())))
                }),
                reload_adapter: None,
                default_mode: ViewMode::View,
            },
        );
    }

    // Todo.txt: editor only
    registry.register(
        "todotxt",
        FileTypeConfig {
            capabilities: FileTypeCapabilities {
                text_search: true,
                wiki_links: false,
                yaml_frontmatter: false,
            },
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
