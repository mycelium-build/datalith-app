use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Map, Value as JsonValue};
use turso::{Builder, Database, Value, params};
use yaml_serde::Value as YamlValue;

use super::links::extract_wiki_link_occurrences;
use super::{
    CatalogComparison, CatalogDocument, CatalogFileField, CatalogFilter, CatalogProperty,
    CatalogQuery, CatalogScalar, DocumentSelection,
};
use crate::document::file_types::RegisteredFileTypes;

const SCHEMA_VERSION: i64 = 2;
const FILE_NAME_SQL: &str = "substr(path, \
    CASE WHEN folder = '' THEN 1 ELSE length(folder) + 2 END, \
    length(path) - CASE WHEN folder = '' THEN 1 ELSE length(folder) + 2 END - length(extension))";

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct StoredLink {
    pub(super) source: PathBuf,
    pub(super) target: PathBuf,
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
                    source_path TEXT NOT NULL REFERENCES documents(path) ON DELETE CASCADE,
                    ordinal     INTEGER NOT NULL,
                    target      TEXT NOT NULL,
                    target_path TEXT REFERENCES documents(path) ON DELETE SET NULL,
                    PRIMARY KEY (source_path, ordinal)
                );
                CREATE INDEX IF NOT EXISTS wiki_links_target_idx ON wiki_links(target);
                CREATE INDEX IF NOT EXISTS wiki_links_target_path_idx ON wiki_links(target_path);
                PRAGMA user_version = 2;
                "#,
            )
            .await?;
        Ok(())
    }

    pub(super) async fn reconcile(
        &self,
        paths: &BTreeSet<PathBuf>,
        file_types: &RegisteredFileTypes,
    ) -> Result<Vec<PathBuf>> {
        let existing = self.document_versions().await?;
        let mut changed = Vec::new();
        let mut first_error = None;
        for path in paths {
            let relative = relative_vault_path(&self.root, path)?;
            let version = file_version(path)?;
            if existing.get(&relative) == Some(&version) {
                continue;
            }
            match self.upsert_file(path, file_types).await {
                Ok(()) => changed.push(path.clone()),
                Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
        }
        for relative in existing.keys() {
            let absolute = self.root.join(relative);
            if !paths.contains(&absolute) {
                self.delete_document(relative).await?;
                changed.push(absolute);
            }
        }
        self.update_all_links().await?;
        if let Some(error) = first_error {
            Err(error)
        } else {
            Ok(changed)
        }
    }

    async fn document_versions(&self) -> Result<HashMap<String, (u64, i64)>> {
        let connection = self.connection().await?;
        let mut rows = connection
            .query("SELECT path, size_bytes, modified_ns FROM documents", ())
            .await?;
        let mut versions = HashMap::new();
        while let Some(row) = rows.next().await? {
            versions.insert(
                row.get::<String>(0)?,
                (row.get::<i64>(1)? as u64, row.get::<i64>(2)?),
            );
        }
        Ok(versions)
    }

    pub(super) async fn upsert_file(
        &self,
        path: &Path,
        file_types: &RegisteredFileTypes,
    ) -> Result<()> {
        let capabilities = file_types
            .capabilities(path)
            .ok_or_else(|| anyhow!("File extension is not registered: {}", path.display()))?;
        let relative = relative_vault_path(&self.root, path)?;
        let (size_bytes, modified_ns) = file_version(path)?;
        let extension = path
            .extension()
            .and_then(|v| v.to_str())
            .unwrap_or_default();
        let folder = Path::new(&relative)
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_string_lossy()
            .replace('\\', "/");
        let source = (capabilities.yaml_frontmatter || capabilities.wiki_links)
            .then(|| fs::read_to_string(path))
            .transpose()
            .with_context(|| format!("Failed to read {} for cataloging", path.display()))?;
        let metadata = if capabilities.yaml_frontmatter {
            source.as_deref().and_then(metadata_from_markdown)
        } else {
            None
        };
        let metadata_text = metadata.as_ref().map(serde_json::to_string).transpose()?;

        let connection = self.connection().await?;
        connection.execute("BEGIN IMMEDIATE", ()).await?;
        let result = async {
            connection
                .execute(
                    r#"INSERT INTO documents
                       (path, extension, folder, size_bytes, modified_ns, metadata)
                       VALUES (?1, ?2, ?3, ?4, ?5,
                               CASE WHEN ?6 IS NULL THEN NULL ELSE jsonb(?6) END)
                       ON CONFLICT(path) DO UPDATE SET
                         extension=excluded.extension,
                         folder=excluded.folder,
                         size_bytes=excluded.size_bytes,
                         modified_ns=excluded.modified_ns,
                         metadata=excluded.metadata"#,
                    params![
                        relative.clone(),
                        extension,
                        folder,
                        size_bytes as i64,
                        modified_ns,
                        metadata_text
                    ],
                )
                .await?;
            connection
                .execute("DELETE FROM wiki_links WHERE source_path = ?1", [relative.clone()])
                .await?;
            if capabilities.wiki_links {
                for (ordinal, occurrence) in extract_wiki_link_occurrences(source.as_deref().unwrap_or_default())
                    .into_iter()
                    .enumerate()
                {
                    connection
                        .execute(
                            "INSERT INTO wiki_links (source_path, ordinal, target, target_path) VALUES (?1, ?2, ?3, NULL)",
                            params![relative.clone(), ordinal as i64, occurrence.target],
                        )
                        .await?;
                }
            }
            Ok::<_, anyhow::Error>(())
        }
        .await;
        match result {
            Ok(()) => connection
                .execute("COMMIT", ())
                .await
                .map(|_| ())
                .map_err(Into::into),
            Err(error) => {
                let _ = connection.execute("ROLLBACK", ()).await;
                Err(error)
            }
        }
    }

    pub(super) async fn remove_file(&self, path: &Path) -> Result<()> {
        let relative = relative_vault_path(&self.root, path)?;
        let keys = target_keys(&relative);
        self.delete_document(&relative).await?;
        self.update_matching_links(None, &keys).await
    }

    async fn delete_document(&self, relative: &str) -> Result<()> {
        let connection = self.connection().await?;
        connection
            .execute("DELETE FROM documents WHERE path = ?1", [relative])
            .await?;
        Ok(())
    }

    pub(super) async fn tracked_paths(&self) -> Result<Vec<PathBuf>> {
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

    pub(super) async fn resolve(
        &self,
        _source: Option<&Path>,
        target: &str,
    ) -> Result<Option<PathBuf>> {
        let target = target.trim_start_matches('/');
        let connection = self.connection().await?;
        let has_extension = Path::new(target).extension().is_some();
        let sql = if has_extension {
            "SELECT path, extension FROM documents \
             WHERE path = ?1 \
                OR substr(path, -(length(?1) + 1)) = '/' || ?1"
        } else {
            "SELECT path, extension FROM documents \
             WHERE path = ?1 || '.' || extension \
                OR substr(path, -(length(?1) + length(extension) + 2)) \
                   = '/' || ?1 || '.' || extension"
        };
        let mut rows = connection.query(sql, [target]).await?;
        let mut candidates = Vec::new();
        while let Some(row) = rows.next().await? {
            candidates.push((row.get::<String>(0)?, row.get::<String>(1)?));
        }
        candidates.sort_by(compare_resolution_candidates);
        Ok(candidates.first().map(|(path, _)| self.root.join(path)))
    }

    pub(super) async fn link_occurrences_to(
        &self,
        targets: &BTreeSet<PathBuf>,
    ) -> Result<HashMap<PathBuf, Vec<(usize, PathBuf)>>> {
        let targets = targets
            .iter()
            .map(|path| relative_vault_path(&self.root, path))
            .collect::<Result<BTreeSet<_>>>()?;
        let connection = self.connection().await?;
        let mut rows = connection
            .query(
                "SELECT source_path, ordinal, target_path FROM wiki_links WHERE target_path IS NOT NULL",
                (),
            )
            .await?;
        let mut occurrences = HashMap::<PathBuf, Vec<(usize, PathBuf)>>::new();
        while let Some(row) = rows.next().await? {
            let target = row.get::<String>(2)?;
            if targets.contains(&target) {
                occurrences
                    .entry(self.root.join(row.get::<String>(0)?))
                    .or_default()
                    .push((row.get::<i64>(1)? as usize, self.root.join(target)));
            }
        }
        Ok(occurrences)
    }

    pub(super) async fn replacement_after_rename(
        &self,
        _source: &Path,
        target: &Path,
        from: &Path,
        to: &Path,
        explicit_extension: bool,
    ) -> Result<String> {
        let remap = |path: &str| -> Result<String> {
            let absolute = self.root.join(path);
            let remapped = super::remap_renamed_path(&absolute, from, to);
            relative_vault_path(&self.root, &remapped)
        };
        let documents = self
            .document_paths_and_extensions()
            .await?
            .into_iter()
            .map(|(path, extension)| {
                let path = remap(&path)?;
                Ok((path, extension))
            })
            .collect::<Result<Vec<_>>>()?;
        let target = super::remap_renamed_path(target, from, to);
        let target_relative = relative_vault_path(&self.root, &target)?;
        let authored_path = if explicit_extension {
            target_relative.clone()
        } else {
            Path::new(&target_relative)
                .with_extension("")
                .to_string_lossy()
                .replace('\\', "/")
        };
        let components: Vec<_> = authored_path.split('/').collect();
        for start in (0..components.len()).rev() {
            let candidate = components[start..].join("/");
            if resolve_target(&documents, &candidate).as_deref() == Some(&target_relative) {
                return Ok(candidate);
            }
        }
        Ok(authored_path)
    }

    pub(super) async fn update_all_links(&self) -> Result<()> {
        self.update_matching_links(None, &BTreeSet::new()).await
    }

    pub(super) async fn update_affected_links(&self, path: &Path) -> Result<()> {
        let relative = relative_vault_path(&self.root, path)?;
        self.update_matching_links(Some(&relative), &target_keys(&relative))
            .await
    }

    async fn update_matching_links(
        &self,
        source: Option<&str>,
        targets: &BTreeSet<String>,
    ) -> Result<()> {
        let documents = self.document_paths_and_extensions().await?;
        let connection = self.connection().await?;
        let mut parameters = Vec::new();
        let mut predicates = Vec::new();
        if let Some(source) = source {
            predicates.push("source_path = ?".to_string());
            parameters.push(Value::Text(source.to_string()));
        }
        if !targets.is_empty() {
            predicates.push(format!(
                "target IN ({})",
                std::iter::repeat_n("?", targets.len())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            parameters.extend(targets.iter().cloned().map(Value::Text));
        }
        let where_clause = if predicates.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", predicates.join(" OR "))
        };
        let mut rows = connection
            .query(
                format!("SELECT source_path, ordinal, target FROM wiki_links{where_clause}"),
                turso::params_from_iter(parameters),
            )
            .await?;
        let mut resolutions = Vec::new();
        while let Some(row) = rows.next().await? {
            let source = row.get::<String>(0)?;
            let ordinal = row.get::<i64>(1)?;
            let target = row.get::<String>(2)?;
            let resolved = resolve_target(&documents, &target);
            resolutions.push((source, ordinal, resolved));
        }
        drop(rows);
        connection.execute("BEGIN IMMEDIATE", ()).await?;
        for (source, ordinal, resolved) in resolutions {
            connection
                .execute(
                    "UPDATE wiki_links SET target_path = ?1 WHERE source_path = ?2 AND ordinal = ?3",
                    params![resolved, source, ordinal],
                )
                .await?;
        }
        connection.execute("COMMIT", ()).await?;
        Ok(())
    }

    async fn document_paths_and_extensions(&self) -> Result<Vec<(String, String)>> {
        let connection = self.connection().await?;
        let mut rows = connection
            .query("SELECT path, extension FROM documents ORDER BY path", ())
            .await?;
        let mut documents = Vec::new();
        while let Some(row) = rows.next().await? {
            documents.push((row.get::<String>(0)?, row.get::<String>(1)?));
        }
        Ok(documents)
    }
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

fn relative_vault_path(root: &Path, path: &Path) -> Result<String> {
    let relative = path
        .strip_prefix(root)
        .with_context(|| format!("Path escapes Vault: {}", path.display()))?;
    let value = relative
        .to_str()
        .ok_or_else(|| anyhow!("Catalog path is not UTF-8: {}", relative.display()))?
        .replace('\\', "/");
    if value.starts_with('/')
        || relative.components().any(|part| {
            matches!(
                part,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
    {
        bail!("Catalog path is not canonical: {}", relative.display());
    }
    Ok(value)
}

fn file_version(path: &Path) -> Result<(u64, i64)> {
    let metadata = fs::metadata(path)?;
    let modified = metadata.modified()?.duration_since(UNIX_EPOCH)?;
    let nanos =
        i64::try_from(modified.as_nanos()).context("File modification time is too large")?;
    Ok((metadata.len(), nanos))
}

fn metadata_from_markdown(source: &str) -> Option<JsonValue> {
    let frontmatter = source
        .strip_prefix("---\n")
        .or_else(|| source.strip_prefix("---\r\n"))?;
    let end = frontmatter
        .find("\n---\n")
        .or_else(|| frontmatter.find("\r\n---\r\n"))?;
    let value: YamlValue = yaml_serde::from_str(&frontmatter[..end]).ok()?;
    yaml_to_json(&value)
}

fn yaml_to_json(value: &YamlValue) -> Option<JsonValue> {
    match value {
        YamlValue::Null => Some(JsonValue::Null),
        YamlValue::Bool(value) => Some(JsonValue::Bool(*value)),
        YamlValue::Number(value) => {
            if let Some(value) = value.as_i64() {
                Some(JsonValue::Number(value.into()))
            } else if let Some(value) = value.as_u64() {
                Some(JsonValue::Number(value.into()))
            } else {
                serde_json::Number::from_f64(value.as_f64()?).map(JsonValue::Number)
            }
        }
        YamlValue::String(value) => Some(JsonValue::String(value.clone())),
        YamlValue::Sequence(values) => values
            .iter()
            .map(yaml_to_json)
            .collect::<Option<Vec<_>>>()
            .map(JsonValue::Array),
        YamlValue::Mapping(values) => {
            let mut map = Map::new();
            for (key, value) in values {
                map.insert(key.as_str()?.to_string(), yaml_to_json(value)?);
            }
            Some(JsonValue::Object(map))
        }
        YamlValue::Tagged(_) => None,
    }
}

fn resolve_target(documents: &[(String, String)], target: &str) -> Option<String> {
    let target = target.trim_start_matches('/');
    let explicit_extension = Path::new(target).extension().is_some();
    documents
        .iter()
        .filter(|(path, _)| {
            let comparable = if explicit_extension {
                path.clone()
            } else {
                Path::new(path)
                    .with_extension("")
                    .to_string_lossy()
                    .replace('\\', "/")
            };
            comparable == target || comparable.ends_with(&format!("/{target}"))
        })
        .min_by(|left, right| compare_resolution_candidates(left, right))
        .map(|(path, _)| path.clone())
}

fn compare_resolution_candidates(
    left: &(String, String),
    right: &(String, String),
) -> std::cmp::Ordering {
    left.0
        .matches('/')
        .count()
        .cmp(&right.0.matches('/').count())
        .then_with(|| {
            let left_is_markdown = left.1.eq_ignore_ascii_case("md");
            let right_is_markdown = right.1.eq_ignore_ascii_case("md");
            right_is_markdown.cmp(&left_is_markdown)
        })
        .then_with(|| left.0.cmp(&right.0))
}

fn target_keys(relative: &str) -> BTreeSet<String> {
    let path = Path::new(relative);
    let without_extension = path.with_extension("").to_string_lossy().replace('\\', "/");
    let full: Vec<_> = relative.split('/').collect();
    let bare: Vec<_> = without_extension.split('/').collect();
    let mut keys = BTreeSet::new();
    for start in 0..full.len() {
        keys.insert(full[start..].join("/"));
        keys.insert(bare[start..].join("/"));
    }
    keys
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::file_types::FileTypeCapabilities;

    #[test]
    fn rejects_non_json_yaml_values() {
        let tagged: YamlValue = yaml_serde::from_str("value: !tag tagged").unwrap();
        assert!(yaml_to_json(&tagged).is_none());
    }

    #[test]
    fn resolves_nearest_suffix_for_any_registered_extension() {
        let documents = vec![
            ("A.md".into(), "md".into()),
            ("folder/A.graph".into(), "graph".into()),
            ("folder/A.md".into(), "md".into()),
            ("big/folder/A.png".into(), "png".into()),
        ];
        assert_eq!(resolve_target(&documents, "A"), Some("A.md".into()));
        assert_eq!(
            resolve_target(&documents, "folder/A"),
            Some("folder/A.md".into())
        );
        assert_eq!(
            resolve_target(&documents, "A.png"),
            Some("big/folder/A.png".into())
        );
    }

    #[test]
    fn queries_nearest_suffix_for_any_registered_extension() {
        let root = std::env::temp_dir().join(format!(
            "datalith-catalog-resolution-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("folder")).unwrap();
        fs::create_dir_all(root.join("big/folder")).unwrap();
        let paths = [
            root.join("A.md"),
            root.join("folder/A.graph"),
            root.join("folder/A.md"),
            root.join("big/folder/A.png"),
        ];
        for path in &paths {
            fs::write(path, "").unwrap();
        }
        let capabilities = FileTypeCapabilities {
            text_search: false,
            wiki_links: false,
            yaml_frontmatter: false,
        };
        let types = RegisteredFileTypes::new([
            ("md".into(), capabilities),
            ("graph".into(), capabilities),
            ("png".into(), capabilities),
        ]);
        pollster::block_on(async {
            let database = CatalogDatabase::open(&root).await.unwrap();
            for path in &paths {
                database.upsert_file(path, &types).await.unwrap();
            }
            assert_eq!(
                database.resolve(None, "A").await.unwrap(),
                Some(paths[0].clone())
            );
            assert_eq!(
                database.resolve(None, "folder/A").await.unwrap(),
                Some(paths[2].clone())
            );
            assert_eq!(
                database.resolve(None, "A.png").await.unwrap(),
                Some(paths[3].clone())
            );
        });
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn stores_jsonb_and_filters_typed_metadata() {
        let root = std::env::temp_dir().join(format!(
            "datalith-catalog-database-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let note = root.join("Note.md");
        fs::write(
            &note,
            "---\nstatus: done\npriority: 4\ntags: [rust, project]\n---\n[[Other]]",
        )
        .unwrap();
        let types = RegisteredFileTypes::new([(
            "md".into(),
            FileTypeCapabilities {
                text_search: true,
                wiki_links: true,
                yaml_frontmatter: true,
            },
        )]);
        pollster::block_on(async {
            let database = CatalogDatabase::open(&root).await.unwrap();
            database.upsert_file(&note, &types).await.unwrap();
            let selection = database
                .query_documents(CatalogQuery {
                    extension: Some("md".into()),
                    filter: CatalogFilter::Compare {
                        property: CatalogProperty::Metadata(vec!["status".into()]),
                        comparison: CatalogComparison::Equal,
                        value: CatalogScalar::String("done".into()),
                    },
                    limit: 10,
                })
                .await
                .unwrap();
            assert_eq!(selection.documents.len(), 1);
            assert_eq!(
                selection.documents[0].metadata.as_ref().unwrap()["priority"],
                4
            );
            let contains = database
                .query_documents(CatalogQuery {
                    extension: Some("md".into()),
                    filter: CatalogFilter::Contains {
                        property: CatalogProperty::Metadata(vec!["tags".into()]),
                        value: CatalogScalar::String("rust".into()),
                    },
                    limit: 10,
                })
                .await
                .unwrap();
            assert_eq!(contains.documents.len(), 1);
            let missing_is_not_value = database
                .query_documents(CatalogQuery {
                    extension: Some("md".into()),
                    filter: CatalogFilter::Compare {
                        property: CatalogProperty::Metadata(vec!["missing".into()]),
                        comparison: CatalogComparison::NotEqual,
                        value: CatalogScalar::String("value".into()),
                    },
                    limit: 10,
                })
                .await
                .unwrap();
            assert_eq!(missing_is_not_value.documents.len(), 1);
            let not_in_folder = database
                .query_documents(CatalogQuery {
                    extension: Some("md".into()),
                    filter: CatalogFilter::And(vec![CatalogFilter::Not(Box::new(
                        CatalogFilter::InFolder("02 - Marque-pages".into()),
                    ))]),
                    limit: 2_000,
                })
                .await
                .unwrap();
            assert_eq!(not_in_folder.documents.len(), 1);
        });
        let _ = fs::remove_dir_all(root);
    }
}
