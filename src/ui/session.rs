use std::path::PathBuf;

use gpui_kit::component::tree::TreeItem;
use gpui_kit::{App, Context, Window};

use crate::app::{session::Session, settings};

use super::DatalithView;

impl DatalithView {
    pub(super) fn restore_session(
        &mut self,
        session: Session,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(root) = session.vault.filter(|root| root.is_dir()) {
            self.set_root_path(root, cx);
        }
        for path in session.tabs {
            if path.as_os_str().is_empty() {
                self.new_empty_tab(cx);
            } else if path.is_file() {
                self.open_file(path, true, window, cx);
            }
        }
        let active = self.tabs.iter().find_map(|(index, path, _)| {
            (Some(path) == session.active_tab.as_deref()).then_some(index)
        });
        self.tabs.select(active.unwrap_or(0));

        if let Some(root) = &self.root_path {
            self.expanded_tree_ids = session
                .expanded_folders
                .into_iter()
                .filter(|path| path.starts_with(root) && path.is_dir())
                .map(|path| path.to_string_lossy().into_owned().into())
                .collect();
            self.refresh_tree(cx);
            if let Some(path) = session
                .sidebar_selection
                .filter(|path| path.starts_with(root) && path.exists())
            {
                let item = TreeItem::new(path.to_string_lossy().into_owned(), "");
                self.tree_state
                    .update(cx, |state, cx| state.set_selected_item(Some(&item), cx));
                self.last_sidebar_selection = Some(path);
            }
        }
        self.focus_active_tab(window, cx);
        cx.notify();
    }

    pub(super) fn save_session(&self, cx: &App) {
        let session = Session {
            vault: self.root_path.clone(),
            tabs: self.tabs.open_paths(),
            active_tab: self.tabs.active_path().map(ToOwned::to_owned),
            expanded_folders: self
                .expanded_tree_ids
                .iter()
                .map(|id| PathBuf::from(id.as_str()))
                .collect(),
            sidebar_selection: self
                .tree_state
                .read(cx)
                .selected_entry()
                .map(|entry| PathBuf::from(entry.item().id.as_str())),
        };
        if let Err(error) = settings::save_session(session) {
            eprintln!("Failed to save workspace session: {error:#}");
        }
    }
}
