use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use gpui::*;
use gpui_component::{
    ActiveTheme, Icon, IconName, h_flex,
    input::Input,
    list::ListItem,
    tree::{self, TreeItem},
};

use crate::consts::{DRAG_HOVER_EXPAND_DELAY_MS, TREE_INDENT_PX, TREE_PADDING_PX};
use crate::utils::file_name_str;

use super::DatalithView;
use super::DragFile;

#[must_use]
pub(crate) fn build_file_items(path: &Path) -> Vec<TreeItem> {
    let mut dirs = Vec::new();
    let mut files = Vec::new();

    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            let name = file_name_str(&entry_path).to_string();

            if name.starts_with('.') {
                continue;
            }

            if entry_path.is_dir() {
                let children = build_file_items(&entry_path);
                dirs.push((
                    name.clone(),
                    TreeItem::new(entry_path.to_string_lossy().to_string(), name).children(children),
                ));
            } else {
                files.push((
                    name.clone(),
                    TreeItem::new(entry_path.to_string_lossy().to_string(), name),
                ));
            }
        }
    }

    dirs.sort_by_key(|a| a.0.to_lowercase());
    files.sort_by_key(|a| a.0.to_lowercase());

    let mut items: Vec<TreeItem> = dirs.into_iter().map(|(_, item)| item).collect();
    items.extend(files.into_iter().map(|(_, item)| item));
    items
}

impl DatalithView {
    pub(crate) fn render_file_tree(
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
                let path = Self::path_from_id(&item_id);
                let is_folder = entry.is_folder() || path.is_dir();
                let is_expanded = entry.is_expanded();
                let item_label = entry.item().label.clone();
                let depth = entry.depth();

                let v = view.clone();
                let ts = tree_state.clone();
                view.update(cx, move |this, cx| {
                    let is_renaming = this
                        .rename_target
                        .as_ref()
                        .map(|p| *p == path)
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
                            DragFile { path: path.clone() },
                            |drag, _offset, _window, cx| {
                                cx.stop_propagation();
                                cx.new(|_| drag.clone())
                            },
                        );

                    if is_folder {
                        list_item = list_item
                            .drag_over::<DragFile>({
                                let folder_path = path.clone();
                                let tree_state = ts.clone();
                                let v_for_notify = v.clone();
                                move |mut style, _drag, _window, cx| {
                                    style = style.bg(cx.theme().drop_target);

                                    let should_expand =
                                        v_for_notify.update(cx, |view, _| match &view.drag_hover {
                                            Some((p, instant)) if p == &folder_path => {
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
                                        let saved_selection = tree_state
                                            .read(cx)
                                            .selected_entry()
                                            .map(|e| (e.item().id.clone(), e.item().label.clone()));
                                        tree_state.update(cx, |state, cx| {
                                            state.expand_by_id(&id, cx);
                                        });
                                        if let Some((item_id, item_label)) = saved_selection {
                                            let item = tree::TreeItem::new(item_id, item_label);
                                            tree_state.update(cx, |state, cx| {
                                                state.set_selected_item(Some(&item), cx);
                                            });
                                        }
                                    }

                                    cx.notify(v_for_notify.entity_id());
                                    style
                                }
                            })
                            .on_drop(cx.listener({
                                let target_dir = path.clone();
                                move |this, drag: &DragFile, _window, cx| {
                                    if let Some(name) = drag.path.file_name() {
                                        let new_path = target_dir.join(name);
                                        this.handle_file_move(drag.path.clone(), new_path, cx);
                                    }
                                }
                            }));
                    }

                    list_item.on_click(cx.listener({
                        let path = path.clone();
                        move |this, event: &ClickEvent, window, cx| {
                            if !is_folder {
                                this.last_sidebar_selection = Some(path.clone());
                                let new_tab = event.modifiers().platform;
                                this.open_file(path.clone(), new_tab, window, cx);
                            }
                        }
                    }))
                })
            }
        })
    }
}
