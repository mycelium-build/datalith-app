use gpui::*;
use gpui_component::{ActiveTheme, IconName, h_flex};

use super::super::palette::PaletteKind;
use super::DatalithView;
use crate::consts::{BORDER_WIDTH, TREE_PADDING_PX};

impl DatalithView {
    pub(crate) fn render_sidebar_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .p(px(TREE_PADDING_PX))
            .border_b(px(BORDER_WIDTH))
            .border_color(cx.theme().border)
            .child(
                h_flex()
                    .w_full()
                    .gap(px(TREE_PADDING_PX))
                    .child(
                        div()
                            .id("search-trigger")
                            .cursor_pointer()
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.palette.open_as(PaletteKind::Search);
                                cx.notify();
                            }))
                            .child(IconName::Search),
                    )
                    .child(
                        div()
                            .id("switcher-trigger")
                            .cursor_pointer()
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.palette.open_as(PaletteKind::QuickSwitcher);
                                cx.notify();
                            }))
                            .child(IconName::LayoutDashboard),
                    ),
            )
    }
}
