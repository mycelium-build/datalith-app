use super::*;
use gpui_kit::component::{Root, Theme};
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{TestAppContext, WindowHandle, px, size};

fn open_editor(
    cx: &mut TestAppContext,
) -> (
    WindowHandle<Root>,
    Entity<crate::ui::DatalithView>,
    Entity<ThemeEditor>,
) {
    open_editor_at(cx, 1200., 900.)
}

fn open_editor_at(
    cx: &mut TestAppContext,
    width: f32,
    height: f32,
) -> (
    WindowHandle<Root>,
    Entity<crate::ui::DatalithView>,
    Entity<ThemeEditor>,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        fonts::load_embedded_fonts(cx);
        FontCatalog::init(cx);
        crate::app::themes::load_embedded_themes(cx);
        ThemeLibrary::init(cx);
        crate::ui::settings::SettingsView::init_theme_options(cx);
        crate::app::preferences::apply(cx);
        cx.set_global(crate::app::AppState::default());
    });
    let mut app = None;
    let handle = cx.open_window(size(px(width), px(height)), |window, cx| {
        let view = cx.new(|cx| crate::ui::DatalithView::new(false, vec![], window, cx));
        view.update(cx, |view, cx| {
            view.startup_driver = gpui_kit::Task::ready(());
            view.startup = None;
            view.open_theme_editor(window, cx);
        });
        app = Some(view.clone());
        Root::new(view, window, cx)
    });
    cx.update_window(handle.into(), |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    let app = app.unwrap();
    cx.update(|cx| {
        cx.global_mut::<crate::app::AppState>().view = Some(app.clone());
        crate::app::actions::register(cx);
        crate::app::keymap::register(cx);
    });
    let editor = cx.update(|cx| app.read(cx).tabs.theme_editor().cloned().unwrap());
    (handle, app, editor)
}

fn enter(cx: &mut TestAppContext, handle: WindowHandle<Root>, id: &'static str, text: &str) {
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click(id, cx);
        window.press("secondary-a", cx);
        window.input(text, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

#[test]
fn live_color_preview_can_be_continued_then_discarded_without_changing_the_saved_theme() {
    let mut cx = TestAppContext::single();
    let (handle, app, editor) = open_editor(&mut cx);
    let before = cx.update(|cx| cx.theme().background);
    enter(&mut cx, handle, "theme-color-value", "#123456");
    cx.update(|cx| {
        assert_eq!(
            cx.theme().background,
            gpui_kit::component::try_parse_color("#123456").unwrap()
        );
        assert!(editor.read(cx).dirty);
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.click(("close-tab", editor.entity_id()), cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("theme-continue").visible());
        window.click("theme-continue", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(app.read(cx).tabs.theme_editor().is_some());
        assert_eq!(
            cx.theme().background,
            gpui_kit::component::try_parse_color("#123456").unwrap()
        );
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.click(("close-tab", editor.entity_id()), cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("theme-discard", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(app.read(cx).tabs.theme_editor().is_none());
        assert_eq!(cx.theme().background, before);
    });
}

#[test]
fn save_and_leave_creates_an_independent_custom_theme_and_reloads_it() {
    let mut cx = TestAppContext::single();
    let (handle, app, editor) = open_editor(&mut cx);
    let name = format!("UI theme {:016x}", rand::random::<u64>());
    enter(&mut cx, handle, "theme-name", &name);
    enter(&mut cx, handle, "theme-color-value", "#456789");
    cx.update_window(handle.into(), |_, window, cx| {
        window.click(("close-tab", editor.entity_id()), cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("theme-save-and-leave", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(
            editor.read(cx).error.is_none(),
            "{:?}",
            editor.read(cx).error
        );
        assert!(app.read(cx).tabs.theme_editor().is_none());
        assert_eq!(cx.theme().background.to_hex(), "#456789");
        assert!(cx.global::<ThemeLibrary>().is_custom(&name));
        ThemeLibrary::init(cx);
        assert!(crate::app::preferences::apply(cx).is_empty());
        assert_eq!(Theme::global(cx).theme_name().as_str(), name);
        assert_eq!(cx.theme().background.to_hex(), "#456789");
    });
}

#[test]
fn invalid_color_blocks_saving_and_escape_cancels_the_close_request() {
    let mut cx = TestAppContext::single();
    let (handle, app, editor) = open_editor(&mut cx);
    let original = cx.update(|cx| editor.read(cx).color_hex.read(cx).value());
    enter(&mut cx, handle, "theme-color-value", "invalid");
    cx.update(|cx| {
        assert_eq!(
            editor.read(cx).color_hex.read(cx).value().as_str(),
            "invalid"
        );
        assert!(editor.read(cx).color_error.is_some());
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("save-theme", cx);
        assert!(editor.read(cx).color_error.is_some());
        assert!(app.read(cx).tabs.theme_editor().is_some());
        window.press("secondary-w", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert!(editor.read(cx).pending.is_some()));
    cx.update_window(handle.into(), |_, window, cx| window.press("escape", cx))
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(editor.read(cx).pending.is_none());
        assert!(app.read(cx).tabs.theme_editor().is_some());
    });
    enter(&mut cx, handle, "theme-color-value", &original);
    cx.update(|cx| assert!(editor.read(cx).color_error.is_none()));
}

#[test]
fn selecting_another_theme_requires_a_draft_decision_and_offers_theme_fonts() {
    let mut cx = TestAppContext::single();
    let (handle, _, editor) = open_editor(&mut cx);
    let name = format!("With fonts {:016x}", rand::random::<u64>());
    cx.update_window(handle.into(), |_, window, cx| {
        let mut document = themes::active_document(cx);
        document.set_name(&name);
        document.set_font(FontRole::Reading, Some("Arial".into()));
        cx.global_mut::<ThemeLibrary>()
            .save(&document, None)
            .unwrap();
        fonts::select(FontRole::Reading, Some("Helvetica".into()), cx).unwrap();
        editor.update(cx, |editor, cx| editor.load_active(window, cx));
    })
    .unwrap();
    cx.run_until_parked();
    enter(&mut cx, handle, "theme-color-value", "#112233");
    let selector = cx.update(|cx| editor.read(cx).selector.clone());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click(("select", selector.entity_id()), cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| window.input(&name, cx))
        .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.press("down", cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(editor.read(cx).pending.is_some());
        assert_ne!(themes::active_document(cx).name(), name);
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("theme-discard", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(themes::active_document(cx).name(), name);
        assert_eq!(fonts::family(FontRole::Reading, cx).as_str(), "Helvetica");
        assert_eq!(window.find("ok").label(), Some("Use theme fonts"));
        assert_eq!(window.find("cancel").label(), Some("Keep personal fonts"));
    })
    .unwrap();
    // Dialog uses a 250 ms wall-clock entrance animation in GPUI Kit 0.6.1.
    std::thread::sleep(std::time::Duration::from_millis(300));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("cancel", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window.try_find("ok").is_none(),
            "Keep must dismiss the font question"
        );
    })
    .unwrap();
    cx.update(|cx| assert_eq!(fonts::family(FontRole::Reading, cx).as_str(), "Helvetica"));
    cx.update_window(handle.into(), |_, window, cx| {
        // Exercise the same selection policy from the settings entry point.
        super::super::select_theme(&name, false, cx);
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("ok", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(!fonts::has_personal_fonts(cx));
        assert_eq!(fonts::family(FontRole::Reading, cx).as_str(), "Arial");
    });
}

#[test]
fn editor_controls_fit_small_windows_in_both_modes_and_at_larger_text_sizes() {
    for (width, height) in [(800., 600.), (1200., 900.)] {
        let mut cx = TestAppContext::single();
        let (handle, _, editor) = open_editor_at(&mut cx, width, height);
        let selector = cx.update(|cx| editor.read(cx).selector.clone());
        cx.update_window(handle.into(), |_, window, cx| {
            for mode in [
                gpui_kit::component::ThemeMode::Light,
                gpui_kit::component::ThemeMode::Dark,
            ] {
                Theme::change(mode, Some(window), cx);
                for font_size in [16., 24., 48.] {
                    Theme::global_mut(cx).font_size = px(font_size);
                    window.render_frame(cx);
                    let close = window.find(("close-tab", editor.entity_id()));
                    let selection = window.find(("select", selector.entity_id()));
                    let save = window.find("save-theme");
                    assert!(close.visible() && selection.visible() && save.visible(), "{mode:?}, {width}x{height}, font {font_size}: close {:?} {}, selection {:?} {}, save {:?} {}", close.bounds(), close.visible(), selection.bounds(), selection.visible(), save.bounds(), save.visible());
                    assert!(close.bounds().right() <= px(width));
                    assert!(selection.bounds().right() <= px(width));
                    assert!(selection.bounds().top() >= close.bounds().bottom());
                    assert!(save.bounds().top() > selection.bounds().bottom());
                    assert!(
                        save.bounds().bottom() <= px(height),
                        "{mode:?}, {width}x{height}, font {font_size}: save {:?}, viewport {:?}",
                        save.bounds(),
                        window.viewport_size()
                    );
                }
            }
        })
        .unwrap();
    }
}

#[test]
fn escape_closes_a_font_menu_without_closing_the_editor_tab() {
    use gpui_kit::Focusable as _;
    let mut cx = TestAppContext::single();
    let (handle, app, editor) = open_editor(&mut cx);
    enter(&mut cx, handle, "theme-color-value", "#112233");
    let picker = cx.update(|cx| editor.read(cx).fonts.first().unwrap().1.clone());
    cx.update_window(handle.into(), |_, window, cx| {
        window.focus(&picker.focus_handle(cx), cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| window.press("escape", cx))
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(app.read(cx).tabs.theme_editor().is_some());
        assert!(editor.read(cx).pending.is_none());
    });
    cx.update_window(handle.into(), |_, window, cx| window.press("escape", cx))
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert!(editor.read(cx).pending.is_none()));
}

#[test]
fn changing_mode_from_settings_returns_to_the_draft_before_applying() {
    let mut cx = TestAppContext::single();
    let (handle, app, editor) = open_editor(&mut cx);
    let before = crate::app::settings::snapshot().theme_preference;
    let mode = cx.update(|cx| cx.theme().mode);
    let requested = if mode.is_dark() {
        crate::app::settings::ThemePreference::Light
    } else {
        crate::app::settings::ThemePreference::Dark
    };
    enter(&mut cx, handle, "theme-color-value", "#789abc");
    cx.update_window(handle.into(), |_, window, cx| {
        app.update(cx, |view, cx| {
            view.open_shortcuts(window, cx);
            view.settings.open();
            view.settings.focus(window, cx);
        });
        super::super::change_mode(requested, cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(!app.read(cx).settings.open);
        assert_eq!(cx.theme().mode, mode);
        assert_eq!(crate::app::settings::snapshot().theme_preference, before);
        assert!(editor.read(cx).pending.is_some());
        window.click("theme-continue", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(
            cx.theme().background,
            gpui_kit::component::try_parse_color("#789abc").unwrap()
        );
        assert_eq!(crate::app::settings::snapshot().theme_preference, before);
        super::super::change_mode(requested, cx);
    });
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("theme-discard", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(crate::app::settings::snapshot().theme_preference, requested);
        assert_ne!(cx.theme().mode, mode);
        assert_eq!(editor.read(cx).draft.mode(), cx.theme().mode);
        assert!(!editor.read(cx).has_unsaved_changes());
    });
}

#[test]
fn preferences_menu_opens_each_real_surface_through_application_actions() {
    let mut cx = TestAppContext::single();
    let (handle, app, editor) = open_editor(&mut cx);
    cx.update_window(handle.into(), |_, window, cx| {
        window.click(("close-tab", editor.entity_id()), cx);
    })
    .unwrap();
    cx.run_until_parked();
    for choice in [0usize, 1, 2] {
        cx.update_window(handle.into(), |_, window, cx| {
            window.click("settings-trigger", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.within("popup-menu").click(choice, cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            match choice {
                0 => {
                    assert!(app.read(cx).settings.open);
                    window.click("close-settings", cx);
                }
                1 => {
                    let editor = app.read(cx).tabs.theme_editor().unwrap();
                    assert!(!app.read(cx).settings.open);
                    assert!(window.find("save-theme").visible());
                    window.click(("close-tab", editor.entity_id()), cx);
                }
                _ => {
                    assert!(!app.read(cx).settings.open);
                    assert!(window.find("shortcuts-editor").visible());
                }
            }
        })
        .unwrap();
        cx.run_until_parked();
    }
}
