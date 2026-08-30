use std::fs;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use anyhow::{Context, Result, anyhow};

use crate::vault::DATALITH_DIR_NAME;

mod document;
mod link_resolution;
mod query;
mod schema;

pub(super) const FILE_NAME_SQL: &str = "substr(path, \
    CASE WHEN folder = '' THEN 1 ELSE length(folder) + 2 END, \
    length(path) - CASE WHEN folder = '' THEN 1 ELSE length(folder) + 2 END - length(extension))";
pub(super) const FILE_BASENAME_SQL: &str =
    "substr(path, CASE WHEN folder = '' THEN 1 ELSE length(folder) + 2 END)";
pub(super) const PATH_WITHOUT_EXTENSION_SQL: &str =
    "substr(path, 1, length(path) - length(extension) - 1)";

#[derive(Clone, Debug)]
pub struct Backlink {
    pub source: PathBuf,
    pub ordinal: usize,
    pub authored_target: String,
    pub target_path: PathBuf,
}

#[derive(Clone, Debug)]
pub(super) struct SynchronizedFiles {
    pub(super) all: Vec<PathBuf>,
    pub(super) changed: Vec<PathBuf>,
}

#[derive(Clone)]
pub(super) struct CatalogDatabase {
    pub(super) root: PathBuf,
    connection: turso::Connection,
    connect_fresh: Arc<dyn Fn() -> Result<turso::Connection> + Send + Sync>,
    read_connection: Arc<Mutex<Option<turso::Connection>>>,
}

/// A read handle on the catalog.
/// Reuses the shared read connection while it is free;
/// otherwise opens a dedicated connection that is dropped on use.
pub(super) struct ReadConnection {
    pool: Option<Arc<Mutex<Option<turso::Connection>>>>,
    connection: turso::Connection,
}

impl Deref for ReadConnection {
    type Target = turso::Connection;

    fn deref(&self) -> &turso::Connection {
        &self.connection
    }
}

impl Drop for ReadConnection {
    fn drop(&mut self) {
        if let Some(pool) = &self.pool
            && let Ok(mut parked) = pool.lock()
            && parked.is_none()
        {
            *parked = Some(self.connection.clone());
        }
    }
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
        let (connection, read_connection, connect_fresh) = {
            let database =
                if let Ok(database) = turso::Builder::new_local(database_path_text).build().await {
                    database
                } else {
                    for path in [
                        database_path.clone(),
                        database_path.with_extension("db-wal"),
                        database_path.with_extension("db-shm"),
                    ] {
                        if let Err(error) = fs::remove_file(&path)
                            && error.kind() != std::io::ErrorKind::NotFound
                        {
                            // Can utimately work if rebuilt succeeds
                            eprintln!(
                                "Failed to remove stale catalog file {}: {error}",
                                path.display()
                            );
                        }
                    }
                    turso::Builder::new_local(database_path_text)
                        .build()
                        .await
                        .context("Failed to rebuild embedded Turso catalog")?
                };
            let connection = database.connect()?;
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

            let read_connection = Arc::new(Mutex::new(Some(database.connect()?)));
            let connect_fresh: Arc<dyn Fn() -> Result<turso::Connection> + Send + Sync> = {
                let database = database.clone();
                Arc::new(move || Ok(database.connect()?))
            };
            drop(database);
            (connection, read_connection, connect_fresh)
        };
        let this = Self {
            root: root.to_path_buf(),
            connection,
            connect_fresh,
            read_connection,
        };
        this.initialize_schema().await?;
        Ok(this)
    }

    pub(super) fn connection(&self) -> turso::Connection {
        self.connection.clone()
    }

    pub(super) fn read_connection(&self) -> Result<ReadConnection> {
        let mut parked = self
            .read_connection
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let pooled = parked.take();
        drop(parked);
        if let Some(connection) = pooled {
            return Ok(ReadConnection {
                pool: Some(self.read_connection.clone()),
                connection,
            });
        }
        Ok(ReadConnection {
            pool: None,
            connection: (self.connect_fresh)()?,
        })
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
