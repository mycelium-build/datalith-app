use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::document::file_types::RegisteredFileTypes;
use crate::vault::path::display_name;

const CACHE_FILE: &str = "link_cache.json";

#[derive(Clone, Serialize)]
pub(crate) struct LinkCache {
    root: PathBuf,
    name_to_path: HashMap<String, PathBuf>,
    #[serde(skip)]
    file_types: RegisteredFileTypes,
}

impl LinkCache {
    pub(crate) fn new(root: &Path, file_types: &RegisteredFileTypes) -> Self {
        Self::build(root, file_types)
    }

    fn build(root: &Path, file_types: &RegisteredFileTypes) -> Self {
        let files = walk_linkable_files(root, file_types);
        let mut name_to_path: HashMap<String, PathBuf> = HashMap::with_capacity(files.len());
        for path in files {
            let name = file_stem_str(&path);
            name_to_path.insert(name, path);
        }
        let cache = Self {
            root: root.to_path_buf(),
            name_to_path,
            file_types: file_types.clone(),
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
        if !self
            .file_types
            .capabilities(path)
            .is_some_and(|capabilities| capabilities.wiki_links)
            || !path.is_file()
        {
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
}

fn walk_linkable_files(root: &Path, file_types: &RegisteredFileTypes) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.file_name().is_some_and(|name| name == ".datalith") {
                    continue;
                }
                if path.is_dir() {
                    stack.push(path);
                } else if file_types
                    .capabilities(&path)
                    .is_some_and(|capabilities| capabilities.wiki_links)
                {
                    paths.push(path);
                }
            }
        }
    }
    paths
}

fn file_stem_str(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_else(|| display_name(path))
        .to_string()
}
