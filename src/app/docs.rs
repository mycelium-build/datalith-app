use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

pub const DOCS_VAULT_NAME: &str = "Datalith Docs";

pub const WELCOME_NOTE: &str = "Welcome.md";

include!(concat!(env!("OUT_DIR"), "/docs_vault.rs"));

pub fn ensure_docs_vault() -> Result<PathBuf> {
    let docs_vault = docs_vault_path();
    fs::create_dir_all(&docs_vault)
        .with_context(|| format!("Failed to create docs Vault: {}", docs_vault.display()))?;
    seed_into(&docs_vault)?;
    Ok(docs_vault)
}

/// Write bundled documentation into `root`, creating parent folders as needed.
/// Existing files are preserved.
pub fn seed_into(root: &Path) -> Result<()> {
    for (name, bytes) in DOCS_FILES {
        let target = root.join(name);
        let parent = target.parent().context("Docs file has no parent")?;
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create docs folder: {}", parent.display()))?;
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
        {
            Ok(mut file) => file
                .write_all(bytes)
                .with_context(|| format!("Failed to seed docs file: {}", target.display()))?,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("Failed to seed docs file: {}", target.display()));
            }
        }
    }
    Ok(())
}

pub fn docs_vault_path() -> PathBuf {
    super::data_dir().join(DOCS_VAULT_NAME)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::*;

    fn docs_vault_source() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/vault")
    }

    fn source_files(dir: &Path) -> Vec<PathBuf> {
        let mut files = Vec::new();
        for entry in fs::read_dir(dir).unwrap().flatten() {
            let name = entry.file_name();
            if name.to_string_lossy().starts_with('.') {
                continue;
            }
            let path = entry.path();
            if path.is_dir() {
                files.extend(source_files(&path));
            } else {
                files.push(path);
            }
        }
        files
    }

    fn temp_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "datalith-docs-{label}-{}-{}",
            std::process::id(),
            std::thread::current()
                .name()
                .unwrap_or("test")
                .replace("::", "-")
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn shipped_docs_are_registered_extensions() {
        const BINARY_DOC_ASSETS: &[&str] = &["png"];
        for path in source_files(&docs_vault_source()) {
            let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
            assert!(
                ["md", "base", "todotxt"].contains(&extension)
                    || BINARY_DOC_ASSETS.contains(&extension),
                "unregistered extension for seeded doc: {}",
                path.display()
            );
        }
    }

    #[test]
    fn welcome_is_bundled() {
        assert!(DOCS_FILES.iter().any(|(name, _)| *name == WELCOME_NOTE));
    }

    #[test]
    fn seeding_writes_every_doc_and_preserves_edits() {
        let root = temp_root("seed");
        let source = docs_vault_source();

        seed_into(&root).expect("seed");
        for path in source_files(&source) {
            let relative = path.strip_prefix(&source).unwrap();
            let target = root.join(relative);
            assert!(
                target.exists(),
                "missing seeded doc: {}",
                relative.display()
            );
            assert_eq!(
                fs::read(&target).unwrap(),
                fs::read(&path).unwrap(),
                "seeded doc differs: {}",
                relative.display()
            );
        }

        let edited = root.join("Welcome.md");
        fs::write(&edited, "my edits\n").unwrap();
        seed_into(&root).expect("reseeds without touching existing files");
        assert_eq!(
            fs::read_to_string(&edited).unwrap(),
            "my edits\n",
            "user edits must be preserved"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn seeding_restores_deleted_docs() {
        let root = temp_root("restore");

        seed_into(&root).expect("seed");
        let deleted = root.join("Tour.todotxt");
        fs::remove_file(&deleted).unwrap();
        seed_into(&root).expect("reseeds a deleted doc");
        assert!(deleted.exists(), "deleted doc must be restored");

        let _ = fs::remove_dir_all(root);
    }
}
