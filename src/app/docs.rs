use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::settings;

pub(crate) const DOCS_VAULT_NAME: &str = "Datalith Docs";

const SHIPPED_DOCS: &[(&str, &str)] = &[
    ("Welcome.md", include_str!("../../docs/vault/Welcome.md")),
    ("Basics.md", include_str!("../../docs/vault/Basics.md")),
    (
        "FileTypes.md",
        include_str!("../../docs/vault/FileTypes.md"),
    ),
    (
        "Overview.graph",
        include_str!("../../docs/vault/Overview.graph"),
    ),
    ("Search.md", include_str!("../../docs/vault/Search.md")),
    (
        "Shortcuts.md",
        include_str!("../../docs/vault/Shortcuts.md"),
    ),
    ("Settings.md", include_str!("../../docs/vault/Settings.md")),
    (
        "Tour.todotxt",
        include_str!("../../docs/vault/Tour.todotxt"),
    ),
    (
        "formats/Properties.md",
        include_str!("../../docs/vault/formats/Properties.md"),
    ),
    (
        "formats/Graph.md",
        include_str!("../../docs/vault/formats/Graph.md"),
    ),
    (
        "formats/Markdown.md",
        include_str!("../../docs/vault/formats/Markdown.md"),
    ),
    (
        "formats/TodoTxt.md",
        include_str!("../../docs/vault/formats/TodoTxt.md"),
    ),
];

#[derive(Clone, Debug)]
pub(crate) struct DocsVaultOutcome {
    pub(crate) docs_vault: PathBuf,
    pub(crate) first_run: bool,
}

pub(crate) fn ensure_docs_vault() -> Result<DocsVaultOutcome> {
    let first_run = settings::snapshot().last_vault.is_none();
    let docs_vault = docs_vault_path();
    fs::create_dir_all(&docs_vault)
        .with_context(|| format!("Failed to create docs Vault: {}", docs_vault.display()))?;
    seed_into(&docs_vault)?;
    settings::register_recent_vault(&docs_vault)?;
    Ok(DocsVaultOutcome {
        docs_vault,
        first_run,
    })
}

fn seed_into(root: &Path) -> Result<()> {
    for (relative, content) in SHIPPED_DOCS {
        let target = root.join(relative);
        if target.exists() {
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create docs Vault folder: {}", parent.display())
            })?;
        }
        fs::write(&target, content)
            .with_context(|| format!("Failed to seed docs Vault file: {}", target.display()))?;
    }
    Ok(())
}

fn docs_vault_path() -> PathBuf {
    datalith_data_dir().join(DOCS_VAULT_NAME)
}

fn datalith_data_dir() -> PathBuf {
    dirs::data_dir().unwrap_or_default().join("datalith")
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn shipped_docs_are_registered_extensions() {
        for (relative, _) in SHIPPED_DOCS {
            let extension = Path::new(relative)
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("");
            assert!(
                ["md", "graph", "todotxt"].contains(&extension),
                "unregistered extension for seeded doc: {relative}"
            );
        }
    }

    #[test]
    fn seeding_writes_every_doc_and_preserves_edits() {
        let root = std::env::temp_dir().join(format!(
            "datalith-docs-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        seed_into(&root).expect("seed");
        for (relative, content) in SHIPPED_DOCS {
            let target = root.join(relative);
            assert!(target.exists(), "missing seeded doc: {relative}");
            assert_eq!(
                fs::read_to_string(&target).unwrap(),
                *content,
                "seeded doc differs: {relative}"
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
        let root = std::env::temp_dir().join(format!(
            "datalith-docs-restore-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        seed_into(&root).expect("seed");
        let deleted = root.join("Tour.todotxt");
        fs::remove_file(&deleted).unwrap();
        seed_into(&root).expect("reseeds a deleted doc");
        assert!(deleted.exists(), "deleted doc must be restored");

        let _ = fs::remove_dir_all(root);
    }
}
