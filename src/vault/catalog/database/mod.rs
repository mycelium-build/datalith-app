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
    parked: Arc<Mutex<Option<turso::Connection>>>,
}

/// A connection checked out of the catalog pool.
/// Reuses the parked pool connection while it is free;
/// otherwise opens a dedicated connection that is dropped on use.
pub(super) struct PooledConnection {
    pool: Option<Arc<Mutex<Option<turso::Connection>>>>,
    connection: turso::Connection,
}

impl Deref for PooledConnection {
    type Target = turso::Connection;

    fn deref(&self) -> &turso::Connection {
        &self.connection
    }
}

impl Drop for PooledConnection {
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
        let (connection, connect_fresh) = {
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
            Self::configure_connection(&connection).await?;
            connection
                .query("PRAGMA journal_mode = WAL", ())
                .await?
                .next()
                .await?;
            let connect_fresh: Arc<dyn Fn() -> Result<turso::Connection> + Send + Sync> = {
                let database = database.clone();
                Arc::new(move || Ok(database.connect()?))
            };
            drop(database);
            (connection, connect_fresh)
        };
        let this = Self {
            root: root.to_path_buf(),
            connection,
            connect_fresh,
            parked: Arc::new(Mutex::new(None)),
        };
        if let Ok(mut parked) = this.parked.lock() {
            *parked = Some(this.connection.clone());
        }
        this.initialize_schema().await?;
        Ok(this)
    }

    async fn configure_connection(connection: &turso::Connection) -> Result<()> {
        connection.busy_timeout(std::time::Duration::from_secs(5))?; // resolves write contention
        connection.execute("PRAGMA foreign_keys = ON", ()).await?;
        connection
            .execute("PRAGMA synchronous = NORMAL", ())
            .await?;
        Ok(())
    }

    pub(super) async fn connection(&self) -> Result<PooledConnection> {
        let parked = self
            .parked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(connection) = parked {
            return Ok(PooledConnection {
                pool: Some(self.parked.clone()),
                connection,
            });
        }
        let connection = (self.connect_fresh)()?;
        Self::configure_connection(&connection).await?;
        Ok(PooledConnection {
            pool: Some(self.parked.clone()),
            connection,
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
