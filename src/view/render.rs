use std::path::PathBuf;

use gpui::*;
use gpui_component::{
    button::{Button, ButtonVariants as _},
    input::Input,
    list::ListItem,
    sidebar::SidebarHeader,
    tree::{self},
    ActiveTheme, Icon, IconName, h_flex, v_flex, v_virtual_list,
};

use super::DatalithView;

impl DatalithView {
    pub fn render_search_overlay(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let search_input = self.search_input.clone();
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
            .child(
                div()
                    .w(px(600.))
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_md()
                    .shadow_lg()
                    .id("search-panel")
                    .on_click(cx.listener(|_, _, _, cx| {
                        cx.stop_propagation();
                    }))
                    .child(
                        v_flex()
                            .overflow_hidden()
                            .on_key_down(cx.listener(
                                |this, event: &KeyDownEvent, _window, cx| {
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
                                },
                            ))
                            .child(Input::new(&search_input))
                            .child(
                                v_virtual_list(
                                    cx.entity().clone(),
                                    "search-results",
                                    self.search_item_sizes.clone(),
                                    move |view, visible_range, _, cx| {
                                        let selected_idx = view.search_selected;
                                        visible_range.map(move |i| {
                                            let r = &view.search_results[i];
                                            let file_name = r.path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
                                            let bg = if Some(i) == selected_idx { cx.theme().muted } else { gpui::Hsla::default() };
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
                                        }).collect()
                                    },
                                )
                                .track_scroll(&self.search_scroll_handle)
                                .h(px(400.)),
                            ),
                    ),
            )
    }
}

impl Render for DatalithView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.needs_search_focus {
            self.needs_search_focus = false;
            self.search_input.focus_handle(cx).focus(_window, cx);
        }

        let root_path = self.root_path.clone();
        let search_open = self.search_open;

        let mut layout = h_flex().size_full().relative();

        if search_open {
            layout = layout.child(deferred(self.render_search_overlay(cx)));
        }

        layout
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w(px(260.))
                    .h_full()
                    .bg(cx.theme().sidebar)
                    .border_r_1()
                    .border_color(cx.theme().border)
                    .child(
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
                        ),
                    )
                    .child(div().flex_1().child(tree::tree(&self.tree_state, {
                        let view = cx.entity();
                        move |ix, entry, selected, _window, cx| {
                            let is_folder = entry.is_folder();
                            let is_expanded = entry.is_expanded();
                            let item_id = entry.item().id.clone();
                            let item_label = entry.item().label.clone();
                            let depth = entry.depth();

                            view.update(cx, move |_this, cx| {
                                let icon = if !is_folder {
                                    IconName::File
                                } else if is_expanded {
                                    IconName::FolderOpen
                                } else {
                                    IconName::Folder
                                };

                                ListItem::new(ix)
                                    .selected(selected)
                                    .pl(px(16.) * depth + px(12.))
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(Icon::new(icon).size_4())
                                            .child(item_label.clone()),
                                    )
                                    .on_click(cx.listener({
                                        let item_id = item_id.clone();
                                        let is_folder = is_folder;
                                        move |this, _, window, cx| {
                                            if !is_folder {
                                                let path = PathBuf::from(item_id.to_string());
                                                this.open_file(path, window, cx);
                                            }
                                        }
                                    }))
                            })
                        }
                    }))),
            )
            .child(match self.editor_state.as_ref() {
                Some(editor) => Input::new(editor).h_full().into_any_element(),
                None => div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(match root_path {
                        Some(_) => "Select a file from the sidebar",
                        None => "Select a folder from the menu bar",
                    })
                    .into_any_element(),
            })
    }
}
