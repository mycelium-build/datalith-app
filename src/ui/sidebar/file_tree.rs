use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gpui_kit::component::{
    ActiveTheme, Icon, IconName, StyledExt, h_flex,
    input::Input,
    list::ListItem,
    tree::{self, TreeEntry, TreeItem},
};
use gpui_kit::{
    App, AppContext, ClickEvent, Context, Div, Entity, InteractiveElement, IntoElement,
    MouseButton, MouseDownEvent, ParentElement, SharedString, Stateful, StatefulInteractiveElement,
    Styled, div, px,
};

use conv::{ConvUtil, UnwrapOrInf};

use crate::vault::path::display_name;

use super::{DatalithView, DragFile, TREE_PADDING_PX};

const TREE_INDENT_PX: f32 = 16.0;
const DRAG_HOVER_EXPAND_DELAY_MS: u64 = 800;

#[must_use]
pub fn build_file_items_with_expanded(path: &Path, expanded_ids: &[SharedString]) -> Vec<TreeItem> {
    let mut dirs = Vec::new();
    let mut files = Vec::new();

    if let Ok(entries) = crate::vault::source::read_dir(path) {
        for entry_path in entries {
            let name = display_name(&entry_path).to_string();

            if name.starts_with('.') {
                continue;
            }

            if crate::vault::source::is_dir(&entry_path) {
                let children = build_file_items_with_expanded(&entry_path, expanded_ids);
                let id = entry_path.to_string_lossy().to_string();
                let expanded = expanded_ids
                    .iter()
                    .any(|expanded_id| expanded_id.as_ref() == id);
                dirs.push((
                    name.clone(),
                    TreeItem::new(id, name)
                        .children(children)
                        .expanded(expanded),
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
    pub fn render_file_tree(
        cx: &Context<Self>,
        tree_state_entity: &Entity<gpui_kit::component::tree::TreeState>,
    ) -> impl IntoElement {
        let view = cx.entity();

        tree::tree(
            tree_state_entity,
            move |ix, entry, selected, _window, cx| {
                Self::render_tree_item(&view, ix, entry, selected, cx)
            },
        )
    }

    fn render_tree_item(
        view: &Entity<Self>,
        ix: usize,
        entry: &TreeEntry,
        selected: bool,
        cx: &mut App,
    ) -> ListItem {
        let item_id = entry.item().id.clone();
        let path = Self::path_from_id(&item_id);
        let is_folder = entry.is_folder() || crate::vault::source::is_dir(&path);
        let is_expanded = entry.is_expanded();
        let item_label = entry.item().label.clone();
        let depth = entry.depth();

        view.update(cx, move |this, cx| {
            Self::build_tree_row(
                this,
                cx,
                &path,
                &item_id,
                &item_label,
                view,
                ix,
                depth,
                is_folder,
                is_expanded,
                selected,
            )
        })
    }

    // A row needs the entry's derived properties plus view handles;
    // grouping them further would obscure the rendering pipeline.
    #[allow(clippy::too_many_arguments)]
    fn build_tree_row(
        this: &Self,
        cx: &Context<Self>,
        path: &Path,
        item_id: &SharedString,
        item_label: &SharedString,
        view: &Entity<Self>,
        ix: usize,
        depth: usize,
        is_folder: bool,
        is_expanded: bool,
        selected: bool,
    ) -> ListItem {
        let is_renaming = this.rename_target.as_ref().is_some_and(|p| *p == path);

        let icon = if is_folder {
            Icon::new(if is_expanded {
                IconName::FolderOpen
            } else {
                IconName::Folder
            })
        } else {
            Icon::new(this.registry.config_for(path).icon)
        };

        let mut list_item = ListItem::new(ix).selected(selected).pl(px(depth
            .approx_as::<f32>()
            .unwrap_or_inf()
            .mul_add(TREE_INDENT_PX, TREE_PADDING_PX)));

        if is_renaming && let Some(rename_state) = this.rename_state.clone() {
            return list_item.child(
                h_flex().gap_2().child(Icon::new(icon).size_4()).child(
                    Input::new(&rename_state)
                        .appearance(false)
                        .flex_1()
                        .px_0()
                        .py_0()
                        .h_7()
                        .h_flex()
                        .text_base(),
                ),
            );
        }

        // gpui-base toggles a row on left mouse-down;
        // while a context menu is open, that event also reaches the row underneath the menu.
        // Stop it so `on_click` stays the only path that selects or toggles.
        list_item = list_item.on_mouse_down(MouseButton::Left, {
            let focus_handle = this.sidebar_focus_handle.clone();
            move |_, window, cx| {
                focus_handle.focus(window, cx);
                cx.stop_propagation();
            }
        });

        let mut row = h_flex()
            .id(("file-tree-row", ix))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener({
                    let path = path.to_path_buf();
                    move |this, _event: &MouseDownEvent, _window, _cx| {
                        this.context_menu_target = Some(path.clone());
                        this.context_menu_from_row = true;
                    }
                }),
            )
            .gap_2()
            .child(Icon::new(icon).size_4())
            .child(
                div()
                    .flex_1()
                    .h_7() // better with h_6 but need
                    //.h_flex() // which make truncate not work
                    .truncate()
                    .child(item_label.clone()),
            );

        if !crate::vault::source::is_read_only(path) {
            row = row.on_drag(
                DragFile {
                    path: path.to_path_buf(),
                },
                |drag, _offset, _window, cx| {
                    cx.stop_propagation();
                    cx.new(|_| drag.clone())
                },
            );
        }

        if is_folder && !crate::vault::source::is_read_only(path) {
            row = Self::attach_folder_drop(row, path.to_path_buf(), view, cx);
        }

        list_item = list_item.child(row);

        list_item.on_click(cx.listener({
            let path = path.to_path_buf();
            let id = item_id.clone();
            move |this, event: &ClickEvent, window, cx| {
                this.tree_state
                    .update(cx, |state, cx| state.set_selected_index(Some(ix), cx));
                if is_folder {
                    this.mark_tree_item_expanded(&id, !is_expanded);
                    this.refresh_tree(cx);
                } else {
                    this.last_sidebar_selection = Some(path.clone());
                    let new_tab = event.modifiers().secondary();
                    this.open_file(path.clone(), new_tab, window, cx);
                }
            }
        }))
    }

    fn attach_folder_drop(
        row: Stateful<Div>,
        folder_path: PathBuf,
        view: &Entity<Self>,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        row.drag_over::<DragFile>({
            let folder_path = folder_path.clone();
            let v = view.clone();
            move |style, _drag, _window, cx| {
                let style = style.bg(cx.theme().drop_target);

                let should_expand = v.update(cx, |view, _| match &view.drag_hover {
                    Some((p, instant)) if p == &folder_path => {
                        if instant.elapsed() > Duration::from_millis(DRAG_HOVER_EXPAND_DELAY_MS) {
                            view.drag_hover = None;
                            true
                        } else {
                            false
                        }
                    }
                    _ => {
                        view.drag_hover = Some((folder_path.clone(), Instant::now()));
                        false
                    }
                });

                if should_expand {
                    let id: SharedString = folder_path.to_string_lossy().to_string().into();
                    v.update(cx, |view, cx| {
                        view.expand_tree_item(&id, cx);
                    });
                }

                cx.notify(v.entity_id());
                style
            }
        })
        .on_drop(cx.listener({
            let target_dir = folder_path;
            move |this, drag: &DragFile, _window, cx| {
                if let Some(name) = drag.path.file_name() {
                    let new_path = target_dir.join(name);
                    this.handle_file_move(&drag.path, &new_path, cx);
                }
            }
        }))
    }
}
