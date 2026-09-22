use std::path::PathBuf;
use std::time::Duration;

use gpui_kit::component::tree::TreeItem;
use gpui_kit::{App, AppContext, Context, Window};

use crate::app::{session::Session, settings};
use crate::vault::{CatalogState, VaultCatalog};

use super::DatalithView;

impl DatalithView {
    pub(super) fn restore_session(
        &mut self,
        session: Session,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let saved_active = session.active_tab_index.or_else(|| {
            session
                .tabs
                .iter()
                .position(|path| Some(path) == session.active_tab.as_ref())
        });
        let mut active = None;
        for (index, path) in session.tabs.into_iter().enumerate() {
            if path.as_os_str().is_empty() {
                self.new_empty_tab(cx);
            } else if path.is_file() && self.registry.is_supported(&path) {
                self.restore_file(path, window, cx);
            } else {
                continue;
            }
            if let Some(root) = session.tab_vaults.get(&index)
                && let Some(handler) = self.tabs.active_handler()
            {
                handler.update(cx, |handler, _| handler.restore_vault_root(root.clone()));
                self.restoring_catalogs
                    .entry(root.clone())
                    .or_default()
                    .push(handler.downgrade());
            }
            if saved_active == Some(index) {
                active = self.tabs.active_index();
            }
        }
        self.restore_tab_catalogs(cx);
        if let Some(root) = session.vault.filter(|root| root.is_dir()) {
            self.set_root_path(root, cx);
        }
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
            tab_vaults: self
                .tabs
                .iter()
                .filter_map(|(index, _, handler)| {
                    handler.read(cx).vault_root(cx).map(|root| (index, root))
                })
                .collect(),
            active_tab_index: self.tabs.active_index(),
            active_tab: None,
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

    fn restore_tab_catalogs(&self, cx: &Context<Self>) {
        for (root, handlers) in &self.restoring_catalogs {
            let root = root.clone();
            let handlers = handlers.clone();
            let file_types = self.registry.registered_file_types();
            let catalog_root = root.clone();
            let load = cx.background_spawn(async move {
                anyhow::ensure!(catalog_root.is_dir(), "Saved Vault folder is unavailable");
                VaultCatalog::open(catalog_root, file_types)
            });
            cx.spawn(async move |this, cx| {
                let result = load.await;
                let Ok(catalog) = result else {
                    let _ = this.update(cx, |view, cx| {
                        view.restoring_catalogs.remove(&root);
                        view.pending_notifications
                            .push(super::notifications::vault_db_failed_to_load());
                        cx.notify();
                    });
                    return;
                };
                if this
                    .update(cx, |view, cx| {
                        view.restoring_catalogs.remove(&root);
                        for handler in handlers.iter().filter_map(gpui_kit::WeakEntity::upgrade) {
                            handler.update(cx, |handler, cx| {
                                handler.set_vault_catalog(catalog.clone(), cx);
                            });
                        }
                        if view.root_path.as_ref() == Some(&root) {
                            view.use_vault_catalog(catalog.clone(), cx);
                        }
                        cx.notify();
                    })
                    .is_err()
                {
                    return;
                }
                while catalog.state() == CatalogState::Syncing {
                    if handlers.iter().all(|handler| handler.upgrade().is_none()) {
                        return;
                    }
                    cx.background_executor()
                        .timer(Duration::from_millis(200))
                        .await;
                }
                let _ = this.update(cx, |_, cx| {
                    for handler in handlers.iter().filter_map(gpui_kit::WeakEntity::upgrade) {
                        handler.update(cx, |handler, cx| {
                            handler.set_vault_catalog(catalog.clone(), cx);
                        });
                    }
                });
            })
            .detach();
        }
    }
}
