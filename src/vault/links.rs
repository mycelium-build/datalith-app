use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::document::file_types::RegisteredFileTypes;

const CACHE_FILE: &str = "link_cache.json";

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
pub(crate) struct WikiLinkEdge {
    pub(crate) source: PathBuf,
    pub(crate) target: PathBuf,
}

#[derive(Clone, Serialize)]
pub(crate) struct WikiLinkIndex {
    root: PathBuf,
    stems: HashMap<String, BTreeSet<PathBuf>>,
    outgoing: HashMap<PathBuf, Vec<String>>,
    #[serde(skip)]
    file_types: RegisteredFileTypes,
}

impl WikiLinkIndex {
    pub(crate) fn new(root: &Path, file_types: &RegisteredFileTypes) -> Self {
        let mut index = Self {
            root: root.to_path_buf(),
            stems: HashMap::new(),
            outgoing: HashMap::new(),
            file_types: file_types.clone(),
        };
        for path in walk_linkable_files(root, file_types) {
            index.index_file(&path);
        }
        index.save();
        index
    }

    pub(crate) fn save(&self) {
        let cache_path = self.root.join(".datalith").join(CACHE_FILE);
        if let Some(parent) = cache_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(data) = serde_json::to_string(self) {
            let _ = fs::write(cache_path, data);
        }
    }

    pub(crate) fn resolve(&self, source: Option<&Path>, name: &str) -> Option<PathBuf> {
        let target = normalize_target(name);
        if target.contains('/') || target.ends_with(".md") {
            let relative = PathBuf::from(&target);
            for candidate in [relative.clone(), relative.with_extension("md")] {
                let absolute = self.root.join(candidate);
                if absolute.is_file() && self.is_linkable(&absolute) {
                    return Some(absolute);
                }
            }
            return None;
        }

        let candidates = self.stems.get(&target)?;
        if candidates.len() == 1 {
            return candidates.first().cloned();
        }
        let source_parent = source.and_then(Path::parent);
        let same_folder: Vec<_> = candidates
            .iter()
            .filter(|candidate| candidate.parent() == source_parent)
            .cloned()
            .collect();
        (same_folder.len() == 1).then(|| same_folder[0].clone())
    }

    pub(crate) fn edges(&self) -> Vec<WikiLinkEdge> {
        let mut edges = BTreeSet::new();
        for (source, targets) in &self.outgoing {
            for target in targets {
                if let Some(target) = self.resolve(Some(source), target) {
                    edges.insert((source.clone(), target));
                }
            }
        }
        edges
            .into_iter()
            .map(|(source, target)| WikiLinkEdge { source, target })
            .collect()
    }

    pub(crate) fn add_file(&mut self, path: &Path) {
        self.remove_file(path);
        if path.is_file() && self.is_linkable(path) {
            self.index_file(path);
        }
        self.save();
    }

    pub(crate) fn remove_file(&mut self, path: &Path) {
        let stem = file_stem_str(path);
        if let Some(paths) = self.stems.get_mut(&stem) {
            paths.remove(path);
            if paths.is_empty() {
                self.stems.remove(&stem);
            }
        }
        self.outgoing.remove(path);
        self.save();
    }

    fn index_file(&mut self, path: &Path) {
        self.stems
            .entry(file_stem_str(path))
            .or_default()
            .insert(path.to_path_buf());
        let links = fs::read_to_string(path)
            .map(|source| extract_wiki_links(&source))
            .unwrap_or_default();
        self.outgoing.insert(path.to_path_buf(), links);
    }

    fn is_linkable(&self, path: &Path) -> bool {
        self.file_types
            .capabilities(path)
            .is_some_and(|capabilities| capabilities.wiki_links)
    }
}

fn normalize_target(name: &str) -> String {
    name.split('|')
        .next()
        .unwrap_or(name)
        .split('#')
        .next()
        .unwrap_or(name)
        .trim()
        .trim_start_matches('/')
        .replace('\\', "/")
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
    paths.sort();
    paths
}

fn file_stem_str(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default()
        .to_string()
}

pub(crate) fn extract_wiki_links(source: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut fenced = false;
    for line in source.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        let bytes = line.as_bytes();
        let mut index = 0;
        let mut inline_code = false;
        while index < bytes.len() {
            if bytes[index] == b'`' {
                inline_code = !inline_code;
                index += 1;
                continue;
            }
            if !inline_code
                && bytes[index..].starts_with(b"[[")
                && (index == 0 || bytes[index - 1] != b'!')
            {
                if let Some(end) = line[index + 2..].find("]]") {
                    let value = &line[index + 2..index + 2 + end];
                    let target = normalize_target(value);
                    if !target.is_empty() {
                        links.push(target);
                    }
                    index += 4 + end;
                    continue;
                }
            }
            index += 1;
        }
    }
    links
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::file_types::FileTypeCapabilities;

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
    fn extracts_body_and_frontmatter_links_but_not_code_or_embeds() {
        let source = "---\nrelated: [[Frontmatter]]\n---\n[[Body]] ![[Embed]] `[[Inline]]`\n```md\n[[Fence]]\n```";
        assert_eq!(extract_wiki_links(source), vec!["Frontmatter", "Body"]);
    }

    #[test]
    fn resolves_paths_unique_names_and_same_folder_ambiguity() {
        let root = std::env::temp_dir().join(format!("datalith-links-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("A")).unwrap();
        fs::create_dir_all(root.join("B")).unwrap();
        fs::write(root.join("A/Source.md"), "[[Note]] [[B/Note]]").unwrap();
        fs::write(root.join("A/Note.md"), "").unwrap();
        fs::write(root.join("B/Note.md"), "").unwrap();
        let index = WikiLinkIndex::new(&root, &types());
        assert_eq!(
            index.resolve(Some(&root.join("A/Source.md")), "Note"),
            Some(root.join("A/Note.md"))
        );
        assert_eq!(index.resolve(None, "Note"), None);
        assert_eq!(index.resolve(None, "B/Note"), Some(root.join("B/Note.md")));
        assert_eq!(index.edges().len(), 2);
        let _ = fs::remove_dir_all(root);
    }
}
