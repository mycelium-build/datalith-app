use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};

use anyhow::{Context, Result, anyhow};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};

mod database;
use database::{Backlink, CatalogDatabase};

use crate::document::file_types::RegisteredFileTypes;
use crate::vault::DATALITH_DIR_NAME;
use crate::vault::search::SearchEngine;

const CATALOG_INITIALIZATION_STACK_SIZE: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CatalogState {
    Syncing,
    Ready,
    Failed,
}

#[derive(Clone, Debug)]
pub(crate) struct CatalogEvent {
    pub(crate) paths: Vec<PathBuf>,
    pub(crate) structure_changed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct WikiLinkEdge {
    pub(crate) source: PathBuf,
    pub(crate) target: PathBuf,
}

#[derive(Clone, Debug)]
pub(crate) struct CatalogDocument {
    pub(crate) path: PathBuf,
    pub(crate) metadata: Option<serde_json::Value>,
}

#[derive(Clone, Debug)]
pub(crate) struct DocumentSelection {
    pub(crate) documents: Vec<CatalogDocument>,
    pub(crate) exceeded_limit: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct LinkedDocumentSelection {
    pub(crate) documents: Vec<CatalogDocument>,
    pub(crate) links: Vec<WikiLinkEdge>,
    pub(crate) exceeded_limit: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct CatalogQuery {
    pub(crate) extension: Option<String>,
    pub(crate) filter: CatalogFilter,
    pub(crate) limit: usize,
}

#[derive(Clone, Debug)]
pub(crate) enum CatalogFilter {
    MatchAll,
    Compare {
        property: CatalogProperty,
        comparison: CatalogComparison,
        value: CatalogScalar,
    },
    Contains {
        property: CatalogProperty,
        value: CatalogScalar,
    },
    InFolder(String),
    And(Vec<CatalogFilter>),
    Or(Vec<CatalogFilter>),
    Not(Box<CatalogFilter>),
}

#[derive(Clone, Debug)]
pub(crate) enum CatalogProperty {
    Metadata(Vec<String>),
    File(CatalogFileField),
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum CatalogFileField {
    Name,
    Extension,
    Path,
    Folder,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum CatalogComparison {
    Equal,
    NotEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
}

#[derive(Clone, Debug)]
pub(crate) enum CatalogScalar {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
}

struct CatalogInner {
    root: PathBuf,
    database: CatalogDatabase,
    file_types: RegisteredFileTypes,
    search: Mutex<SearchEngine>,
    subscribers: Mutex<Vec<mpsc::Sender<CatalogEvent>>>,
    _watcher: Mutex<Option<RecommendedWatcher>>,
    state: Mutex<CatalogState>,
}

#[derive(Clone)]
pub(crate) struct VaultCatalog {
    inner: Arc<CatalogInner>,
}

impl VaultCatalog {
    pub(crate) fn open(root: PathBuf, file_types: RegisteredFileTypes) -> Result<Self> {
        let database = pollster::block_on(CatalogDatabase::open(&root))?;
        let search = SearchEngine::open_existing(&root, file_types.clone())?;

        let inner = Arc::new(CatalogInner {
            root,
            database,
            file_types: file_types.clone(),
            search: Mutex::new(search),
            subscribers: Mutex::new(Vec::new()),
            _watcher: Mutex::new(None),
            state: Mutex::new(CatalogState::Syncing),
        });
        let catalog = Self {
            inner: inner.clone(),
        };

        std::thread::Builder::new()
            .name("vault-catalog-sync".into())
            .stack_size(CATALOG_INITIALIZATION_STACK_SIZE)
            .spawn(move || {
                Self::sync_on_background_thread(&inner, &file_types);
            })
            .context("Failed to start Vault Catalog sync thread")?;

        Ok(catalog)
    }

    fn sync_on_background_thread(inner: &Arc<CatalogInner>, file_types: &RegisteredFileTypes) {
        let result = (|| -> Result<()> {
            let initial = walk_tracked_files(&inner.root, file_types);
            let stored = pollster::block_on(inner.database.stored_paths())?;
            let removed = stored
                .into_iter()
                .filter(|path| !initial.contains(path))
                .collect::<Vec<_>>();
            let initial = initial.into_iter().collect::<Vec<_>>();
            let synchronized =
                pollster::block_on(inner.database.synchronize(&removed, &initial, file_types))?;

            if let Ok(search) = inner.search.lock() {
                let _ = search.indexer.synchronize(&synchronized);
            }

            let (notify_tx, notify_rx) = mpsc::channel();
            let observed_root = inner.root.canonicalize().unwrap_or_else(|_| inner.root.clone());
            let logical_root = inner.root.clone();
            let mut watcher =
                notify::recommended_watcher(move |mut event: notify::Result<notify::Event>| {
                    if let Ok(event) = &mut event {
                        for path in &mut event.paths {
                            if let Ok(relative) = path.strip_prefix(&observed_root) {
                                *path = logical_root.join(relative);
                            }
                        }
                    }
                    let _ = notify_tx.send(event);
                })?;
            watcher.watch(&inner.root, RecursiveMode::Recursive)?;

            if let Ok(mut guard) = inner._watcher.lock() {
                *guard = Some(watcher);
            }

            spawn_reconciler(inner, notify_rx)?;

            publish_event(inner, synchronized, true);

            if let Ok(mut state) = inner.state.lock() {
                *state = CatalogState::Ready;
            }
            Ok(())
        })();

        if let Err(e) = result {
            eprintln!("Catalog sync failed: {e}");
            if let Ok(mut state) = inner.state.lock() {
                *state = CatalogState::Failed;
            }
            publish_event(inner, Vec::new(), false);
        }
    }

    #[cfg(test)]
    pub(crate) fn wait_until_ready(&self, timeout: std::time::Duration) {
        let start = std::time::Instant::now();
        loop {
            let state = self.inner.state.lock().ok().map(|s| *s);
            if matches!(state, Some(CatalogState::Ready | CatalogState::Failed)) {
                break;
            }
            if start.elapsed() > timeout {
                panic!("VaultCatalog sync did not complete within {timeout:?}");
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[must_use]
    pub(crate) fn state(&self) -> CatalogState {
        self.inner
            .state
            .lock()
            .map(|s| *s)
            .unwrap_or(CatalogState::Syncing)
    }

    #[must_use]
    pub(crate) fn root(&self) -> PathBuf {
        self.inner.root.clone()
    }

    #[must_use]
    pub(crate) fn paths(&self) -> Vec<PathBuf> {
        pollster::block_on(self.inner.database.stored_paths()).unwrap_or_default()
    }

    #[must_use]
    pub(crate) fn events(&self) -> mpsc::Receiver<CatalogEvent> {
        let (sender, receiver) = mpsc::channel();
        if let Ok(mut subscribers) = self.inner.subscribers.lock() {
            subscribers.push(sender);
        }
        receiver
    }

    #[must_use]
    pub(crate) fn search(&self, query: &str) -> Vec<PathBuf> {
        self.inner
            .search
            .lock()
            .map(|search| search.search(query))
            .unwrap_or_default()
    }

    #[must_use]
    pub(crate) fn resolve(&self, authored: &str) -> Option<PathBuf> {
        pollster::block_on(self.inner.database.resolve_path(authored)).unwrap_or_default()
    }

    pub(crate) fn backlinks_under(&self, target: &Path) -> Result<Vec<Backlink>> {
        let root = self.root();
        pollster::block_on(self.inner.database.backlinks_under(target)).map(|links| {
            links
                .into_iter()
                .map(|mut link| {
                    link.source = root.join(link.source);
                    link.target_path = root.join(link.target_path);
                    link
                })
                .collect()
        })
    }

    #[allow(dead_code)]
    pub(crate) async fn query_documents(&self, query: CatalogQuery) -> Result<DocumentSelection> {
        let database = self.inner.database.clone();
        std::thread::Builder::new()
            .name("vault-catalog-query".into())
            .stack_size(8 * 1024 * 1024)
            .spawn(move || pollster::block_on(database.query_documents(query)))
            .context("Failed to start catalog query thread")?
            .join()
            .map_err(|_| anyhow!("Catalog query thread panicked"))?
    }

    pub(crate) async fn query_documents_with_links(
        &self,
        query: CatalogQuery,
    ) -> Result<LinkedDocumentSelection> {
        let database = self.inner.database.clone();
        let (selection, stored_links) = std::thread::Builder::new()
            .name("vault-catalog-query".into())
            .stack_size(8 * 1024 * 1024)
            .spawn(move || pollster::block_on(database.query_documents_with_links(query)))
            .context("Failed to start catalog query thread")?
            .join()
            .map_err(|_| anyhow!("Catalog query thread panicked"))??;
        let links = stored_links
            .into_iter()
            .map(|link| WikiLinkEdge {
                source: link.source,
                target: link.target,
            })
            .collect();
        Ok(LinkedDocumentSelection {
            documents: selection.documents,
            links,
            exceeded_limit: selection.exceeded_limit,
        })
    }
}

fn spawn_reconciler(
    inner: &Arc<CatalogInner>,
    receiver: mpsc::Receiver<notify::Result<notify::Event>>,
) -> Result<()> {
    let weak = Arc::downgrade(inner);
    std::thread::Builder::new()
        .name("vault-catalog-reconciler".into())
        .spawn(move || {
            while let Ok(event) = receiver.recv() {
                let Some(inner) = weak.upgrade() else { break };
                let Ok(event) = event else { continue };
                let mut event_paths = event.paths;
                while let Ok(next) = receiver.try_recv() {
                    if let Ok(next) = next {
                        event_paths.extend(next.paths);
                    }
                }
                reconcile_paths(&inner, event_paths);
            }
        })
        .context("Failed to start Vault Catalog reconciler")?;
    Ok(())
}

fn reconcile_paths(inner: &CatalogInner, event_paths: Vec<PathBuf>) {
    let mut changed = BTreeSet::new();
    let mut removed = BTreeSet::new();
    let Ok(known) = pollster::block_on(inner.database.stored_paths()) else {
        return;
    };
    let known = known.into_iter().collect::<BTreeSet<_>>();

    for path in event_paths {
        if !path.starts_with(&inner.root) || path.starts_with(inner.root.join(DATALITH_DIR_NAME)) {
            continue;
        }
        if path.is_dir() {
            let current = walk_tracked_files(&path, &inner.file_types);
            changed.extend(current.iter().cloned());
            removed.extend(
                known
                    .iter()
                    .filter(|known_path| {
                        known_path.starts_with(&path) && !current.contains(*known_path)
                    })
                    .cloned(),
            );
        } else if path.is_file() && inner.file_types.is_tracked(&path) {
            changed.insert(path);
        } else {
            removed.extend(
                known
                    .iter()
                    .filter(|known_path| **known_path == path || known_path.starts_with(&path))
                    .cloned(),
            );
        }
    }
    changed.retain(|path| !removed.contains(path));
    if changed.is_empty() && removed.is_empty() {
        return;
    }

    let changed = changed.into_iter().collect::<Vec<_>>();
    let removed = removed.into_iter().collect::<Vec<_>>();
    let Ok(synchronized) = pollster::block_on(inner.database.synchronize(
        &removed,
        &changed,
        &inner.file_types,
    )) else {
        return;
    };
    let synchronized_set = synchronized.iter().collect::<BTreeSet<_>>();
    let omitted = changed
        .iter()
        .filter(|path| !synchronized_set.contains(path))
        .cloned()
        .collect::<Vec<_>>();
    let mut removed_from_derived = removed.clone();
    removed_from_derived.extend(omitted);
    let structure_changed = synchronized.iter().any(|path| !known.contains(path))
        || removed_from_derived.iter().any(|path| known.contains(path));
    if let Ok(search) = inner.search.lock() {
        let _ = search.indexer.apply(&removed_from_derived, &synchronized);
    }
    let mut paths = removed_from_derived;
    paths.extend(synchronized);
    publish_event(inner, paths, structure_changed);
}

fn publish_event(inner: &CatalogInner, mut paths: Vec<PathBuf>, structure_changed: bool) {
    if paths.is_empty() {
        return;
    }
    paths.sort();
    paths.dedup();
    let event = CatalogEvent {
        paths,
        structure_changed,
    };
    if let Ok(mut subscribers) = inner.subscribers.lock() {
        subscribers.retain(|subscriber| subscriber.send(event.clone()).is_ok());
    }
}

fn walk_tracked_files(root: &Path, file_types: &RegisteredFileTypes) -> BTreeSet<PathBuf> {
    let mut files = BTreeSet::new();
    let mut directories = vec![root.to_path_buf()];
    while let Some(directory) = directories.pop() {
        let Ok(entries) = std::fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.starts_with(root.join(DATALITH_DIR_NAME)) {
                continue;
            }
            if path.is_dir() {
                directories.push(path);
            } else if file_types.is_tracked(&path) {
                files.insert(path);
            }
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::file_types::FileTypeCapabilities;

    #[test]
    fn changed_paths_reconcile_catalog_and_search_index() {
        let root =
            std::env::temp_dir().join(format!("datalith-catalog-notify-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let file_types = RegisteredFileTypes::new([(
            "md".into(),
            FileTypeCapabilities {
                text_search: true,
                wiki_links: true,
                yaml_frontmatter: true,
            },
        )]);
        let catalog = VaultCatalog::open(root.clone(), file_types).unwrap();
        let events = catalog.events();
        catalog.wait_until_ready(std::time::Duration::from_secs(5));
        while events.try_recv().is_ok() {}
        let path = root.join("Observed.md");

        std::fs::write(&path, "distinctive watcher content").unwrap();
        reconcile_paths(&catalog.inner, vec![path.clone()]);

        let event = events
            .recv_timeout(std::time::Duration::from_secs(1))
            .unwrap();
        assert!(event.structure_changed);
        assert!(catalog.paths().contains(&path));
        assert_eq!(catalog.resolve("Observed"), Some(path.clone()));
        assert_eq!(catalog.search("distinctive watcher"), vec![path.clone()]);
        drop(catalog);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn directory_observation_publishes_only_tracked_files() {
        let root = std::env::temp_dir().join(format!(
            "datalith-catalog-observed-paths-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let folder = root.join("folder");
        std::fs::create_dir_all(&folder).unwrap();
        let tracked = folder.join("Tracked.md");
        let hidden_tracked = folder.join(".Hidden.md");
        std::fs::write(&tracked, "tracked").unwrap();
        std::fs::write(&hidden_tracked, "hidden but tracked").unwrap();
        std::fs::write(folder.join("Ignored.txt"), "ignored").unwrap();
        let file_types = RegisteredFileTypes::new([(
            "md".into(),
            FileTypeCapabilities {
                text_search: true,
                wiki_links: true,
                yaml_frontmatter: true,
            },
        )]);
        let catalog = VaultCatalog::open(root.clone(), file_types).unwrap();
        let events = catalog.events();
        catalog.wait_until_ready(std::time::Duration::from_secs(5));
        while events.try_recv().is_ok() {}
        assert_eq!(catalog.search("hidden"), vec![hidden_tracked.clone()]);

        reconcile_paths(&catalog.inner, vec![folder]);

        let event = events
            .recv_timeout(std::time::Duration::from_secs(1))
            .unwrap();
        assert_eq!(event.paths, vec![hidden_tracked, tracked]);
        assert!(!event.structure_changed);
        drop(catalog);
        let _ = std::fs::remove_dir_all(root);
    }
}
