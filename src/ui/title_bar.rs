use conv::{ConvUtil as _, UnwrapOrInf as _};
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, TitleBar,
    button::{Button, ButtonVariants as _},
    h_flex,
    menu::AppMenuBar,
    progress::ProgressCircle,
};
use gpui_kit::{
    App, Context, Entity, InteractiveElement as _, IntoElement, MouseButton, ParentElement as _,
    Render, StatefulInteractiveElement as _, Styled as _, Subscription, Window, div,
    prelude::FluentBuilder as _,
};

use super::icons::DatalithIcon;
use crate::app::{
    actions::{OpenSettings, ToggleQuickSwitcher, ToggleSearch},
    update::{UpdatePresentation, Updater},
};

/// Compose Datalith's commands inside the platform-aware window chrome.
pub(crate) fn render(
    menu_bar: Option<Entity<AppMenuBar>>,
    update_control: Option<Entity<UpdateControl>>,
    cx: &App,
) -> impl IntoElement {
    TitleBar::new()
        .min_h_8()
        .bg(cx.theme().tab_bar)
        .border_color(cx.theme().border)
        .child(
            h_flex()
                .flex_1()
                .min_w_0()
                .h_full()
                .gap_3()
                .when_some(menu_bar, |bar, menu| {
                    bar.child(div().min_w_0().h_full().occlude().child(menu))
                })
                .child(
                    h_flex()
                        .id("title-bar-search")
                        .flex_none()
                        .gap_1()
                        .occlude()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(|_, _, cx| cx.stop_propagation())
                        .child(
                            Button::new("search-trigger")
                                .ghost()
                                .small()
                                .icon(IconName::Search)
                                .accessibility_label("Search files")
                                .tooltip_with_action("Search files", &ToggleSearch, None)
                                .on_click(|_, window, cx| {
                                    window.dispatch_action(Box::new(ToggleSearch), cx);
                                }),
                        )
                        .child(
                            Button::new("switcher-trigger")
                                .ghost()
                                .small()
                                .icon(IconName::LayoutDashboard)
                                .accessibility_label("Quick switcher")
                                .tooltip_with_action("Quick switcher", &ToggleQuickSwitcher, None)
                                .on_click(|_, window, cx| {
                                    window.dispatch_action(Box::new(ToggleQuickSwitcher), cx);
                                }),
                        ),
                )
                // Leave an unobstructed region for dragging, even when the menus scroll.
                .child(div().flex_1().min_w_12().h_full())
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
                                .accessibility_label("Settings…")
                                .tooltip_with_action("Settings…", &OpenSettings, None)
                                .on_click(|_, window, cx| {
                                    window.dispatch_action(Box::new(OpenSettings), cx);
                                }),
                        ),
                ),
        )
}

pub(crate) struct UpdateControl {
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
                button.disabled(true).label("Downloading update").icon(
                    ProgressCircle::new("update-progress")
                        .size_5()
                        .value(progress)
                        .loading(total.is_none())
                        .accessibility_label("Downloading update")
                        .child(Icon::new(DatalithIcon::Download).size_3()),
                )
            }
            UpdatePresentation::Ready { version } => button
                .icon(Icon::new(DatalithIcon::Download))
                .label("Restart to update")
                .tooltip(format!("Restart to update to {version}")),
            UpdatePresentation::External { version } => button
                .icon(Icon::new(DatalithIcon::Download))
                .label("Download update")
                .tooltip(format!(
                    "Download {} {version}",
                    crate::channel::Channel::current().product_name()
                )),
            UpdatePresentation::Applying => button
                .icon(Icon::new(DatalithIcon::Download))
                .label("Installing…")
                .disabled(true),
        };
        div()
            .id("title-bar-update")
            .tab_group()
            .child(button.on_click(cx.listener(|this, _, _, cx| {
                this.updater.update(cx, Updater::activate);
            })))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use gpui_kit::component::{Root, Theme, ThemeMode};
    use gpui_kit::test::TestWindowExt as _;
    use gpui_kit::{AppContext as _, InputEvent as _, TestAppContext, px, size};

    use super::*;

    struct TestTitleBar {
        menu_bar: Option<Entity<AppMenuBar>>,
    }

    impl Render for TestTitleBar {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            super::render(self.menu_bar.clone(), None, cx)
        }
    }

    #[test]
    fn application_menu_opens_and_dispatches_settings() {
        let mut cx = TestAppContext::single();
        let settings_opened = Rc::new(std::cell::Cell::new(false));
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::app::menus::install(cx);
            let opened = settings_opened.clone();
            cx.on_action(move |_: &OpenSettings, _| opened.set(true));
        });
        let handle = cx.open_window(size(px(800.), px(480.)), |window, cx| {
            let view = cx.new(|cx| TestTitleBar {
                menu_bar: Some(AppMenuBar::new(cx)),
            });
            Root::new(view, window, cx)
        });
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window
                .within("app-menu-bar")
                .within(0_usize)
                .click("menu", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.find("popup-menu").visible());
            let settings = window.within("popup-menu").find(2_usize);
            assert_eq!(settings.label(), Some("Settings"));
            window.within("popup-menu").click(2_usize, cx);
        })
        .unwrap();
        cx.run_until_parked();
        assert!(settings_opened.get());
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("popup-menu").is_none());
        })
        .unwrap();
    }

    #[test]
    fn commands_are_visible_and_dispatch_from_the_title_bar() {
        for show_menus in [false, true] {
            let mut cx = TestAppContext::single();
            let commands = Rc::new(RefCell::new(Vec::new()));
            cx.update(|cx| {
                gpui_kit::init(cx);
                crate::app::menus::install(cx);
                let search = commands.clone();
                cx.on_action(move |_: &ToggleSearch, _| search.borrow_mut().push("search"));
                let switcher = commands.clone();
                cx.on_action(move |_: &ToggleQuickSwitcher, _| {
                    switcher.borrow_mut().push("switcher");
                });
                let settings = commands.clone();
                cx.on_action(move |_: &OpenSettings, _| settings.borrow_mut().push("settings"));
            });
            let handle = cx.open_window(size(px(800.), px(480.)), |window, cx| {
                let view = cx.new(|cx| TestTitleBar {
                    menu_bar: show_menus.then(|| AppMenuBar::new(cx)),
                });
                Root::new(view, window, cx)
            });

            cx.update_window(handle.into(), |_, window, cx| {
                for mode in [ThemeMode::Light, ThemeMode::Dark] {
                    Theme::change(mode, Some(window), cx);
                    for font_size in [14., 20.] {
                        Theme::global_mut(cx).font_size = px(font_size);
                        window.render_frame(cx);
                        let search = window.find("search-trigger");
                        let switcher = window.find("switcher-trigger");
                        let settings = window.find("settings-trigger");
                        assert!(search.visible() && switcher.visible() && settings.visible());
                        assert_eq!(search.label(), Some("Search files"));
                        assert_eq!(switcher.label(), Some("Quick switcher"));
                        assert_eq!(settings.label(), Some("Settings…"));
                        assert!(search.bounds().right() <= switcher.bounds().left());
                        assert!(switcher.bounds().right() < settings.bounds().left());
                        assert_eq!(search.bounds().center().y, settings.bounds().center().y);
                        assert!(settings.bounds().right() <= px(800.));
                        assert!(settings.bounds().top() >= px(0.));
                    }
                }
                Theme::global_mut(cx).font_size = px(16.);
                window.render_frame(cx);
            })
            .unwrap();

            for id in ["search-trigger", "switcher-trigger", "settings-trigger"] {
                cx.update_window(handle.into(), |_, window, cx| window.click(id, cx))
                    .unwrap();
                cx.run_until_parked();
            }
            assert_eq!(*commands.borrow(), ["search", "switcher", "settings"]);

            cx.update_window(handle.into(), |_, window, cx| {
                // All commands remain keyboard accessible after moving out of the sidebar.
                for _ in 0..8 {
                    window.focus_next(cx);
                    window.render_frame(cx);
                    if window.find("settings-trigger").focused() == Some(true) {
                        break;
                    }
                }
                assert_eq!(window.find("settings-trigger").focused(), Some(true));
                let keystroke = gpui_kit::Keystroke::parse("enter").unwrap();
                window.dispatch_event(
                    gpui_kit::KeyDownEvent {
                        keystroke: keystroke.clone(),
                        is_held: false,
                        prefer_character_input: false,
                    }
                    .to_platform_input(),
                    cx,
                );
                window.dispatch_event(gpui_kit::KeyUpEvent { keystroke }.to_platform_input(), cx);
            })
            .unwrap();
            cx.run_until_parked();
            assert_eq!(
                *commands.borrow(),
                ["search", "switcher", "settings", "settings"]
            );
        }
    }
}
