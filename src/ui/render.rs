use crate::consts::SIDEBAR_WIDTH;
use gpui::*;
use gpui_component::{
    h_flex,
    resizable::{h_resizable, resizable_panel},
    v_flex,
};
use std::path::PathBuf;

use super::DatalithView;

impl Render for DatalithView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.palette.needs_focus {
            let open_paths: Vec<PathBuf> = self.open_files.iter().map(|f| f.path.clone()).collect();
            self.palette
                .focus_input(_window, cx, self.vault_catalog.as_ref(), &open_paths);
        }

        for path in std::mem::take(&mut self.pending_external_updates) {
            if let Some(open_file) = self.open_files.iter().find(|file| file.path == path) {
                open_file.handler.update(cx, |handler, cx| {
                    if let Err(error) = handler.reload_from_disk(&path, _window, cx) {
                        eprintln!("Failed to reload {}: {error}", path.display());
                    }
                });
            }
        }

        if self.focus_sidebar_requested {
            self.focus_sidebar_requested = false;
            self.focus_sidebar(_window, cx);
        }

        if self.focus_editor_requested {
            self.focus_editor_requested = false;
            let active_tab = self.active_tab.min(self.open_files.len().saturating_sub(1));
            if let Some(ref file) = self.open_files.get(active_tab) {
                file.handler.read(cx).focus_handle(cx).focus(_window, cx);
            }
        }

        if self.rename_target.is_none() {
            if let Some(path) = self.pending_open.take() {
                self.open_file(path, true, _window, cx);
            }
        }

        if let Some(action) = self.pending_navigation.take() {
            match action {
                super::NavigationAction::GoBack => self.go_back(_window, cx),
                super::NavigationAction::GoForward => self.go_forward(_window, cx),
            }
        }

        let tree_state = self.tree_state.clone();

        let mut layout = h_flex().size_full().relative();

        layout = layout.child(
            h_resizable("datalith-main-layout")
                .child(
                    resizable_panel()
                        .size(px(SIDEBAR_WIDTH))
                        .size_range(px(180.)..px(500.))
                        .child(self.render_sidebar(_window, cx)),
                )
                .child(
                    resizable_panel().child(
                        div()
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
                    ),
                ),
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

        v_flex()
            .size_full()
            .overflow_hidden()
            .child(self.render_tab_bar(active_tab, cx))
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
            } else {
                div()
                    .flex_1()
                    .overflow_hidden()
                    .child(active_file.handler.clone())
                    .into_any_element()
            })
            .into_any_element()
    }
}
