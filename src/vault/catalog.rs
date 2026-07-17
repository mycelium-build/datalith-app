use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};

use anyhow::{Context, Result, bail};
use notify::{
    Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher, event::RemoveKind,
};

use crate::document::file_types::RegisteredFileTypes;
use crate::vault::file_ops;
use crate::vault::links::{WikiLinkEdge, WikiLinkIndex};
use crate::vault::search::SearchEngine;

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
    files: BTreeSet<PathBuf>,
    sync: CatalogSyncState,
    initialization_complete: bool,
    subscribers: Vec<mpsc::Sender<CatalogUpdate>>,
    completed_writes: HashMap<PathBuf, u64>,
}

struct CatalogInner {
    root: PathBuf,
    file_types: RegisteredFileTypes,
    state: Mutex<CatalogState>,
    search: Mutex<Option<SearchEngine>>,
    links: Mutex<Option<WikiLinkIndex>>,
    watcher: Mutex<Option<RecommendedWatcher>>,
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

        let inner = Arc::new(CatalogInner {
            root: root.clone(),
            file_types,
            state: Mutex::new(CatalogState {
                files: BTreeSet::new(),
                sync: CatalogSyncState::Discovering,
                initialization_complete: false,
                subscribers: Vec::new(),
                completed_writes: HashMap::new(),
            }),
            search: Mutex::new(None),
            links: Mutex::new(None),
            watcher: Mutex::new(None),
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
                let search = SearchEngine::new(&initialization_root, &initialization_types).ok();
                let links = WikiLinkIndex::new(&initialization_root, &initialization_types);
                let Some(inner) = initialization_inner.upgrade() else {
                    return;
                };
                *inner.search.lock().unwrap() = search;
                *inner.links.lock().unwrap() = Some(links);
                let files = discover(&initialization_root, &initialization_types);
                let changed_paths = files.iter().cloned().collect();
                {
                    let mut state = inner.state.lock().unwrap();
                    state.files = files;
                    state.initialization_complete = true;
                    if state.sync != CatalogSyncState::Degraded {
                        state.sync = CatalogSyncState::Current;
                    }
                }
                inner.publish(changed_paths, true, true);
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
            .state
            .lock()
            .unwrap()
            .files
            .iter()
            .cloned()
            .collect()
    }

    #[must_use]
    pub(crate) fn root(&self) -> PathBuf {
        self.inner.root.clone()
    }

    #[must_use]
    pub(crate) fn initialization_complete(&self) -> bool {
        self.inner.state.lock().unwrap().initialization_complete
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
            .as_ref()
            .map(|search| search.search(query))
            .unwrap_or_default()
    }

    #[must_use]
    pub(crate) fn resolve_wiki_link_from(&self, source: &Path, name: &str) -> Option<PathBuf> {
        self.inner
            .links
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|links| links.resolve(Some(source), name))
    }

    #[must_use]
    pub(crate) fn wiki_link_edges(&self) -> Vec<WikiLinkEdge> {
        self.inner
            .links
            .lock()
            .unwrap()
            .as_ref()
            .map(WikiLinkIndex::edges)
            .unwrap_or_default()
    }

    pub(crate) fn create_file(&self, target: &Path) -> Result<PathBuf> {
        let path = file_ops::new_file_from_target(target)?;
        self.record_completed_write(&path);
        self.inner.apply_path(&path);
        Ok(path)
    }

    pub(crate) fn create_folder(&self, target: &Path) -> Result<PathBuf> {
        let path = file_ops::new_folder_from_target(target)?;
        self.inner.publish(vec![path.clone()], true, false);
        Ok(path)
    }

    pub(crate) fn duplicate(&self, target: &Path) -> Result<PathBuf> {
        let path = file_ops::duplicate_target(target)?;
        self.inner.reconcile();
        Ok(path)
    }

    pub(crate) fn delete(&self, target: &Path) -> Result<()> {
        self.ensure_inside(target)?;
        file_ops::delete_target(target)?;
        self.inner.reconcile();
        Ok(())
    }

    pub(crate) fn rename(&self, from: &Path, to: &Path) -> Result<()> {
        self.ensure_inside(from)?;
        self.ensure_inside(to)?;
        fs::rename(from, to).with_context(|| format!("Failed to rename {:?} to {:?}", from, to))?;
        self.inner.reconcile();
        Ok(())
    }

    pub(crate) fn save(&self, path: &Path, contents: &str) -> Result<()> {
        self.ensure_tracked(path)?;
        fs::write(path, contents).with_context(|| format!("Failed to save {:?}", path))?;
        self.record_completed_write(path);
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

impl CatalogInner {
    fn observe(&self, event: Event) {
        if matches!(event.kind, EventKind::Access(_)) {
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
                self.update_projections(&path);
                continue;
            }
            self.state.lock().unwrap().completed_writes.remove(&path);
            self.apply_path(&path);
        }
    }

    fn reconcile(&self) {
        let files = discover(&self.root, &self.file_types);
        let mut state = self.state.lock().unwrap();
        let removed: Vec<PathBuf> = state.files.difference(&files).cloned().collect();
        let added: Vec<PathBuf> = files.difference(&state.files).cloned().collect();
        state.files = files;
        if state.sync != CatalogSyncState::Degraded {
            state.sync = CatalogSyncState::Current;
        }
        drop(state);
        if let Some(search) = self.search.lock().unwrap().as_ref() {
            for path in &removed {
                let _ = search.indexer.remove_file(path);
            }
            for path in &added {
                let _ = search.indexer.add_file(path);
            }
        }
        if let Some(links) = self.links.lock().unwrap().as_mut() {
            for path in &removed {
                links.remove_file(path);
            }
            for path in &added {
                links.add_file(path);
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
        let mut state = self.state.lock().unwrap();
        let tracked_paths_changed = if path.is_file() && self.file_types.is_tracked(path) {
            state.files.insert(path.to_path_buf())
        } else {
            state.files.remove(path)
        };
        drop(state);
        self.update_projections(path);
        self.publish(
            vec![path.to_path_buf()],
            tracked_paths_changed,
            tracked_paths_changed,
        );
    }

    fn update_projections(&self, path: &Path) {
        if path.is_file() {
            if let Some(search) = self.search.lock().unwrap().as_ref() {
                let _ = search.indexer.add_file(path);
            }
            if let Some(links) = self.links.lock().unwrap().as_mut() {
                links.add_file(path);
            }
        } else {
            if let Some(search) = self.search.lock().unwrap().as_ref() {
                let _ = search.indexer.remove_file(path);
            }
            if let Some(links) = self.links.lock().unwrap().as_mut() {
                links.remove_file(path);
            }
        }
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
    fn mutation_is_visible_in_tracked_paths_before_projection_work() {
        let root =
            std::env::temp_dir().join(format!("datalith-catalog-mutation-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let catalog = VaultCatalog::open(root.clone(), types()).unwrap();

        let created = catalog.create_file(&root).unwrap();
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
        assert!(matches!(
            updates.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));

        drop(catalog);
        let _ = fs::remove_dir_all(root);
    }
}
