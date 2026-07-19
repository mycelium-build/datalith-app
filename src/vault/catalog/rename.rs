use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

const JOURNAL_FILE: &str = "rename-journal.json";

#[derive(Debug, Deserialize, Serialize)]
struct RenameJournal {
    from: PathBuf,
    to: PathBuf,
    replacements: Vec<Replacement>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Replacement {
    source: PathBuf,
    staged: PathBuf,
    expected_hash: u64,
    replacement_hash: u64,
}

pub(super) fn execute(
    root: &Path,
    from: &Path,
    to: &Path,
    replacements: Vec<(PathBuf, String, String)>,
) -> Result<()> {
    let metadata_dir = root.join(".datalith");
    fs::create_dir_all(&metadata_dir)?;
    let mut journal = RenameJournal {
        from: from.to_path_buf(),
        to: to.to_path_buf(),
        replacements: Vec::new(),
    };
    for (index, (source, expected, replacement)) in replacements.into_iter().enumerate() {
        let staged = metadata_dir.join(format!("rename-stage-{}-{index}", std::process::id()));
        fs::write(&staged, replacement.as_bytes())?;
        fs::File::open(&staged)?.sync_all()?;
        journal.replacements.push(Replacement {
            source,
            staged,
            expected_hash: content_hash(&expected),
            replacement_hash: content_hash(&replacement),
        });
    }
    write_journal(&metadata_dir, &journal)?;
    roll_forward(&metadata_dir, journal)
}

pub(super) fn recover(root: &Path) -> Result<bool> {
    let metadata_dir = root.join(".datalith");
    let path = metadata_dir.join(JOURNAL_FILE);
    if !path.exists() {
        return Ok(false);
    }
    let journal: RenameJournal = serde_json::from_slice(&fs::read(&path)?)?;
    roll_forward(&metadata_dir, journal)?;
    Ok(true)
}

fn write_journal(metadata_dir: &Path, journal: &RenameJournal) -> Result<()> {
    let path = metadata_dir.join(JOURNAL_FILE);
    fs::write(&path, serde_json::to_vec(journal)?)?;
    fs::File::open(path)?.sync_all()?;
    Ok(())
}

fn roll_forward(metadata_dir: &Path, journal: RenameJournal) -> Result<()> {
    if journal.from.exists() {
        if journal.to.exists() {
            bail!("Cannot recover rename because both source and destination exist");
        }
        fs::rename(&journal.from, &journal.to).with_context(|| {
            format!(
                "Failed to roll forward rename {} to {}",
                journal.from.display(),
                journal.to.display()
            )
        })?;
    } else if !journal.to.exists() {
        bail!("Cannot recover rename because neither source nor destination exists");
    }

    for replacement in &journal.replacements {
        let current = fs::read(&replacement.source).with_context(|| {
            format!(
                "Failed to validate rename source {}",
                replacement.source.display()
            )
        })?;
        let current_hash = bytes_hash(&current);
        if current_hash == replacement.replacement_hash && !replacement.staged.exists() {
            continue;
        }
        if current_hash != replacement.expected_hash {
            bail!(
                "Refusing to overwrite independently changed file {}",
                replacement.source.display()
            );
        }
        fs::rename(&replacement.staged, &replacement.source).with_context(|| {
            format!(
                "Failed to install rewritten links in {}",
                replacement.source.display()
            )
        })?;
    }
    fs::remove_file(metadata_dir.join(JOURNAL_FILE))?;
    Ok(())
}

fn content_hash(contents: &str) -> u64 {
    bytes_hash(contents.as_bytes())
}

fn bytes_hash(contents: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    contents.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completes_a_staged_rename_and_rewrite() {
        let root =
            std::env::temp_dir().join(format!("datalith-rename-journal-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let from = root.join("Note.md");
        let to = root.join("Renamed.md");
        let source = root.join("Source.md");
        fs::write(&from, "note").unwrap();
        fs::write(&source, "[[Note]]").unwrap();
        execute(
            &root,
            &from,
            &to,
            vec![(source.clone(), "[[Note]]".into(), "[[Renamed]]".into())],
        )
        .unwrap();
        assert!(to.exists());
        assert_eq!(fs::read_to_string(source).unwrap(), "[[Renamed]]");
        assert!(!root.join(".datalith").join(JOURNAL_FILE).exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn recovery_finishes_after_target_was_renamed() {
        let root =
            std::env::temp_dir().join(format!("datalith-rename-recovery-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let metadata_dir = root.join(".datalith");
        fs::create_dir_all(&metadata_dir).unwrap();
        let from = root.join("Note.md");
        let to = root.join("Renamed.md");
        let source = root.join("Source.md");
        let staged = metadata_dir.join("rename-stage-test");
        fs::write(&to, "note").unwrap();
        fs::write(&source, "[[Note]]").unwrap();
        fs::write(&staged, "[[Renamed]]").unwrap();
        write_journal(
            &metadata_dir,
            &RenameJournal {
                from,
                to,
                replacements: vec![Replacement {
                    source: source.clone(),
                    staged,
                    expected_hash: content_hash("[[Note]]"),
                    replacement_hash: content_hash("[[Renamed]]"),
                }],
            },
        )
        .unwrap();

        assert!(recover(&root).unwrap());
        assert_eq!(fs::read_to_string(source).unwrap(), "[[Renamed]]");
        assert!(!metadata_dir.join(JOURNAL_FILE).exists());
        let _ = fs::remove_dir_all(root);
    }
}
