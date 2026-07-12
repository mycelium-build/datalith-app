use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FileTypeCapabilities {
    pub(crate) text_search: bool,
    pub(crate) wiki_links: bool,
}

#[derive(Clone, Default)]
pub(crate) struct RegisteredFileTypes {
    by_extension: Arc<HashMap<String, FileTypeCapabilities>>,
}

impl RegisteredFileTypes {
    #[must_use]
    pub(crate) fn new(
        registrations: impl IntoIterator<Item = (String, FileTypeCapabilities)>,
    ) -> Self {
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
    pub(crate) fn capabilities(&self, path: &Path) -> Option<FileTypeCapabilities> {
        let extension = path.extension()?.to_str()?.to_lowercase();
        self.by_extension.get(&extension).copied()
    }

    #[must_use]
    pub(crate) fn is_tracked(&self, path: &Path) -> bool {
        self.capabilities(path).is_some()
    }
}
