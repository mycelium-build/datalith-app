use gpui::*;
use gpui_component::{
    Disableable, Icon, IconName, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    tab::{Tab, TabBar},
};

use super::NavigationAction;
use crate::app::assets::PEN_ICON;
use crate::ui::DatalithView;
use crate::vault::path::display_name;

impl DatalithView {
    pub(crate) fn render_tab_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let active_index = self.tabs.active_index().unwrap_or(0);
        let can_go_back = self.can_go_back();
        let can_go_forward = self.can_go_forward();
        let tab_data: Vec<_> = self
            .tabs
            .iter()
            .map(|(index, path, _)| (index, SharedString::from(display_name(path))))
            .collect();

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
                if let Some(handler) = handler
                    && can_toggle_mode
                {
                    let icon = if is_editing {
                        Icon::new(IconName::Eye)
                    } else {
                        Icon::default().path(SharedString::from(PEN_ICON))
                    };
                    suffix = suffix.child(
                        Button::new("toggle-mode")
                            .ghost()
                            .xsmall()
                            .icon(icon)
                            .on_click(cx.listener(move |_, _, _, cx| {
                                handler.update(cx, |handler, cx| handler.toggle_editing(cx));
                            })),
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
            .children(tab_data.into_iter().map(|(index, name)| {
                Tab::new().label(name).suffix(
                    Button::new(format!("close-tab-{index}"))
                        .icon(IconName::Close)
                        .ghost()
                        .xsmall()
                        .mx_1()
                        .on_click(cx.listener(move |view, _, _, cx| {
                            cx.stop_propagation();
                            view.close_tab(index, cx);
                        })),
                )
            }))
    }
}
