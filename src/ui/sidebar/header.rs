use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    div, px,
};
use gpui_component::{ActiveTheme, Icon, IconName, h_flex};

use super::super::palette::PaletteKind;
use super::{DatalithView, TREE_PADDING_PX};
use crate::ui::icons::DatalithIcon;

const BORDER_WIDTH: f32 = 2.0;
const ICON_PADDING: f32 = 4.0;

impl DatalithView {
    pub(crate) fn render_sidebar_header(cx: &Context<Self>) -> impl IntoElement {
        div()
            .p(px(TREE_PADDING_PX - ICON_PADDING))
            .border_b(px(BORDER_WIDTH))
            .border_color(cx.theme().border)
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .gap(px(ICON_PADDING.mul_add(-2.0, TREE_PADDING_PX)))
                    .child(
                        h_flex()
                            .gap(px(ICON_PADDING.mul_add(-2.0, TREE_PADDING_PX)))
                            .child(
                                div()
                                    .id("search-trigger")
                                    .p(px(ICON_PADDING))
                                    .rounded_sm()
                                    .hover(|this| this.bg(cx.theme().sidebar_accent))
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
                                    .p(px(ICON_PADDING))
                                    .rounded_sm()
                                    .hover(|this| this.bg(cx.theme().sidebar_accent))
                                    .cursor_pointer()
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.palette.open_as(PaletteKind::QuickSwitcher);
                                        cx.notify();
                                    }))
                                    .child(IconName::LayoutDashboard),
                            ),
                    )
                    .child(
                        div()
                            .id("settings-trigger")
                            .p(px(ICON_PADDING))
                            .rounded_sm()
                            .hover(|this| this.bg(cx.theme().sidebar_accent))
                            .cursor_pointer()
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.settings.open();
                                cx.notify();
                            }))
                            .child(Icon::new(DatalithIcon::Settings)),
                    ),
            )
    }
}
