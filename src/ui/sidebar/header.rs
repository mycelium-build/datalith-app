use crate::app::update::{UpdatePresentation, Updater};
use conv::{ConvUtil as _, UnwrapOrInf as _};
use gpui_kit::component::{
    Disableable as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    progress::ProgressCircle,
};
use gpui_kit::{Entity, Render, Subscription, Window};

use gpui_kit::component::{ActiveTheme, Icon, IconName, h_flex};
use gpui_kit::{
    Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    div, px,
};

use super::super::palette::PaletteKind;
use super::{DatalithView, TREE_PADDING_PX};
use crate::ui::icons::DatalithIcon;

const BORDER_WIDTH: f32 = 2.0;
const ICON_PADDING: f32 = 4.0;

impl DatalithView {
    pub(crate) fn render_sidebar_header(&self, cx: &Context<Self>) -> impl IntoElement {
        div()
            .p(px(TREE_PADDING_PX - ICON_PADDING))
            .border_b(px(BORDER_WIDTH))
            .border_color(cx.theme().border)
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .flex_wrap()
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
                        h_flex()
                            .gap_1()
                            .children(self.update_control.clone())
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
                    ),
            )
    }
}

pub struct UpdateControl {
    updater: Entity<Updater>,
    _subscription: Subscription,
}

impl UpdateControl {
    pub(crate) fn new(updater: Entity<Updater>, cx: &mut Context<Self>) -> Self {
        let subscription = cx.observe(&updater, |_, _, cx| cx.notify());
        Self {
            updater,
            _subscription: subscription,
        }
    }
}

impl Render for UpdateControl {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let button = Button::new("update-control").ghost().small();
        let button = match self.updater.read(cx).presentation() {
            UpdatePresentation::Hidden => return div().into_any_element(),
            UpdatePresentation::Downloading { received, total } => {
                let total = total.filter(|total| *total > 0);
                let progress = total.map_or(0., |total| {
                    received.approx_as::<f32>().unwrap_or_inf()
                        / total.approx_as::<f32>().unwrap_or_inf()
                        * 100.
                });
                button
                    .disabled(true)
                    .accessibility_label("Downloading update")
                    .tooltip("Downloading update")
                    .icon(
                        ProgressCircle::new("update-progress")
                            .value(progress)
                            .loading(total.is_none())
                            .accessibility_label("Downloading update"),
                    )
            }
            UpdatePresentation::Ready { version } => button
                .icon(IconName::ArrowUp)
                .label("Restart to update")
                .tooltip(format!("Restart to update to {version}")),
            UpdatePresentation::External { version } => button
                .icon(IconName::ArrowUp)
                .label("Download update")
                .tooltip(format!("Download Datalith {version}")),
            UpdatePresentation::Applying => button
                .icon(IconName::ArrowUp)
                .label("Installing…")
                .disabled(true),
        };
        div()
            .id("sidebar-update")
            .tab_group()
            .child(button.on_click(cx.listener(|this, _, _, cx| {
                this.updater.update(cx, Updater::activate);
            })))
            .into_any_element()
    }
}
