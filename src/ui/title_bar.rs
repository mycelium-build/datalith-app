use conv::{ConvUtil as _, UnwrapOrInf as _};
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, TitleBar,
    button::{Button, ButtonVariants as _},
    h_flex,
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
    actions::{OpenSettings, ToggleQuickSwitcher, ToggleSearch},
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
        .bg(cx.theme().tab_bar)
        .border_color(cx.theme().border)
        // The content owns the traffic-light inset so both sides of the wordmark
        // can have equal width, keeping it at the actual window center on macOS.
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
                bar.child(h_flex().flex_1().min_w_0().child(
                    // Keep the platform inset inside the equal-width column.
                    // GPUI Component 0.6.1 keeps its 80px macOS inset private.
                    div().pl(px(80.)).child(search_controls()),
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
                        .child(command_button(
                            "settings-trigger",
                            Icon::new(DatalithIcon::Settings),
                            "Settings…",
                            OpenSettings,
                        )),
                ),
        )
}

fn branding(window: &Window, cx: &App) -> impl IntoElement {
    use gpui_kit::base::TestSupportExt as _;

    // Scale the 32-cell source art to the same 1.25rem frame as toolbar icons.
    let cell = window.rem_size().as_f32() * 1.25 / 32.;
    h_flex()
        .id("title-bar-brand")
        .test_support()
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

        // At large text sizes, keep the update action reachable without covering
        // the centered wordmark or the other title-bar commands.
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

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use gpui_kit::component::input::{Input, InputState};
    use gpui_kit::component::{Root, Theme, ThemeMode};
    use gpui_kit::test::TestWindowExt as _;
    use gpui_kit::{AppContext as _, InputEvent as _, TestAppContext, px, size};

    use super::*;

    struct TestTitleBar {
        menu_bar: Option<Entity<ApplicationMenu>>,
        editor: Entity<InputState>,
    }

    impl Render for TestTitleBar {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            gpui_kit::component::v_flex()
                .size_full()
                .child(super::render(self.menu_bar.clone(), None, window, cx))
                .child(div().flex_1())
                .child(Input::new(&self.editor).id("test-editor"))
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
                menu_bar: Some(cx.new(|cx| ApplicationMenu::new(window, cx))),
                editor: cx.new(|cx| InputState::new(window, cx)),
            });
            Root::new(view, window, cx)
        });
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("popup-menu").is_none());
            assert!(window.find("search-trigger").visible());
            window.click("app-menu-trigger", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.find("popup-menu").visible());
            assert!(window.try_find("search-trigger").is_none());
            assert!(window.try_find("switcher-trigger").is_none());
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
            assert!(window.find("search-trigger").visible());
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
                    menu_bar: show_menus.then(|| cx.new(|cx| ApplicationMenu::new(window, cx))),
                    editor: cx.new(|cx| InputState::new(window, cx)),
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
                activate_focused_button("enter", window, cx);
            })
            .unwrap();
            cx.run_until_parked();
            assert_eq!(
                *commands.borrow(),
                ["search", "switcher", "settings", "settings"]
            );
        }
    }

    fn menu_window(cx: &mut TestAppContext) -> gpui_kit::WindowHandle<Root> {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::app::menus::install(cx);
        });
        let handle = cx.open_window(size(px(800.), px(480.)), |window, cx| {
            let view = cx.new(|cx| TestTitleBar {
                menu_bar: Some(cx.new(|cx| ApplicationMenu::new(window, cx))),
                editor: cx.new(|cx| InputState::new(window, cx)),
            });
            Root::new(view, window, cx)
        });
        cx.update_window(handle.into(), |_, window, _| window.activate_window())
            .unwrap();
        cx.run_until_parked();
        handle
    }

    fn activate_focused_button(key: &str, window: &mut Window, cx: &mut App) {
        // GPUI buttons activate on key-up; TestWindowExt::press sends key-down only.
        let keystroke = gpui_kit::Keystroke::parse(key).unwrap();
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
        window.render_frame(cx);
    }

    #[test]
    fn switching_menus_keeps_the_session_open_and_escape_restores_focus() {
        let mut cx = TestAppContext::single();
        let handle = menu_window(&mut cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.click("test-editor", cx);
            window.click("app-menu-trigger", cx);
            let first_item = window.within("popup-menu").find(0_usize);
            window.press("left", cx);
            assert_eq!(
                window.within("popup-menu").find(0_usize).label(),
                Some("Datalith Documentation")
            );
            window.press("right", cx);
            assert_eq!(
                window.within("popup-menu").find(0_usize).label(),
                first_item.label()
            );
            window.within("file-menu").hover("menu", cx);
            assert!(window.try_find("popup-menu").is_some(), "hover File");
            assert_eq!(
                window.within("popup-menu").find(0_usize).label(),
                Some("New File")
            );
            window.press("right", cx);
            assert!(window.try_find("popup-menu").is_some(), "right to Navigate");
            assert_eq!(
                window.within("popup-menu").find(0_usize).label(),
                Some("Open Vault")
            );
            window.press("left", cx);
            assert!(window.try_find("popup-menu").is_some(), "left to File");
            assert_eq!(
                window.within("popup-menu").find(0_usize).label(),
                Some("New File")
            );
            window.within("help-menu").click("menu", cx);
            assert!(window.try_find("popup-menu").is_some(), "click Help");
            assert_eq!(
                window.within("popup-menu").find(0_usize).label(),
                Some("Datalith Documentation")
            );
            window.hover("test-editor", cx);
            assert!(window.find("popup-menu").visible());
            window.press("escape", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("popup-menu").is_none());
            assert!(window.find("search-trigger").visible());
            assert_eq!(window.find("test-editor").focused(), Some(true));
        })
        .unwrap();
    }

    #[test]
    fn outside_click_restores_the_burger_and_search_controls() {
        let mut cx = TestAppContext::single();
        let handle = menu_window(&mut cx);
        for _ in 0..2 {
            cx.update_window(handle.into(), |_, window, cx| {
                window.click("test-editor", cx);
                window.click("app-menu-trigger", cx);
                assert!(window.find("popup-menu").visible());
                assert!(
                    window
                        .try_find("app-menu-trigger")
                        .is_none_or(|button| !button.visible())
                );
                window.click("test-editor", cx);
            })
            .unwrap();
            cx.run_until_parked();
            cx.update_window(handle.into(), |_, window, cx| {
                window.render_frame(cx);
                assert!(window.try_find("popup-menu").is_none());
                assert!(window.find("search-trigger").visible());
                assert!(window.find("app-menu-trigger").visible());
                assert_eq!(window.find("test-editor").focused(), Some(true));
            })
            .unwrap();
        }
    }

    #[test]
    fn keyboard_opens_menu_and_focus_departure_collapses_it() {
        let mut cx = TestAppContext::single();
        let handle = menu_window(&mut cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            for _ in 0..8 {
                window.focus_next(cx);
                window.render_frame(cx);
                if window.find("app-menu-trigger").focused() == Some(true) {
                    break;
                }
            }
            assert_eq!(window.find("app-menu-trigger").focused(), Some(true));
            activate_focused_button("enter", window, cx);
            assert!(window.find("popup-menu").visible());
            window.press("escape", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("app-menu-trigger").focused(), Some(true));
            activate_focused_button("space", window, cx);
            assert!(window.find("popup-menu").visible());
            window.blur(cx);
            window.render_frame(cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("popup-menu").is_none());
            assert!(window.find("search-trigger").visible());
            assert!(window.focused(cx).is_none());
        })
        .unwrap();
    }

    #[test]
    fn deactivating_the_window_closes_menus_without_reopening_on_return() {
        let mut cx = TestAppContext::single();
        let handle = menu_window(&mut cx);
        let other = cx.open_window(size(px(400.), px(300.)), |_, _| gpui_kit::Empty);
        cx.update_window(handle.into(), |_, window, cx| {
            window.activate_window();
            window.click("test-editor", cx);
            window.click("app-menu-trigger", cx);
            assert!(window.find("popup-menu").visible());
        })
        .unwrap();
        cx.update_window(other.into(), |_, window, _| window.activate_window())
            .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("popup-menu").is_none());
            window.activate_window();
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("popup-menu").is_none());
            assert!(window.find("search-trigger").visible());
            assert_eq!(window.find("test-editor").focused(), Some(true));
        })
        .unwrap();
    }

    #[test]
    fn expanded_menus_fit_the_minimum_window_at_larger_text_sizes() {
        let mut cx = TestAppContext::single();
        let handle = menu_window(&mut cx);
        cx.update_window(handle.into(), |_, window, cx| {
            for mode in [ThemeMode::Light, ThemeMode::Dark] {
                Theme::change(mode, Some(window), cx);
                for font_size in [14., 20.] {
                    Theme::global_mut(cx).font_size = px(font_size);
                    window.render_frame(cx);
                    let brand = window.find("title-bar-brand").bounds();
                    let settings = window.find("settings-trigger").bounds();
                    let trigger = window.find("app-menu-trigger").bounds();
                    window.click("app-menu-trigger", cx);
                    assert_eq!(window.find("title-bar-brand").bounds(), brand);
                    assert_eq!(window.find("settings-trigger").bounds(), settings);
                    assert!(
                        window
                            .try_find("app-menu-trigger")
                            .is_none_or(|button| !button.visible())
                    );
                    let application_menu = window.within("application-menu").find("menu").bounds();
                    assert_eq!(application_menu.left(), trigger.left());
                    assert!(application_menu.contains(&trigger.center()));
                    assert!(brand.right() <= application_menu.left());
                    for id in [
                        "application-menu",
                        "file-menu",
                        "navigate-menu",
                        "help-menu",
                    ] {
                        let menu = window.within(id).find("menu");
                        assert!(menu.visible());
                        assert!(menu.bounds().right() < settings.left());
                    }
                    let popup = window.find("popup-menu").bounds();
                    assert!(popup.left() >= px(0.) && popup.right() <= px(800.));
                    assert!(popup.bottom() <= px(480.));
                    window.click("test-editor", cx);
                }
            }
        })
        .unwrap();
    }

    #[test]
    fn native_menu_layout_centers_branding_at_each_text_size() {
        struct NativeTitleBar;
        impl Render for NativeTitleBar {
            fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                h_flex()
                    .w_full()
                    .h_8()
                    .child(super::render_content(None, None, window, cx))
            }
        }
        let mut cx = TestAppContext::single();
        cx.update(gpui_kit::init);
        let handle = cx.open_window(size(px(800.), px(480.)), |window, cx| {
            let view = cx.new(|_| NativeTitleBar);
            Root::new(view, window, cx)
        });
        for width in [800., 1440.] {
            cx.simulate_window_resize(handle.into(), size(px(width), px(480.)));
            cx.update_window(handle.into(), |_, window, cx| {
                for mode in [ThemeMode::Light, ThemeMode::Dark] {
                    Theme::change(mode, Some(window), cx);
                    for font_size in [14., 20.] {
                        Theme::global_mut(cx).font_size = px(font_size);
                        window.render_frame(cx);
                        let brand = window.find("title-bar-brand").bounds();
                        // Flex/text widths may be fractional; compare the rendered pixel.
                        assert_eq!(px(brand.center().x.as_f32().round()), px(width * 0.5));
                        assert!(window.find("switcher-trigger").bounds().right() < brand.left());
                        assert!(brand.right() < window.find("settings-trigger").bounds().left());
                        assert!(window.try_find("app-menu-trigger").is_none());
                    }
                }
            })
            .unwrap();
        }
    }
}
