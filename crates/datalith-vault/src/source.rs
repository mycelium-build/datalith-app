//! Document access for filesystem vaults and read-only resources embedded in the binary.
//!
//! Embedded paths are logical keys in a reserved namespace, never extraction destinations.
//! Callers keep path-shaped document identities while all content access is routed here.

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::io;
use std::path::{Component, Path, PathBuf};

const EMBEDDED_NAMESPACE: &str = "datalith-embedded:";

/// An immutable resource collection. Register additional bundles alongside documentation.
#[derive(Clone, Copy)]
pub struct EmbeddedVault {
    id: &'static str,
    name: &'static str,
    files: &'static [(&'static str, &'static [u8])],
}

impl EmbeddedVault {
    /// Declares a vault from files embedded in the binary.
    ///
    /// `id` names its cache directory, `name` is shown to people and each file is a path
    /// relative to the vault root, with `/` separators, and its content.
    #[must_use]
    pub const fn new(
        id: &'static str,
        name: &'static str,
        files: &'static [(&'static str, &'static [u8])],
    ) -> Self {
        Self { id, name, files }
    }

    #[must_use]
    pub const fn id(self) -> &'static str {
        self.id
    }
    #[must_use]
    pub const fn name(self) -> &'static str {
        self.name
    }
    #[must_use]
    pub fn root(self) -> PathBuf {
        Path::new(EMBEDDED_NAMESPACE).join(self.id)
    }
    #[must_use]
    pub fn paths(self) -> Vec<PathBuf> {
        let root = self.root();
        self.files.iter().map(|(name, _)| root.join(name)).collect()
    }
    fn bytes(self, relative: &Path) -> Option<&'static [u8]> {
        let relative = normalized_relative(relative)?;
        self.files
            .iter()
            .find_map(|(name, bytes)| (Path::new(name) == relative).then_some(*bytes))
    }
}

/// The backing source of a vault; source capability does not depend on its display name.
#[derive(Clone)]
pub enum VaultSource {
    Directory(PathBuf),
    Embedded(EmbeddedVault),
}

impl VaultSource {
    ///
    /// # Errors
    /// Returns an error if the root is unavailable or the embedded vault is unknown.
    pub fn for_root(root: &Path) -> io::Result<Self> {
        if is_read_only(root) {
            let vault = embedded_vault(root)
                .filter(|vault| vault.root() == root)
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Unknown embedded vault"))?;
            Ok(Self::Embedded(vault))
        } else if root.is_dir() {
            Ok(Self::Directory(root.to_path_buf()))
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "Vault folder is unavailable",
            ))
        }
    }

    /// The vault's root path; derived on the fly for embedded vaults.
    #[must_use]
    pub fn root(&self) -> Cow<'_, Path> {
        match self {
            Self::Directory(root) => Cow::Borrowed(root),
            Self::Embedded(vault) => Cow::Owned(vault.root()),
        }
    }

    /// Lists the files in this vault.
    ///
    /// # Errors
    /// Returns an error if the root directory cannot be enumerated.
    pub fn paths(&self) -> io::Result<Vec<PathBuf>> {
        match self {
            Self::Embedded(vault) => Ok(vault.paths()),
            Self::Directory(root) => {
                let mut files = Vec::new();
                let mut pending = vec![root.clone()];
                while let Some(directory) = pending.pop() {
                    let entries = match std::fs::read_dir(&directory) {
                        Ok(entries) => entries,
                        Err(error) if directory == *root => return Err(error),
                        // Preserve discovery of accessible files in a personal vault.
                        Err(_) => continue,
                    };
                    for entry in entries.flatten() {
                        if entry.file_name() == crate::DATALITH_DIR_NAME {
                            continue;
                        }
                        let path = entry.path();
                        if path.is_dir() {
                            pending.push(path);
                        } else if path.is_file() {
                            files.push(path);
                        }
                    }
                }
                Ok(files)
            }
        }
    }
}

#[must_use]
pub fn embedded_vault(path: &Path) -> Option<EmbeddedVault> {
    crate::environment::embedded_vaults()
        .iter()
        .copied()
        .find(|vault| path.starts_with(vault.root()))
}

/// Also protects unknown resources in the reserved namespace from filesystem writes.
#[must_use]
pub fn is_read_only(path: &Path) -> bool {
    path.starts_with(EMBEDDED_NAMESPACE)
}

///
/// # Errors
/// Returns a permission error for a path in the embedded vault namespace.
pub fn ensure_writable(path: &Path) -> io::Result<()> {
    if is_read_only(path) {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "This vault is read-only",
        ))
    } else {
        Ok(())
    }
}

fn normalized_relative(path: &Path) -> Option<PathBuf> {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(name) => result.push(name),
            Component::CurDir => {}
            Component::ParentDir => {
                if !result.pop() {
                    return None;
                }
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(result)
}

#[must_use]
pub fn embedded_bytes(path: &Path) -> Option<&'static [u8]> {
    let vault = embedded_vault(path)?;
    vault.bytes(path.strip_prefix(vault.root()).ok()?)
}

///
/// # Errors
/// Returns an I/O error if the file is missing or inaccessible.
pub fn read(path: &Path) -> io::Result<Cow<'static, [u8]>> {
    if is_read_only(path) {
        embedded_bytes(path)
            .map(Cow::Borrowed)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Embedded document not found"))
    } else {
        std::fs::read(path).map(Cow::Owned)
    }
}

///
/// # Errors
/// Returns an error if the file cannot be read or decoded as UTF-8.
pub fn read_to_string(path: &Path) -> io::Result<String> {
    String::from_utf8(read(path)?.into_owned())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

#[must_use]
pub fn is_file(path: &Path) -> bool {
    if is_read_only(path) {
        embedded_bytes(path).is_some()
    } else {
        path.is_file()
    }
}

#[must_use]
pub fn is_dir(path: &Path) -> bool {
    if is_read_only(path) {
        embedded_vault(path).is_some_and(|vault| {
            path.strip_prefix(vault.root())
                .ok()
                .and_then(normalized_relative)
                .is_some_and(|relative| {
                    relative.as_os_str().is_empty()
                        || vault.files.iter().any(|(name, _)| {
                            Path::new(name).starts_with(&relative) && Path::new(name) != relative
                        })
                })
        })
    } else {
        path.is_dir()
    }
}

#[must_use]
pub fn exists(path: &Path) -> bool {
    is_file(path) || is_dir(path)
}

/// Lists immediate child identities, equally for disk directories and embedded resource folders.
///
/// # Errors
/// Returns an I/O error if the directory cannot be read.
pub fn read_dir(path: &Path) -> io::Result<Vec<PathBuf>> {
    if is_read_only(path) {
        if !is_dir(path) {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "Resource folder not found",
            ));
        }
        let vault = embedded_vault(path)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Unknown embedded vault"))?;
        let relative =
            normalized_relative(path.strip_prefix(vault.root()).map_err(io::Error::other)?)
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "Invalid resource path")
                })?;
        let children = vault
            .files
            .iter()
            .filter_map(|(name, _)| {
                let suffix = Path::new(name).strip_prefix(&relative).ok()?;
                let first = suffix.components().next()?;
                Some(path.join(first.as_os_str()))
            })
            .collect::<BTreeSet<_>>();
        Ok(children.into_iter().collect())
    } else {
        std::fs::read_dir(path)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::DOCUMENTATION;

    #[test]
    fn embedded_documents_are_read_without_materialization() {
        let root = DOCUMENTATION.root();
        assert!(!root.exists());
        assert!(is_dir(&root));
        assert!(is_dir(&root.join("formats")));
        let welcome = root.join("Welcome.md");
        assert!(read_to_string(&welcome).unwrap().contains("Datalith"));
        assert!(read_dir(&root).unwrap().contains(&welcome));
        assert!(read_dir(&root).unwrap().contains(&root.join("formats")));
        for path in DOCUMENTATION.paths() {
            assert!(is_file(&path));
            assert!(!read(&path).unwrap().is_empty());
            assert!(ensure_writable(&path).is_err());
        }
        assert!(read(&root.join("../other/Welcome.md")).is_err());
        assert!(ensure_writable(&root.join("missing.md")).is_err());
        assert!(!root.exists());
    }

    #[test]
    fn unknown_embedded_paths_are_read_only_and_absent() {
        let unknown = Path::new(EMBEDDED_NAMESPACE).join("unknown");
        assert!(is_read_only(&unknown));
        assert!(embedded_vault(&unknown).is_none());
        assert!(!exists(&unknown.join("Welcome.md")));
    }
}
