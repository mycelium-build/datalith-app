use gpui::*;
use gpui_component::{
    IconName, Sizable,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::Input,
    tab::{Tab, TabBar},
    v_flex,
};

use crate::utils::file_name_str;

use super::DatalithView;

impl Render for DatalithView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.palette.needs_focus {
            self.palette.focus_input(_window, cx);
        }

        if self.focus_sidebar_requested {
            self.focus_sidebar_requested = false;
            self.focus_sidebar(_window, cx);
        }

        if self.focus_editor_requested {
            self.focus_editor_requested = false;
            let active_tab = self.active_tab.min(self.open_files.len().saturating_sub(1));
            if let Some(ref file) = self.open_files.get(active_tab) {
                if let Some(ref md_editor) = file.markdown_editor {
                    md_editor
                        .read(cx)
                        .input()
                        .focus_handle(cx)
                        .focus(_window, cx);
                } else if let Some(ref state) = file.state {
                    state.focus_handle(cx).focus(_window, cx);
                }
            }
        }

        if self.rename_target.is_none() {
            if let Some(path) = self.pending_open.take() {
                self.open_file(path, true, _window, cx);
            }
        }

        let tree_state = self.tree_state.clone();

        let mut layout = h_flex().size_full().relative();

        layout = layout.child(self.render_sidebar(_window, cx)).child(
            div()
                .flex_1()
                .size_full()
                .overflow_hidden()
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(move |this, _event: &MouseDownEvent, _window, cx| {
                        tree_state.update(cx, |state, cx| {
                            state.set_selected_index(None, cx);
                        });
                        this.last_sidebar_selection = None;
                    }),
                )
                .child(self.render_editor(cx)),
        );

        if self.palette.open {
            layout = layout.child(self.palette.render_overlay(cx));
        }

        if self.settings.open {
            layout = layout.child(self.settings.render_overlay(cx));
        }

        layout
    }
}

impl DatalithView {
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
                let name: SharedString = file_name_str(&f.path).into();
                (i, name)
            })
            .collect();

        v_flex()
            .size_full()
            .overflow_hidden()
            .child(
                TabBar::new("editor-tabs")
                    .selected_index(active_tab)
                    .on_click({
                        let tree_state = self.tree_state.clone();
                        cx.listener(move |view, index, _, cx| {
                            tree_state.update(cx, |state, cx| {
                                state.set_selected_index(None, cx);
                            });
                            view.last_sidebar_selection = None;
                            view.active_tab = *index;
                            cx.notify();
                        })
                    })
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
            } else if let Some(ref md_editor) = active_file.markdown_editor {
                div()
                    .flex_1()
                    .overflow_hidden()
                    .child(md_editor.clone())
                    .into_any_element()
            } else if let Some(ref active_state) = active_file.state {
                div()
                    .flex_1()
                    .child(Input::new(active_state).h_full().appearance(false))
                    .into_any_element()
            } else {
                div().flex_1().into_any_element()
            })
            .into_any_element()
    }
}
