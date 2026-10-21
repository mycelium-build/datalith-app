use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gpui_kit::component::{
    ActiveTheme, Icon, IconName, StyledExt, h_flex,
    input::{Input, InputState},
    list::ListItem,
    tree::{self, TreeEntry, TreeItem, TreeState},
};
use gpui_kit::{
    App, AppContext, ClickEvent, Context, Entity, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, ParentElement, SharedString, StatefulInteractiveElement,
    Styled, div, px,
};

#[cfg(test)]
use gpui_kit::test::TestSupportExt as _;

use conv::{ConvUtil, UnwrapOrInf};

use crate::vault;
use crate::vault::path::display_name;

use super::{DatalithView, DragDropTarget, DragFile, TREE_PADDING_PX};

const TREE_INDENT_PX: f32 = 16.0;
const DRAG_HOVER_EXPAND_DELAY_MS: u64 = 800;

#[must_use]
pub fn build_file_items_with_expanded(path: &Path, expanded_ids: &[SharedString]) -> Vec<TreeItem> {
    let mut dirs = Vec::new();
    let mut files = Vec::new();

    if let Ok(entries) = vault::source::read_dir(path) {
        for entry_path in entries {
            let name = display_name(&entry_path).to_string();

            if name.starts_with('.') {
                continue;
            }

            if vault::source::is_dir(&entry_path) {
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

/// The folder that receives a drop on `path`'s row.
///
/// Folders receive into themselves so hovering an open folder's contents keeps the drop inside it;
/// other rows receive into their parent folder.
/// Rows at the vault root return `None`, which lets the sidebar handle the drop there.
fn drop_folder_for_row(path: &Path, root: &Path, is_folder: bool) -> Option<PathBuf> {
    if is_folder {
        return Some(path.to_path_buf());
    }
    let parent = path.parent()?;
    (parent != root && parent.starts_with(root)).then(|| parent.to_path_buf())
}

/// First and last visible row belonging to `target`'s subtree, in list order.
fn drop_box_range_for(
    target: &Path,
    paths: impl Iterator<Item = PathBuf>,
) -> Option<(usize, usize)> {
    let mut range = None;
    for (ix, path) in paths.enumerate() {
        if path == target || path.starts_with(target) {
            range = Some(range.map_or((ix, ix), |(start, _)| (start, ix)));
        }
    }
    range
}

impl DatalithView {
    pub fn render_file_tree(
        cx: &Context<Self>,
        tree_state_entity: &Entity<TreeState>,
        drop_box: Option<(usize, usize)>,
    ) -> impl IntoElement {
        let view = cx.entity();

        tree::tree(
            tree_state_entity,
            move |ix, entry, selected, _window, cx| {
                Self::render_tree_item(&view, ix, entry, selected, drop_box, cx)
            },
        )
    }

    /// Drops stale hover state once the drag ends, then reports the highlighted rows.
    pub(crate) fn prepare_sidebar_drag_state(
        &mut self,
        cx: &Context<Self>,
    ) -> Option<(usize, usize)> {
        if !cx.has_active_drag() {
            self.drag_drop_target = None;
        }
        self.drop_box_range(cx, &self.tree_state)
    }

    /// Visible row range covered by the folder currently under the drag, if any.
    ///
    /// The folder row opens the box and every following visible descendant extends it,
    /// so the whole subtree highlights as one drop target.
    fn drop_box_range(
        &self,
        cx: &Context<Self>,
        tree_state: &Entity<TreeState>,
    ) -> Option<(usize, usize)> {
        let target = self.drag_drop_target.as_ref()?.folder.as_path();
        let state = tree_state.read(cx);
        let paths = (0..)
            .map_while(|ix| state.entry(ix))
            .map(|entry| Self::path_from_id(&entry.item().id));
        drop_box_range_for(target, paths)
    }

    fn set_drag_drop_target(
        &mut self,
        folder: &Path,
        expand_on_hover: bool,
        cx: &mut Context<Self>,
    ) {
        match &mut self.drag_drop_target {
            Some(target) if target.folder == folder => {
                // Once the folder opens, its expand timer is done.
                if !expand_on_hover {
                    target.expand_at = None;
                }
            }
            _ => {
                self.drag_drop_target =
                    Some(DragDropTarget::new(folder.to_path_buf(), expand_on_hover));
                cx.notify();
            }
        }
    }

    /// Clears the highlighted drop box once the pointer leaves the file tree.
    pub(crate) fn clear_drag_target(&mut self, cx: &mut Context<Self>) {
        if self.drag_drop_target.take().is_some() {
            cx.notify();
        }
    }

    /// Whether the target folder has been hovered long enough to expand.
    fn hover_expand_due(&mut self, folder: &Path) -> bool {
        let Some(target) = self.drag_drop_target.as_mut() else {
            return false;
        };
        if target.folder != folder {
            return false;
        }
        let expand_at = target.expand_at.get_or_insert_with(Instant::now);
        if expand_at.elapsed() > Duration::from_millis(DRAG_HOVER_EXPAND_DELAY_MS) {
            target.expand_at = None;
            true
        } else {
            false
        }
    }

    fn row_drop_target(this: &Self, path: &Path, is_folder: bool) -> Option<PathBuf> {
        if vault::source::is_read_only(path) {
            return None;
        }
        this.root_path
            .as_deref()
            .and_then(|root| drop_folder_for_row(path, root, is_folder))
    }

    fn rename_row(icon: Icon, rename_state: &Entity<InputState>) -> impl IntoElement {
        h_flex().gap_2().child(Icon::new(icon).size_4()).child(
            Input::new(rename_state)
                .appearance(false)
                .flex_1()
                .px_0()
                .py_0()
                .h_7()
                .h_flex()
                .text_base(),
        )
    }

    fn paint_drop_box(
        mut list_item: ListItem,
        ix: usize,
        drop_box: Option<(usize, usize)>,
        cx: &App,
    ) -> ListItem {
        let Some((start, end)) = drop_box else {
            return list_item;
        };
        if !(start..=end).contains(&ix) {
            return list_item;
        }
        list_item = list_item.bg(cx.theme().drop_target);
        if ix == start {
            list_item = list_item.rounded_t_sm();
        }
        if ix == end {
            list_item = list_item.rounded_b_sm();
        }
        list_item
    }

    fn render_tree_item(
        view: &Entity<Self>,
        ix: usize,
        entry: &TreeEntry,
        selected: bool,
        drop_box: Option<(usize, usize)>,
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
                drop_box,
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
        drop_box: Option<(usize, usize)>,
    ) -> ListItem {
        let is_renaming = this.rename_target.as_ref().is_some_and(|p| *p == path);

        let icon = if is_folder {
            Icon::new(if is_expanded {
                IconName::FolderOpen
            } else {
                IconName::Folder
            })
        } else {
            Icon::new(this.registry.config_for(Some(path)).icon)
        };

        let mut list_item = ListItem::new(ix).selected(selected).pl(px(depth
            .approx_as::<f32>()
            .unwrap_or_inf()
            .mul_add(TREE_INDENT_PX, TREE_PADDING_PX)));

        if is_renaming && let Some(rename_state) = this.rename_state.clone() {
            return list_item.child(Self::rename_row(icon, &rename_state));
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

        // Paint the folder row and its visible descendants as a single drop box.
        list_item = Self::paint_drop_box(list_item, ix, drop_box, cx);

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

        if let Some(target) = Self::row_drop_target(this, path, is_folder) {
            let expand_on_hover = is_folder && !is_expanded;
            list_item = Self::attach_drop_target(list_item, target, expand_on_hover, view, cx);
        }

        #[cfg(test)]
        let row = row.test_support();
        list_item = list_item.child(row);

        list_item = list_item.on_aux_click(cx.listener({
            let path = path.to_path_buf();
            move |this, event: &ClickEvent, window, cx| {
                if !is_folder && event.is_middle_click() {
                    this.tree_state
                        .update(cx, |state, cx| state.set_selected_index(Some(ix), cx));
                    this.last_sidebar_selection = Some(path.clone());
                    this.open_file(path.clone(), true, window, cx);
                }
            }
        }));

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

    fn attach_drop_target(
        mut list_item: ListItem,
        target: PathBuf,
        expand_on_hover: bool,
        view: &Entity<Self>,
        cx: &Context<Self>,
    ) -> ListItem {
        list_item = list_item.on_mouse_move(cx.listener({
            let target = target.clone();
            move |this, _event: &MouseMoveEvent, _window, cx| {
                if !cx.has_active_drag() {
                    return;
                }
                this.set_drag_drop_target(&target, expand_on_hover, cx);
                // Claim the move, otherwise the sidebar resets the target to the root.
                cx.stop_propagation();
            }
        }));

        list_item = list_item.drag_over::<DragFile>({
            let target = target.clone();
            let v = view.clone();
            move |style, _drag, _window, cx| {
                let should_expand = v.update(cx, |view, cx| {
                    view.set_drag_drop_target(&target, expand_on_hover, cx);
                    expand_on_hover && view.hover_expand_due(&target)
                });

                if should_expand {
                    let id: SharedString = target.to_string_lossy().to_string().into();
                    v.update(cx, |view, cx| view.expand_tree_item(&id, cx));
                }

                if expand_on_hover {
                    // Keep repainting while a collapsed folder is hovered so its expand timer can fire.
                    cx.notify(v.entity_id());
                }
                style
            }
        });

        list_item.on_drop(cx.listener(move |this, drag: &DragFile, _window, cx| {
            this.drag_drop_target = None;
            cx.notify();
            // A folder cannot be moved onto itself or into one of its descendants.
            if drag.path == target || target.starts_with(&drag.path) {
                return;
            }
            if let Some(name) = drag.path.file_name() {
                let new_path = target.join(name);
                this.handle_file_move(&drag.path, &new_path, cx);
            }
        }))
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{drop_box_range_for, drop_folder_for_row};

    #[test]
    fn folder_rows_receive_into_themselves() {
        let root = Path::new("/vault");
        assert_eq!(
            drop_folder_for_row(Path::new("/vault/Notes"), root, true),
            Some(PathBuf::from("/vault/Notes"))
        );
    }

    #[test]
    fn child_rows_receive_into_their_open_parent_folder() {
        let root = Path::new("/vault");
        assert_eq!(
            drop_folder_for_row(Path::new("/vault/Notes/Idea.md"), root, false),
            Some(PathBuf::from("/vault/Notes"))
        );
        assert_eq!(
            drop_folder_for_row(Path::new("/vault/Notes/Deep/Idea.md"), root, false),
            Some(PathBuf::from("/vault/Notes/Deep"))
        );
    }

    #[test]
    fn root_rows_fall_back_to_the_vault_root() {
        let root = Path::new("/vault");
        assert_eq!(
            drop_folder_for_row(Path::new("/vault/Idea.md"), root, false),
            None
        );
    }

    #[test]
    fn drop_box_spans_the_folder_and_its_visible_descendants() {
        let target = Path::new("/vault/Notes");
        let paths = [
            "/vault/Notes",
            "/vault/Notes/Idea.md",
            "/vault/Notes/Deep/Plan.md",
            "/vault/Notes2/Nope.md",
            "/vault/Other.md",
        ];
        assert_eq!(
            drop_box_range_for(target, paths.into_iter().map(PathBuf::from)),
            Some((0, 2))
        );
    }

    #[test]
    fn collapsed_folder_box_is_its_own_row() {
        let target = Path::new("/vault/Notes");
        let paths = ["/vault/Notes", "/vault/Other.md"];
        assert_eq!(
            drop_box_range_for(target, paths.into_iter().map(PathBuf::from)),
            Some((0, 0))
        );
    }
}
