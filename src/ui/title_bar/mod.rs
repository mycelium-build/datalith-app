use conv::{ConvUtil as _, UnwrapOrInf as _};
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, TitleBar,
    button::{Button, ButtonVariants as _},
    h_flex,
    menu::DropdownMenu as _,
    progress::ProgressCircle,
};
use gpui_kit::{
    Action, App, Context, Entity, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Subscription, Window,
    div, prelude::FluentBuilder as _, px,
};

use super::icons::DatalithIcon;
mod application_menu;
use crate::app::{
    actions::{OpenSettings, OpenShortcuts, OpenThemes, ToggleQuickSwitcher, ToggleSearch},
    update::{UpdatePresentation, Updater},
};
pub use application_menu::ApplicationMenu;

/// Compose Datalith's commands inside the platform-aware window chrome.
pub fn render(
    menu_bar: Option<Entity<ApplicationMenu>>,
    update_control: Option<Entity<UpdateControl>>,
    window: &Window,
    cx: &App,
) -> impl IntoElement {
    TitleBar::new()
        .min_h_8()
        .bg(cx.theme().title_bar)
        .border_color(cx.theme().title_bar_border)
        // The content owns the traffic-light inset so both sides of the wordmark can have equal width,
        // keeping it at the actual window center on macOS.
        .when(cfg!(target_os = "macos"), gpui_kit::Styled::pl_0)
        .child(render_content(menu_bar, update_control, window, cx))
}

fn render_content(
    menu_bar: Option<Entity<ApplicationMenu>>,
    update_control: Option<Entity<UpdateControl>>,
    window: &Window,
    cx: &App,
) -> impl IntoElement {
    let native_menus = menu_bar.is_none();
    h_flex()
        .flex_1()
        .min_w_0()
        .h_full()
        .gap_3()
        .map(|bar| {
            if let Some(menu_bar) = menu_bar {
                bar.child(branding(window, cx)).child(menu_bar)
            } else {
                // macOS insets the controls past the traffic lights, which fullscreen hides.
                let inset = if window.is_fullscreen() {
                    px(0.)
                } else {
                    px(80.)
                };
                bar.child(h_flex().flex_1().min_w_0().child(
                    // Keep the platform inset inside the equal-width column.
                    // GPUI Component 0.6.1 keeps its 80px macOS inset private.
                    div().pl(inset).child(search_controls()),
                ))
                .child(branding(window, cx))
            }
        })
        .child(
            h_flex()
                .when(native_menus, |row| row.flex_1().min_w_0().justify_end())
                .when(!native_menus, gpui_kit::Styled::flex_none)
                .child(
                    h_flex()
                        .id("title-bar-actions")
                        .flex_none()
                        .gap_1()
                        .pr_2()
                        .occlude()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(|_, _, cx| cx.stop_propagation())
                        .children(update_control)
                        .child(
                            Button::new("settings-trigger")
                                .ghost()
                                .small()
                                .icon(Icon::new(DatalithIcon::Settings))
                                .accessibility_label("Preferences menu")
                                .tooltip("Settings, theme and shortcuts")
                                .dropdown_menu(|menu, _, _| {
                                    menu.menu("Settings", Box::new(OpenSettings))
                                        .menu("Theme", Box::new(OpenThemes))
                                        .menu("Shortcuts", Box::new(OpenShortcuts))
                                }),
                        ),
                ),
        )
}

fn branding(_window: &Window, cx: &App) -> impl IntoElement {
    let cell = 16. / 32.;
    h_flex()
        .id("title-bar-brand")
        .flex_none()
        .gap_2()
        .font_family(crate::app::fonts::PIXELOID_FONT)
        .text_sm()
        .text_color(cx.theme().foreground)
        .child(super::monolith::monolith_mark(cell, cx.theme().primary))
        .child("Datalith")
}

fn search_controls() -> impl IntoElement {
    h_flex()
        .id("title-bar-search")
        .flex_none()
        .gap_1()
        .occlude()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(|_, _, cx| cx.stop_propagation())
        .child(command_button(
            "search-trigger",
            IconName::Search,
            "Search files",
            ToggleSearch,
        ))
        .child(command_button(
            "switcher-trigger",
            IconName::LayoutDashboard,
            "Quick switcher",
            ToggleQuickSwitcher,
        ))
}

fn command_button(
    id: &'static str,
    icon: impl Into<Icon>,
    label: &'static str,
    action: impl Action,
) -> Button {
    Button::new(id)
        .ghost()
        .small()
        .icon(icon.into())
        .accessibility_label(label)
        .tooltip_with_action(label, &action, None)
        .on_click(move |_, window, cx| window.dispatch_action(action.boxed_clone(), cx))
}

/// Renders update progress and commands, observing state owned by `Updater`.
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        const COMPACT_WIDTH_REM: f32 = 40.;

        // At large text sizes, keep the update action reachable
        // without covering the centered wordmark or the other title-bar commands.
        let compact =
            window.viewport_size().width.as_f32() < window.rem_size().as_f32() * COMPACT_WIDTH_REM;
        let button = Button::new("update-control")
            .ghost()
            .small()
            .icon(Icon::new(DatalithIcon::Download));
        let (button, label) = match self.updater.read(cx).presentation() {
            UpdatePresentation::Hidden => return div().into_any_element(),
            UpdatePresentation::Downloading { received, total } => {
                let total = total.filter(|total| *total > 0);
                let progress = total.map_or(0., |total| {
                    received.approx_as::<f32>().unwrap_or_inf()
                        / total.approx_as::<f32>().unwrap_or_inf()
                        * 100.
                });
                (
                    button.disabled(true).tooltip("Downloading update").icon(
                        ProgressCircle::new("update-progress")
                            .size_5()
                            .value(progress)
                            .loading(total.is_none())
                            .accessibility_label("Downloading update")
                            .child(Icon::new(DatalithIcon::Download).size_3()),
                    ),
                    "Downloading update",
                )
            }
            UpdatePresentation::Ready { version } => (
                button.tooltip(format!("Restart to update to {version}")),
                "Restart to update",
            ),
            UpdatePresentation::External { version } => (
                button.tooltip(format!(
                    "Download {} {version}",
                    crate::channel::Channel::current().product_name()
                )),
                "Download update",
            ),
            UpdatePresentation::Applying => (
                button.tooltip("Installing update").disabled(true),
                "Installing…",
            ),
        };
        let button = button
            .accessibility_label(label)
            .when(!compact, |button| button.label(label));
        div()
            .id("title-bar-update")
            .tab_group()
            .child(button.on_click(cx.listener(|this, _, _, cx| {
                this.updater.update(cx, Updater::activate);
            })))
            .into_any_element()
    }
}
