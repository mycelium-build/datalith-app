use gpui_component::tree::TreeItem;
use std::fs;
use std::path::Path;

pub fn build_file_items(path: &Path) -> Vec<TreeItem> {
    let mut dirs = Vec::new();
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Unknown")
                .to_string();

            if name.starts_with('.') {
                continue;
            }

            if path.is_dir() {
                let children = build_file_items(&path);
                dirs.push((
                    name.clone(),
                    TreeItem::new(path.to_string_lossy().to_string(), name).children(children),
                ));
            } else {
                files.push((
                    name.clone(),
                    TreeItem::new(path.to_string_lossy().to_string(), name),
                ));
            }
        }
    }
    dirs.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    files.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    let mut items: Vec<TreeItem> = dirs.into_iter().map(|(_, item)| item).collect();
    items.extend(files.into_iter().map(|(_, item)| item));
    items
}
