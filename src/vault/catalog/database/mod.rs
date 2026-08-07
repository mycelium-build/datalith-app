use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};

use crate::vault::DATALITH_DIR_NAME;

mod document;
mod filter_compiler;
mod link_resolution;

const SCHEMA_VERSION: i64 = 3;

pub(super) const FILE_NAME_SQL: &str = "substr(path, \
    CASE WHEN folder = '' THEN 1 ELSE length(folder) + 2 END, \
    length(path) - CASE WHEN folder = '' THEN 1 ELSE length(folder) + 2 END - length(extension))";
pub(super) const FILE_BASENAME_SQL: &str =
    "substr(path, CASE WHEN folder = '' THEN 1 ELSE length(folder) + 2 END)";
pub(super) const PATH_WITHOUT_EXTENSION_SQL: &str =
    "substr(path, 1, length(path) - length(extension) - 1)";

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct StoredLink {
    pub(super) source: PathBuf,
    pub(super) target: PathBuf,
}

#[derive(Clone, Debug)]
pub struct Backlink {
    pub source: PathBuf,
    pub ordinal: usize,
    pub authored_target: String,
    pub target_path: PathBuf,
}

#[derive(Clone)]
pub(super) struct CatalogDatabase {
    pub(super) root: PathBuf,
    connection: turso::Connection,
}

impl CatalogDatabase {
    pub(super) async fn open(root: &Path) -> Result<Self> {
        let metadata_dir = root.join(DATALITH_DIR_NAME);
        fs::create_dir_all(&metadata_dir).with_context(|| {
            format!(
                "Failed to create catalog directory {}",
                metadata_dir.display()
            )
        })?;
        let database_path = metadata_dir.join("catalog.db");
        let database_path_text = database_path
            .to_str()
            .ok_or_else(|| anyhow!("Catalog database path is not UTF-8"))?;
        let database =
            if let Ok(database) = turso::Builder::new_local(database_path_text).build().await {
                database
            } else {
                let _ = fs::remove_file(&database_path);
                let _ = fs::remove_file(database_path.with_extension("db-wal"));
                let _ = fs::remove_file(database_path.with_extension("db-shm"));
                turso::Builder::new_local(database_path_text)
                    .build()
                    .await
                    .context("Failed to rebuild embedded Turso catalog")?
            };
        let connection = database.connect()?;
        drop(database);
        connection.execute("PRAGMA foreign_keys = ON", ()).await?;
        connection
            .query("PRAGMA journal_mode = WAL", ())
            .await?
            .next()
            .await?;
        connection
            .execute("PRAGMA synchronous = NORMAL", ())
            .await?;
        // connection.execute("PRAGMA cache_size = -2000", ()).await?; // Default 2MB
        let this = Self {
            root: root.to_path_buf(),
            connection,
        };
        this.initialize_schema().await?;
        Ok(this)
    }

    pub(super) fn connection(&self) -> turso::Connection {
        self.connection.clone()
    }

    async fn initialize_schema(&self) -> Result<()> {
        let connection = self.connection();
        connection
            .query("PRAGMA journal_mode = WAL", ())
            .await?
            .next()
            .await?;
        let mut rows = connection.query("PRAGMA user_version", ()).await?;
        let version = rows
            .next()
            .await?
            .map(|row| row.get::<i64>(0))
            .transpose()?
            .unwrap_or_default();
        if version != 0 && version != SCHEMA_VERSION {
            connection
                .execute_batch(
                    "DROP TABLE IF EXISTS wiki_links; DROP TABLE IF EXISTS documents; PRAGMA user_version = 0;",
                )
                .await
                .with_context(|| format!("Failed to rebuild catalog schema version {version}"))?;
        }
        connection
            .execute_batch(
                r"
                CREATE TABLE IF NOT EXISTS documents (
                    path        TEXT PRIMARY KEY,
                    extension   TEXT NOT NULL,
                    folder      TEXT NOT NULL,
                    size_bytes  INTEGER NOT NULL,
                    modified_ns INTEGER NOT NULL,
                    metadata    BLOB
                );
                CREATE INDEX IF NOT EXISTS documents_extension_idx ON documents(extension);
                CREATE INDEX IF NOT EXISTS documents_folder_idx ON documents(folder);

                CREATE TABLE IF NOT EXISTS wiki_links (
                    source_path TEXT NOT NULL REFERENCES documents(path) ON DELETE CASCADE ON UPDATE CASCADE,
                    ordinal     INTEGER NOT NULL,
                    target      TEXT NOT NULL,
                    target_path TEXT REFERENCES documents(path) ON DELETE SET NULL ON UPDATE CASCADE,
                    PRIMARY KEY (source_path, ordinal)
                );
                CREATE INDEX IF NOT EXISTS wiki_links_target_nocase_idx ON wiki_links(target COLLATE NOCASE);
                CREATE INDEX IF NOT EXISTS wiki_links_target_path_idx ON wiki_links(target_path);
                PRAGMA user_version = 3;
                ",
            )
            .await?;
        Ok(())
    }
}

pub(super) fn path_text(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub(super) fn escape_like_pattern(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::indexing_slicing,
        clippy::string_slice
    )]
    use super::*;
    use crate::document::file_types::{FileTypeCapabilities, RegisteredFileTypes};
    use crate::vault::catalog::{
        CatalogComparison, CatalogFilter, CatalogProperty, CatalogQuery, CatalogScalar,
    };

    fn markdown_file_types() -> RegisteredFileTypes {
        RegisteredFileTypes::new([(
            "md".into(),
            FileTypeCapabilities {
                text_search: true,
                wiki_links: true,
                yaml_frontmatter: true,
            },
        )])
    }

    #[test]
    fn queries_and_filters_typed_document_properties() {
        let root =
            std::env::temp_dir().join(format!("datalith-catalog-query-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        pollster::block_on(async {
            let database = CatalogDatabase::open(&root).await.unwrap();
            database.connection().execute(
                "INSERT INTO documents(path, extension, folder, size_bytes, modified_ns, metadata) VALUES ('Note.md', 'md', '', 0, 0, jsonb(?1))",
                [r#"{"status":"done","priority":4,"tags":["rust","project"]}"#],
            ).await.unwrap();
            let selection = database
                .query_documents(CatalogQuery {
                    extension: Some("md".into()),
                    filter: CatalogFilter::And(vec![
                        CatalogFilter::Compare {
                            property: CatalogProperty::Metadata(vec!["status".into()]),
                            comparison: CatalogComparison::Equal,
                            value: CatalogScalar::String("done".into()),
                        },
                        CatalogFilter::Contains {
                            property: CatalogProperty::Metadata(vec!["tags".into()]),
                            value: CatalogScalar::String("rust".into()),
                        },
                    ]),
                    limit: 10,
                })
                .await
                .unwrap();
            assert_eq!(selection.documents.len(), 1);
            assert_eq!(
                selection.documents[0].metadata.as_ref().unwrap()["priority"],
                4
            );
        });
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn negated_comparison_includes_documents_with_missing_properties() {
        let root =
            std::env::temp_dir().join(format!("datalith-catalog-negation-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        pollster::block_on(async {
            let database = CatalogDatabase::open(&root).await.unwrap();
            let connection = database.connection();
            connection
                .execute(
                    "INSERT INTO documents(path, extension, folder, size_bytes, modified_ns, metadata) \
                     VALUES ('Done.md', 'md', '', 0, 0, jsonb('{\"status\":\"done\"}')), \
                            ('Missing.md', 'md', '', 0, 0, jsonb('{}'))",
                    (),
                )
                .await
                .unwrap();

            let selection = database
                .query_documents(CatalogQuery {
                    extension: Some("md".into()),
                    filter: CatalogFilter::Not(Box::new(CatalogFilter::Compare {
                        property: CatalogProperty::Metadata(vec!["status".into()]),
                        comparison: CatalogComparison::Equal,
                        value: CatalogScalar::String("done".into()),
                    })),
                    limit: 10,
                })
                .await
                .unwrap();

            assert_eq!(
                selection
                    .documents
                    .iter()
                    .map(|document| document.path.as_path())
                    .collect::<Vec<_>>(),
                vec![root.join("Missing.md").as_path()]
            );
        });
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn folder_filter_treats_like_wildcards_as_path_text() {
        let root =
            std::env::temp_dir().join(format!("datalith-folder-like-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        pollster::block_on(async {
            let database = CatalogDatabase::open(&root).await.unwrap();
            let connection = database.connection();
            for (path, folder) in [
                ("a_%/Direct.md", "a_%"),
                ("a_%/child/Descendant.md", "a_%/child"),
                ("axb/Other.md", "axb"),
                ("axb/child/Other.md", "axb/child"),
            ] {
                connection
                    .execute(
                        "INSERT INTO documents(path, extension, folder, size_bytes, modified_ns, metadata) \
                         VALUES (?, 'md', ?, 0, 0, NULL)",
                        turso::params![path, folder],
                    )
                    .await
                    .unwrap();
            }

            let selection = database
                .query_documents(CatalogQuery {
                    extension: Some("md".into()),
                    filter: CatalogFilter::InFolder("a_%".into()),
                    limit: 10,
                })
                .await
                .unwrap();

            assert_eq!(
                selection
                    .documents
                    .iter()
                    .map(|document| document.path.as_path())
                    .collect::<Vec<_>>(),
                vec![
                    root.join("a_%/Direct.md").as_path(),
                    root.join("a_%/child/Descendant.md").as_path(),
                ]
            );
        });
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_links_from_catalogued_paths_using_ambiguity_order() {
        let root =
            std::env::temp_dir().join(format!("datalith-catalog-resolve-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        pollster::block_on(async {
            let database = CatalogDatabase::open(&root).await.unwrap();
            let connection = database.connection();
            for (path, extension, folder) in [
                ("Note.txt", "txt", ""),
                ("a/Note.md", "md", "a"),
                ("b/Other.txt", "txt", "b"),
                ("c/Other.md", "md", "c"),
                ("a/Same.md", "md", "a"),
                ("b/Same.md", "md", "b"),
            ] {
                connection
                    .execute(
                        "INSERT INTO documents(path, extension, folder, size_bytes, modified_ns, metadata) \
                         VALUES (?, ?, ?, 0, 0, NULL)",
                        turso::params![path, extension, folder],
                    )
                    .await
                    .unwrap();
            }

            assert_eq!(
                database.resolve_path("Note").await.unwrap(),
                Some(root.join("Note.txt"))
            );
            assert_eq!(
                database.resolve_path("Other").await.unwrap(),
                Some(root.join("c/Other.md"))
            );
            assert_eq!(
                database.resolve_path("Same").await.unwrap(),
                Some(root.join("a/Same.md"))
            );
            assert_eq!(
                database.resolve_path("a/Same").await.unwrap(),
                Some(root.join("a/Same.md"))
            );
        });
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn backlink_descendant_query_treats_like_wildcards_as_path_text() {
        let root =
            std::env::temp_dir().join(format!("datalith-backlinks-like-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        pollster::block_on(async {
            let database = CatalogDatabase::open(&root).await.unwrap();
            let connection = database.connection();
            for (path, folder) in [
                ("Source.md", ""),
                ("Other.md", ""),
                ("%_/Target.md", "%_"),
                ("ab/Target.md", "ab"),
            ] {
                connection
                    .execute(
                        "INSERT INTO documents(path, extension, folder, size_bytes, modified_ns, metadata) \
                         VALUES (?, 'md', ?, 0, 0, NULL)",
                        turso::params![path, folder],
                    )
                    .await
                    .unwrap();
            }
            connection
                .execute(
                    "INSERT INTO wiki_links(source_path, ordinal, target, target_path) \
                     VALUES ('Source.md', 0, '%_/Target', '%_/Target.md'), \
                            ('Other.md', 0, 'ab/Target', 'ab/Target.md')",
                    (),
                )
                .await
                .unwrap();

            let backlinks = database.backlinks_under(&root.join("%_")).await.unwrap();

            assert_eq!(backlinks.len(), 1);
            assert_eq!(backlinks[0].source, PathBuf::from("Source.md"));
            assert_eq!(backlinks[0].target_path, PathBuf::from("%_/Target.md"));
        });
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn source_edit_does_not_update_unrelated_link_resolutions() {
        let root = std::env::temp_dir().join(format!(
            "datalith-incremental-link-resolution-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let source_a = root.join("SourceA.md");
        let source_b = root.join("SourceB.md");
        let target_a = root.join("TargetA.md");
        let target_b = root.join("TargetB.md");
        fs::write(&source_a, "[[TargetA]]").unwrap();
        fs::write(&source_b, "[[TargetB]]").unwrap();
        fs::write(&target_a, "").unwrap();
        fs::write(&target_b, "").unwrap();

        pollster::block_on(async {
            let database = CatalogDatabase::open(&root).await.unwrap();
            let file_types = markdown_file_types();
            database
                .synchronize(
                    &[],
                    &[
                        source_a.clone(),
                        source_b.clone(),
                        target_a,
                        target_b.clone(),
                    ],
                    &file_types,
                )
                .await
                .unwrap();
            let connection = database.connection();
            connection
                .execute_batch(
                    "CREATE TABLE resolution_updates(source_path TEXT NOT NULL);
                     CREATE TRIGGER record_resolution_update
                     AFTER UPDATE OF target_path ON wiki_links
                     BEGIN
                         INSERT INTO resolution_updates(source_path) VALUES (NEW.source_path);
                     END;",
                )
                .await
                .unwrap();

            fs::write(&source_a, "edited [[TargetA]]").unwrap();
            database
                .synchronize(&[], std::slice::from_ref(&source_a), &file_types)
                .await
                .unwrap();

            let unrelated_updates: i64 = connection
                .query(
                    "SELECT count(*) FROM resolution_updates WHERE source_path = 'SourceB.md'",
                    (),
                )
                .await
                .unwrap()
                .next()
                .await
                .unwrap()
                .unwrap()
                .get(0)
                .unwrap();
            assert_eq!(unrelated_updates, 0);

            let mut rows = connection
                .query(
                    "SELECT target_path FROM wiki_links WHERE source_path = 'SourceA.md'",
                    (),
                )
                .await
                .unwrap();
            assert_eq!(
                rows.next()
                    .await
                    .unwrap()
                    .unwrap()
                    .get::<String>(0)
                    .unwrap(),
                "TargetA.md"
            );
        });
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn link_resolution_tracks_added_and_removed_candidates() {
        let root = std::env::temp_dir().join(format!(
            "datalith-incremental-link-candidates-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let source = root.join("Source.md");
        let markdown_target = root.join("Target.md");
        let text_target = root.join("Target.txt");
        fs::write(&source, "[[Target]]").unwrap();
        fs::write(&text_target, "").unwrap();
        let capabilities = FileTypeCapabilities {
            text_search: true,
            wiki_links: true,
            yaml_frontmatter: true,
        };
        let file_types =
            RegisteredFileTypes::new([("md".into(), capabilities), ("txt".into(), capabilities)]);

        pollster::block_on(async {
            let database = CatalogDatabase::open(&root).await.unwrap();
            database
                .synchronize(&[], &[source.clone(), text_target.clone()], &file_types)
                .await
                .unwrap();
            let connection = database.connection();

            let resolved_target = async || {
                connection
                    .query(
                        "SELECT target_path FROM wiki_links WHERE source_path = 'Source.md'",
                        (),
                    )
                    .await
                    .unwrap()
                    .next()
                    .await
                    .unwrap()
                    .unwrap()
                    .get::<String>(0)
                    .unwrap()
            };
            assert_eq!(resolved_target().await, "Target.txt");

            fs::write(&markdown_target, "").unwrap();
            database
                .synchronize(&[], std::slice::from_ref(&markdown_target), &file_types)
                .await
                .unwrap();
            assert_eq!(resolved_target().await, "Target.md");

            fs::remove_file(&markdown_target).unwrap();
            database
                .synchronize(std::slice::from_ref(&markdown_target), &[], &file_types)
                .await
                .unwrap();
            assert_eq!(resolved_target().await, "Target.txt");
        });
        let _ = fs::remove_dir_all(root);
    }
}
