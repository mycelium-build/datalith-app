use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::vault::VaultCatalog;
use crate::vault::links;

#[must_use]
pub(crate) fn parent_dir(target: &Path) -> PathBuf {
    if target.is_dir() {
        target.to_path_buf()
    } else {
        target
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("/"))
    }
}

#[must_use]
pub(crate) fn unique_name(base_dir: &Path, name: &str) -> PathBuf {
    let (stem, ext) = if let Some(dot) = name.rfind('.') {
        (&name[..dot], &name[dot..])
    } else {
        (name, "")
    };
    let mut candidate = base_dir.join(name);
    let mut counter = 1;
    while candidate.exists() {
        candidate = base_dir.join(format!("{stem} {counter}{ext}"));
        counter += 1;
    }
    candidate
}

pub(crate) fn create(target: &Path) -> Result<PathBuf> {
    let directory = parent_dir(target);
    let path = unique_name(&directory, "New Note.md");
    fs::write(&path, "").with_context(|| format!("Failed to create file {}", path.display()))?;
    Ok(path)
}

pub(crate) fn create_folder(target: &Path) -> Result<PathBuf> {
    let directory = parent_dir(target);
    let path = unique_name(&directory, "New Folder");
    fs::create_dir(&path).with_context(|| format!("Failed to create folder {}", path.display()))?;
    Ok(path)
}

pub(crate) fn update(path: &Path, content: &str) -> Result<()> {
    fs::write(path, content).with_context(|| format!("Failed to update {}", path.display()))
}

pub(crate) struct RenameResult {
    pub(crate) total_links: usize,
    pub(crate) updated_links: usize,
}

pub(crate) fn rename(
    catalog: &VaultCatalog,
    old_path: &Path,
    new_path: &Path,
) -> Result<RenameResult> {
    let root = catalog.root();
    let mut by_source: BTreeMap<PathBuf, Vec<(usize, String)>> = BTreeMap::new();
    for backlink in catalog.backlinks_under(old_path)? {
        let suffix = backlink
            .target_path
            .strip_prefix(old_path)
            .unwrap_or_else(|_| Path::new(""));
        let renamed_target = new_path.join(suffix);
        let replacement = replacement_target(&backlink.authored_target, &renamed_target, &root);
        by_source
            .entry(backlink.source)
            .or_default()
            .push((backlink.ordinal, replacement));
    }

    let total_links = by_source.len();

    fs::rename(old_path, new_path).with_context(|| {
        format!(
            "Failed to rename {} to {}",
            old_path.display(),
            new_path.display()
        )
    })?;

    let mut updated_links = 0usize;
    for (source_path, replacements) in by_source {
        let source_path = source_path
            .strip_prefix(old_path)
            .map_or_else(|_| source_path.clone(), |suffix| new_path.join(suffix));
        let content = match fs::read_to_string(&source_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let rewritten = links::rewrite(&content, &replacements);
        if rewritten != content {
            if update(&source_path, &rewritten).is_ok() {
                updated_links += 1;
            }
        } else {
            updated_links += 1;
        }
    }
    Ok(RenameResult {
        total_links,
        updated_links,
    })
}

pub(crate) fn delete(target: &Path) -> Result<()> {
    if target.is_dir() {
        fs::remove_dir_all(target)
            .with_context(|| format!("Failed to delete directory {:?}", target))?;
    } else {
        fs::remove_file(target).with_context(|| format!("Failed to delete file {:?}", target))?;
    }
    Ok(())
}

pub(crate) fn duplicate(target: &Path) -> Result<PathBuf> {
    if target.is_dir() {
        let parent = target.parent().unwrap_or_else(|| Path::new("/"));
        let name = target
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("copy");
        let new_path = unique_name(parent, name);
        copy_dir(target, &new_path)
            .with_context(|| format!("Failed to duplicate dir {:?}", target))?;
        Ok(new_path)
    } else {
        let parent = target.parent().unwrap_or_else(|| Path::new("/"));
        let name = target
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("copy");
        let new_path = unique_name(parent, name);
        fs::copy(target, &new_path)
            .with_context(|| format!("Failed to duplicate file {:?}", target))?;
        Ok(new_path)
    }
}

fn replacement_target(authored: &str, path: &Path, root: &Path) -> String {
    let qualified = authored.contains(['/', '\\']);
    let include_extension = Path::new(authored).extension().is_some();
    let relative = path.strip_prefix(root).unwrap_or(path);
    let selected = if qualified {
        relative.to_path_buf()
    } else {
        PathBuf::from(relative.file_name().unwrap_or(relative.as_os_str()))
    };
    let selected = if include_extension {
        selected
    } else {
        selected.with_extension("")
    };
    selected.to_string_lossy().replace('\\', "/")
}

fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let dest = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir(&path, &dest)?;
        } else {
            fs::copy(&path, &dest)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::file_types::{FileTypeCapabilities, RegisteredFileTypes};

    #[test]
    fn rename_rewrites_catalogued_backlinks_without_writing_catalog_state() {
        let root =
            std::env::temp_dir().join(format!("datalith-file-ops-rename-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let note = root.join("Note.md");
        let renamed = root.join("Renamed.md");
        let source = root.join("Source.md");
        fs::write(&note, "target").unwrap();
        fs::write(&source, "before [[Note#Heading|label]] after").unwrap();
        let file_types = RegisteredFileTypes::new([(
            "md".into(),
            FileTypeCapabilities {
                text_search: true,
                wiki_links: true,
                yaml_frontmatter: true,
            },
        )]);
        let catalog = VaultCatalog::open(root.clone(), file_types).unwrap();

        rename(&catalog, &note, &renamed).unwrap();

        assert!(!note.exists());
        assert!(renamed.exists());
        assert_eq!(
            fs::read_to_string(&source).unwrap(),
            "before [[Renamed#Heading|label]] after"
        );
        drop(catalog);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rename_rewrites_backlinks_inside_a_renamed_folder() {
        let root = std::env::temp_dir().join(format!(
            "datalith-file-ops-folder-rename-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let old_folder = root.join("Old");
        let new_folder = root.join("New");
        fs::create_dir_all(&old_folder).unwrap();
        fs::write(old_folder.join("Target.md"), "target").unwrap();
        fs::write(old_folder.join("Source.md"), "[[Old/Target]]").unwrap();
        let file_types = RegisteredFileTypes::new([(
            "md".into(),
            FileTypeCapabilities {
                text_search: true,
                wiki_links: true,
                yaml_frontmatter: true,
            },
        )]);
        let catalog = VaultCatalog::open(root.clone(), file_types).unwrap();

        rename(&catalog, &old_folder, &new_folder).unwrap();

        assert!(!old_folder.exists());
        assert_eq!(
            fs::read_to_string(new_folder.join("Source.md")).unwrap(),
            "[[New/Target]]"
        );
        drop(catalog);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn failed_rename_does_not_rewrite_backlinks() {
        let root = std::env::temp_dir().join(format!(
            "datalith-file-ops-failed-rename-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let note = root.join("Note.md");
        let source = root.join("Source.md");
        fs::write(&note, "target").unwrap();
        fs::write(&source, "[[Note]]").unwrap();
        let file_types = RegisteredFileTypes::new([(
            "md".into(),
            FileTypeCapabilities {
                text_search: true,
                wiki_links: true,
                yaml_frontmatter: true,
            },
        )]);
        let catalog = VaultCatalog::open(root.clone(), file_types).unwrap();

        let result = rename(
            &catalog,
            &note,
            &root.join("missing-directory").join("Renamed.md"),
        );

        assert!(result.is_err());
        assert!(note.exists());
        assert_eq!(fs::read_to_string(source).unwrap(), "[[Note]]");
        drop(catalog);
        let _ = fs::remove_dir_all(root);
    }
}
