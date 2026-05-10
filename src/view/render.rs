use std::path::PathBuf;

use gpui::*;
use gpui_component::{
    button::{Button, ButtonVariants as _},
    input::{Input, InputEvent},
    list::ListItem,
    menu::ContextMenuExt,
    sidebar::SidebarHeader,
    tree::{self},
    ActiveTheme, Icon, IconName, h_flex, v_flex, v_virtual_list,
};

use crate::actions::{
    CopyPath, Delete, Duplicate, NewFile, NewFolder, OpenInExplorer, Rename,
};

use super::DatalithView;

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

impl DatalithView {
    pub fn render_search_overlay(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .absolute()
            .inset_0()
            .bg(gpui::black().opacity(0.3))
            .flex()
            .items_center()
            .justify_center()
            .id("search-backdrop")
            .on_click(cx.listener(|this, _, _, cx| {
                this.search_open = false;
                cx.notify();
            }))
            .child(self.render_search_panel(cx))
    }

    fn render_search_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let search_input = self.search_input.clone();

        div()
            .w(px(600.))
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border)
            .rounded_md()
            .shadow_lg()
            .id("search-panel")
            .on_click(cx.listener(|_, _, _, cx| cx.stop_propagation()))
            .child(
                v_flex()
                    .overflow_hidden()
                    .on_key_down(cx.listener(Self::handle_search_keydown))
                    .child(Input::new(&search_input))
                    .child(self.render_search_results(cx)),
            )
    }

    fn handle_search_keydown(
        this: &mut Self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let count = this.search_results.len();
        if count == 0 {
            return;
        }
        match event.keystroke.key.as_str() {
            "down" => {
                let next = match this.search_selected {
                    Some(i) if i + 1 < count => i + 1,
                    None => 0,
                    _ => return,
                };
                this.search_selected = Some(next);
                this.scroll_to_selected(next);
                cx.notify();
            }
            "up" => {
                let prev = match this.search_selected {
                    Some(i) if i > 0 => i - 1,
                    Some(_) => return,
                    None => count - 1,
                };
                this.search_selected = Some(prev);
                this.scroll_to_selected(prev);
                cx.notify();
            }
            _ => {}
        }
    }

    fn render_search_results(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_virtual_list(
            cx.entity().clone(),
            "search-results",
            self.search_item_sizes.clone(),
            move |view, visible_range, _, cx| {
                let selected_idx = view.search_selected;
                visible_range
                    .map(move |i| {
                        let r = &view.search_results[i];
                        let file_name = r
                            .path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("")
                            .to_string();
                        let bg = if Some(i) == selected_idx {
                            cx.theme().muted
                        } else {
                            gpui::Hsla::default()
                        };
                        let path = r.path.clone();
                        let snippet = r.snippet.clone();
                        div()
                            .px_2()
                            .py_1()
                            .bg(bg)
                            .hover(|s| s.bg(cx.theme().muted))
                            .cursor_pointer()
                            .child(
                                v_flex()
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(Icon::new(IconName::File).size_3())
                                            .child(file_name),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .pl_5()
                                            .child(snippet),
                                    ),
                            )
                            .id(ElementId::Name(format!("result-{i}").into()))
                            .on_click(cx.listener(move |view, _, window, cx| {
                                view.search_open = false;
                                view.open_file(path.clone(), window, cx);
                            }))
                    })
                    .collect()
            },
        )
        .track_scroll(&self.search_scroll_handle)
        .h(px(400.))
    }
}

impl Render for DatalithView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.needs_search_focus {
            self.needs_search_focus = false;
            self.search_input.focus_handle(cx).focus(_window, cx);
        }

        let mut layout = h_flex().size_full().relative();

        if self.search_open {
            layout = layout.child(deferred(self.render_search_overlay(cx)));
        }

        layout
            .child(self.render_sidebar(_window, cx))
            .child(self.render_editor())
    }
}

impl DatalithView {
    fn render_sidebar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
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
            .child(self.render_sidebar_header(cx))
            .child(div().flex_1().child(self.render_file_tree(cx)))
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
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.search_open = true;
                            this.needs_search_focus = true;
                            cx.notify();
                        })),
                ),
        )
    }

    fn render_file_tree(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();

        tree::tree(&self.tree_state, {
            let view = view.clone();
            move |ix, entry, selected, _window, cx| {
                let is_folder = entry.is_folder();
                let is_expanded = entry.is_expanded();
                let item_id = entry.item().id.clone();
                let item_label = entry.item().label.clone();
                let depth = entry.depth();

                let v = view.clone();
                view.update(cx, move |this, cx| {
                    let is_renaming = this
                        .rename_target
                        .as_ref()
                        .map(|p| p.to_string_lossy().to_string() == item_id.to_string())
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
                            .drag_over::<DragFile>(|this, _drag, _window, cx| {
                                this.bg(cx.theme().drop_target)
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

                    list_item.on_click(
                        cx.listener({
                            let drag_path = drag_path.clone();
                            move |this, _, window, cx| {
                                if !is_folder {
                                    this.open_file(drag_path.clone(), window, cx);
                                }
                            }
                        }),
                    )
                })
            }
        })
    }

    fn render_editor(&self) -> impl IntoElement {
        match self.editor_state.as_ref() {
            Some(editor) => Input::new(editor).h_full().into_any_element(),
            None => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(match self.root_path {
                    Some(_) => "Select a file from the sidebar",
                    None => "Select a folder from the menu bar",
                })
                .into_any_element(),
        }
    }
}
