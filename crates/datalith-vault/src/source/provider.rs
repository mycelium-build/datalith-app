//! Document providers registered by the native host. Logical paths are scoped
//! to one grant and never passed to the OS as filesystem paths.
use std::{
    collections::HashMap,
    io,
    path::{Component, Path, PathBuf},
    sync::{Arc, LazyLock, RwLock},
};

const NAMESPACE: &str = "datalith-provider:";
type Providers = HashMap<PathBuf, Arc<dyn DocumentProvider>>;
static PROVIDERS: LazyLock<RwLock<Providers>> = LazyLock::new(RwLock::default);

/// File attributes used by the catalog. Zero timestamps represent missing dates.
#[derive(Clone, Copy, Default)]
pub struct Attributes {
    pub size: i64,
    pub modified_ns: i64,
    pub created_ns: i64,
}
impl From<&std::fs::Metadata> for Attributes {
    fn from(meta: &std::fs::Metadata) -> Self {
        let nanos = |time: io::Result<std::time::SystemTime>| {
            time.ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| i64::try_from(d.as_nanos()).unwrap_or(i64::MAX))
        };
        Self {
            size: i64::try_from(meta.len()).unwrap_or(i64::MAX),
            modified_ns: nanos(meta.modified()),
            created_ns: nanos(meta.created()),
        }
    }
}

/// Operations on names relative to a user-selected folder.
pub trait DocumentProvider: Send + Sync {
    ///
    /// # Errors
    /// Returns an error when the folder cannot be enumerated.
    fn list(&self, directory: &Path) -> io::Result<Vec<String>>;
    ///
    /// # Errors
    /// Returns an error when the item cannot be queried.
    fn is_directory(&self, path: &Path) -> io::Result<bool>;
    ///
    /// # Errors
    /// Returns an error when the document cannot be read.
    fn read(&self, path: &Path) -> io::Result<Vec<u8>>;
    /// Reads available size and dates without downloading the document.
    ///
    /// # Errors
    /// Returns the provider error when metadata is unavailable.
    fn attributes(&self, path: &Path) -> io::Result<Attributes>;
    /// Refuse to replace a concurrently changed document. A missing expected
    /// value requests exclusive creation.
    ///
    /// # Errors
    /// Returns an error when the expected content differs or writing fails.
    fn save(&self, path: &Path, expected: Option<&[u8]>, content: &[u8]) -> io::Result<()>;
    ///
    /// # Errors
    /// Returns an error when the folder exists or creation fails.
    fn mkdir(&self, path: &Path) -> io::Result<()>;
    ///
    /// # Errors
    /// Returns an error when the destination exists or the provider refuses the move.
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;
}

pub struct Mount {
    root: PathBuf,
    provider: Arc<dyn DocumentProvider>,
}
impl Mount {
    /// Registers the grant under a stable, app-private identifier.
    ///
    /// # Errors
    /// Returns an error for invalid identifiers.
    pub fn new(id: &str, provider: Arc<dyn DocumentProvider>) -> io::Result<Self> {
        if id.is_empty() || !id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-') {
            return Err(io::Error::other("Invalid provider identifier"));
        }
        let root = Path::new(NAMESPACE).join(id);
        PROVIDERS
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(root.clone(), provider.clone());
        Ok(Self { root, provider })
    }
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }
}
impl Drop for Mount {
    fn drop(&mut self) {
        let mut providers = PROVIDERS
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if providers
            .get(&self.root)
            .is_some_and(|p| Arc::ptr_eq(p, &self.provider))
        {
            providers.remove(&self.root);
        }
    }
}

#[must_use]
pub fn is_provider(path: &Path) -> bool {
    path.starts_with(NAMESPACE)
}

pub(crate) fn resolve(path: &Path) -> io::Result<(Arc<dyn DocumentProvider>, &Path)> {
    let mut components = path
        .strip_prefix(NAMESPACE)
        .map_err(io::Error::other)?
        .components();
    let Some(Component::Normal(id)) = components.next() else {
        return Err(io::Error::other("Missing folder grant"));
    };
    let relative = components.as_path();
    if !relative
        .components()
        .all(|c| matches!(c, Component::Normal(_)))
    {
        return Err(io::Error::other("Invalid document path"));
    }
    let provider = PROVIDERS
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&Path::new(NAMESPACE).join(id))
        .cloned()
        .ok_or_else(|| io::Error::other("Folder access unavailable"))?;
    Ok((provider, relative))
}
