use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use rayon::prelude::*;
use turso::Value;

use super::filter_compiler::FilterCompiler;
use super::link_resolution::{collect_matching_targets, link_target_candidates, resolve_links};
use super::{CatalogDatabase, StoredLink, SynchronizedFiles, path_text};
use crate::document::file_types::RegisteredFileTypes;
use crate::vault::catalog::{CatalogDocument, CatalogQuery, DocumentSelection};
use crate::vault::links;

const BATCH_SIZE: usize = 64;

pub(super) struct TrackedDocument {
    pub(super) path: PathBuf,
    pub(super) extension: String,
    pub(super) folder: String,
    pub(super) size_bytes: i64,
    pub(super) modified_ns: i64,
    pub(super) metadata: Option<serde_json::Value>,
    pub(super) links: Vec<String>,
}

#[derive(Clone, Copy)]
struct StoredMeta {
    size_bytes: i64,
    modified_ns: i64,
}

impl TrackedDocument {
    pub(super) fn read(root: &Path, path: &Path, file_types: &RegisteredFileTypes) -> Option<Self> {
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
            .map(|duration| i64::try_from(duration.as_nanos()).unwrap_or(i64::MAX))
            .unwrap_or_default();
        Some(Self {
            path: relative,
            extension,
            folder,
            size_bytes: i64::try_from(metadata.len()).unwrap_or(i64::MAX),
            modified_ns,
            metadata: document_metadata,
            links: document_links,
        })
    }

    /// Read only filesystem metadata (size, mtime) without file content.
    pub(super) fn read_meta_only(
        path: &Path,
        file_types: &RegisteredFileTypes,
    ) -> Option<(i64, i64)> {
        file_types.capabilities(path)?;
        let metadata = fs::metadata(path).ok()?;
        let modified_ns = metadata
            .modified()
            .ok()
            .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| i64::try_from(duration.as_nanos()).unwrap_or(i64::MAX))
            .unwrap_or_default();
        let size_bytes = i64::try_from(metadata.len()).unwrap_or(i64::MAX);
        Some((size_bytes, modified_ns))
    }
}

fn frontmatter_metadata(content: &str) -> Option<serde_json::Value> {
    let normalized = content.replace("\r\n", "\n");
    let rest = normalized.strip_prefix("---\n")?;
    let closing = rest.find("\n---")?;
    let yaml = rest.get(..closing)?;
    yaml_serde::from_str::<serde_json::Value>(yaml)
        .ok()
        .filter(serde_json::Value::is_object)
}

impl CatalogDatabase {
    pub(crate) async fn stored_paths(&self) -> Result<Vec<PathBuf>> {
        let connection = self.read_connection()?;
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
    ) -> Result<SynchronizedFiles> {
        let root = &self.root;
        let connection = self.connection();
        connection.execute("BEGIN IMMEDIATE", ()).await?;
        let result = async {
            // load stored meta data
            let stored_meta = load_stored_meta(&connection).await?;

            // compare stored metadata with actual file metadata
            let (truly_changed, unchanged) =
                partition_changed(changed, root, &stored_meta, file_types);

            // read full content for truly changed files
            let mut documents: Vec<TrackedDocument> = truly_changed
                .par_iter()
                .filter_map(|path| TrackedDocument::read(root, path, file_types))
                .collect();
            let changed_document_count = documents.len();

            // for unchanged files, reconstruct TrackedDocument from stored metadata
            documents.extend(reconstruct_unchanged(root, &unchanged, &stored_meta));

            let document_paths = documents
                .iter()
                .map(|document| path_text(&document.path))
                .collect::<BTreeSet<_>>();
            let changed_paths = changed
                .iter()
                .filter_map(|path| path.strip_prefix(root).ok())
                .map(path_text)
                .collect::<BTreeSet<_>>();
            let removed_paths = removed
                .iter()
                .filter_map(|path| path.strip_prefix(root).ok())
                .map(path_text)
                .collect::<BTreeSet<_>>();

            let stored_affected_paths: BTreeSet<String> = changed_paths
                .iter()
                .chain(&removed_paths)
                .filter(|p| stored_meta.contains_key(p.as_str()))
                .cloned()
                .collect();

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

            // batch delete removed/changed documents
            let to_delete: Vec<&str> = removed_paths
                .iter()
                .chain(changed_paths.difference(&document_paths))
                .map(String::as_str)
                .collect();
            batch_delete_documents(&connection, &to_delete).await?;

            let (changed_documents, _) = documents.split_at(changed_document_count);
            upsert_documents_and_links(&connection, changed_documents).await?;

            resolve_links(&connection, affected_targets).await?;

            Ok::<_, anyhow::Error>(synchronized_paths(&documents, changed_document_count, root))
        }
        .await;

        match result {
            Ok(result) => {
                connection.execute("COMMIT", ()).await?;
                Ok(result)
            }
            Err(error) => {
                let _ = connection.execute("ROLLBACK", ()).await;
                Err(error)
            }
        }
    }

    pub(crate) async fn query_documents(&self, query: CatalogQuery) -> Result<DocumentSelection> {
        let connection = self.read_connection()?;
        self.query_documents_on(&connection, query).await
    }

    pub(crate) async fn query_documents_with_outgoing_links(
        &self,
        query: CatalogQuery,
    ) -> Result<(DocumentSelection, Vec<StoredLink>)> {
        let connection = self.read_connection()?;
        connection.execute("BEGIN DEFERRED", ()).await?;
        let result = async {
            let selection = self.query_documents_on(&connection, query).await?;
            let selected_paths: Vec<String> = selection
                .documents
                .iter()
                .map(|document| {
                    document
                        .path
                        .strip_prefix(&self.root)
                        .unwrap_or(&document.path)
                        .to_string_lossy()
                        .replace('\\', "/")
                })
                .collect();
            if selected_paths.is_empty() {
                return Ok::<_, anyhow::Error>((selection, Vec::new()));
            }
            let placeholders = selected_paths
                .iter()
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(",");
            let sql = format!(
                "SELECT source_path, target_path FROM wiki_links \
                 WHERE target_path IS NOT NULL AND source_path IN ({placeholders}) \
                 ORDER BY source_path, target_path"
            );
            let params = selected_paths
                .into_iter()
                .map(Value::Text)
                .collect::<Vec<_>>();
            let mut rows = connection
                .query(sql, turso::params_from_iter(params))
                .await?;
            let mut links = Vec::new();
            while let Some(row) = rows.next().await? {
                links.push(StoredLink {
                    source: self.root.join(row.get::<String>(0)?),
                    target: self.root.join(row.get::<String>(1)?),
                });
            }
            Ok((selection, links))
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
        let (sql, limit_param) = query.limit.map_or_else(
            || {
                (
                    format!(
                        "SELECT path, json(metadata), size_bytes, modified_ns \
                         FROM documents{where_clause} ORDER BY path"
                    ),
                    None,
                )
            },
            |limit| {
                (
                    format!(
                        "SELECT path, json(metadata), size_bytes, modified_ns \
                         FROM documents{where_clause} ORDER BY path LIMIT ?"
                    ),
                    Some(Value::Integer(
                        i64::try_from(limit.saturating_add(1)).unwrap_or(i64::MAX),
                    )),
                )
            },
        );
        if let Some(limit_param) = limit_param {
            compiler.parameters.push(limit_param);
        }
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
                size_bytes: row.get(2)?,
                modified_ns: row.get(3)?,
                links: Vec::new(),
            });
        }
        if let Some(limit) = query.limit {
            documents.truncate(limit);
        }
        Ok(DocumentSelection { documents })
    }
}

async fn load_stored_meta(connection: &turso::Connection) -> Result<HashMap<String, StoredMeta>> {
    let mut stored_rows = connection
        .query("SELECT path, size_bytes, modified_ns FROM documents", ())
        .await?;
    let mut map = HashMap::new();
    while let Some(row) = stored_rows.next().await? {
        map.insert(
            row.get::<String>(0)?,
            StoredMeta {
                size_bytes: row.get::<i64>(1)?,
                modified_ns: row.get::<i64>(2)?,
            },
        );
    }
    Ok(map)
}

fn partition_changed(
    changed: &[PathBuf],
    root: &Path,
    stored_meta: &HashMap<String, StoredMeta>,
    file_types: &RegisteredFileTypes,
) -> (Vec<PathBuf>, Vec<PathBuf>) {
    changed
        .par_iter()
        .filter(|path| path.strip_prefix(root).is_ok())
        .cloned()
        .partition(|path| {
            let relative = path.strip_prefix(root).unwrap_or(path);
            let relative_str = relative.to_string_lossy().replace('\\', "/");
            stored_meta.get(relative_str.as_str()).is_none_or(|stored| {
                TrackedDocument::read_meta_only(path, file_types).is_none_or(|(size, mtime)| {
                    size != stored.size_bytes || mtime != stored.modified_ns
                })
            })
        })
}

fn reconstruct_unchanged(
    root: &Path,
    unchanged: &[PathBuf],
    stored_meta: &HashMap<String, StoredMeta>,
) -> Vec<TrackedDocument> {
    let mut documents = Vec::new();
    for path in unchanged {
        let Some(relative) = path.strip_prefix(root).ok() else {
            continue;
        };
        let relative_str = relative.to_string_lossy().replace('\\', "/");
        let Some(stored) = stored_meta.get(relative_str.as_str()) else {
            continue;
        };
        let extension = relative
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        let folder = relative
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        documents.push(TrackedDocument {
            path: relative.to_path_buf(),
            extension,
            folder,
            size_bytes: stored.size_bytes,
            modified_ns: stored.modified_ns,
            metadata: None, // will be overwritten by existing DB row
            links: Vec::new(),
        });
    }
    documents
}

fn placeholders(count: usize) -> String {
    (1..=count)
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn synchronized_paths(
    documents: &[TrackedDocument],
    changed_count: usize,
    root: &Path,
) -> SynchronizedFiles {
    let (changed_documents, _) = documents.split_at(changed_count);
    let all = documents
        .iter()
        .map(|document| root.join(&document.path))
        .collect();
    let changed = changed_documents
        .iter()
        .map(|document| root.join(&document.path))
        .collect();
    SynchronizedFiles { all, changed }
}

async fn batch_delete_documents(connection: &turso::Connection, to_delete: &[&str]) -> Result<()> {
    for chunk in to_delete.chunks(BATCH_SIZE) {
        let placeholders = placeholders(chunk.len());
        let sql = format!("DELETE FROM documents WHERE path IN ({placeholders})");
        let params: Vec<Value> = chunk.iter().map(|p| Value::Text(p.to_string())).collect();
        connection
            .execute(sql, turso::params_from_iter(params))
            .await?;
    }
    Ok(())
}

async fn upsert_documents_and_links(
    connection: &turso::Connection,
    documents: &[TrackedDocument],
) -> Result<()> {
    for chunk in documents.chunks(BATCH_SIZE) {
        // batch document upserts
        let mut value_groups = Vec::new();
        let mut params: Vec<Value> = Vec::new();
        for document in chunk {
            let relative = path_text(&document.path);
            let metadata_json = document
                .metadata
                .as_ref()
                .map(serde_json::to_string)
                .transpose()?;
            // metadata is optional: bind it twice
            // - once for the NULL test,
            // - once for the jsonb() conversion
            // so a single NULL maps to a NULL column value.
            value_groups.push("(?, ?, ?, ?, ?, CASE WHEN ? IS NULL THEN NULL ELSE jsonb(?) END)");
            params.push(Value::Text(relative));
            params.push(Value::Text(document.extension.clone()));
            params.push(Value::Text(document.folder.clone()));
            params.push(Value::Integer(document.size_bytes));
            params.push(Value::Integer(document.modified_ns));
            params.push(metadata_json.clone().map_or(Value::Null, Value::Text));
            params.push(metadata_json.map_or(Value::Null, Value::Text));
        }
        let sql = format!(
            "INSERT INTO documents(path, extension, folder, size_bytes, modified_ns, metadata) \
             VALUES {} \
             ON CONFLICT(path) DO UPDATE SET \
                extension=excluded.extension, \
                folder=excluded.folder, \
                size_bytes=excluded.size_bytes, \
                modified_ns=excluded.modified_ns, \
                metadata=excluded.metadata",
            value_groups.join(", ")
        );
        connection
            .execute(sql, turso::params_from_iter(params))
            .await?;

        // batch delete old links for this chunk
        let chunk_paths: Vec<String> = chunk.iter().map(|d| path_text(&d.path)).collect();
        for delete_chunk in chunk_paths.chunks(BATCH_SIZE) {
            let placeholders = placeholders(delete_chunk.len());
            let sql = format!("DELETE FROM wiki_links WHERE source_path IN ({placeholders})");
            let params: Vec<Value> = delete_chunk
                .iter()
                .map(|p| Value::Text(p.clone()))
                .collect();
            connection
                .execute(sql, turso::params_from_iter(params))
                .await?;
        }

        // batch wiki link inserts
        let all_links: Vec<(String, i64, String)> = chunk
            .iter()
            .flat_map(|document| {
                let relative = path_text(&document.path);
                document
                    .links
                    .iter()
                    .enumerate()
                    .map(move |(ordinal, target)| {
                        (
                            relative.clone(),
                            i64::try_from(ordinal).unwrap_or(i64::MAX),
                            target.clone(),
                        )
                    })
            })
            .collect();
        for link_chunk in all_links.chunks(BATCH_SIZE) {
            let mut link_values = Vec::new();
            let mut link_params: Vec<Value> = Vec::new();
            for (source, ordinal, target) in link_chunk {
                link_values.push("(?, ?, ?, NULL)");
                link_params.push(Value::Text(source.clone()));
                link_params.push(Value::Integer(*ordinal));
                link_params.push(Value::Text(target.clone()));
            }
            let link_sql = format!(
                "INSERT INTO wiki_links(source_path, ordinal, target, target_path) VALUES {}",
                link_values.join(", ")
            );
            connection
                .execute(link_sql, turso::params_from_iter(link_params))
                .await?;
        }
    }
    Ok(())
}
