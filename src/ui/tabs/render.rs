use std::path::Path;

use gpui_kit::component::{
    Disableable, Icon, IconName, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    tab::{Tab, TabBar},
};
use gpui_kit::{Context, IntoElement, ParentElement, SharedString, Styled};

use super::NavigationAction;
use crate::app::actions::ToggleEditorMode;
use crate::ui::DatalithView;
use crate::ui::icons::DatalithIcon;
use crate::vault::path::display_name;

impl DatalithView {
    pub(crate) fn render_tab_bar(&self, cx: &Context<Self>) -> impl IntoElement {
        let active_index = self.tabs.active_index().unwrap_or(0);
        let can_go_back = self.can_go_back();
        let can_go_forward = self.can_go_forward();

        TabBar::new("editor-tabs")
            .prefix(
                h_flex()
                    .px_1()
                    .gap_0()
                    .child(
                        Button::new("go-back")
                            .ghost()
                            .xsmall()
                            .icon(IconName::ArrowLeft)
                            .disabled(!can_go_back)
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.pending_navigation = Some(NavigationAction::GoBack);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("go-forward")
                            .ghost()
                            .xsmall()
                            .icon(IconName::ArrowRight)
                            .disabled(!can_go_forward)
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.pending_navigation = Some(NavigationAction::GoForward);
                                cx.notify();
                            })),
                    ),
            )
            .selected_index(active_index)
            .suffix({
                let handler = self.tabs.active_handler().cloned();
                let can_toggle_mode = handler
                    .as_ref()
                    .is_some_and(|handler| handler.read(cx).can_toggle_mode());
                let is_editing = handler
                    .as_ref()
                    .is_some_and(|handler| handler.read(cx).is_editing());
                let mut suffix = h_flex().gap_0().px_1();
                if can_toggle_mode {
                    let (icon, label) = if is_editing {
                        (Icon::new(IconName::Eye), "Switch tab to reading mode")
                    } else {
                        (Icon::new(DatalithIcon::Pen), "Switch tab to editing mode")
                    };
                    suffix = suffix.child(
                        Button::new("toggle-mode")
                            .ghost()
                            .xsmall()
                            .icon(icon)
                            .accessibility_label(label)
                            .tooltip(label)
                            .on_click(|_, window, cx| {
                                window.dispatch_action(Box::new(ToggleEditorMode), cx);
                            }),
                    );
                }
                suffix.child(
                    Button::new("new-tab")
                        .ghost()
                        .xsmall()
                        .icon(IconName::Plus)
                        .on_click(cx.listener(|view, _, _, cx| view.new_empty_tab(cx))),
                )
            })
            .on_click({
                let tree_state = self.tree_state.clone();
                cx.listener(move |view, index, _, cx| {
                    tree_state.update(cx, |state, cx| state.set_selected_index(None, cx));
                    view.last_sidebar_selection = None;
                    view.tabs.select(*index);
                    cx.notify();
                })
            })
            .children(
                self.tabs
                    .iter_with_id()
                    .map(|(index, id, path, _)| self.render_tab(index, id.as_str(), path, cx)),
            )
    }

    fn render_tab(&self, index: usize, identity: &str, path: &Path, cx: &Context<Self>) -> Tab {
        let display_name = display_name(path);
        let close_label = format!("Close {display_name}");
        Tab::new()
            .label(SharedString::from(display_name))
            .prefix(
                Icon::new(self.registry.config_for(path).icon)
                    .size_3()
                    .ml_2(),
            )
            .suffix(
                Button::new(format!("close-tab-{identity}"))
                    .icon(IconName::Close)
                    .ghost()
                    .xsmall()
                    .mr_1()
                    .accessibility_label(close_label)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        cx.stop_propagation();
                        view.close_tab(index, cx);
                    })),
            )
    }
}
