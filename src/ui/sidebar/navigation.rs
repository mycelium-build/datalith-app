use std::path::Path;

use gpui_kit::base::actions::{
    Cancel, Confirm, SelectDown, SelectFirst, SelectLast, SelectLeft, SelectPageDown, SelectPageUp,
    SelectRight, SelectUp,
};
use gpui_kit::component::tree::TreeItem;
use gpui_kit::{
    App, Context, Div, InteractiveElement as _, KeyBinding, KeyBindingContextPredicate,
    ScrollStrategy, SharedString, Window, actions,
};

use crate::app::keymap::FILE_TREE_CONTEXT;
use crate::vault::path::display_name;

use super::DatalithView;

/// Rows that Page Up and Page Down move the selection by.
const TREE_PAGE_ROWS: usize = 10;

actions!(
    file_tree,
    [
        /// Restores the last deleted file or folder.
        #[derive(Eq)]
        UndoDelete
    ]
);

/// Binds the file tree's navigation keys, which aren't customizable.
pub fn bind_keys(cx: &mut App) {
    let predicate = KeyBindingContextPredicate::parse(FILE_TREE_CONTEXT)
        .ok()
        .map(std::rc::Rc::new);
    let bindings: [(&str, Box<dyn gpui_kit::Action>); 12] = [
        ("up", Box::new(SelectUp)),
        ("down", Box::new(SelectDown)),
        ("home", Box::new(SelectFirst)),
        ("end", Box::new(SelectLast)),
        ("pageup", Box::new(SelectPageUp)),
        ("pagedown", Box::new(SelectPageDown)),
        ("left", Box::new(SelectLeft)),
        ("right", Box::new(SelectRight)),
        ("enter", Box::new(Confirm { secondary: false })),
        ("secondary-enter", Box::new(Confirm { secondary: true })),
        ("escape", Box::new(Cancel)),
        ("secondary-z", Box::new(UndoDelete)),
    ];
    let bindings = bindings
        .into_iter()
        .filter_map(|(keys, action)| {
            KeyBinding::load(keys, action, predicate.clone(), false, None, cx).ok()
        })
        .collect::<Vec<_>>();
    cx.bind_keys(bindings);
}

impl DatalithView {
    pub(crate) fn with_file_tree_actions(element: Div, cx: &Context<Self>) -> Div {
        element
            .key_context(FILE_TREE_CONTEXT)
            .on_action(cx.listener(Self::on_file_tree_up))
            .on_action(cx.listener(Self::on_file_tree_down))
            .on_action(cx.listener(|this, _: &SelectFirst, _, cx| {
                this.jump_in_tree(|_, _| 0, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectLast, _, cx| {
                this.jump_in_tree(|_, count| count.saturating_sub(1), cx);
            }))
            .on_action(cx.listener(|this, _: &SelectPageUp, _, cx| {
                this.jump_in_tree(|ix, _| ix.saturating_sub(TREE_PAGE_ROWS), cx);
            }))
            .on_action(cx.listener(|this, _: &SelectPageDown, _, cx| {
                this.jump_in_tree(
                    |ix, count| {
                        ix.saturating_add(TREE_PAGE_ROWS)
                            .min(count.saturating_sub(1))
                    },
                    cx,
                );
            }))
            .on_action(cx.listener(Self::on_file_tree_left))
            .on_action(cx.listener(Self::on_file_tree_right))
            .on_action(cx.listener(Self::on_file_tree_confirm))
            .on_action(cx.listener(Self::on_file_tree_cancel))
            .on_action(cx.listener(Self::on_file_tree_undo_delete))
    }

    fn on_file_tree_undo_delete(
        &mut self,
        _: &UndoDelete,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.rename_target.is_none() {
            self.undo_delete(None, window, cx);
        } else {
            cx.propagate();
        }
    }

    fn on_file_tree_cancel(&mut self, _: &Cancel, _: &mut Window, cx: &mut Context<Self>) {
        if self.rename_target.is_some() {
            self.cancel_rename(cx);
        } else {
            cx.propagate();
        }
    }

    fn on_file_tree_confirm(
        &mut self,
        confirm: &Confirm,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.rename_target.is_none() {
            self.handle_enter_key(confirm.secondary, window, cx);
        } else {
            cx.propagate();
        }
    }

    fn on_file_tree_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        self.navigate_tree_up(cx);
    }

    fn on_file_tree_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        self.navigate_tree_down(cx);
    }

    fn on_file_tree_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.navigate_tree_left(cx);
    }

    fn on_file_tree_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.navigate_tree_right(cx);
    }

    fn handle_enter_key(&mut self, new_tab: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some((is_folder, id, path)) = ({
            let ts = self.tree_state.read(cx);
            ts.selected_entry().map(|e| {
                (
                    e.is_folder(),
                    e.item().id.clone(),
                    Self::path_from_id(&e.item().id),
                )
            })
        }) else {
            return;
        };

        if is_folder {
            let is_expanded = self
                .expanded_tree_ids
                .iter()
                .any(|expanded_id| expanded_id == &id);
            if is_expanded {
                self.collapse_tree_item(&id, cx);
            } else {
                self.expand_tree_item(&id, cx);
            }
        } else {
            self.open_file(path, new_tab, window, cx);
        }
    }

    /// Expands the ancestors of `path`,
    /// selects it in the tree,
    /// and gives the sidebar keyboard focus
    /// so navigation can continue from there.
    pub(crate) fn reveal_in_sidebar(
        &mut self,
        path: &Path,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.root_path.as_deref() == Some(path) {
            self.focus_sidebar(window, cx);
        } else if self.select_in_sidebar(path, cx) {
            let closed_folder = self
                .tree_state
                .read(cx)
                .selected_entry()
                .filter(|entry| !entry.is_expanded() && crate::vault::source::is_dir(path))
                .map(|entry| entry.item().id.clone());
            if let Some(id) = closed_folder {
                self.expand_tree_item(&id, cx);
            }
            self.sidebar_focus_handle.focus(window, cx);
        }
    }

    /// Reveal the document in the cached tree without taking keyboard focus.
    pub(crate) fn select_in_sidebar(&mut self, path: &Path, cx: &mut Context<Self>) -> bool {
        if !self
            .root_path
            .as_ref()
            .is_some_and(|root| path.starts_with(root))
        {
            return false;
        }
        let item = TreeItem::new(
            path.to_string_lossy().into_owned(),
            display_name(path).to_owned(),
        );
        if !self.select_cached_item(&item, cx) {
            // The file may have been created before the vault watcher delivered its update.
            self.refresh_tree(cx);
            if !self.select_cached_item(&item, cx) {
                return false;
            }
        }
        self.last_sidebar_selection = Some(path.to_path_buf());
        true
    }

    fn select_cached_item(&self, item: &TreeItem, cx: &mut Context<Self>) -> bool {
        self.tree_state.update(cx, |state, cx| {
            let previous = state.selected_index();
            state.set_selected_item(Some(item), cx);
            if let Some(ix) = state.selected_index() {
                state.scroll_to_item(ix, ScrollStrategy::Center);
                true
            } else {
                state.set_selected_index(previous, cx);
                false
            }
        })
    }

    pub(crate) fn focus_sidebar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.ensure_sidebar_selection(cx);
        self.sidebar_focus_handle.focus(window, cx);
        if let Some(ix) = self.tree_state.read(cx).selected_index() {
            self.tree_state.update(cx, |state, _| {
                state.scroll_to_item(ix, gpui_kit::ScrollStrategy::Center);
            });
        }
    }

    pub(crate) fn ensure_sidebar_selection(&mut self, cx: &mut Context<Self>) {
        let has_selection = self.tree_state.read(cx).selected_entry().is_some();

        if !has_selection {
            let active_path = self.tabs.active_path().map(Path::to_path_buf);

            if let Some(ref path) = active_path {
                let id = path.to_string_lossy().to_string();
                let label = display_name(path).to_string();
                let item = gpui_kit::component::tree::TreeItem::new(id, label);
                self.tree_state.update(cx, |state, cx| {
                    state.set_selected_item(Some(&item), cx);
                });
            }

            if self.tree_state.read(cx).selected_entry().is_none()
                && self.visible_tree_entry_count() > 0
            {
                self.tree_state.update(cx, |state, cx| {
                    state.set_selected_index(Some(0), cx);
                });
            }
        }
        self.update_last_selection(cx);
    }

    fn update_last_selection(&mut self, cx: &Context<Self>) {
        if let Some(entry) = self.tree_state.read(cx).selected_entry() {
            self.last_sidebar_selection = Some(Self::path_from_id(&entry.item().id));
        }
    }

    fn navigate_tree_up(&mut self, cx: &mut Context<Self>) {
        let count = self.visible_tree_entry_count();
        self.tree_state.update(cx, |state, cx| {
            if let Some(ix) = state.selected_index() {
                let new_ix = ix.checked_sub(1).unwrap_or_else(|| count.saturating_sub(1));
                state.set_selected_index(Some(new_ix), cx);
                state.scroll_to_item(new_ix, gpui_kit::ScrollStrategy::Top);
            }
        });
        self.update_last_selection(cx);
    }

    /// Selects the row `target` picks from the selected row and the row count.
    fn jump_in_tree(&mut self, target: fn(usize, usize) -> usize, cx: &mut Context<Self>) {
        let count = self.visible_tree_entry_count();
        if count == 0 {
            return;
        }
        self.tree_state.update(cx, |state, cx| {
            let new_ix = target(state.selected_index().unwrap_or(0), count);
            state.set_selected_index(Some(new_ix), cx);
            state.scroll_to_item(new_ix, gpui_kit::ScrollStrategy::Center);
        });
        self.update_last_selection(cx);
    }

    fn navigate_tree_down(&mut self, cx: &mut Context<Self>) {
        self.tree_state.update(cx, |state, cx| {
            if let Some(ix) = state.selected_index() {
                state.set_selected_index(Some(ix.saturating_add(1)), cx);
                if state.selected_entry().is_none() {
                    state.set_selected_index(Some(0), cx);
                }
                if let Some(new_ix) = state.selected_index() {
                    state.scroll_to_item(new_ix, gpui_kit::ScrollStrategy::Bottom);
                }
            }
        });
        self.update_last_selection(cx);
    }

    fn navigate_tree_left(&mut self, cx: &mut Context<Self>) {
        let entry = self.tree_state.read(cx).selected_entry();
        let (is_folder, is_expanded, id) = entry.filter(|e| e.is_folder()).map_or_else(
            || (false, false, SharedString::default()),
            |e| (true, e.is_expanded(), e.item().id.clone()),
        );
        if is_folder && is_expanded {
            self.collapse_tree_item(&id, cx);
        }
    }

    fn navigate_tree_right(&mut self, cx: &mut Context<Self>) {
        let entry = self.tree_state.read(cx).selected_entry();
        let (is_folder, is_expanded, id) = entry.filter(|e| e.is_folder()).map_or_else(
            || (false, false, SharedString::default()),
            |e| (true, e.is_expanded(), e.item().id.clone()),
        );
        if is_folder && !is_expanded {
            self.expand_tree_item(&id, cx);
        }
    }

    pub(crate) fn collapse_tree_item(&mut self, id: &SharedString, cx: &mut Context<Self>) {
        self.mark_tree_item_expanded(id, false);
        self.refresh_tree(cx);
        let item = gpui_kit::component::tree::TreeItem::new(id.clone(), SharedString::default());
        self.tree_state.update(cx, |state, cx| {
            state.set_selected_item(Some(&item), cx);
        });
    }
}
