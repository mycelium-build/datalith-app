//! Document access for filesystem vaults and read-only resources embedded in the binary.
//!
//! Embedded paths are logical keys in a reserved namespace, never extraction destinations.
//! Callers keep path-shaped document identities while all content access is routed here.

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::io;
use std::path::{Component, Path, PathBuf};

include!(concat!(env!("OUT_DIR"), "/docs_vault.rs"));

const EMBEDDED_NAMESPACE: &str = "datalith-embedded:";

/// An immutable resource collection. Register additional bundles alongside documentation.
#[derive(Clone, Copy)]
pub struct EmbeddedVault {
    id: &'static str,
    name: &'static str,
    files: &'static [(&'static str, &'static [u8])],
}

pub const DOCUMENTATION: EmbeddedVault = EmbeddedVault {
    id: "documentation",
    name: "Datalith Documentation",
    files: DOCS_FILES,
};

const EMBEDDED_VAULTS: &[EmbeddedVault] = &[DOCUMENTATION];

impl EmbeddedVault {
    pub const fn id(self) -> &'static str {
        self.id
    }
    pub const fn name(self) -> &'static str {
        self.name
    }
    pub fn root(self) -> PathBuf {
        Path::new(EMBEDDED_NAMESPACE).join(self.id)
    }
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
                        if entry.file_name() == crate::vault::DATALITH_DIR_NAME {
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

pub fn embedded_vault(path: &Path) -> Option<EmbeddedVault> {
    EMBEDDED_VAULTS
        .iter()
        .copied()
        .find(|vault| path.starts_with(vault.root()))
}

/// Also protects unknown resources in the reserved namespace from filesystem writes.
pub fn is_read_only(path: &Path) -> bool {
    path.starts_with(EMBEDDED_NAMESPACE)
}

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

pub fn embedded_bytes(path: &Path) -> Option<&'static [u8]> {
    let vault = embedded_vault(path)?;
    vault.bytes(path.strip_prefix(vault.root()).ok()?)
}

pub fn read(path: &Path) -> io::Result<Cow<'static, [u8]>> {
    if is_read_only(path) {
        embedded_bytes(path)
            .map(Cow::Borrowed)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Embedded document not found"))
    } else {
        std::fs::read(path).map(Cow::Owned)
    }
}

pub fn read_to_string(path: &Path) -> io::Result<String> {
    String::from_utf8(read(path)?.into_owned())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

pub fn is_file(path: &Path) -> bool {
    if is_read_only(path) {
        embedded_bytes(path).is_some()
    } else {
        path.is_file()
    }
}

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

pub fn exists(path: &Path) -> bool {
    is_file(path) || is_dir(path)
}

/// Lists immediate child identities, equally for disk directories and embedded resource folders.
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

    #[test]
    fn embedded_documents_are_read_without_materialization() {
        use gpui_kit::AssetSource as _;
        let root = DOCUMENTATION.root();
        assert!(!root.exists());
        assert!(is_dir(&root));
        let welcome = root.join("Welcome.md");
        assert!(read_to_string(&welcome).unwrap().contains("Datalith"));
        assert!(read_dir(&root).unwrap().contains(&welcome));
        for path in DOCUMENTATION.paths() {
            assert!(is_file(&path));
            assert!(!read(&path).unwrap().is_empty());
            assert!(ensure_writable(&path).is_err());
            if path.extension().is_some_and(|extension| extension == "png") {
                assert_eq!(
                    crate::app::assets::DatalithAssets
                        .load(&path.to_string_lossy())
                        .unwrap()
                        .unwrap(),
                    read(&path).unwrap()
                );
            }
        }
        assert!(read(&root.join("../other/Welcome.md")).is_err());
        assert!(ensure_writable(&root.join("missing.md")).is_err());
        assert!(!root.exists());
    }
}
