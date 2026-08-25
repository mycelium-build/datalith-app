//! Catalog schema creation and version handling.

use anyhow::{Context, Result};

use super::CatalogDatabase;

pub(super) const SCHEMA_VERSION: i64 = 4;

impl CatalogDatabase {
    pub(super) async fn initialize_schema(&self) -> Result<()> {
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
                    "DROP TABLE IF EXISTS document_tags; \
                     DROP TABLE IF EXISTS wiki_links; \
                     DROP TABLE IF EXISTS documents; \
                     PRAGMA user_version = 0;",
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
                    created_ns  INTEGER NOT NULL DEFAULT 0,
                    metadata    BLOB
                );
                CREATE INDEX IF NOT EXISTS documents_extension_idx ON documents(extension);
                CREATE INDEX IF NOT EXISTS documents_folder_idx ON documents(folder);

                CREATE TABLE IF NOT EXISTS wiki_links (
                    source_path TEXT NOT NULL REFERENCES documents(path) ON DELETE CASCADE ON UPDATE CASCADE,
                    ordinal     INTEGER NOT NULL,
                    target      TEXT NOT NULL,
                    target_path TEXT REFERENCES documents(path) ON DELETE SET NULL ON UPDATE CASCADE,
                    is_embed    INTEGER NOT NULL DEFAULT 0,
                    PRIMARY KEY (source_path, ordinal)
                );
                CREATE INDEX IF NOT EXISTS wiki_links_target_nocase_idx ON wiki_links(target COLLATE NOCASE);
                CREATE INDEX IF NOT EXISTS wiki_links_target_path_idx ON wiki_links(target_path);
                PRAGMA user_version = 4;
                ",
            )
            .await?;
        Ok(())
    }
}
