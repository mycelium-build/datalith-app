mod navigation;
mod render;

pub use navigation::NavigationAction;

use std::path::{Path, PathBuf};

use gpui_kit::{Entity, Subscription};

use crate::app::workspace::{TabId, WorkspaceTab};
use crate::document::handler::FileHandler;

pub struct Tab {
    id: TabId,
    path: PathBuf,
    handler: Entity<FileHandler>,
    _input_subscription: Option<Subscription>,
    _event_subscription: Option<Subscription>,
    history: Vec<PathBuf>,
    history_position: usize,
}

pub struct Tabs {
    entries: Vec<Tab>,
    active: Option<usize>,
}

impl Tabs {
    pub(crate) const fn new() -> Self {
        Self {
            entries: Vec::new(),
            active: None,
        }
    }

    pub(crate) const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn active_index(&self) -> Option<usize> {
        self.active.filter(|&index| index < self.entries.len())
    }

    pub(crate) fn active(&self) -> Option<&Tab> {
        self.active_index()
            .and_then(|index| self.entries.get(index))
    }

    pub(crate) fn active_path(&self) -> Option<&Path> {
        self.active()
            .map(|tab| tab.path.as_path())
            .filter(|path| !path.as_os_str().is_empty())
    }

    pub(crate) fn active_handler(&self) -> Option<&Entity<FileHandler>> {
        self.active().map(|tab| &tab.handler)
    }

    pub(crate) fn active_tab_id(&self) -> Option<&TabId> {
        self.active().map(|tab| &tab.id)
    }

    pub(crate) fn handler_for_path(&self, path: &Path) -> Option<&Entity<FileHandler>> {
        self.entries
            .iter()
            .find(|tab| same_document(&tab.path, path))
            .map(|tab| &tab.handler)
    }

    pub(crate) fn open_paths(&self) -> Vec<PathBuf> {
        self.entries.iter().map(|tab| tab.path.clone()).collect()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (usize, &Path, &Entity<FileHandler>)> {
        self.entries
            .iter()
            .enumerate()
            .map(|(index, tab)| (index, tab.path.as_path(), &tab.handler))
    }

    pub(crate) fn iter_with_id(
        &self,
    ) -> impl Iterator<Item = (usize, &TabId, &Path, &Entity<FileHandler>)> {
        self.entries
            .iter()
            .enumerate()
            .map(|(index, tab)| (index, &tab.id, tab.path.as_path(), &tab.handler))
    }

    pub(crate) fn snapshot(&self, cx: &gpui_kit::App) -> (Vec<WorkspaceTab>, Option<TabId>) {
        let tabs = self
            .entries
            .iter()
            .map(|tab| {
                WorkspaceTab::new(
                    tab.id.clone(),
                    (!tab.path.as_os_str().is_empty()).then(|| tab.path.clone()),
                    tab.handler.read(cx).mode(),
                )
            })
            .collect();
        let active_id = self.active_tab_id().cloned();
        (tabs, active_id)
    }

    pub(crate) const fn select(&mut self, index: usize) -> bool {
        if index >= self.entries.len() {
            return false;
        }
        self.active = Some(index);
        true
    }

    pub(crate) const fn select_last(&mut self) -> bool {
        let Some(index) = self.entries.len().checked_sub(1) else {
            return false;
        };
        self.active = Some(index);
        true
    }

    fn find_path(&self, path: &Path) -> Option<usize> {
        if path.as_os_str().is_empty() {
            return None;
        }
        let active = self.active_index();
        if let Some(index) = active
            && self
                .entries
                .get(index)
                .is_some_and(|tab| same_document(&tab.path, path))
        {
            return Some(index);
        }
        self.entries
            .iter()
            .position(|tab| same_document(&tab.path, path))
    }

    pub(crate) fn select_by_id(&mut self, id: &TabId) -> bool {
        let Some(index) = self.entries.iter().position(|tab| &tab.id == id) else {
            return false;
        };
        self.active = Some(index);
        true
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.active = None;
    }

    fn insert(&mut self, tab: Tab, new_tab: bool) {
        if new_tab || self.entries.is_empty() {
            let index = self.entries.len();
            self.entries.push(tab);
            self.active = Some(index);
        } else if let Some(active) = self.active_index()
            && let Some(entry) = self.entries.get_mut(active)
        {
            *entry = tab;
        }
    }

    fn remove(&mut self, index: usize) -> bool {
        if index >= self.entries.len() {
            return false;
        }
        let active = self.active;
        self.entries.remove(index);
        self.active = active_after_removal(active, index, self.entries.len());
        true
    }

    pub(crate) fn rename_path(&mut self, old_path: &Path, new_path: &Path) {
        for tab in &mut self.entries {
            if let Ok(suffix) = tab.path.strip_prefix(old_path) {
                tab.path = new_path.join(suffix);
            }
            for history_path in &mut tab.history {
                if let Ok(suffix) = history_path.strip_prefix(old_path) {
                    *history_path = new_path.join(suffix);
                }
            }
        }
    }
}

fn active_after_removal(active: Option<usize>, removed: usize, remaining: usize) -> Option<usize> {
    if remaining == 0 {
        return None;
    }
    match active {
        Some(active) if active > removed => Some(active.saturating_sub(1)),
        Some(active) => Some(active.min(remaining.saturating_sub(1))),
        None => Some(0),
    }
}

impl Tab {
    pub(crate) const fn id(&self) -> &TabId {
        &self.id
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) const fn handler(&self) -> &Entity<FileHandler> {
        &self.handler
    }
}

fn same_document(left: &Path, right: &Path) -> bool {
    if left.as_os_str().is_empty() || right.as_os_str().is_empty() {
        return false;
    }
    normalized_path(left) == normalized_path(right)
}

fn normalized_path(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().map_or_else(|_| path.to_path_buf(), |cwd| cwd.join(path))
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push(component.as_os_str());
                }
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::{active_after_removal, same_document};
    use std::path::Path;

    #[test]
    fn closing_tabs_keeps_a_valid_selection_or_clears_the_last_one() {
        assert_eq!(active_after_removal(Some(2), 0, 2), Some(1));
        assert_eq!(active_after_removal(Some(2), 2, 2), Some(1));
        assert_eq!(active_after_removal(Some(0), 0, 0), None);
    }

    #[test]
    fn document_identity_uses_normalized_paths_and_excludes_empty_tabs() {
        assert!(same_document(
            Path::new("./notes/../notes/today.md"),
            Path::new("notes/today.md")
        ));
        assert!(!same_document(Path::new(""), Path::new("notes/today.md")));
    }
}
