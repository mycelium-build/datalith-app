use gpui::*;

use crate::vault::path::display_name;

use super::DatalithView;

impl DatalithView {
    pub(crate) fn on_sidebar_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event.keystroke.key.as_str() {
            "escape" if self.rename_target.is_some() => self.cancel_rename(cx),
            "enter" => self.handle_enter_key(event, window, cx),
            "up" => self.navigate_tree_up(cx),
            "down" => self.navigate_tree_down(cx),
            "left" => self.navigate_tree_left(cx),
            "right" => self.navigate_tree_right(cx),
            _ => cx.propagate(),
        }
    }

    fn handle_enter_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
            let new_tab = event.keystroke.modifiers.platform;
            self.open_file(path, new_tab, window, cx);
        }
    }

    pub(crate) fn focus_sidebar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.ensure_sidebar_selection(cx);
        self.sidebar_focus_handle.focus(window, cx);
        if let Some(ix) = self.tree_state.read(cx).selected_index() {
            self.tree_state.update(cx, |state, _| {
                state.scroll_to_item(ix, gpui::ScrollStrategy::Center);
            });
        }
    }

    pub(crate) fn ensure_sidebar_selection(&mut self, cx: &mut Context<Self>) {
        let has_selection = self.tree_state.read(cx).selected_entry().is_some();

        if !has_selection {
            let active_path = {
                let active = self.active_tab.min(self.open_files.len().saturating_sub(1));
                self.open_files
                    .get(active)
                    .filter(|f| !f.path.as_os_str().is_empty())
                    .map(|f| f.path.clone())
            };

            if let Some(ref path) = active_path {
                let id = path.to_string_lossy().to_string();
                let label = display_name(path).to_string();
                let item = gpui_component::tree::TreeItem::new(id, label);
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

    fn update_last_selection(&mut self, cx: &mut Context<Self>) {
        if let Some(entry) = self.tree_state.read(cx).selected_entry() {
            self.last_sidebar_selection = Some(Self::path_from_id(&entry.item().id));
        }
    }

    fn navigate_tree_up(&mut self, cx: &mut Context<Self>) {
        let count = self.visible_tree_entry_count();
        self.tree_state.update(cx, |state, cx| {
            if let Some(ix) = state.selected_index() {
                let new_ix = if ix > 0 {
                    ix - 1
                } else {
                    count.saturating_sub(1)
                };
                state.set_selected_index(Some(new_ix), cx);
                state.scroll_to_item(new_ix, gpui::ScrollStrategy::Top);
            }
        });
        self.update_last_selection(cx);
    }

    fn navigate_tree_down(&mut self, cx: &mut Context<Self>) {
        self.tree_state.update(cx, |state, cx| {
            if let Some(ix) = state.selected_index() {
                state.set_selected_index(Some(ix + 1), cx);
                if state.selected_entry().is_none() {
                    state.set_selected_index(Some(0), cx);
                }
                if let Some(new_ix) = state.selected_index() {
                    state.scroll_to_item(new_ix, gpui::ScrollStrategy::Bottom);
                }
            }
        });
        self.update_last_selection(cx);
    }

    fn navigate_tree_left(&mut self, cx: &mut Context<Self>) {
        let entry = self.tree_state.read(cx).selected_entry();
        let (is_folder, is_expanded, id) = entry
            .filter(|e| e.is_folder())
            .map(|e| (true, e.is_expanded(), e.item().id.clone()))
            .unwrap_or((false, false, SharedString::default()));
        if is_folder && is_expanded {
            self.collapse_tree_item(&id, cx);
        }
    }

    fn navigate_tree_right(&mut self, cx: &mut Context<Self>) {
        let entry = self.tree_state.read(cx).selected_entry();
        let (is_folder, is_expanded, id) = entry
            .filter(|e| e.is_folder())
            .map(|e| (true, e.is_expanded(), e.item().id.clone()))
            .unwrap_or((false, false, SharedString::default()));
        if is_folder && !is_expanded {
            self.expand_tree_item(&id, cx);
        }
    }

    pub(crate) fn collapse_tree_item(&mut self, id: &SharedString, cx: &mut Context<Self>) {
        self.mark_tree_item_expanded(id, false);
        self.refresh_tree(cx);
        let item = gpui_component::tree::TreeItem::new(id.clone(), SharedString::default());
        self.tree_state.update(cx, |state, cx| {
            state.set_selected_item(Some(&item), cx);
        });
    }
}
