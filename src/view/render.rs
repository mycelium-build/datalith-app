use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::*;
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent},
    list::ListItem,
    menu::ContextMenuExt,
    sidebar::SidebarHeader,
    tab::{Tab, TabBar},
    tree::{self},
    v_flex,
};

use crate::actions::{CopyPath, Delete, Duplicate, NewFile, NewFolder, OpenInExplorer, Rename};

use super::DatalithView;
use super::palette::PaletteKind;

#[derive(Clone)]
struct DragFile {
    path: PathBuf,
}

impl Render for DragFile {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let name = self
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
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

impl Render for DatalithView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.palette.needs_focus {
            self.palette.focus_input(_window, cx);
        }

        let mut layout = h_flex().size_full().relative();

        layout = layout
            .child(self.render_sidebar(_window, cx))
            .child(self.render_editor(cx));

        if self.palette.open {
            layout = layout.child(self.palette.render_overlay(cx));
        }

        layout
    }
}

impl DatalithView {
    fn render_sidebar(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tree_state = self.tree_state.clone();
        let view = cx.entity().clone();

        self.ensure_rename_state(window, cx);

        div()
            .flex()
            .flex_col()
            .w(px(260.))
            .h_full()
            .bg(cx.theme().sidebar)
            .border_r_1()
            .border_color(cx.theme().border)
            .on_key_down({
                let tree_state = tree_state.clone();
                cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                    if event.keystroke.key.as_str() == "enter" {
                        let ts = tree_state.read(cx);
                        if let Some(entry) = ts.selected_entry() {
                            if !entry.is_folder() {
                                let path = PathBuf::from(entry.item().id.to_string());
                                let new_tab = event.keystroke.modifiers.platform;
                                this.open_file(path, new_tab, window, cx);
                            }
                        }
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
                            let _ = std::fs::rename(&drag.path, &new_path);
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
            .child(div().flex_1().child(self.render_file_tree(
                cx,
                &self.tree_state,
                &self.drag_hover,
            )))
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

        let current = target
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        let old_ext = if !target.is_dir() {
            current
                .rfind('.')
                .and_then(|dot| if dot > 0 { Some(&current[dot..]) } else { None })
                .map(|e| e.to_string())
        } else {
            None
        };

        let state = cx.new(|cx| {
            gpui_component::input::InputState::new(window, cx).default_value(current.as_str())
        });
        let dir = target.parent().map(|p| p.to_path_buf());
        let target_clone = target.clone();

        self._rename_sub = Some(cx.subscribe_in(
            &state,
            window,
            move |this, input, event, _window, cx| match event {
                InputEvent::PressEnter { .. } => {
                    let mut new_name = input.read(cx).value().to_string();
                    if !new_name.is_empty() {
                        if let Some(ref ext) = old_ext {
                            if !new_name.contains('.') {
                                new_name.push_str(ext);
                            }
                        }
                        if let Some(parent) = &dir {
                            let new_path = parent.join(&new_name);
                            if new_path != target_clone {
                                let _ = std::fs::rename(&target_clone, &new_path);
                            }
                        }
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
            h_flex()
                .gap_2()
                .items_center()
                .w_full()
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(Icon::new(IconName::Folder))
                        .child(self.root_name.clone()),
                )
                .child(div().flex_1())
                .child(
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

    fn render_file_tree(
        &self,
        cx: &mut Context<Self>,
        tree_state_entity: &Entity<gpui_component::tree::TreeState>,
        drag_hover: &Rc<RefCell<Option<(PathBuf, Instant)>>>,
    ) -> impl IntoElement {
        let view = cx.entity();
        let drag_hover = drag_hover.clone();
        let tree_state = tree_state_entity.clone();

        tree::tree(tree_state_entity, {
            let view = view.clone();
            let drag_hover = drag_hover.clone();
            let tree_state = tree_state.clone();
            move |ix, entry, selected, _window, cx| {
                let item_id = entry.item().id.clone();
                let is_folder = entry.is_folder() || PathBuf::from(item_id.to_string()).is_dir();
                let is_expanded = entry.is_expanded();
                let item_label = entry.item().label.clone();
                let depth = entry.depth();

                let v = view.clone();
                let dh = drag_hover.clone();
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
                        .pl(px(16.) * depth + px(12.));

                    if is_renaming {
                        if let Some(rename_state) = this.rename_state.clone() {
                            return list_item.child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .overflow_hidden()
                                    .child(Icon::new(icon).size_4())
                                    .child(Input::new(&rename_state)),
                            );
                        }
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
                                let drag_hover = dh.clone();
                                let tree_state = ts.clone();
                                let eid = v.entity_id();
                                move |mut style, _drag, _window, cx| {
                                    style = style.bg(cx.theme().drop_target);

                                    let mut hover = drag_hover.borrow_mut();
                                    match &*hover {
                                        Some((path, instant)) if path == &folder_path => {
                                            if instant.elapsed() > Duration::from_millis(800) {
                                                *hover = None;
                                                let id: SharedString = folder_path
                                                    .to_string_lossy()
                                                    .to_string()
                                                    .into();
                                                tree_state.update(cx, |state, cx| {
                                                    state.expand_by_id(&id, cx);
                                                });
                                            }
                                        }
                                        _ => {
                                            *hover = Some((folder_path.clone(), Instant::now()));
                                        }
                                    }

                                    cx.notify(eid);
                                    style
                                }
                            })
                            .on_drop(cx.listener({
                                let v2 = v.clone();
                                let target_dir = drag_path.clone();
                                move |_this, drag: &DragFile, window, cx| {
                                    if let Some(name) = drag.path.file_name() {
                                        let new_path = target_dir.join(name);
                                        if new_path != drag.path {
                                            let _ = std::fs::rename(&drag.path, &new_path);
                                        }
                                    }
                                    let view = v2.clone();
                                    window.defer(cx, move |_window, cx| {
                                        view.update(cx, |view, cx| {
                                            view.refresh_tree(cx);
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

    fn render_editor(&self, cx: &mut Context<Self>) -> impl IntoElement {
        if self.open_files.is_empty() {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(match self.root_path {
                    Some(_) => "Select a file from the sidebar",
                    None => "Select a folder from the menu bar",
                })
                .into_any_element();
        }

        let active_tab = self.active_tab.min(self.open_files.len().saturating_sub(1));
        let active_file = &self.open_files[active_tab];
        let is_empty = active_file.path.as_os_str().is_empty();

        let tab_data: Vec<(usize, SharedString)> = self
            .open_files
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let name: SharedString = f
                    .path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("New Tab")
                    .into();
                (i, name)
            })
            .collect();

        v_flex()
            .size_full()
            .child(
                TabBar::new("editor-tabs")
                    .selected_index(active_tab)
                    .on_click(cx.listener(|view, index, _, cx| {
                        view.active_tab = *index;
                        cx.notify();
                    }))
                    .children(tab_data.into_iter().map(|(i, name)| {
                        Tab::new().label(name).suffix(
                            Button::new(format!("close-tab-{}", i))
                                .icon(IconName::Close)
                                .ghost()
                                .xsmall()
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    cx.stop_propagation();
                                    view.close_tab(i, cx);
                                })),
                        )
                    })),
            )
            .child(if is_empty {
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_4()
                    .child("Empty tab")
                    .into_any_element()
            } else if let Some(ref active_state) = active_file.state {
                div()
                    .flex_1()
                    .child(Input::new(active_state).h_full())
                    .into_any_element()
            } else {
                div().flex_1().into_any_element()
            })
            .into_any_element()
    }
}
