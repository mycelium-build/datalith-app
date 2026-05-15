use std::path::PathBuf;

use super::SearchEngine;

#[derive(Clone)]
pub struct QuickSwitcherEntry {
    pub path: PathBuf,
    pub name: String,
    pub open: bool,
}

pub fn collect_from_engine(engine: &SearchEngine) -> Vec<QuickSwitcherEntry> {
    let mut entries: Vec<QuickSwitcherEntry> = engine
        .all_paths()
        .into_iter()
        .map(|path| {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Unknown")
                .to_string();
            QuickSwitcherEntry {
                path,
                name,
                open: false,
            }
        })
        .collect();
    entries.sort_by_key(|a| a.name.to_lowercase());
    entries
}

pub fn filter(
    all_files: &[QuickSwitcherEntry],
    open_files: &[PathBuf],
    query: &str,
) -> Vec<QuickSwitcherEntry> {
    let query = query.trim();
    let mut results: Vec<QuickSwitcherEntry> = all_files.to_vec();
    for entry in &mut results {
        entry.open = open_files.contains(&entry.path);
    }
    if query.is_empty() {
        results.retain(|e| e.open);
    } else {
        let query_lower = query.to_lowercase();
        results.retain(|e| e.name.to_lowercase().contains(&query_lower));
        results.sort_by(|a, b| {
            b.open
                .cmp(&a.open)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
    }
    results
}

pub fn nav_idx(down: bool, selected: Option<usize>, count: usize) -> usize {
    match (down, selected) {
        (true, Some(i)) if i + 1 < count => i + 1,
        (true, _) => 0,
        (false, Some(i)) if i > 0 => i - 1,
        (false, _) => count - 1,
    }
}
