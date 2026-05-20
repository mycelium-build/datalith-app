use gpui::*;
use gpui_component::{
    IconName,
    button::{Button, ButtonVariants as _},
    h_flex,
    sidebar::SidebarHeader,
};

use super::super::palette::PaletteKind;
use super::DatalithView;

impl DatalithView {
    pub(crate) fn render_sidebar_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        SidebarHeader::new().p_2().child(
            h_flex()
                .w_full()
                .gap_1()
                .child(
                    Button::new("search-trigger")
                        .ghost()
                        .icon(IconName::Search)
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.palette.open_as(PaletteKind::Search);
                            cx.notify();
                        })),
                )
                .child(
                    Button::new("logo-trigger")
                        .ghost()
                        .icon(IconName::LayoutDashboard)
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.palette.open_as(PaletteKind::QuickSwitcher);
                            cx.notify();
                        })),
                ),
        )
    }
}
