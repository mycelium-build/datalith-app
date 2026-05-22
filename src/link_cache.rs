use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::search::index::{is_indexable, walk_indexable_files};
use crate::utils::file_name_str;

const CACHE_FILE: &str = "link_cache.json";

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct LinkCache {
    root: PathBuf,
    name_to_path: HashMap<String, PathBuf>,
}

impl LinkCache {
    pub(crate) fn new(root: &Path) -> Self {
        let cache_path = root.join(".datalith").join(CACHE_FILE);
        if let Ok(data) = fs::read_to_string(&cache_path)
            && let Ok(cache) = serde_json::from_str::<LinkCache>(&data)
            && cache.root.as_path() == root
        {
            return cache;
        }
        Self::build(root)
    }

    fn build(root: &Path) -> Self {
        let files = walk_indexable_files(root);
        let mut name_to_path: HashMap<String, PathBuf> = HashMap::with_capacity(files.len());
        for path in files {
            let name = file_stem_str(&path);
            name_to_path.insert(name, path);
        }
        let cache = Self {
            root: root.to_path_buf(),
            name_to_path,
        };
        cache.save();
        cache
    }

    pub(crate) fn save(&self) {
        let cache_path = self.root.join(".datalith").join(CACHE_FILE);
        if let Some(parent) = cache_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(data) = serde_json::to_string(&self) {
            let _ = fs::write(&cache_path, data);
        }
    }

    pub(crate) fn resolve(&self, name: &str) -> Option<PathBuf> {
        if let Some(path) = self.name_to_path.get(name) {
            return Some(path.clone());
        }
        if let Some(stripped) = name.strip_suffix(".md") {
            self.name_to_path.get(stripped).cloned()
        } else {
            None
        }
    }

    pub(crate) fn add_file(&mut self, path: &Path) {
        if !is_indexable(path) || !path.is_file() {
            return;
        }
        let name = file_stem_str(path);
        self.name_to_path.insert(name, path.to_path_buf());
        self.save();
    }

    pub(crate) fn remove_file(&mut self, path: &Path) {
        let key = file_stem_str(path);
        if let Some(existing) = self.name_to_path.get(&key) {
            if existing == path {
                self.name_to_path.remove(&key);
                self.save();
            }
        }
    }

    pub(crate) fn rename_file(&mut self, old_path: &Path, new_path: &Path) {
        self.remove_file(old_path);
        self.add_file(new_path);
    }

    pub(crate) fn remove_under(&mut self, root: &Path) {
        let prefix = root.to_string_lossy().to_string();
        let keys_to_remove: Vec<String> = self
            .name_to_path
            .iter()
            .filter(|(_, path)| path.to_string_lossy().starts_with(&prefix))
            .map(|(k, _)| k.clone())
            .collect();
        let had_removals = !keys_to_remove.is_empty();
        for key in keys_to_remove {
            self.name_to_path.remove(&key);
        }
        if had_removals {
            self.save();
        }
    }
}

fn file_stem_str(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_else(|| file_name_str(path))
        .to_string()
}
