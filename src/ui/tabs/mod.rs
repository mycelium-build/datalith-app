mod navigation;
mod render;
mod workspace;

pub use navigation::NavigationAction;

use std::path::{Path, PathBuf};

use gpui_kit::{Entity, EntityId, Subscription};

use crate::app::workspace::{TabId, WorkspaceTab};
use crate::document::handler::FileHandler;
use crate::vault::file_ops;

use super::{shortcuts::ShortcutsView, themes::ThemeEditor};

pub enum Tab {
    Document(DocumentTab),
    Theme {
        editor: Entity<ThemeEditor>,
        _change_subscription: Subscription,
    },
    Shortcuts(Entity<ShortcutsView>),
}

pub struct DocumentTab {
    id: TabId,
    path: Option<PathBuf>,
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
        self.active_document().and_then(|tab| tab.path.as_deref())
    }

    pub(crate) fn active_handler(&self) -> Option<&Entity<FileHandler>> {
        self.active_document().map(|tab| &tab.handler)
    }

    /// ID of the active document, or `None` when a tool tab is active or no tab is active.
    pub(crate) fn active_document_id(&self) -> Option<&TabId> {
        self.active_document().map(|tab| &tab.id)
    }

    pub(crate) fn handlers_for_path(
        &self,
        path: &Path,
    ) -> impl Iterator<Item = &Entity<FileHandler>> {
        self.entries
            .iter()
            .filter_map(Tab::document)
            .filter(move |tab| same_document(tab.path.as_deref(), Some(path)))
            .map(|tab| &tab.handler)
    }

    /// Paths of tabs that hold a document. Empty tabs are omitted.
    pub(crate) fn open_paths(&self) -> Vec<PathBuf> {
        self.entries
            .iter()
            .filter_map(Tab::document)
            .filter_map(|tab| tab.path.clone())
            .collect()
    }

    /// Iterate over document tabs with their indices in the full tab collection.
    /// Indices can have gaps where theme or shortcuts tabs are omitted.
    pub(crate) fn iter_documents(
        &self,
    ) -> impl Iterator<Item = (usize, Option<&Path>, &Entity<FileHandler>)> {
        self.entries.iter().enumerate().filter_map(|(index, tab)| {
            tab.document()
                .map(|tab| (index, tab.path.as_deref(), &tab.handler))
        })
    }

    fn active_document(&self) -> Option<&DocumentTab> {
        self.active().and_then(Tab::document)
    }

    pub(crate) fn theme_editor_for(
        &self,
        family_id: u64,
        cx: &gpui_kit::App,
    ) -> Option<&Entity<ThemeEditor>> {
        self.entries.iter().find_map(|tab| match tab {
            Tab::Theme { editor, .. } if editor.read(cx).family_id() == family_id => Some(editor),
            _ => None,
        })
    }

    /// Persist documents only; theme editors and shortcuts are session-only tabs.
    pub(crate) fn snapshot(&self, cx: &gpui_kit::App) -> (Vec<WorkspaceTab>, Option<TabId>) {
        let tabs = self
            .entries
            .iter()
            .filter_map(Tab::document)
            .map(|tab| {
                WorkspaceTab::new(
                    tab.id.clone(),
                    tab.path.clone(),
                    tab.handler.read(cx).mode(),
                )
            })
            .collect();
        let active_id = self.active_document_id().cloned();
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
        let active = self.active_index();
        if let Some(index) = active
            && self
                .entries
                .get(index)
                .and_then(Tab::document)
                .is_some_and(|tab| same_document(tab.path.as_deref(), Some(path)))
        {
            return Some(index);
        }
        self.entries.iter().position(|tab| {
            tab.document()
                .is_some_and(|tab| same_document(tab.path.as_deref(), Some(path)))
        })
    }

    pub(crate) fn select_by_id(&mut self, id: &TabId) -> bool {
        let Some(index) = self
            .entries
            .iter()
            .position(|tab| tab.document().is_some_and(|tab| &tab.id == id))
        else {
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
        // Opening a document must never replace a long-lived workspace tab.
        if new_tab || self.active_document().is_none() {
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
            let Tab::Document(tab) = tab else {
                continue;
            };
            if let Some(path) = &mut tab.path
                && let Ok(suffix) = path.strip_prefix(old_path)
            {
                *path = new_path.join(suffix);
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
    const fn document(&self) -> Option<&DocumentTab> {
        match self {
            Self::Document(tab) => Some(tab),
            _ => None,
        }
    }

    fn entity_id(&self) -> EntityId {
        match self {
            Self::Document(tab) => tab.handler.entity_id(),
            Self::Theme { editor, .. } => editor.entity_id(),
            Self::Shortcuts(view) => view.entity_id(),
        }
    }
}

impl DocumentTab {
    pub(crate) const fn id(&self) -> &TabId {
        &self.id
    }

    pub(crate) fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub(crate) const fn handler(&self) -> &Entity<FileHandler> {
        &self.handler
    }
}

fn same_document(left: Option<&Path>, right: Option<&Path>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => {
            file_ops::normalized_path(left) == file_ops::normalized_path(right)
        }
        _ => false,
    }
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
            Some(Path::new("./notes/../notes/today.md")),
            Some(Path::new("notes/today.md"))
        ));
        assert!(!same_document(None, Some(Path::new("notes/today.md"))));
        assert!(!same_document(Some(Path::new("notes/today.md")), None));
        assert!(!same_document(None, None));

        let embedded = crate::vault::source::DOCUMENTATION.root();
        assert!(same_document(
            Some(&embedded.join("examples/../Welcome.md")),
            Some(&embedded.join("Welcome.md"))
        ));
    }
}
