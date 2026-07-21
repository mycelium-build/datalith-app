use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use turso::{Builder, Database, Value};

use super::{
    CatalogComparison, CatalogDocument, CatalogFileField, CatalogFilter, CatalogProperty,
    CatalogQuery, CatalogScalar, DocumentSelection,
};

const SCHEMA_VERSION: i64 = 3;
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
}
