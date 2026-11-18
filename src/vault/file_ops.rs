use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};

use crate::vault::path::{parent_dir, unique_path};
use crate::vault::{VaultCatalog, links, source};

/// The content of a new base: one table of every file.
pub const NEW_BASE_CONTENT: &str = "views:\n  - type: table\n    name: All files\n";

/// Creates a file named after `base_name`, made unique, in `target` or its folder.
pub fn create(target: &Path, base_name: &str, content: &str) -> Result<PathBuf> {
    source::ensure_writable(target)?;

    let directory = parent_dir(target);
    let path = unique_path(&directory, base_name);
    source::ensure_writable(&path)?;
    fs::write(&path, content)
        .with_context(|| format!("Failed to create file {}", path.display()))?;
    Ok(path)
}

pub fn create_folder(target: &Path) -> Result<PathBuf> {
    source::ensure_writable(target)?;

    let directory = parent_dir(target);
    let path = unique_path(&directory, "New Folder");
    fs::create_dir(&path).with_context(|| format!("Failed to create folder {}", path.display()))?;
    Ok(path)
}

pub fn update(path: &Path, content: &str) -> Result<()> {
    source::ensure_writable(path)?;

    fs::write(path, content).with_context(|| format!("Failed to update {}", path.display()))
}

const FRONTMATTER_SCAN_CAP_BYTES: usize = 1024 * 1024;

/// Sets one frontmatter key to `value`, or removes it when `value` is `None`.
/// A note without frontmatter gains a fresh block on set,
/// deleting the last key removes the block entirely.
/// The file is replaced atomically via temp file + rename.
pub fn update_frontmatter(path: &Path, key: &str, value: Option<&serde_json::Value>) -> Result<()> {
    source::ensure_writable(path)?;

    let mut file =
        fs::File::open(path).with_context(|| format!("Failed to read {}", path.display()))?;
    // Read only the scan prefix (at most 1 MiB) to locate the closing `\n---`.
    let mut prefix = Vec::with_capacity(FRONTMATTER_SCAN_CAP_BYTES);
    let scanned = std::io::Read::take(
        &mut file,
        u64::try_from(FRONTMATTER_SCAN_CAP_BYTES).unwrap_or(u64::MAX),
    )
    .read_to_end(&mut prefix)
    .with_context(|| format!("Failed to read {}", path.display()))?;
    prefix.truncate(scanned);

    let (frontmatter_range, body_offset) = locate_frontmatter(&prefix);

    let mut mapping: Vec<(yaml_serde::Value, yaml_serde::Value)> =
        if let Some(range) = frontmatter_range {
            let content = String::from_utf8_lossy(prefix.get(range.0..range.1).unwrap_or(&[]))
                .trim()
                .to_string();
            match yaml_serde::from_str::<yaml_serde::Value>(&content) {
                Ok(yaml_serde::Value::Mapping(entries)) => entries.into_iter().collect(),
                _ => bail!("Invalid YAML frontmatter in {}", path.display()),
            }
        } else {
            Vec::new()
        };

    mapping.retain(|(k, _)| k.as_str() != Some(key));
    if let Some(value) = value {
        mapping.push((
            yaml_serde::Value::String(key.to_string()),
            yaml_serde::to_value(value)?,
        ));
    }

    let new_prefix: Vec<u8> = if mapping.is_empty() {
        Vec::new()
    } else {
        let mut new_mapping = yaml_serde::Mapping::new();
        for (k, v) in mapping {
            new_mapping.insert(k, v);
        }
        let dumped = yaml_serde::to_string(&yaml_serde::Value::Mapping(new_mapping))
            .with_context(|| format!("Failed to serialize frontmatter for {}", path.display()))?;
        let mut block = String::from("---\n");
        block.push_str(dumped.trim());
        block.push_str("\n---\n");
        block.into_bytes()
    };

    // Write to a temp sibling, then atomically replace.
    let temp_name = format!(
        ".{}.datalith-tmp-{:016x}",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("note"),
        rand::random::<u64>()
    );
    let temp_path = path.with_file_name(temp_name);
    {
        let mut tmp = fs::File::create(&temp_path)
            .with_context(|| format!("Failed to write {}", path.display()))?;
        tmp.write_all(&new_prefix)
            .with_context(|| format!("Failed to write {}", path.display()))?;
        file.seek(SeekFrom::Start(
            u64::try_from(body_offset).unwrap_or(u64::MAX),
        ))?;
        if frontmatter_range.is_some() {
            let mut head = [0u8; 2];
            let read = file
                .read(&mut head)
                .with_context(|| format!("Failed to read {}", path.display()))?;
            if read == 2 && head.as_slice() == b"\r\n" {
                // consumed the CRLF
            } else if read >= 1 && head.first() == Some(&b'\n') {
                file.seek(SeekFrom::Current(-1))?;
            } else if read > 0 {
                file.seek(SeekFrom::Start(
                    u64::try_from(body_offset).unwrap_or(u64::MAX),
                ))?;
            }
        }
        std::io::copy(&mut file, &mut tmp)
            .with_context(|| format!("Failed to write {}", path.display()))?;
        tmp.sync_all()
            .with_context(|| format!("Failed to write {}", path.display()))?;
    }
    if let Err(error) = fs::rename(&temp_path, path) {
        let _ = fs::remove_file(&temp_path);
        return Err(error).with_context(|| format!("Failed to write {}", path.display()));
    }
    Ok(())
}

/// Returns `((frontmatter_content_start, closing_newline_index), body_offset)`
/// matching `extract_frontmatter`: leading `---`, closing `\n---`.
fn locate_frontmatter(prefix: &[u8]) -> (Option<(usize, usize)>, usize) {
    if !prefix.starts_with(b"---") {
        return (None, 0);
    }
    if prefix.len() < 4 {
        return (None, 0);
    }
    // Search for b"\n---" starting after the opening delimiter.
    let needle = b"\n---";
    for (i, window) in prefix.windows(4).enumerate() {
        if i < 3 {
            continue;
        }
        if window == needle {
            let body_offset = i.saturating_add(4);
            return (Some((3, i)), body_offset);
        }
    }
    (None, 0)
}

pub struct RenameResult {
    pub total_sources: usize,
    pub updated_sources: usize,
}

pub fn rename(catalog: &VaultCatalog, old_path: &Path, new_path: &Path) -> Result<RenameResult> {
    source::ensure_writable(old_path)?;
    source::ensure_writable(new_path)?;

    let root = catalog.root();
    let mut by_source: BTreeMap<PathBuf, Vec<(usize, String)>> = BTreeMap::new();
    for backlink in catalog.backlinks_under(old_path)? {
        let suffix = backlink
            .target_path
            .strip_prefix(old_path)
            .unwrap_or_else(|_| Path::new(""));
        let renamed_target = new_path.join(suffix);
        let replacement =
            links::replacement_target(&backlink.authored_target, &renamed_target, &root);
        by_source
            .entry(backlink.source)
            .or_default()
            .push((backlink.ordinal, replacement));
    }

    let total_sources = by_source.len();

    fs::rename(old_path, new_path).with_context(|| {
        format!(
            "Failed to rename {} to {}",
            old_path.display(),
            new_path.display()
        )
    })?;

    let updated_sources = by_source
        .into_iter()
        .filter_map(|(source_path, replacements)| {
            let source_path = source_path
                .strip_prefix(old_path)
                .map_or_else(|_| source_path.clone(), |suffix| new_path.join(suffix));
            let Ok(content) = fs::read_to_string(&source_path) else {
                return None;
            };
            let rewritten = links::rewrite(&content, &replacements);
            (rewritten == content || update(&source_path, &rewritten).is_ok()).then_some(())
        })
        .count();
    Ok(RenameResult {
        total_sources,
        updated_sources,
    })
}

/// Deleted items stay restorable for this long before a later deletion purges them.
const DELETED_RETENTION: std::time::Duration = std::time::Duration::from_hours(1);

/// A file or folder moved aside by [`delete`], restorable with [`restore`].
#[derive(Clone, Debug)]
pub struct Deleted {
    pub original: PathBuf,
    staged: PathBuf,
}

fn deleted_dir(vault_root: &Path) -> Result<PathBuf> {
    Ok(crate::channel::Channel::current()
        .vault_dir(vault_root)?
        .join("deleted"))
}

/// Move `target` into the vault's `deleted` folder so the deletion can be undone.
pub fn delete(vault_root: &Path, target: &Path) -> Result<Deleted> {
    source::ensure_writable(target)?;
    let directory = deleted_dir(vault_root)?;
    purge_deleted(&directory, DELETED_RETENTION);
    fs::create_dir_all(&directory)
        .with_context(|| format!("Failed to create {}", directory.display()))?;
    // Name staged items by deletion time: a rename keeps the original mtime.
    let staged = directory.join(format!(
        "{}-{:016x}",
        unix_seconds(std::time::SystemTime::now()),
        rand::random::<u64>()
    ));
    fs::rename(target, &staged)
        .with_context(|| format!("Failed to delete {}", target.display()))?;
    Ok(Deleted {
        original: target.to_path_buf(),
        staged,
    })
}

/// Put a deleted item back where it was, unless something else now uses that path.
pub fn restore(deleted: &Deleted) -> Result<()> {
    ensure!(
        !deleted.original.exists(),
        "{} already exists",
        deleted.original.display()
    );
    if let Some(parent) = deleted.original.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(&deleted.staged, &deleted.original)
        .with_context(|| format!("Failed to restore {}", deleted.original.display()))
}

fn unix_seconds(time: std::time::SystemTime) -> u64 {
    time.duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// Permanently remove deleted items older than `retention`.
fn purge_deleted(directory: &Path, retention: std::time::Duration) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    let now = unix_seconds(std::time::SystemTime::now());
    for entry in entries.flatten() {
        let deleted_at = entry
            .file_name()
            .to_str()
            .and_then(|name| name.split_once('-'))
            .and_then(|(seconds, _)| seconds.parse::<u64>().ok());
        let expired =
            deleted_at.is_none_or(|seconds| now.saturating_sub(seconds) >= retention.as_secs());
        if expired {
            let path = entry.path();
            let _ = if path.is_dir() {
                fs::remove_dir_all(&path)
            } else {
                fs::remove_file(&path)
            };
        }
    }
}

pub fn duplicate(target: &Path) -> Result<PathBuf> {
    source::ensure_writable(target)?;

    let parent = target.parent().unwrap_or_else(|| Path::new("/"));
    let name = target
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("copy");
    let new_path = unique_path(parent, name);
    if source::is_dir(target) {
        copy_dir(target, &new_path)
            .with_context(|| format!("Failed to duplicate dir {}", target.display()))?;
    } else {
        fs::copy(target, &new_path)
            .with_context(|| format!("Failed to duplicate file {}", target.display()))?;
    }
    Ok(new_path)
}

fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let destination = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir(&path, &destination)?;
        } else {
            fs::copy(&path, &destination)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::file_types::{FileTypeCapabilities, RegisteredFileTypes};
    use std::fmt::Write as _;

    fn temp_vault(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("datalith-{name}-{:032x}", rand::random::<u128>()));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn deleted_files_and_folders_can_be_restored() {
        let root = temp_vault("delete-restore");
        let note = root.join("Note.md");
        fs::write(&note, "hello").unwrap();
        let folder = root.join("Folder");
        fs::create_dir_all(folder.join("Inner")).unwrap();
        fs::write(folder.join("Inner/Child.md"), "child").unwrap();

        let deleted_note = delete(&root, &note).unwrap();
        let deleted_folder = delete(&root, &folder).unwrap();
        assert!(!note.exists());
        assert!(!folder.exists());

        restore(&deleted_note).unwrap();
        restore(&deleted_folder).unwrap();
        assert_eq!(fs::read_to_string(&note).unwrap(), "hello");
        assert_eq!(
            fs::read_to_string(folder.join("Inner/Child.md")).unwrap(),
            "child"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn restore_never_overwrites_a_new_item_at_the_same_path() {
        let root = temp_vault("delete-conflict");
        let note = root.join("Note.md");
        fs::write(&note, "old").unwrap();
        let deleted = delete(&root, &note).unwrap();
        fs::write(&note, "new").unwrap();
        assert!(restore(&deleted).is_err());
        assert_eq!(fs::read_to_string(&note).unwrap(), "new");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn purge_removes_only_expired_deleted_items() {
        let root = temp_vault("delete-purge");
        let directory = deleted_dir(&root).unwrap();
        fs::create_dir_all(&directory).unwrap();
        let expired = directory.join("0-0000000000000000");
        let unnamed = directory.join("unexpected");
        fs::write(&expired, "").unwrap();
        fs::create_dir_all(&unnamed).unwrap();
        fs::write(root.join("Note.md"), "").unwrap();
        let recent = delete(&root, &root.join("Note.md")).unwrap();

        purge_deleted(&directory, DELETED_RETENTION);
        assert!(!expired.exists());
        assert!(!unnamed.exists());
        restore(&recent).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn immutable_vault_rejects_every_file_mutation() {
        let root = source::DOCUMENTATION.root();
        let note = root.join("Welcome.md");
        let original = source::read(&note).unwrap();
        let personal = std::env::temp_dir().join(format!(
            "datalith-read-only-{:032x}",
            rand::random::<u128>()
        ));
        fs::create_dir_all(&personal).unwrap();
        let catalog = VaultCatalog::open(&personal, RegisteredFileTypes::new([])).unwrap();
        assert!(create(&root, "New Note.md", "").is_err());
        assert!(create_folder(&root).is_err());
        assert!(update(&note, "changed").is_err());
        assert!(delete(&root, &note).is_err());
        assert!(duplicate(&note).is_err());
        assert!(rename(&catalog, &note, &root.join("Renamed.md")).is_err());
        let personal_note = personal.join("Personal.md");
        fs::write(&personal_note, "personal").unwrap();
        assert!(rename(&catalog, &personal_note, &root.join("Personal.md")).is_err());
        assert!(personal_note.is_file());
        assert_eq!(source::read(&note).unwrap(), original);
        assert!(!root.exists());
        drop(catalog);
        fs::remove_dir_all(personal).unwrap();
    }

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
        let catalog = VaultCatalog::open(&root, file_types).unwrap();
        catalog.wait_until_ready(std::time::Duration::from_secs(5));

        let result = rename(&catalog, &note, &renamed).unwrap();

        assert!(!note.exists());
        assert!(renamed.exists());
        assert_eq!(
            fs::read_to_string(&source).unwrap(),
            "before [[Renamed#Heading|label]] after"
        );
        assert_eq!(result.total_sources, 1);
        assert_eq!(result.updated_sources, 1);
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
        let catalog = VaultCatalog::open(&root, file_types).unwrap();
        catalog.wait_until_ready(std::time::Duration::from_secs(5));

        let result = rename(&catalog, &old_folder, &new_folder).unwrap();

        assert!(!old_folder.exists());
        assert_eq!(
            fs::read_to_string(new_folder.join("Source.md")).unwrap(),
            "[[New/Target]]"
        );
        assert_eq!(result.total_sources, 1);
        assert_eq!(result.updated_sources, 1);
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
        let catalog = VaultCatalog::open(&root, file_types).unwrap();
        catalog.wait_until_ready(std::time::Duration::from_secs(5));

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

    #[test]
    fn create_avoids_existing_names() {
        let root = std::env::temp_dir().join(format!(
            "datalith-file-ops-create-name-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("Note.md"), "").unwrap();
        fs::write(root.join("Note 1.md"), "").unwrap();

        let path = create(&root, "Note.md", "").unwrap();

        assert_eq!(path, root.join("Note 2.md"));
        assert!(path.exists());
        assert_eq!(fs::read_to_string(&path).unwrap(), "");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn create_creates_the_requested_file() {
        let root = std::env::temp_dir().join(format!(
            "datalith-file-ops-create-requested-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let path = create(&root, "Meeting.md", "").unwrap();

        assert_eq!(path, root.join("Meeting.md"));
        assert!(path.exists());
        assert_eq!(fs::read_to_string(&path).unwrap(), "");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn create_uses_the_default_new_note_name() {
        let root = std::env::temp_dir().join(format!(
            "datalith-file-ops-create-default-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let path = create(&root, "New Note.md", "").unwrap();

        assert_eq!(path, root.join("New Note.md"));
        assert!(path.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn frontmatter_edit_updates_one_key_and_preserves_body() {
        let root = temp_vault("frontmatter-update");
        let note = root.join("Note.md");
        fs::write(
            &note,
            "---\ntitle: Old\nstatus: open\n---\n\n# Hello\n\nBody — unicode ✓\n",
        )
        .unwrap();

        update_frontmatter(&note, "status", Some(&serde_json::json!("done"))).unwrap();

        let result = fs::read_to_string(&note).unwrap();
        assert!(result.contains("title: Old"));
        assert!(result.contains("status: done"));
        assert!(result.contains("# Hello"));
        assert!(result.contains("Body — unicode ✓"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn frontmatter_delete_removes_the_key_and_last_key_removes_the_block() {
        let root = temp_vault("frontmatter-delete");
        let note = root.join("Note.md");
        fs::write(&note, "---\ntitle: Only\n---\n\nBody\n").unwrap();

        update_frontmatter(&note, "title", None).unwrap();

        let result = fs::read_to_string(&note).unwrap();
        assert!(!result.starts_with("---"));
        assert!(result.contains("Body"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn frontmatter_set_adds_a_block_to_a_note_without_one() {
        let root = temp_vault("frontmatter-add");
        let note = root.join("Note.md");
        fs::write(&note, "# No frontmatter\nBody\n").unwrap();

        update_frontmatter(&note, "status", Some(&serde_json::json!("new"))).unwrap();

        let result = fs::read_to_string(&note).unwrap();
        assert!(result.starts_with("---\n"));
        assert!(result.contains("Body"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn frontmatter_unclosed_separator_is_treated_as_no_frontmatter() {
        let root = temp_vault("frontmatter-unclosed");
        let note = root.join("Note.md");
        fs::write(&note, "---\nthis never closes\n").unwrap();

        update_frontmatter(&note, "status", Some(&serde_json::json!("new"))).unwrap();

        let result = fs::read_to_string(&note).unwrap();
        assert!(result.contains("status: new"));
        assert!(result.contains("this never closes"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn frontmatter_invalid_yaml_fails_without_touching_the_file() {
        let root = temp_vault("frontmatter-invalid");
        let note = root.join("Note.md");
        let original = "---\n: bad: [\n---\n\nBody\n";
        fs::write(&note, original).unwrap();

        let result = update_frontmatter(&note, "status", Some(&serde_json::json!("x")));
        assert!(result.is_err());
        assert_eq!(fs::read_to_string(&note).unwrap(), original);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn frontmatter_large_body_is_preserved_byte_for_byte() {
        let root = temp_vault("frontmatter-large");
        let note = root.join("Note.md");
        let mut original = String::from("---\ntitle: T\n---\n\n");
        for i in 0..5000 {
            writeln!(original, "line {i} — some body text ✓").unwrap();
        }
        fs::write(&note, &original).unwrap();

        update_frontmatter(&note, "title", Some(&serde_json::json!("Renamed"))).unwrap();

        let result = fs::read_to_string(&note).unwrap();
        assert!(result.contains("title: Renamed"));
        assert!(result.contains("line 4999"));
        fs::remove_dir_all(root).unwrap();
    }
}
