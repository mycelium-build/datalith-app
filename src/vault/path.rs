use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};

use crate::vault::source;

const UNKNOWN_NAME: &str = "Unknown";

#[must_use]
pub fn display_name(path: &Path) -> &str {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(UNKNOWN_NAME)
}

/// Resolves a vault id:
/// an absolute path, or the name of a vault folder to its path,
/// matching paths first and taking the first match.
pub fn resolve_vault_id<I>(id: &str, paths: I) -> Option<PathBuf>
where
    I: IntoIterator,
    I::Item: AsRef<Path>,
{
    let mut by_name = None;
    for path in paths {
        let path = path.as_ref();
        if path.to_string_lossy() == id {
            return Some(path.to_path_buf());
        }
        if by_name.is_none()
            && path
                .file_name()
                .is_some_and(|name| name.to_string_lossy() == id)
        {
            by_name = Some(path.to_path_buf());
        }
    }
    by_name
}

/// The directory that receives a new item for `target`.
///
/// A directory target creates inside itself; a file target creates beside it.
#[must_use]
pub fn parent_dir(target: &Path) -> PathBuf {
    if source::is_dir(target) {
        target.to_path_buf()
    } else {
        target
            .parent()
            .map_or_else(|| PathBuf::from("/"), Path::to_path_buf)
    }
}

/// Normalizes `path` to an absolute path with `.` and `..` resolved.
///
/// Existing paths are canonicalized first;
/// embedded and missing paths are normalized lexically.
#[must_use]
pub fn normalized_path(path: &Path) -> PathBuf {
    let source_path = source::is_read_only(path);
    let absolute = if path.is_absolute() || source_path {
        path.to_path_buf()
    } else {
        std::env::current_dir().map_or_else(|_| path.to_path_buf(), |cwd| cwd.join(path))
    };
    let absolute = if source_path {
        absolute
    } else {
        fs::canonicalize(&absolute).unwrap_or(absolute)
    };
    normalize_components(&absolute)
}

/// Resolves `.` and `..` lexically, without touching the filesystem.
fn normalize_components(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push(component.as_os_str());
                }
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

/// Resolves a link authored relative to `source` (for example `../Note.md`)
/// against the source document's directory.
///
/// Returns `None` when the resolved path escapes `root`, so links cannot reach
/// outside the vault.
pub fn resolve_relative_link(source: &Path, target: &str, root: &Path) -> Option<PathBuf> {
    let target = target.replace('\\', "/");
    let target = Path::new(&target);
    if target.is_absolute() {
        return None;
    }
    let resolved = normalize_components(&source.parent()?.join(target));
    resolved.starts_with(root).then_some(resolved)
}

/// Portable vault-relative form of `path`, such as `notes/today.md`.
///
/// Returns `None` for the vault root itself:
/// the format never represents it with an empty string, so callers drop such entries.
pub fn path_to_vault_relative(path: &Path, vault_root: &Path) -> Result<Option<String>> {
    let relative = path
        .strip_prefix(vault_root)
        .with_context(|| format!("Vault path is outside its vault: {}", path.display()))?;
    let parts = relative
        .components()
        .map(|component| match component {
            Component::Normal(part) => Ok(part.to_string_lossy().into_owned()),
            _ => bail!("Vault path must be relative to its vault"),
        })
        .collect::<Result<Vec<_>>>()?;
    if parts.is_empty() {
        return Ok(None);
    }
    Ok(Some(parts.join("/")))
}

/// Absolute path inside `vault_root` for a portable vault-relative `path`.
///
/// Returns `None` for the empty string:
/// the format never represents the vault root, so callers drop such entries.
pub fn path_from_vault_relative(path: &str, vault_root: &Path) -> Result<Option<PathBuf>> {
    if path.is_empty() {
        return Ok(None);
    }
    let relative = Path::new(path);
    ensure!(
        relative
            .components()
            .all(|component| matches!(component, Component::Normal(_))),
        "Vault path must be relative to its vault"
    );
    Ok(Some(vault_root.join(relative)))
}

/// An unused path in `base_dir` for `name`,
/// adding a numbered suffix when `name` is already taken (`Note.md`, `Note 1.md`, ...).
#[must_use]
pub fn unique_path(base_dir: &Path, name: &str) -> PathBuf {
    let (stem, extension) = name.rfind('.').map_or((name, ""), |dot| {
        (
            name.get(..dot).unwrap_or(name),
            name.get(dot..).unwrap_or(""),
        )
    });
    let mut candidate = base_dir.join(name);
    for counter in 1usize.. {
        if !source::exists(&candidate) {
            break;
        }
        candidate = base_dir.join(format!("{stem} {counter}{extension}"));
    }
    candidate
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_dir(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "datalith-path-{label}-{:032x}",
            rand::random::<u128>()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn resolves_paths_then_names() {
        let paths = [
            PathBuf::from("/vaults/First"),
            PathBuf::from("/other/Second"),
        ];
        assert_eq!(
            resolve_vault_id("/other/Second", &paths),
            Some(PathBuf::from("/other/Second"))
        );
        assert_eq!(
            resolve_vault_id("Second", &paths),
            Some(PathBuf::from("/other/Second"))
        );
        assert_eq!(resolve_vault_id("first", &paths), None);
        assert_eq!(resolve_vault_id("/nowhere/Missing", &paths), None);
    }

    #[test]
    fn parent_dir_targets_directories_and_file_parents() {
        let root = temporary_dir("parent");
        let folder = root.join("notes");
        fs::create_dir(&folder).unwrap();
        let note = folder.join("today.md");
        fs::write(&note, "").unwrap();

        assert_eq!(parent_dir(&folder), folder);
        assert_eq!(parent_dir(&note), folder);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn normalized_path_resolves_dot_segments() {
        let root = temporary_dir("normalized");
        let folder = root.join("notes");
        fs::create_dir(&folder).unwrap();
        let note = folder.join("today.md");
        fs::write(&note, "").unwrap();

        assert_eq!(
            normalized_path(&folder.join("..").join("notes").join("today.md")),
            fs::canonicalize(&note).unwrap()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resolves_relative_links_inside_the_vault() {
        let root = Path::new("/vault");
        let source = Path::new("/vault/notes/today.md");
        assert_eq!(
            resolve_relative_link(source, "../Today.md", root),
            Some(PathBuf::from("/vault/Today.md"))
        );
        assert_eq!(
            resolve_relative_link(source, "./later/Evening.md", root),
            Some(PathBuf::from("/vault/notes/later/Evening.md"))
        );
        assert_eq!(
            resolve_relative_link(source, "later/Evening.md", root),
            Some(PathBuf::from("/vault/notes/later/Evening.md"))
        );
        assert_eq!(
            resolve_relative_link(source, "..\\Today.md", root),
            Some(PathBuf::from("/vault/Today.md"))
        );
        assert_eq!(
            resolve_relative_link(source, "../../outside.md", root),
            None
        );
        assert_eq!(resolve_relative_link(source, "/etc/passwd", root), None);
    }

    #[test]
    fn vault_relative_paths_round_trip() {
        let root = Path::new("/vault");
        let note = Path::new("/vault/notes/today.md");
        let relative = path_to_vault_relative(note, root).unwrap();
        assert_eq!(relative, Some("notes/today.md".to_string()));
        assert_eq!(
            path_from_vault_relative(relative.as_deref().unwrap(), root).unwrap(),
            Some(note.to_path_buf())
        );

        assert_eq!(path_to_vault_relative(root, root).unwrap(), None);
        assert_eq!(path_from_vault_relative("", root).unwrap(), None);
        assert!(path_to_vault_relative(Path::new("/outside/today.md"), root).is_err());
        assert!(path_from_vault_relative("../outside.md", root).is_err());
    }

    #[test]
    fn unique_path_numbering_skips_existing_names() {
        let root = temporary_dir("unique");
        fs::write(root.join("Note.md"), "").unwrap();
        fs::write(root.join("Note 1.md"), "").unwrap();

        assert_eq!(unique_path(&root, "Note.md"), root.join("Note 2.md"));
        assert_eq!(unique_path(&root, "New Folder"), root.join("New Folder"));
        fs::remove_dir_all(root).unwrap();
    }
}
