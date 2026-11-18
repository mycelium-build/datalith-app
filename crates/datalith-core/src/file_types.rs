use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileTypeCapabilities {
    pub text_search: bool,
    pub wiki_links: bool,
    pub yaml_frontmatter: bool,
}

#[derive(Clone, Default)]
pub struct RegisteredFileTypes {
    by_extension: Arc<HashMap<String, FileTypeCapabilities>>,
}

impl RegisteredFileTypes {
    #[must_use]
    pub fn new(registrations: impl IntoIterator<Item = (String, FileTypeCapabilities)>) -> Self {
        Self {
            by_extension: Arc::new(
                registrations
                    .into_iter()
                    .map(|(extension, capabilities)| (extension.to_lowercase(), capabilities))
                    .collect(),
            ),
        }
    }

    #[must_use]
    pub fn capabilities(&self, path: &Path) -> Option<FileTypeCapabilities> {
        let extension = path.extension()?.to_str()?.to_lowercase();
        self.by_extension.get(&extension).copied()
    }

    #[must_use]
    pub fn is_tracked(&self, path: &Path) -> bool {
        self.capabilities(path).is_some()
    }
}

/// A format recognized by the catalog and the document UI.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FileKind {
    Markdown,
    Base,
    Image,
    TodoTxt,
}

const FORMATS: &[(FileKind, &[&str])] = &[
    (FileKind::Markdown, &["md"]),
    (FileKind::Base, &["base"]),
    (
        FileKind::Image,
        &["png", "jpg", "jpeg", "gif", "webp", "bmp", "tiff", "avif"],
    ),
    (FileKind::TodoTxt, &["todotxt"]),
];

impl FileKind {
    #[must_use]
    pub fn for_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?;
        FORMATS
            .iter()
            .find(|(_, extensions)| {
                extensions
                    .iter()
                    .any(|candidate| ext.eq_ignore_ascii_case(candidate))
            })
            .map(|(kind, _)| *kind)
    }

    const fn capabilities(self) -> FileTypeCapabilities {
        FileTypeCapabilities {
            text_search: matches!(self, Self::Markdown | Self::TodoTxt),
            wiki_links: matches!(self, Self::Markdown),
            yaml_frontmatter: matches!(self, Self::Markdown),
        }
    }
}

/// The file types tracked by Datalith's catalog and search index.
#[must_use]
pub fn registered_file_types() -> RegisteredFileTypes {
    RegisteredFileTypes::new(FORMATS.iter().flat_map(|(kind, extensions)| {
        extensions
            .iter()
            .map(move |extension| (String::from(*extension), kind.capabilities()))
    }))
}
