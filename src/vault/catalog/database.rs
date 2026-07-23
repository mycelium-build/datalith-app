use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use turso::{Builder, Database, Value};

use super::{
    CatalogComparison, CatalogDocument, CatalogFileField, CatalogFilter, CatalogProperty,
    CatalogQuery, CatalogScalar, DocumentSelection,
};
use crate::document::file_types::RegisteredFileTypes;
use crate::vault::links;

const SCHEMA_VERSION: i64 = 3;
const FILE_NAME_SQL: &str = "substr(path, \
    CASE WHEN folder = '' THEN 1 ELSE length(folder) + 2 END, \
    length(path) - CASE WHEN folder = '' THEN 1 ELSE length(folder) + 2 END - length(extension))";
const FILE_BASENAME_SQL: &str =
    "substr(path, CASE WHEN folder = '' THEN 1 ELSE length(folder) + 2 END)";
const PATH_WITHOUT_EXTENSION_SQL: &str = "substr(path, 1, length(path) - length(extension) - 1)";

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct StoredLink {
    pub(super) source: PathBuf,
    pub(super) target: PathBuf,
}

#[derive(Clone, Debug)]
pub(crate) struct Backlink {
    pub(crate) source: PathBuf,
    pub(crate) ordinal: usize,
    pub(crate) authored_target: String,
    pub(crate) target_path: PathBuf,
}

pub(super) struct TrackedDocument {
    path: PathBuf,
    extension: String,
    folder: String,
    size_bytes: i64,
    modified_ns: i64,
    metadata: Option<serde_json::Value>,
    links: Vec<String>,
}

impl TrackedDocument {
    fn read(root: &Path, path: &Path, file_types: &RegisteredFileTypes) -> Option<Self> {
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

#[derive(Clone)]
pub(super) struct CatalogDatabase {
    root: PathBuf,
    database: Database,
}

impl CatalogDatabase {
    pub(super) async fn open(root: &Path) -> Result<Self> {
        let metadata_dir = root.join(".datalith");
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
        let database = match Builder::new_local(database_path_text).build().await {
            Ok(database) => database,
            Err(_) => {
                let _ = fs::remove_file(&database_path);
                let _ = fs::remove_file(database_path.with_extension("db-wal"));
                let _ = fs::remove_file(database_path.with_extension("db-shm"));
                Builder::new_local(database_path_text)
                    .build()
                    .await
                    .context("Failed to rebuild embedded Turso catalog")?
            }
        };
        let this = Self {
            root: root.to_path_buf(),
            database,
        };
        this.initialize_schema().await?;
        Ok(this)
    }

    async fn connection(&self) -> Result<turso::Connection> {
        let connection = self.database.connect()?;
        connection.execute("PRAGMA foreign_keys = ON", ()).await?;
        Ok(connection)
    }

    pub(super) async fn stored_paths(&self) -> Result<Vec<PathBuf>> {
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

    pub(super) async fn resolve_path(&self, authored: &str) -> Result<Option<PathBuf>> {
        let connection = self.connection().await?;
        Ok(resolve_path_on(&connection, authored)
            .await?
            .map(|path| self.root.join(path)))
    }

    pub(super) async fn synchronize(
        &self,
        removed: &[PathBuf],
        changed: &[PathBuf],
        file_types: &RegisteredFileTypes,
    ) -> Result<Vec<PathBuf>> {
        let documents = changed
            .iter()
            .filter_map(|path| TrackedDocument::read(&self.root, path, file_types))
            .collect::<Vec<_>>();
        let synchronized = documents
            .iter()
            .map(|document| self.root.join(&document.path))
            .collect::<Vec<_>>();
        let connection = self.connection().await?;
        connection.execute("BEGIN IMMEDIATE", ()).await?;
        let result = async {
            for path in removed.iter().chain(changed) {
                if let Ok(relative) = path.strip_prefix(&self.root) {
                    connection
                        .execute(
                            "DELETE FROM documents WHERE path = ?",
                            [relative.to_string_lossy().replace('\\', "/")],
                        )
                        .await?;
                }
            }
            for document in &documents {
                let relative = document.path.to_string_lossy().replace('\\', "/");
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
                for (ordinal, target) in document.links.iter().enumerate() {
                    connection
                        .execute(
                            "INSERT INTO wiki_links(source_path, ordinal, target, target_path) VALUES (?, ?, ?, NULL)",
                            turso::params![relative.clone(), ordinal as i64, target.clone()],
                        )
                        .await?;
                }
            }
            resolve_all_links(&connection).await?;
            Ok::<_, anyhow::Error>(())
        }
        .await;
        match result {
            Ok(()) => {
                connection.execute("COMMIT", ()).await?;
                Ok(synchronized)
            }
            Err(error) => {
                let _ = connection.execute("ROLLBACK", ()).await;
                Err(error)
            }
        }
    }

    pub(super) async fn backlinks_under(&self, target: &Path) -> Result<Vec<Backlink>> {
        let relative_target = target
            .strip_prefix(&self.root)
            .context("Rename target is outside the Vault")?;
        let relative_target = relative_target.to_string_lossy().replace('\\', "/");
        let descendant_pattern = format!("{}/%", escape_like_pattern(&relative_target));
        let connection = self.connection().await?;
        let mut rows = connection
            .query(
                "SELECT source_path, ordinal, target, target_path \
                 FROM wiki_links \
                 WHERE target_path = ? \
                    OR target_path LIKE ? ESCAPE '\\' \
                 ORDER BY source_path, ordinal",
                turso::params![relative_target, descendant_pattern],
            )
            .await?;
        let mut backlinks = Vec::new();
        while let Some(row) = rows.next().await? {
            backlinks.push(Backlink {
                source: PathBuf::from(row.get::<String>(0)?),
                ordinal: row.get::<i64>(1)? as usize,
                authored_target: row.get::<String>(2)?,
                target_path: PathBuf::from(row.get::<String>(3)?),
            });
        }
        Ok(backlinks)
    }

    async fn initialize_schema(&self) -> Result<()> {
        let connection = self.connection().await?;
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
                r#"
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
                CREATE INDEX IF NOT EXISTS wiki_links_target_idx ON wiki_links(target);
                CREATE INDEX IF NOT EXISTS wiki_links_target_path_idx ON wiki_links(target_path);
                PRAGMA user_version = 3;
                "#,
            )
            .await?;
        Ok(())
    }

    pub(super) async fn query_documents(&self, query: CatalogQuery) -> Result<DocumentSelection> {
        let connection = self.connection().await?;
        self.query_documents_on(&connection, query).await
    }

    pub(super) async fn query_documents_with_links(
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

fn escape_like_pattern(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

async fn resolve_path_on(
    connection: &turso::Connection,
    authored: &str,
) -> Result<Option<PathBuf>> {
    let target = links::normalized_target(authored);
    if target.is_empty() {
        return Ok(None);
    }
    let qualified = target.contains('/');
    let target_has_extension = Path::new(&target).extension().is_some();
    let compared_field = match (qualified, target_has_extension) {
        (true, true) => "path",
        (true, false) => PATH_WITHOUT_EXTENSION_SQL,
        (false, true) => FILE_BASENAME_SQL,
        (false, false) => FILE_NAME_SQL,
    };
    let sql = format!(
        "SELECT path FROM documents \
         WHERE lower({compared_field}) = lower(?) \
         ORDER BY \
            length(path) - length(replace(path, '/', '')) ASC, \
            CASE WHEN lower(extension) = 'md' THEN 0 ELSE 1 END ASC, \
            lower(path) ASC, \
            path ASC \
         LIMIT 1"
    );
    let mut rows = connection.query(sql, [target]).await?;
    rows.next()
        .await?
        .map(|row| row.get::<String>(0).map(PathBuf::from))
        .transpose()
        .map_err(Into::into)
}

async fn resolve_all_links(connection: &turso::Connection) -> Result<()> {
    let targets = {
        let mut rows = connection
            .query("SELECT DISTINCT target FROM wiki_links", ())
            .await?;
        let mut targets = Vec::new();
        while let Some(row) = rows.next().await? {
            targets.push(row.get::<String>(0)?);
        }
        targets
    };
    let mut resolutions = Vec::with_capacity(targets.len());
    for target in targets {
        let resolved = resolve_path_on(connection, &target).await?;
        resolutions.push((target, resolved));
    }
    connection
        .execute("UPDATE wiki_links SET target_path = NULL", ())
        .await?;
    for (authored_target, resolved_path) in resolutions {
        if let Some(resolved_path) = resolved_path {
            connection
                .execute(
                    "UPDATE wiki_links SET target_path = ? WHERE target = ?",
                    turso::params![
                        resolved_path.to_string_lossy().replace('\\', "/"),
                        authored_target
                    ],
                )
                .await?;
        }
    }
    Ok(())
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

#[derive(Default)]
struct FilterCompiler {
    parameters: Vec<Value>,
}

impl FilterCompiler {
    fn parameter(&mut self, value: Value) -> &'static str {
        self.parameters.push(value);
        "?"
    }

    fn compile(&mut self, filter: &CatalogFilter) -> String {
        match filter {
            CatalogFilter::MatchAll => "1".into(),
            CatalogFilter::And(filters) => self.join(filters, "AND", "1"),
            CatalogFilter::Or(filters) => self.join(filters, "OR", "0"),
            CatalogFilter::Not(filter) => format!("({}) = 0", self.compile(filter)),
            CatalogFilter::InFolder(folder) => {
                self.parameters.push(Value::Text(folder.clone()));
                self.parameters.push(Value::Text(format!("{folder}/%")));
                "(folder = ? OR folder LIKE ?)".into()
            }
            CatalogFilter::Contains { property, value } => {
                let CatalogProperty::Metadata(parts) = property else {
                    return "0".into();
                };
                let path = json_path(parts);
                self.parameters.push(Value::Text(path.clone()));
                self.parameters.push(Value::Text(path));
                let value_clause = self.each_value_clause(value);
                format!(
                    "(json_type(metadata, ?) = 'array' AND EXISTS \
                     (SELECT 1 FROM json_each(metadata, ?) AS each \
                      WHERE {value_clause}))"
                )
            }
            CatalogFilter::Compare {
                property,
                comparison,
                value,
            } => self.compare(property, *comparison, value),
        }
    }

    fn join(&mut self, filters: &[CatalogFilter], operator: &str, empty: &str) -> String {
        if filters.is_empty() {
            return empty.into();
        }
        let values = filters
            .iter()
            .map(|filter| format!("({})", self.compile(filter)))
            .collect::<Vec<_>>();
        values.join(&format!(" {operator} "))
    }

    fn compare(
        &mut self,
        property: &CatalogProperty,
        comparison: CatalogComparison,
        value: &CatalogScalar,
    ) -> String {
        match property {
            CatalogProperty::File(field) => {
                let column = match field {
                    CatalogFileField::Name => FILE_NAME_SQL,
                    CatalogFileField::Extension => "extension",
                    CatalogFileField::Path => "path",
                    CatalogFileField::Folder => "folder",
                };
                self.file_compare(column, comparison, value)
            }
            CatalogProperty::Metadata(parts) => {
                let path = json_path(parts);
                match (comparison, value) {
                    (CatalogComparison::Equal, CatalogScalar::Null) => {
                        self.push_path(&path, 2);
                        "(json_type(metadata, ?) IS NULL OR json_type(metadata, ?) = 'null')".into()
                    }
                    (CatalogComparison::NotEqual, CatalogScalar::Null) => {
                        self.push_path(&path, 2);
                        "(json_type(metadata, ?) IS NOT NULL AND json_type(metadata, ?) != 'null')"
                            .into()
                    }
                    (CatalogComparison::NotEqual, value) => {
                        self.push_path(&path, 3);
                        let Some(type_clause) = json_type_clause(value) else {
                            return "1".into();
                        };
                        self.scalar_parameter(value);
                        format!(
                            "(json_type(metadata, ?) IS NULL OR NOT \
                             ({type_clause} AND json_extract(metadata, ?) = ?))"
                        )
                    }
                    (CatalogComparison::Equal, value) => {
                        self.push_path(&path, 2);
                        let Some(type_clause) = json_type_clause(value) else {
                            return "0".into();
                        };
                        self.scalar_parameter(value);
                        format!("({type_clause} AND json_extract(metadata, ?) = ?)")
                    }
                    (comparison, CatalogScalar::Number(value)) => {
                        self.push_path(&path, 2);
                        self.parameters.push(Value::Real(*value));
                        format!(
                            "(json_type(metadata, ?) IN ('integer', 'real') AND \
                             json_extract(metadata, ?) {} ?)",
                            comparison_sql(comparison)
                        )
                    }
                    _ => "0".into(),
                }
            }
        }
    }

    fn file_compare(
        &mut self,
        column: &str,
        comparison: CatalogComparison,
        value: &CatalogScalar,
    ) -> String {
        let CatalogScalar::String(value) = value else {
            return match (comparison, value) {
                (CatalogComparison::NotEqual, CatalogScalar::Null) => "1".into(),
                _ => "0".into(),
            };
        };
        self.parameters.push(Value::Text(value.clone()));
        format!("{column} {} ?", comparison_sql(comparison))
    }

    fn scalar_parameter(&mut self, scalar: &CatalogScalar) -> &'static str {
        let value = match scalar {
            CatalogScalar::Null => Value::Null,
            CatalogScalar::Bool(value) => Value::Integer(i64::from(*value)),
            CatalogScalar::Number(value) => Value::Real(*value),
            CatalogScalar::String(value) => Value::Text(value.clone()),
        };
        self.parameter(value)
    }

    fn push_path(&mut self, path: &str, count: usize) {
        self.parameters
            .extend((0..count).map(|_| Value::Text(path.to_string())));
    }

    fn each_value_clause(&mut self, scalar: &CatalogScalar) -> String {
        match scalar {
            CatalogScalar::Null => "each.type = 'null'".into(),
            CatalogScalar::Bool(value) => {
                let kind = if *value { "true" } else { "false" };
                format!("each.type = '{kind}'")
            }
            CatalogScalar::Number(value) => {
                self.parameters.push(Value::Real(*value));
                "each.type IN ('integer', 'real') AND each.atom = ?".into()
            }
            CatalogScalar::String(value) => {
                self.parameters.push(Value::Text(value.clone()));
                "each.type = 'text' AND each.atom = ?".into()
            }
        }
    }
}

fn json_type_clause(value: &CatalogScalar) -> Option<&'static str> {
    match value {
        CatalogScalar::Null => None,
        CatalogScalar::Bool(true) => Some("json_type(metadata, ?) = 'true'"),
        CatalogScalar::Bool(false) => Some("json_type(metadata, ?) = 'false'"),
        CatalogScalar::Number(_) => Some("json_type(metadata, ?) IN ('integer', 'real')"),
        CatalogScalar::String(_) => Some("json_type(metadata, ?) = 'text'"),
    }
}

fn comparison_sql(comparison: CatalogComparison) -> &'static str {
    match comparison {
        CatalogComparison::Equal => "=",
        CatalogComparison::NotEqual => "!=",
        CatalogComparison::Greater => ">",
        CatalogComparison::GreaterEqual => ">=",
        CatalogComparison::Less => "<",
        CatalogComparison::LessEqual => "<=",
    }
}

fn json_path(parts: &[String]) -> String {
    parts.iter().fold("$".to_string(), |mut path, part| {
        path.push('.');
        path.push_str(&serde_json::to_string(part).unwrap_or_else(|_| "\"\"".into()));
        path
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queries_and_filters_typed_document_properties() {
        let root =
            std::env::temp_dir().join(format!("datalith-catalog-query-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        pollster::block_on(async {
            let database = CatalogDatabase::open(&root).await.unwrap();
            database.connection().await.unwrap().execute(
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
    fn resolves_links_from_catalogued_paths_using_ambiguity_order() {
        let root =
            std::env::temp_dir().join(format!("datalith-catalog-resolve-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        pollster::block_on(async {
            let database = CatalogDatabase::open(&root).await.unwrap();
            let connection = database.connection().await.unwrap();
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
            let connection = database.connection().await.unwrap();
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
}
