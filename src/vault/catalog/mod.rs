use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};

use anyhow::{Context, Result, anyhow, bail};
use notify::{
    Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
    event::{ModifyKind, RemoveKind, RenameMode},
};

use crate::document::file_types::RegisteredFileTypes;
use crate::vault::file_ops;
use crate::vault::search::SearchEngine;

mod database;
mod links;
mod rename;
use database::CatalogDatabase;

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

const METADATA_DIR: &str = ".datalith";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CatalogSyncState {
    Discovering,
    Current,
    Degraded,
}

#[derive(Clone, Debug)]
pub(crate) struct CatalogUpdate {
    pub(crate) changed_paths: Arc<[PathBuf]>,
    pub(crate) structure_changed: bool,
    pub(crate) tracked_paths_changed: bool,
}

struct CatalogState {
    sync: CatalogSyncState,
    initialization_complete: bool,
    subscribers: Vec<mpsc::Sender<CatalogUpdate>>,
    completed_writes: HashMap<PathBuf, u64>,
    pending_events: Vec<Event>,
}

struct CatalogInner {
    root: PathBuf,
    file_types: RegisteredFileTypes,
    state: Mutex<CatalogState>,
    search: Mutex<Option<Arc<SearchEngine>>>,
    database: Mutex<Option<CatalogDatabase>>,
    watcher: Mutex<Option<RecommendedWatcher>>,
    commands: mpsc::Sender<CatalogCommand>,
}

enum CatalogCommand {
    Event(Event),
    ApplyPath(PathBuf),
    Reconcile,
    Rename {
        from: PathBuf,
        to: PathBuf,
    },
    #[cfg(test)]
    Barrier(mpsc::Sender<()>),
}

#[derive(Clone)]
pub(crate) struct VaultCatalog {
    inner: Arc<CatalogInner>,
}

impl VaultCatalog {
    pub(crate) fn open(root: PathBuf, file_types: RegisteredFileTypes) -> Result<Self> {
        if !root.is_dir() {
            bail!("Vault is not a directory: {}", root.display());
        }

        let (command_sender, command_receiver) = mpsc::channel();
        let inner = Arc::new(CatalogInner {
            root: root.clone(),
            file_types,
            state: Mutex::new(CatalogState {
                sync: CatalogSyncState::Discovering,
                initialization_complete: false,
                subscribers: Vec::new(),
                completed_writes: HashMap::new(),
                pending_events: Vec::new(),
            }),
            search: Mutex::new(None),
            database: Mutex::new(None),
            watcher: Mutex::new(None),
            commands: command_sender,
        });

        let event_inner = Arc::downgrade(&inner);
        let watcher = RecommendedWatcher::new(
            move |result: notify::Result<Event>| {
                let Some(inner) = event_inner.upgrade() else {
                    return;
                };
                match result {
                    Ok(event) => inner.observe(event),
                    Err(_) => inner.set_degraded(),
                }
            },
            Config::default(),
        )
        .and_then(|mut watcher| {
            watcher.watch(&root, RecursiveMode::Recursive)?;
            Ok(watcher)
        })
        .ok();

        if watcher.is_none() {
            inner.set_degraded();
        }

        *inner.watcher.lock().unwrap() = watcher;

        let initialization_inner = Arc::downgrade(&inner);
        let initialization_root = root;
        let initialization_types = inner.file_types.clone();
        std::thread::Builder::new()
            .name("vault-catalog-initialization".into())
            .spawn(move || {
                let search = SearchEngine::new(&initialization_root, &initialization_types)
                    .ok()
                    .map(Arc::new);
                let database = pollster::block_on(CatalogDatabase::open(&initialization_root)).ok();
                let recovery_failed = rename::recover(&initialization_root).is_err();
                let Some(inner) = initialization_inner.upgrade() else {
                    return;
                };
                *inner.search.lock().unwrap() = search;
                let files = discover(&initialization_root, &initialization_types);
                if recovery_failed {
                    inner.set_degraded();
                }
                if let Some(ref database) = database {
                    if pollster::block_on(database.reconcile(&files, &initialization_types))
                        .is_err()
                    {
                        inner.set_degraded();
                    }
                } else {
                    inner.set_degraded();
                }
                *inner.database.lock().unwrap() = database;
                let changed_paths = files.iter().cloned().collect();
                let pending_events = {
                    let mut state = inner.state.lock().unwrap();
                    state.initialization_complete = true;
                    if state.sync != CatalogSyncState::Degraded {
                        state.sync = CatalogSyncState::Current;
                    }
                    std::mem::take(&mut state.pending_events)
                };
                inner.publish(changed_paths, true, true);
                for event in pending_events {
                    inner.process_event(event);
                }
                let worker_inner = Arc::downgrade(&inner);
                drop(inner);
                while let Ok(command) = command_receiver.recv() {
                    let Some(inner) = worker_inner.upgrade() else {
                        break;
                    };
                    inner.process_command(command);
                }
            })
            .context("Failed to start Vault Catalog initialization")?;

        Ok(Self { inner })
    }

    pub(crate) fn subscribe(&self) -> mpsc::Receiver<CatalogUpdate> {
        let (sender, receiver) = mpsc::channel();
        self.inner.state.lock().unwrap().subscribers.push(sender);
        receiver
    }

    #[must_use]
    pub(crate) fn tracked_paths(&self) -> Vec<PathBuf> {
        self.inner
            .database
            .lock()
            .unwrap()
            .clone()
            .as_ref()
            .and_then(|database| pollster::block_on(database.tracked_paths()).ok())
            .unwrap_or_default()
    }

    #[allow(dead_code)]
    pub(crate) async fn tracked_paths_async(&self) -> Result<Vec<PathBuf>> {
        let database = self.inner.database.lock().unwrap().clone();
        match database {
            Some(database) => database.tracked_paths().await,
            None => Ok(Vec::new()),
        }
    }

    #[must_use]
    pub(crate) fn root(&self) -> PathBuf {
        self.inner.root.clone()
    }

    #[must_use]
    pub(crate) fn initialization_complete(&self) -> bool {
        self.inner.state.lock().unwrap().initialization_complete
    }

    #[cfg(test)]
    fn wait_for_idle(&self) {
        let (sender, receiver) = mpsc::channel();
        self.inner.enqueue(CatalogCommand::Barrier(sender));
        receiver.recv().unwrap();
    }

    #[must_use]
    #[allow(dead_code)]
    pub(crate) fn sync_state(&self) -> CatalogSyncState {
        self.inner.state.lock().unwrap().sync
    }

    #[must_use]
    pub(crate) fn search(&self, query: &str) -> Vec<PathBuf> {
        self.inner
            .search
            .lock()
            .unwrap()
            .clone()
            .map(|search| search.search(query))
            .unwrap_or_default()
    }

    #[must_use]
    pub(crate) fn resolve_wiki_link_from(&self, source: &Path, name: &str) -> Option<PathBuf> {
        self.inner
            .database
            .lock()
            .unwrap()
            .clone()
            .as_ref()
            .and_then(|database| {
                pollster::block_on(database.resolve(Some(source), name))
                    .ok()
                    .flatten()
            })
    }

    #[allow(dead_code)]
    pub(crate) async fn resolve_wiki_link_from_async(
        &self,
        source: &Path,
        name: &str,
    ) -> Result<Option<PathBuf>> {
        let database = self.inner.database.lock().unwrap().clone();
        match database {
            Some(database) => database.resolve(Some(source), name).await,
            None => Ok(None),
        }
    }

    #[allow(dead_code)]
    pub(crate) async fn query_documents(&self, query: CatalogQuery) -> Result<DocumentSelection> {
        let database = self.inner.database.lock().unwrap().clone();
        match database {
            Some(database) => std::thread::Builder::new()
                .name("vault-catalog-query".into())
                .stack_size(8 * 1024 * 1024) // Turso's parser exhaust the small native stack used by GPUI
                .spawn(move || pollster::block_on(database.query_documents(query)))
                .context("Failed to start catalog query thread")?
                .join()
                .map_err(|_| anyhow!("Catalog query thread panicked"))?,
            None => Ok(DocumentSelection {
                documents: Vec::new(),
                exceeded_limit: false,
            }),
        }
    }

    pub(crate) async fn query_documents_with_links(
        &self,
        query: CatalogQuery,
    ) -> Result<LinkedDocumentSelection> {
        let database = self.inner.database.lock().unwrap().clone();
        let Some(database) = database else {
            return Ok(LinkedDocumentSelection {
                documents: Vec::new(),
                links: Vec::new(),
                exceeded_limit: false,
            });
        };
        let (selection, stored_links) = std::thread::Builder::new()
            .name("vault-catalog-query".into())
            .stack_size(8 * 1024 * 1024)
            .spawn(move || pollster::block_on(database.query_documents_with_links(query)))
            .context("Failed to start catalog query thread")?
            .join()
            .map_err(|_| anyhow!("Catalog query thread panicked"))??;
        if selection.exceeded_limit {
            return Ok(LinkedDocumentSelection {
                documents: selection.documents,
                links: Vec::new(),
                exceeded_limit: true,
            });
        }
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
            exceeded_limit: false,
        })
    }

    pub(crate) fn create_file(&self, target: &Path) -> Result<PathBuf> {
        let path = file_ops::new_file_from_target(target)?;
        self.record_completed_write(&path);
        self.inner.enqueue(CatalogCommand::ApplyPath(path.clone()));
        Ok(path)
    }

    pub(crate) fn create_folder(&self, target: &Path) -> Result<PathBuf> {
        let path = file_ops::new_folder_from_target(target)?;
        self.inner.publish(vec![path.clone()], true, false);
        Ok(path)
    }

    pub(crate) fn duplicate(&self, target: &Path) -> Result<PathBuf> {
        let path = file_ops::duplicate_target(target)?;
        self.inner.enqueue(CatalogCommand::Reconcile);
        Ok(path)
    }

    pub(crate) fn delete(&self, target: &Path) -> Result<()> {
        self.ensure_inside(target)?;
        file_ops::delete_target(target)?;
        self.inner.enqueue(CatalogCommand::Reconcile);
        Ok(())
    }

    pub(crate) fn rename(&self, from: &Path, to: &Path) -> Result<()> {
        self.ensure_inside(from)?;
        self.ensure_inside(to)?;
        self.inner.enqueue(CatalogCommand::Rename {
            from: from.to_path_buf(),
            to: to.to_path_buf(),
        });
        Ok(())
    }

    fn perform_rename(&self, from: &Path, to: &Path) -> Result<()> {
        let database = self.inner.database.lock().unwrap().clone();
        let affected = database
            .as_ref()
            .map(|database| pollster::block_on(database.link_occurrences_under(from)))
            .transpose()?
            .unwrap_or_default();
        let replacements = database
            .as_ref()
            .map(|database| self.prepare_renamed_links(from, to, affected, database))
            .transpose()?
            .unwrap_or_default();
        let rewritten_sources: BTreeSet<_> = replacements
            .iter()
            .map(|(source, _, _)| source.clone())
            .collect();
        rename::execute(&self.inner.root, from, to, replacements)?;
        self.inner.reconcile_with_changed(&rewritten_sources);
        Ok(())
    }

    fn prepare_renamed_links(
        &self,
        from: &Path,
        to: &Path,
        affected: HashMap<PathBuf, Vec<(usize, PathBuf)>>,
        database: &CatalogDatabase,
    ) -> Result<Vec<(PathBuf, String, String)>> {
        let mut plans = Vec::new();
        for (old_source, occurrences) in affected {
            let source = remap_renamed_path(&old_source, from, to);
            let read_source = if old_source.exists() {
                &old_source
            } else {
                &source
            };
            let contents = fs::read_to_string(read_source).with_context(|| {
                format!("Failed to read linked source {}", read_source.display())
            })?;
            let mut rewritten = contents.clone();
            let parsed = links::extract_wiki_link_occurrences(&contents);
            let mut replacements = Vec::new();
            for (ordinal, old_target) in occurrences {
                let Some(occurrence) = parsed.get(ordinal) else {
                    continue;
                };
                let replacement = pollster::block_on(database.replacement_after_rename(
                    &old_source,
                    &old_target,
                    from,
                    to,
                    occurrence.explicit_extension,
                ))?;
                replacements.push((occurrence.target_range.clone(), replacement));
            }
            replacements.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
            for (range, replacement) in replacements {
                rewritten.replace_range(range, &replacement);
            }
            plans.push((source, contents, rewritten));
        }
        Ok(plans)
    }

    pub(crate) fn save(&self, path: &Path, contents: &str) -> Result<()> {
        self.ensure_tracked(path)?;
        fs::write(path, contents).with_context(|| format!("Failed to save {:?}", path))?;
        self.record_completed_write(path);
        self.inner
            .enqueue(CatalogCommand::ApplyPath(path.to_path_buf()));
        Ok(())
    }

    fn record_completed_write(&self, path: &Path) {
        if let Some(fingerprint) = fingerprint(path) {
            self.inner
                .state
                .lock()
                .unwrap()
                .completed_writes
                .insert(path.to_path_buf(), fingerprint);
        }
    }

    fn ensure_inside(&self, path: &Path) -> Result<()> {
        if !path.starts_with(&self.inner.root) {
            bail!("Path escapes Vault: {}", path.display());
        }
        Ok(())
    }

    fn ensure_tracked(&self, path: &Path) -> Result<()> {
        self.ensure_inside(path)?;
        if !self.inner.file_types.is_tracked(path) {
            bail!("File extension is not registered: {}", path.display());
        }
        Ok(())
    }
}

fn remap_renamed_path(path: &Path, from: &Path, to: &Path) -> PathBuf {
    path.strip_prefix(from)
        .map_or_else(|_| path.to_path_buf(), |suffix| to.join(suffix))
}

impl CatalogInner {
    fn enqueue(&self, command: CatalogCommand) {
        if self.commands.send(command).is_err() {
            self.set_degraded();
        }
    }

    fn observe(self: &Arc<Self>, event: Event) {
        if matches!(event.kind, EventKind::Access(_)) {
            return;
        }
        {
            let mut state = self.state.lock().unwrap();
            if !state.initialization_complete {
                state.pending_events.push(event);
                return;
            }
        }

        self.enqueue(CatalogCommand::Event(event));
    }

    fn process_command(self: &Arc<Self>, command: CatalogCommand) {
        match command {
            CatalogCommand::Event(event) => self.process_event(event),
            CatalogCommand::ApplyPath(path) => self.apply_path(&path),
            CatalogCommand::Reconcile => self.reconcile(),
            CatalogCommand::Rename { from, to } => {
                let catalog = VaultCatalog {
                    inner: self.clone(),
                };
                if catalog.perform_rename(&from, &to).is_err() {
                    self.set_degraded();
                }
            }
            #[cfg(test)]
            CatalogCommand::Barrier(sender) => {
                let _ = sender.send(());
            }
        }
    }

    fn process_event(self: &Arc<Self>, event: Event) {
        if matches!(
            event.kind,
            EventKind::Modify(ModifyKind::Name(RenameMode::Both))
        ) && let [from, to] = event.paths.as_slice()
            && !self.is_internal(from)
            && !self.is_internal(to)
        {
            self.apply_external_rename(from, to);
            return;
        }

        let directory_changed = event.paths.iter().any(|path| {
            !self.is_internal(path)
                && (path.is_dir() || matches!(event.kind, EventKind::Remove(RemoveKind::Folder)))
        });
        if directory_changed {
            self.reconcile();
            return;
        }

        for path in event.paths {
            if self.is_internal(&path) {
                continue;
            }
            if !self.file_types.is_tracked(&path) {
                continue;
            }
            let observed = fingerprint(&path);
            let self_write = {
                let state = self.state.lock().unwrap();
                state.completed_writes.get(&path).copied()
            };
            if observed.is_some() && observed == self_write {
                self.state.lock().unwrap().completed_writes.remove(&path);
                continue;
            }
            self.state.lock().unwrap().completed_writes.remove(&path);
            self.apply_path(&path);
        }
    }

    fn apply_external_rename(self: &Arc<Self>, from: &Path, to: &Path) {
        let database = self.database.lock().unwrap().clone();
        let affected = database
            .as_ref()
            .and_then(|database| pollster::block_on(database.link_occurrences_under(from)).ok())
            .unwrap_or_default();
        if let Some(database) = database {
            let catalog = VaultCatalog {
                inner: self.clone(),
            };
            let result = catalog
                .prepare_renamed_links(from, to, affected, &database)
                .and_then(|replacements| {
                    let rewritten_sources = replacements
                        .iter()
                        .map(|(source, _, _)| source.clone())
                        .collect();
                    rename::execute(&self.root, from, to, replacements).map(|()| rewritten_sources)
                });
            let rewritten_sources = match result {
                Ok(rewritten_sources) => rewritten_sources,
                Err(_) => {
                    self.set_degraded();
                    BTreeSet::new()
                }
            };
            self.reconcile_with_changed(&rewritten_sources);
            return;
        }
        self.reconcile();
    }

    fn reconcile(&self) {
        self.reconcile_with_changed(&BTreeSet::new());
    }

    fn reconcile_with_changed(&self, explicitly_changed: &BTreeSet<PathBuf>) {
        let files = discover(&self.root, &self.file_types);
        let database = self.database.lock().unwrap().clone();
        let previous: BTreeSet<_> = database
            .as_ref()
            .and_then(|database| pollster::block_on(database.tracked_paths()).ok())
            .unwrap_or_default()
            .into_iter()
            .collect();
        let removed: Vec<PathBuf> = previous.difference(&files).cloned().collect();
        let added: Vec<PathBuf> = files.difference(&previous).cloned().collect();
        let topology_changed: BTreeSet<_> = removed.iter().chain(&added).cloned().collect();
        let mut state = self.state.lock().unwrap();
        if state.sync != CatalogSyncState::Degraded {
            state.sync = CatalogSyncState::Current;
        }
        drop(state);
        let search = self.search.lock().unwrap().clone();
        if let Some(search) = search {
            for path in &removed {
                let _ = search.indexer.remove_file(path);
            }
            for path in &added {
                let _ = search.indexer.add_file(path);
            }
            for path in explicitly_changed.difference(&topology_changed) {
                let _ = search.indexer.add_file(path);
            }
        }
        if let Some(database) = database {
            if pollster::block_on(database.reconcile(&files, &self.file_types)).is_err() {
                self.set_degraded();
            }
        }
        let tracked_paths_changed = !removed.is_empty() || !added.is_empty();
        self.publish(
            removed.into_iter().chain(added).collect(),
            true,
            tracked_paths_changed,
        );
    }

    fn apply_path(&self, path: &Path) {
        let was_tracked = self
            .database
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|database| pollster::block_on(database.contains_document(path)).ok())
            .unwrap_or(false);
        let is_tracked = path.is_file() && self.file_types.is_tracked(path);
        let tracked_paths_changed = was_tracked != is_tracked;
        if self.update_projections(path).is_err() {
            self.set_degraded();
        }
        self.publish(
            vec![path.to_path_buf()],
            tracked_paths_changed,
            tracked_paths_changed,
        );
    }

    fn update_projections(&self, path: &Path) -> Result<()> {
        if path.is_file() {
            let search = self.search.lock().unwrap().clone();
            if let Some(search) = search {
                search.indexer.add_file(path)?;
            }
            let database = self.database.lock().unwrap().clone();
            if let Some(database) = database {
                pollster::block_on(database.upsert_file(path, &self.file_types))?;
                pollster::block_on(database.update_affected_links(path))?;
            }
        } else {
            let search = self.search.lock().unwrap().clone();
            if let Some(search) = search {
                search.indexer.remove_file(path)?;
            }
            let database = self.database.lock().unwrap().clone();
            if let Some(database) = database {
                pollster::block_on(database.remove_file(path))?;
            }
        }
        Ok(())
    }

    fn set_degraded(&self) {
        self.state.lock().unwrap().sync = CatalogSyncState::Degraded;
        self.publish(Vec::new(), false, false);
    }

    fn publish(
        &self,
        changed_paths: Vec<PathBuf>,
        structure_changed: bool,
        tracked_paths_changed: bool,
    ) {
        let mut state = self.state.lock().unwrap();
        let update = CatalogUpdate {
            changed_paths: changed_paths.into(),
            structure_changed,
            tracked_paths_changed,
        };
        state
            .subscribers
            .retain(|subscriber| subscriber.send(update.clone()).is_ok());
    }

    fn is_internal(&self, path: &Path) -> bool {
        path.strip_prefix(&self.root)
            .ok()
            .and_then(|path| path.components().next())
            .is_some_and(|part| part.as_os_str() == METADATA_DIR)
    }
}

fn discover(root: &Path, file_types: &RegisteredFileTypes) -> BTreeSet<PathBuf> {
    let mut files = BTreeSet::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let Ok(entries) = fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.file_name().is_some_and(|name| name == METADATA_DIR) {
                continue;
            }
            if path.is_dir() {
                pending.push(path);
            } else if file_types.is_tracked(&path) {
                files.insert(path);
            }
        }
    }
    files
}

fn fingerprint(path: &Path) -> Option<u64> {
    let metadata = fs::metadata(path).ok()?;
    let mut hasher = DefaultHasher::new();
    metadata.len().hash(&mut hasher);
    metadata.modified().ok()?.hash(&mut hasher);
    Some(hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::file_types::FileTypeCapabilities;
    use notify::event::{DataChange, ModifyKind};

    fn types() -> RegisteredFileTypes {
        RegisteredFileTypes::new([(
            "md".to_string(),
            FileTypeCapabilities {
                text_search: true,
                wiki_links: true,
                yaml_frontmatter: true,
            },
        )])
    }

    #[test]
    fn catalog_tracks_only_registered_extensions() {
        let root = std::env::temp_dir().join(format!("datalith-catalog-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("tracked.md"), "tracked").unwrap();
        fs::write(root.join("ignored.bin"), "ignored").unwrap();

        let catalog = VaultCatalog::open(root.clone(), types()).unwrap();
        for _ in 0..100 {
            if !catalog.tracked_paths().is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(
            catalog.tracked_paths().as_slice(),
            &[root.join("tracked.md")]
        );

        drop(catalog);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn mutation_is_visible_after_queued_projection_work() {
        let root =
            std::env::temp_dir().join(format!("datalith-catalog-mutation-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let catalog = VaultCatalog::open(root.clone(), types()).unwrap();
        for _ in 0..100 {
            if catalog.initialization_complete() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        let created = catalog.create_file(&root).unwrap();
        catalog.wait_for_idle();
        assert!(catalog.tracked_paths().contains(&created));

        drop(catalog);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn untracked_file_change_does_not_reconcile_the_vault() {
        let root =
            std::env::temp_dir().join(format!("datalith-catalog-untracked-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let ignored = root.join("ignored.bin");
        fs::write(&ignored, "ignored").unwrap();
        let catalog = VaultCatalog::open(root.clone(), types()).unwrap();
        let updates = catalog.subscribe();
        for _ in 0..100 {
            if catalog.sync_state() != CatalogSyncState::Discovering {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        while updates.try_recv().is_ok() {}
        catalog.inner.observe(
            Event::new(EventKind::Modify(ModifyKind::Data(DataChange::Any))).add_path(ignored),
        );
        catalog.wait_for_idle();
        assert!(matches!(
            updates.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));

        drop(catalog);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rename_rewrites_links_and_embeds_preserving_suffixes() {
        let root = std::env::temp_dir().join(format!(
            "datalith-catalog-rename-links-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("notes")).unwrap();
        fs::write(root.join("notes/Note.md"), "Note").unwrap();
        fs::write(
            root.join("Source.md"),
            "[[notes/Note#Heading|Alias]] ![[notes/Note]]",
        )
        .unwrap();
        let catalog = VaultCatalog::open(root.clone(), types()).unwrap();
        for _ in 0..100 {
            if catalog.initialization_complete() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        catalog
            .rename(&root.join("notes/Note.md"), &root.join("notes/Renamed.md"))
            .unwrap();
        catalog.wait_for_idle();

        assert_eq!(
            fs::read_to_string(root.join("Source.md")).unwrap(),
            "[[Renamed#Heading|Alias]] ![[Renamed]]"
        );
        assert!(
            catalog.search("Renamed").contains(&root.join("Source.md")),
            "Tantivy must reindex Markdown sources rewritten by rename refactoring"
        );
        drop(catalog);
        let _ = fs::remove_dir_all(root);
    }
}
