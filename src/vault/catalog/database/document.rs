use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use turso::Value;

use super::{CatalogDatabase, StoredLink, path_text};
use super::filter_compiler::FilterCompiler;
use super::link_resolution::{collect_matching_targets, link_target_candidates, resolve_links};
use crate::document::file_types::RegisteredFileTypes;
use crate::vault::catalog::{CatalogDocument, CatalogQuery, DocumentSelection};
use crate::vault::links;

pub(super) struct TrackedDocument {
    pub(super) path: PathBuf,
    pub(super) extension: String,
    pub(super) folder: String,
    pub(super) size_bytes: i64,
    pub(super) modified_ns: i64,
    pub(super) metadata: Option<serde_json::Value>,
    pub(super) links: Vec<String>,
}

impl TrackedDocument {
    pub(super) fn read(
        root: &Path,
        path: &Path,
        file_types: &RegisteredFileTypes,
    ) -> Option<Self> {
        let capabilities = file_types.capabilities(path)?;
        let relative = path.strip_prefix(root).ok()?.to_path_buf();
        let metadata = fs::metadata(path).ok()?;
        let needs_text =
            capabilities.text_search || capabilities.wiki_links || capabilities.yaml_frontmatter;
        let content = if needs_text {
            Some(fs::read_to_string(path).ok()?)
        } else {
            None
        };
        let document_metadata = content
            .as_deref()
            .filter(|_| capabilities.yaml_frontmatter)
            .and_then(frontmatter_metadata);
        let document_links = content
            .as_deref()
            .filter(|_| capabilities.wiki_links)
            .map(|content| {
                links::occurrences(content)
                    .into_iter()
                    .map(|occurrence| occurrence.target)
                    .collect()
            })
            .unwrap_or_default();
        let extension = relative.extension()?.to_str()?.to_lowercase();
        let folder = relative
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(|parent| parent.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        let modified_ns = metadata
            .modified()
            .ok()
            .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_nanos().min(i64::MAX as u128) as i64)
            .unwrap_or_default();
        Some(Self {
            path: relative,
            extension,
            folder,
            size_bytes: metadata.len().min(i64::MAX as u64) as i64,
            modified_ns,
            metadata: document_metadata,
            links: document_links,
        })
    }
}

fn frontmatter_metadata(content: &str) -> Option<serde_json::Value> {
    let normalized = content.replace("\r\n", "\n");
    let rest = normalized.strip_prefix("---\n")?;
    let closing = rest.find("\n---")?;
    let yaml = &rest[..closing];
    yaml_serde::from_str::<serde_json::Value>(yaml)
        .ok()
        .filter(serde_json::Value::is_object)
}

impl CatalogDatabase {
    pub(crate) async fn stored_paths(&self) -> Result<Vec<PathBuf>> {
        let connection = self.connection().await?;
        let mut rows = connection
            .query("SELECT path FROM documents ORDER BY path", ())
            .await?;
        let mut paths = Vec::new();
        while let Some(row) = rows.next().await? {
            paths.push(self.root.join(row.get::<String>(0)?));
        }
        Ok(paths)
    }

    pub(crate) async fn synchronize(
        &self,
        removed: &[PathBuf],
        changed: &[PathBuf],
        file_types: &RegisteredFileTypes,
    ) -> Result<Vec<PathBuf>> {
        let documents = changed
            .iter()
            .filter_map(|path| TrackedDocument::read(&self.root, path, file_types))
            .collect::<Vec<_>>();

        let connection = self.connection().await?;
        connection.execute("BEGIN IMMEDIATE", ()).await?;
        let result = async {
            let document_paths = documents
                .iter()
                .map(|document| path_text(&document.path))
                .collect::<BTreeSet<_>>();
            let changed_paths = changed
                .iter()
                .filter_map(|path| path.strip_prefix(&self.root).ok())
                .map(path_text)
                .collect::<BTreeSet<_>>();
            let removed_paths = removed
                .iter()
                .filter_map(|path| path.strip_prefix(&self.root).ok())
                .map(path_text)
                .collect::<BTreeSet<_>>();
            let mut stored_affected_paths = BTreeSet::new();
            for path in changed_paths.iter().chain(&removed_paths) {
                let mut rows = connection
                    .query(
                        "SELECT 1 FROM documents WHERE path = ? LIMIT 1",
                        [path.as_str()],
                    )
                    .await?;
                if rows.next().await?.is_some() {
                    stored_affected_paths.insert(path.clone());
                }
            }

            let removed_affected_paths = stored_affected_paths
                .iter()
                .filter(|path| removed_paths.contains(*path) || !document_paths.contains(*path))
                .cloned()
                .collect::<BTreeSet<_>>();
            let added_affected_paths = document_paths
                .iter()
                .filter(|path| !stored_affected_paths.contains(*path))
                .cloned()
                .collect::<BTreeSet<_>>();
            let mut affected_targets = documents
                .iter()
                .flat_map(|document| document.links.iter().cloned())
                .collect::<BTreeSet<_>>();
            for path in &removed_affected_paths {
                let mut rows = connection
                    .query(
                        "SELECT DISTINCT target FROM wiki_links WHERE target_path = ?",
                        [path.as_str()],
                    )
                    .await?;
                while let Some(row) = rows.next().await? {
                    affected_targets.insert(row.get::<String>(0)?);
                }
            }
            for relative in &added_affected_paths {
                collect_matching_targets(
                    &connection,
                    link_target_candidates(Path::new(relative)),
                    &mut affected_targets,
                )
                .await?;
            }

            for relative in removed_paths
                .iter()
                .chain(changed_paths.difference(&document_paths))
            {
                connection
                    .execute("DELETE FROM documents WHERE path = ?", [relative.as_str()])
                    .await?;
            }
            for document in &documents {
                let relative = path_text(&document.path);
                let metadata_json = document
                    .metadata
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()?;
                connection
                    .execute(
                        "INSERT INTO documents(path, extension, folder, size_bytes, modified_ns, metadata) \
                         VALUES (?, ?, ?, ?, ?, CASE WHEN ? IS NULL THEN NULL ELSE jsonb(?) END) \
                         ON CONFLICT(path) DO
                            UPDATE SET
                                extension=excluded.extension,
                                folder=excluded.folder, \
                                size_bytes=excluded.size_bytes,
                                modified_ns=excluded.modified_ns,
                                metadata=excluded.metadata",
                        turso::params![
                            relative.clone(),
                            document.extension.clone(),
                            document.folder.clone(),
                            document.size_bytes,
                            document.modified_ns,
                            metadata_json.clone(),
                            metadata_json,
                        ],
                    )
                    .await?;
                connection
                    .execute(
                        "DELETE FROM wiki_links WHERE source_path = ?",
                        [relative.clone()],
                    )
                    .await?;
                for (ordinal, target) in document.links.iter().enumerate() {
                    connection
                        .execute(
                            "INSERT INTO wiki_links(source_path, ordinal, target, target_path) VALUES (?, ?, ?, NULL)",
                            turso::params![relative.clone(), ordinal as i64, target.clone()],
                        )
                        .await?;
                }
            }

            resolve_links(&connection, affected_targets).await?;

            Ok::<_, anyhow::Error>(())
        }
        .await;

        match result {
            Ok(()) => {
                connection.execute("COMMIT", ()).await?;
                let synchronized = documents
                    .iter()
                    .map(|document| self.root.join(&document.path))
                    .collect::<Vec<_>>();
                Ok(synchronized)
            }
            Err(error) => {
                let _ = connection.execute("ROLLBACK", ()).await;
                Err(error)
            }
        }
    }

    pub(crate) async fn query_documents(&self, query: CatalogQuery) -> Result<DocumentSelection> {
        let connection = self.connection().await?;
        self.query_documents_on(&connection, query).await
    }

    pub(crate) async fn query_documents_with_links(
        &self,
        query: CatalogQuery,
    ) -> Result<(DocumentSelection, Vec<StoredLink>)> {
        let connection = self.connection().await?;
        connection.execute("BEGIN DEFERRED", ()).await?;
        let result = async {
            let selection = self.query_documents_on(&connection, query).await?;
            let selected: BTreeSet<_> = selection
                .documents
                .iter()
                .map(|document| document.path.clone())
                .collect();
            let mut rows = connection
                .query(
                    "SELECT DISTINCT source_path, target_path \
                     FROM wiki_links \
                     WHERE target_path IS NOT NULL \
                     ORDER BY source_path, target_path",
                    (),
                )
                .await?;
            let mut links = Vec::new();
            while let Some(row) = rows.next().await? {
                let source = self.root.join(row.get::<String>(0)?);
                let target = self.root.join(row.get::<String>(1)?);
                if selected.contains(&source) && selected.contains(&target) {
                    links.push(StoredLink { source, target });
                }
            }
            Ok::<_, anyhow::Error>((selection, links))
        }
        .await;
        let _ = connection.execute("ROLLBACK", ()).await;
        result
    }

    async fn query_documents_on(
        &self,
        connection: &turso::Connection,
        query: CatalogQuery,
    ) -> Result<DocumentSelection> {
        let mut compiler = FilterCompiler::default();
        let filter = compiler.compile(&query.filter);
        let mut clauses = Vec::new();
        if filter != "1" {
            clauses.push(filter);
        }
        if let Some(extension) = query.extension {
            clauses.push("extension = ?".to_string());
            compiler.parameters.push(Value::Text(extension));
        }
        let where_clause = if clauses.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", clauses.join(" AND "))
        };
        let sql = format!(
            "SELECT path, json(metadata) FROM documents{where_clause} ORDER BY path LIMIT ?"
        );
        compiler
            .parameters
            .push(Value::Integer(query.limit.saturating_add(1) as i64));
        let mut rows = connection
            .query(sql, turso::params_from_iter(compiler.parameters))
            .await?;
        let mut documents = Vec::new();
        while let Some(row) = rows.next().await? {
            let metadata = match row.get_value(1)? {
                Value::Null => None,
                Value::Text(json) => Some(serde_json::from_str(&json)?),
                value => bail!("Unexpected metadata value {value:?}"),
            };
            documents.push(CatalogDocument {
                path: self.root.join(row.get::<String>(0)?),
                metadata,
            });
        }
        let exceeded_limit = documents.len() > query.limit;
        if exceeded_limit {
            documents.truncate(query.limit);
        }
        Ok(DocumentSelection {
            documents,
            exceeded_limit,
        })
    }
}
