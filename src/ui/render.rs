use super::DatalithView;
use gpui::*;
use gpui_component::{
    h_flex,
    resizable::{h_resizable, resizable_panel},
    v_flex,
};

const SIDEBAR_WIDTH: f32 = 260.0;

impl Render for DatalithView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.palette.needs_focus {
            let open_paths = self.tabs.open_paths();
            self.palette
                .focus_input(_window, cx, self.vault_catalog.as_ref(), &open_paths);
        }

        for path in std::mem::take(&mut self.pending_external_updates) {
            if let Some(handler) = self.tabs.handler_for_path(&path) {
                handler.update(cx, |handler, cx| {
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
            self.focus_active_tab(_window, cx);
        }

        if self.rename_target.is_none() {
            if let Some(path) = self.pending_open.take() {
                self.open_file(path, true, _window, cx);
            }
        }

        if let Some(action) = self.pending_navigation.take() {
            match action {
                super::tabs::NavigationAction::GoBack => self.go_back(_window, cx),
                super::tabs::NavigationAction::GoForward => self.go_forward(_window, cx),
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
        if self.tabs.is_empty() {
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

        let active_tab = self
            .tabs
            .active()
            .expect("non-empty tabs have an active tab");
        let is_empty = active_tab.path().as_os_str().is_empty();

        v_flex()
            .size_full()
            .overflow_hidden()
            .child(self.render_tab_bar(cx))
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
                    .child(active_tab.handler().clone())
                    .into_any_element()
            })
            .into_any_element()
    }
}
