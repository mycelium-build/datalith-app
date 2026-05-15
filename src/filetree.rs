use std::fs;
use std::path::Path;

use gpui_component::tree::TreeItem;

use crate::utils::file_name_str;

pub fn build_file_items(path: &Path) -> Vec<TreeItem> {
    let mut dirs = Vec::new();
    let mut files = Vec::new();

    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            let name = file_name_str(&entry_path).to_string();

            if name.starts_with('.') {
                continue;
            }

            if entry_path.is_dir() {
                let children = build_file_items(&entry_path);
                dirs.push((
                    name.clone(),
                    TreeItem::new(entry_path.to_string_lossy().to_string(), name).children(children),
                ));
            } else {
                files.push((
                    name.clone(),
                    TreeItem::new(entry_path.to_string_lossy().to_string(), name),
                ));
            }
        }
    }

    dirs.sort_by_key(|a| a.0.to_lowercase());
    files.sort_by_key(|a| a.0.to_lowercase());

    let mut items: Vec<TreeItem> = dirs.into_iter().map(|(_, item)| item).collect();
    items.extend(files.into_iter().map(|(_, item)| item));
    items
}
