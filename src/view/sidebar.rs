use std::path::PathBuf;
use std::time::{Duration, Instant};

use gpui::*;
use gpui_component::{
    ActiveTheme, Icon, IconName,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    list::ListItem,
    menu::ContextMenuExt,
    select::Select,
    sidebar::SidebarHeader,
    tree::{self},
};

use crate::actions::{CopyPath, Delete, Duplicate, NewFile, NewFolder, OpenInExplorer, Rename};
use crate::consts::{DRAG_HOVER_EXPAND_DELAY_MS, SIDEBAR_WIDTH, TREE_INDENT_PX, TREE_PADDING_PX};
use crate::filetree::build_file_items;
use crate::fs_ops;
use crate::utils::file_name_str;

use super::DatalithView;
use super::palette::PaletteKind;

#[derive(Clone)]
struct DragFile {
    path: PathBuf,
}

impl Render for DragFile {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let name = file_name_str(&self.path).to_string();
        div()
            .px_2()
            .py_1()
            .bg(cx.theme().accent)
            .text_color(cx.theme().accent_foreground)
            .rounded_sm()
            .text_sm()
            .child(name)
    }
}

impl DatalithView {
    pub(crate) fn render_sidebar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let tree_state = self.tree_state.clone();
        let view = cx.entity().clone();

        self.ensure_rename_state(window, cx);

        if self.pending_vault_refresh {
            self.refresh_vault_select(window, cx);
        }

        if let Some(ref root) = self.root_path {
            let root_str: SharedString = root.to_string_lossy().to_string().into();
            self.vault_select_state.update(cx, |state, cx| {
                state.set_selected_value(&root_str, window, cx);
            });
        }

        div()
            .flex()
            .flex_col()
            .w(px(SIDEBAR_WIDTH))
            .h_full()
            .bg(cx.theme().sidebar)
            .border_r_1()
            .border_color(cx.theme().border)
            .track_focus(&self.sidebar_focus_handle)
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, _event: &MouseDownEvent, window, cx| {
                    this.sidebar_focus_handle.focus(window, cx);
                    cx.stop_propagation();
                }),
            )
            .on_key_down({
                let tree_state = tree_state.clone();
                cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                    match event.keystroke.key.as_str() {
                        "escape" if this.rename_target.is_some() => {
                            this.rename_target = None;
                            this.rename_state = None;
                            this._rename_sub = None;
                            cx.notify();
                        }
                        "enter" => {
                            let (folder_id, file_path) = {
                                let ts = tree_state.read(cx);
                                if let Some(entry) = ts.selected_entry() {
                                    if entry.is_folder() {
                                        (Some(entry.item().id.clone()), None)
                                    } else {
                                        (None, Some(PathBuf::from(entry.item().id.to_string())))
                                    }
                                } else {
                                    (None, None)
                                }
                            };

                            if let Some(ref id) = folder_id {
                                let is_expanded = tree_state.read(cx).expanded_ids().contains(id);
                                if is_expanded {
                                    this.collapse_tree_item(id, cx);
                                } else {
                                    tree_state.update(cx, |state, cx| {
                                        state.expand_by_id(id, cx);
                                    });
                                }
                            } else if let Some(path) = file_path {
                                let new_tab = event.keystroke.modifiers.platform;
                                this.open_file(path, new_tab, window, cx);
                            }
                        }
                        "up" => this.navigate_tree_up(cx),
                        "down" => this.navigate_tree_down(cx),
                        "left" => this.navigate_tree_left(cx),
                        "right" => this.navigate_tree_right(cx),
                        _ => {}
                    }
                })
            })
            .drag_over::<DragFile>(|style, _drag, _window, cx| style.bg(cx.theme().drop_target))
            .on_drop({
                let view = view.clone();
                let root_path = self.root_path.clone();
                move |drag: &DragFile, window, cx| {
                    if let (Some(root), Some(name)) = (&root_path, drag.path.file_name()) {
                        let new_path = root.join(name);
                        if new_path != drag.path {
                            let old_path = drag.path.clone();
                            let _ = std::fs::rename(&old_path, &new_path);
                            view.update(cx, |v, _cx| {
                                v.track_file_rename(&old_path, &new_path);
                                for f in &mut v.open_files {
                                    if f.path == old_path {
                                        f.path = new_path.clone();
                                    }
                                }
                            });
                        }
                    }
                    let v = view.clone();
                    window.defer(cx, move |_window, cx| {
                        v.update(cx, |view, cx| {
                            view.refresh_tree(cx);
                        });
                    });
                }
            })
            .child(self.render_sidebar_header(cx))
            .child(
                div()
                    .flex_1()
                    .child(self.render_file_tree(cx, &self.tree_state)),
            )
            .child(
                div()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .p_2()
                    .child(Select::new(&self.vault_select_state)),
            )
            .context_menu({
                let view = view.clone();
                move |menu, _window, cx| {
                    if let Some(entry) = tree_state.read(cx).right_clicked_entry() {
                        let item_id = entry.item().id.to_string();
                        let path = PathBuf::from(&item_id);
                        view.update(cx, |v, _| {
                            v.context_menu_target = Some(path);
                        });
                        menu.menu("New File", Box::new(NewFile))
                            .menu("New Folder", Box::new(NewFolder))
                            .separator()
                            .menu("Rename", Box::new(Rename))
                            .menu("Delete", Box::new(Delete))
                            .menu("Duplicate", Box::new(Duplicate))
                            .separator()
                            .menu("Open in Explorer", Box::new(OpenInExplorer))
                            .menu("Copy Path", Box::new(CopyPath))
                    } else {
                        view.update(cx, |v, _| {
                            if let Some(ref root) = v.root_path {
                                v.context_menu_target = Some(root.clone());
                            }
                        });
                        menu.menu("New File", Box::new(NewFile))
                            .menu("New Folder", Box::new(NewFolder))
                            .separator()
                            .menu("Open in Explorer", Box::new(OpenInExplorer))
                            .menu("Copy Path", Box::new(CopyPath))
                    }
                }
            })
    }

    fn ensure_rename_state(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ref target) = self.rename_target.clone() else {
            self.rename_state = None;
            return;
        };
        if self.rename_state.is_some() {
            return;
        }

        let current = file_name_str(target).to_string();

        let old_ext = if !target.is_dir() {
            target
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|name| {
                    name.rfind('.')
                        .and_then(|dot| if dot > 0 { Some(&name[dot..]) } else { None })
                        .map(|e| e.to_string())
                })
        } else {
            None
        };

        let state = cx.new(|cx| InputState::new(window, cx).default_value(current.as_str()));
        let dir = target.parent().map(|p| p.to_path_buf());
        let target_clone = target.clone();

        self._rename_sub = Some(cx.subscribe_in(
            &state,
            window,
            move |this, input, event, _window, cx| match event {
                InputEvent::PressEnter { .. } => {
                    let mut new_name = input.read(cx).value().to_string();
                    let mut final_path = target_clone.clone();
                    if !new_name.is_empty() {
                        if let Some(ref ext) = old_ext
                            && !new_name.contains('.')
                        {
                            new_name.push_str(ext);
                        }
                        if let Some(parent) = &dir {
                            let candidate = parent.join(&new_name);
                            if candidate != target_clone {
                                final_path = fs_ops::unique_name(parent, &new_name);
                                let _ = std::fs::rename(&target_clone, &final_path);
                                this.track_file_rename(&target_clone, &final_path);
                                for open_file in &mut this.open_files {
                                    if open_file.path == target_clone {
                                        open_file.path = final_path.clone();
                                    }
                                }
                            }
                        }
                    }
                    if this.pending_open.as_deref() == Some(target_clone.as_path()) {
                        this.pending_open = Some(final_path);
                    }
                    this.rename_target = None;
                    this.rename_state = None;
                    this._rename_sub = None;
                    this.refresh_tree(cx);
                }
                InputEvent::Blur => {
                    this.rename_target = None;
                    this.rename_state = None;
                    this._rename_sub = None;
                    cx.notify();
                }
                _ => {}
            },
        ));

        state.focus_handle(cx).focus(window, cx);
        self.rename_state = Some(state.clone());
        window.dispatch_action(Box::new(gpui_component::input::SelectAll), cx);
    }

    fn render_sidebar_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        SidebarHeader::new().p_2().child(
            h_flex().w_full().justify_end().child(
                Button::new("search-trigger")
                    .ghost()
                    .icon(IconName::Search)
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.palette.open_as(PaletteKind::Search);
                        cx.notify();
                    })),
            ),
        )
    }

    pub(crate) fn focus_sidebar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
                let label = file_name_str(path).to_string();
                let item = tree::TreeItem::new(id, label);
                self.tree_state.update(cx, |state, cx| {
                    state.set_selected_item(Some(&item), cx);
                });
            }

            if self.tree_state.read(cx).selected_entry().is_none() {
                self.tree_state.update(cx, |state, cx| {
                    state.set_selected_index(Some(0), cx);
                });
            }
        }

        self.sidebar_focus_handle.focus(window, cx);
        if let Some(ix) = self.tree_state.read(cx).selected_index() {
            self.tree_state.update(cx, |state, _| {
                state.scroll_to_item(ix, gpui::ScrollStrategy::Center);
            });
        }
    }

    fn navigate_tree_up(&mut self, cx: &mut Context<Self>) {
        self.tree_state.update(cx, |state, cx| {
            if let Some(ix) = state.selected_index() {
                let new_ix = if ix > 0 {
                    ix - 1
                } else {
                    let count = state.entry_count();
                    count.saturating_sub(1)
                };
                state.set_selected_index(Some(new_ix), cx);
                state.scroll_to_item(new_ix, gpui::ScrollStrategy::Top);
            }
        });
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
    }

    fn navigate_tree_left(&mut self, cx: &mut Context<Self>) {
        let (is_folder, is_expanded, item_id) = {
            let ts = self.tree_state.read(cx);
            let entry = ts.selected_entry();
            entry
                .filter(|e| e.is_folder())
                .filter(|e| e.is_expanded())
                .map(|e| (true, true, e.item().id.clone()))
                .unwrap_or((false, false, SharedString::default()))
        };

        if is_folder && is_expanded {
            self.collapse_tree_item(&item_id, cx);
        }
    }

    fn navigate_tree_right(&mut self, cx: &mut Context<Self>) {
        let (is_folder, is_expanded, item_id) = {
            let ts = self.tree_state.read(cx);
            let entry = ts.selected_entry();
            entry
                .filter(|e| e.is_folder())
                .filter(|e| !e.is_expanded())
                .map(|e| (true, false, e.item().id.clone()))
                .unwrap_or((false, false, SharedString::default()))
        };

        if is_folder && !is_expanded {
            self.tree_state
                .update(cx, |state, cx| state.expand_by_id(&item_id, cx));
        }
    }

    fn collapse_tree_item(&mut self, id: &SharedString, cx: &mut Context<Self>) {
        if let Some(ref root) = self.root_path {
            let mut expanded_ids = self.tree_state.read(cx).expanded_ids();
            expanded_ids.retain(|eid| eid != id);
            let mut items = build_file_items(root);
            for item in &mut items {
                if expanded_ids.contains(&item.id) {
                    item.set_expanded(true);
                }
            }
            self.tree_state.update(cx, |state, cx| {
                state.set_items(items, cx);
            });
            let item = tree::TreeItem::new(id.clone(), SharedString::default());
            self.tree_state.update(cx, |state, cx| {
                state.set_selected_item(Some(&item), cx);
            });
        }
    }

    fn render_file_tree(
        &self,
        cx: &mut Context<Self>,
        tree_state_entity: &Entity<gpui_component::tree::TreeState>,
    ) -> impl IntoElement {
        let view = cx.entity();
        let tree_state = tree_state_entity.clone();

        tree::tree(tree_state_entity, {
            let view = view.clone();
            let tree_state = tree_state.clone();
            move |ix, entry, selected, _window, cx| {
                let item_id = entry.item().id.clone();
                let is_folder = entry.is_folder() || PathBuf::from(item_id.to_string()).is_dir();
                let is_expanded = entry.is_expanded();
                let item_label = entry.item().label.clone();
                let depth = entry.depth();

                let v = view.clone();
                let ts = tree_state.clone();
                view.update(cx, move |this, cx| {
                    let is_renaming = this
                        .rename_target
                        .as_ref()
                        .map(|p| p.to_string_lossy() == item_id.to_string())
                        .unwrap_or(false);

                    let icon = if !is_folder {
                        IconName::File
                    } else if is_expanded {
                        IconName::FolderOpen
                    } else {
                        IconName::Folder
                    };

                    let mut list_item = ListItem::new(ix)
                        .selected(selected)
                        .pl(px(TREE_INDENT_PX) * depth + px(TREE_PADDING_PX));

                    if is_renaming && let Some(rename_state) = this.rename_state.clone() {
                        return list_item.child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .overflow_hidden()
                                .child(Icon::new(icon).size_4())
                                .child(Input::new(&rename_state)),
                        );
                    }

                    let drag_path = PathBuf::from(item_id.to_string());

                    list_item = list_item
                        .child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .overflow_hidden()
                                .child(Icon::new(icon).size_4())
                                .child(div().flex_1().truncate().child(item_label.clone())),
                        )
                        .on_drag(
                            DragFile {
                                path: drag_path.clone(),
                            },
                            |drag, _offset, _window, cx| {
                                cx.stop_propagation();
                                cx.new(|_| drag.clone())
                            },
                        );

                    if is_folder {
                        list_item = list_item
                            .drag_over::<DragFile>({
                                let folder_path = drag_path.clone();
                                let tree_state = ts.clone();
                                let v_for_notify = v.clone();
                                move |mut style, _drag, _window, cx| {
                                    style = style.bg(cx.theme().drop_target);

                                    let should_expand =
                                        v_for_notify.update(cx, |view, _| match &view.drag_hover {
                                            Some((path, instant)) if path == &folder_path => {
                                                if instant.elapsed()
                                                    > Duration::from_millis(
                                                        DRAG_HOVER_EXPAND_DELAY_MS,
                                                    )
                                                {
                                                    view.drag_hover = None;
                                                    true
                                                } else {
                                                    false
                                                }
                                            }
                                            _ => {
                                                view.drag_hover =
                                                    Some((folder_path.clone(), Instant::now()));
                                                false
                                            }
                                        });

                                    if should_expand {
                                        let id: SharedString =
                                            folder_path.to_string_lossy().to_string().into();
                                        tree_state.update(cx, |state, cx| {
                                            state.expand_by_id(&id, cx);
                                        });
                                    }

                                    cx.notify(v_for_notify.entity_id());
                                    style
                                }
                            })
                            .on_drop(cx.listener({
                                let target_dir = drag_path.clone();
                                move |this, drag: &DragFile, window, cx| {
                                    if let Some(name) = drag.path.file_name() {
                                        let new_path = target_dir.join(name);
                                        if new_path != drag.path {
                                            let old_path = drag.path.clone();
                                            let _ = std::fs::rename(&old_path, &new_path);
                                            this.track_file_rename(&old_path, &new_path);
                                            for f in &mut this.open_files {
                                                if f.path == old_path {
                                                    f.path = new_path.clone();
                                                }
                                            }
                                        }
                                    }
                                    window.defer(cx, move |_window, cx| {
                                        cx.update_global(|state: &mut crate::app::AppState, cx| {
                                            if let Some(view) = &state.view {
                                                view.update(cx, |view, cx| {
                                                    view.refresh_tree(cx);
                                                });
                                            }
                                        });
                                    });
                                }
                            }));
                    }

                    list_item.on_click(cx.listener({
                        let drag_path = drag_path.clone();
                        move |this, event: &ClickEvent, window, cx| {
                            if !is_folder {
                                let new_tab = event.modifiers().platform;
                                this.open_file(drag_path.clone(), new_tab, window, cx);
                            }
                        }
                    }))
                })
            }
        })
    }
}
