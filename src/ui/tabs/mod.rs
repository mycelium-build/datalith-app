mod navigation;
mod render;

use std::path::{Path, PathBuf};

use gpui::{Entity, Subscription};

use crate::document::handler::FileHandler;

#[derive(Clone, Copy, Debug)]
pub enum NavigationAction {
    GoBack,
    GoForward,
}

pub struct Tab {
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
        self.active().map(|tab| tab.path.as_path())
    }

    pub(crate) fn active_handler(&self) -> Option<&Entity<FileHandler>> {
        self.active().map(|tab| &tab.handler)
    }

    pub(crate) fn handler_for_path(&self, path: &Path) -> Option<&Entity<FileHandler>> {
        self.entries
            .iter()
            .find(|tab| tab.path == path)
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
        self.entries.iter().position(|tab| tab.path == path)
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
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) const fn handler(&self) -> &Entity<FileHandler> {
        &self.handler
    }
}

#[cfg(test)]
mod tests {
    use super::active_after_removal;

    #[test]
    fn closing_before_active_tab_preserves_the_same_selection() {
        assert_eq!(active_after_removal(Some(2), 0, 2), Some(1));
    }

    #[test]
    fn closing_active_last_tab_selects_new_last_tab() {
        assert_eq!(active_after_removal(Some(2), 2, 2), Some(1));
    }

    #[test]
    fn closing_only_tab_clears_selection() {
        assert_eq!(active_after_removal(Some(0), 0, 0), None);
    }
}
