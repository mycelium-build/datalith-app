use super::DatalithView;
use gpui::{
    Context, InteractiveElement, IntoElement, MouseDownEvent, ParentElement, Render, Styled,
    Window, div, px,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, Root, Sizable, WindowExt,
    button::{Button, ButtonVariants as _},
    h_flex,
    resizable::{h_resizable, resizable_panel},
    v_flex,
};

use crate::ui::monolith::monolith_mark;

const SIDEBAR_WIDTH: f32 = 260.0;
const GLYPH_CELL: f32 = 4.0;

impl Render for DatalithView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.palette.needs_focus {
            let open_paths = self.tabs.open_paths();
            self.palette
                .focus_input(window, cx, self.vault_catalog.as_ref(), &open_paths);
        }

        if self.startup.is_none() {
            for notification in self.pending_notifications.drain(..) {
                window.push_notification(notification, cx);
            }
        }

        for path in std::mem::take(&mut self.pending_external_updates) {
            if let Some(handler) = self.tabs.handler_for_path(&path) {
                handler.update(cx, |handler, cx| {
                    if let Err(error) = handler.reload_from_disk(&path, window, cx) {
                        eprintln!("Failed to reload {}: {error}", path.display());
                    }
                });
            }
        }

        if self.focus_sidebar_requested {
            self.focus_sidebar_requested = false;
            self.focus_sidebar(window, cx);
        }

        if self.focus_editor_requested {
            self.focus_editor_requested = false;
            self.focus_active_tab(window, cx);
        }

        if self.rename_target.is_none()
            && let Some(path) = self.pending_open.take()
        {
            self.open_file(path, true, window, cx);
        }

        if let Some(action) = self.pending_navigation.take() {
            match action {
                super::tabs::NavigationAction::GoBack => self.go_back(window, cx),
                super::tabs::NavigationAction::GoForward => self.go_forward(window, cx),
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
                        .child(self.render_sidebar(window, cx)),
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

        if self.licenses.is_open() {
            layout = layout.child(self.licenses.render_overlay(cx));
        }

        layout
            .children(Root::render_notification_layer(window, cx))
            .children(self.startup.clone())
    }
}

impl DatalithView {
    fn render_empty_hint(cx: &Context<Self>, hint: &'static str) -> impl IntoElement {
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_5()
            .child(monolith_mark(GLYPH_CELL, cx.theme().primary.opacity(0.55)))
            .child(div().text_color(cx.theme().muted_foreground).child(hint))
    }

    fn render_quick_start(cx: &Context<Self>) -> impl IntoElement {
        v_flex()
            .flex_1()
            .size_full()
            .items_center()
            .justify_center()
            .gap_5()
            .child(
                Icon::new(IconName::Plus)
                    .size_8()
                    .text_color(cx.theme().primary.opacity(0.55)),
            )
            .child(
                div()
                    .text_color(cx.theme().muted_foreground)
                    .child("Start writing"),
            )
            .child(
                h_flex()
                    .gap_3()
                    .child(Self::quick_create_button("note", "New note", "md", cx))
                    .child(Self::quick_create_button("todo", "New todo", "todotxt", cx))
                    .child(Self::quick_create_button("graph", "New graph", "graph", cx)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground.opacity(0.7))
                    .child("⌘⇧F search  ·  ⌘T new tab  ·  ⌘0 focus sidebar"),
            )
    }

    fn quick_create_button(
        id: &'static str,
        label: &'static str,
        extension: &'static str,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        Button::new(id)
            .ghost()
            .small()
            .label(label)
            .on_click(cx.listener(move |view, _, _, cx| {
                view.create_quick_file(extension, cx);
            }))
    }

    fn render_editor(&self, cx: &Context<Self>) -> impl IntoElement {
        if self.tabs.is_empty() {
            return if self.root_path.is_some() {
                Self::render_quick_start(cx).into_any_element()
            } else {
                Self::render_empty_hint(cx, "Select a folder from the menu bar").into_any_element()
            };
        }

        let Some(active_tab) = self.tabs.active() else {
            return div().size_full().into_any_element();
        };
        let is_empty = active_tab.path().as_os_str().is_empty();

        v_flex()
            .size_full()
            .overflow_hidden()
            .child(self.render_tab_bar(cx))
            .child(if is_empty {
                Self::render_quick_start(cx).into_any_element()
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
