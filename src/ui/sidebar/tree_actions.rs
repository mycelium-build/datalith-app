use std::path::{Path, PathBuf};

use gpui_kit::component::{
    Sizable as _, WindowExt as _, button::Button, notification::Notification, tree::TreeItem,
};
use gpui_kit::{Context, SharedString, Window};

use super::file_tree::build_file_items_with_expanded;
use crate::vault::path::display_name;

use crate::app::AppState;
use crate::ui::{DatalithView, notifications};
use crate::vault::file_ops::{self, Deleted};

/// How many deletions Undo keeps track of.
const DELETED_HISTORY: usize = 20;

struct FileDeletion;

pub struct DeletedItem {
    deleted: Deleted,
    notification_id: u64,
}

impl DatalithView {
    /// Remember a deletion and offer to undo it.
    pub(crate) fn push_deleted(&mut self, deleted: Deleted) {
        let notification_id = rand::random::<u64>();
        let name = deleted
            .original
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
        if self.deleted_items.len() == DELETED_HISTORY {
            self.deleted_items.remove(0);
        }
        self.deleted_items.push(DeletedItem {
            deleted,
            notification_id,
        });
        self.pending_notifications.push(
            Notification::new()
                .id1::<FileDeletion>(("file-deletion", notification_id))
                .title(format!("\"{name}\" deleted"))
                .action(move |_, _, _| {
                    Button::new("undo-file-delete")
                        .small()
                        .label("Undo")
                        .on_click(move |_, window, cx| {
                            if let Some(view) = AppState::view(cx) {
                                view.update(cx, |view, cx| {
                                    view.undo_delete(Some(notification_id), window, cx);
                                });
                            }
                        })
                })
                .autohide(true),
        );
    }

    /// Restore the deletion shown by `notification_id`, or the most recent one.
    pub(crate) fn undo_delete(
        &mut self,
        notification_id: Option<u64>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let index = match notification_id {
            Some(id) => self
                .deleted_items
                .iter()
                .position(|item| item.notification_id == id),
            None => self.deleted_items.len().checked_sub(1),
        };
        let Some(index) = index else {
            return;
        };
        let item = self.deleted_items.remove(index);
        if let Err(error) = file_ops::restore(&item.deleted) {
            window.push_notification(notifications::file_restore_failed(&error), cx);
            self.deleted_items.insert(index, item);
            return;
        }
        window.remove_notification1::<FileDeletion>(("file-deletion", item.notification_id), cx);
        self.refresh_tree(cx);
        cx.notify();
    }

    pub(crate) fn refresh_tree(&self, cx: &mut Context<Self>) {
        let selected = self.root_path.as_ref().and_then(|_| {
            self.tree_state
                .read(cx)
                .selected_entry()
                .map(|entry| (entry.item().id.clone(), entry.item().label.clone()))
        });
        let items = self.root_path.as_deref().map_or_else(Vec::new, |root| {
            build_file_items_with_expanded(root, &self.expanded_tree_ids)
        });
        self.tree_state.update(cx, |state, cx| {
            state.set_items(items, cx);
            if let Some((item_id, item_label)) = selected {
                let item = TreeItem::new(item_id, item_label);
                state.set_selected_item(Some(&item), cx);
            }
        });
        cx.notify();
    }

    pub(crate) fn refresh_tree_with_rename(
        &mut self,
        old_path: &Path,
        new_path: &Path,
        cx: &mut Context<Self>,
    ) {
        fn remap(path: &Path, old_path: &Path, new_path: &Path) -> PathBuf {
            path.strip_prefix(old_path)
                .map_or_else(|_| path.to_path_buf(), |suffix| new_path.join(suffix))
        }

        fn remap_items(items: &mut [TreeItem], old_path: &Path, new_path: &Path) {
            for item in items {
                let path = PathBuf::from(item.id.to_string());
                if path == old_path || path.starts_with(old_path) {
                    let renamed = remap(&path, old_path, new_path);
                    item.id = renamed.to_string_lossy().to_string().into();
                    if path == old_path {
                        item.label = display_name(new_path).to_string().into();
                    }
                }
                remap_items(&mut item.children, old_path, new_path);
            }
        }

        let Some(ref root) = self.root_path else {
            return;
        };
        let selected = self.tree_state.read(cx).selected_entry().map(|entry| {
            remap(
                &PathBuf::from(entry.item().id.to_string()),
                old_path,
                new_path,
            )
        });
        let mut items = build_file_items_with_expanded(root, &self.expanded_tree_ids);
        remap_items(&mut items, old_path, new_path);
        for expanded in &mut self.expanded_tree_ids {
            let renamed = remap(&PathBuf::from(expanded.to_string()), old_path, new_path);
            *expanded = renamed.to_string_lossy().to_string().into();
        }
        self.tree_state.update(cx, |state, cx| {
            state.set_items(items, cx);
            if let Some(selected) = selected {
                let selected = TreeItem::new(
                    selected.to_string_lossy().to_string(),
                    display_name(&selected).to_string(),
                );
                state.set_selected_item(Some(&selected), cx);
            }
        });
        cx.notify();
    }

    pub(crate) fn mark_tree_item_expanded(&mut self, id: &SharedString, expanded: bool) {
        if expanded {
            if !self
                .expanded_tree_ids
                .iter()
                .any(|expanded_id| expanded_id == id)
            {
                self.expanded_tree_ids.push(id.clone());
            }
        } else {
            self.expanded_tree_ids
                .retain(|expanded_id| expanded_id != id);
        }
    }

    pub(crate) fn expand_tree_item(&mut self, id: &SharedString, cx: &mut Context<Self>) {
        self.mark_tree_item_expanded(id, true);
        self.refresh_tree(cx);
    }

    pub(crate) fn visible_tree_entry_count(&self) -> usize {
        // Counting tree nodes cannot plausibly overflow for real vault sizes.
        #[allow(clippy::arithmetic_side_effects)]
        fn count_items(items: &[TreeItem]) -> usize {
            items
                .iter()
                .map(|item| {
                    1 + if item.is_expanded() {
                        count_items(&item.children)
                    } else {
                        0
                    }
                })
                .sum()
        }

        self.root_path.as_ref().map_or(0, |root| {
            count_items(&build_file_items_with_expanded(
                root,
                &self.expanded_tree_ids,
            ))
        })
    }
}
