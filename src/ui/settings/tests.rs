use std::path::Path;

use gpui_kit::component::{ActiveTheme as _, Root};
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    App, AppContext as _, ElementId, Entity, InputEvent as _, KeyUpEvent, Keystroke, Pixels, Point,
    ScrollDelta, ScrollWheelEvent, TestAppContext, Window, WindowHandle, point, px, size,
};

use super::SettingsView;
use crate::app::fonts::{self, FontCatalog};
use crate::document::handler::ViewMode;
use crate::ui::DatalithView;

#[test]
fn settings_scroll_does_not_move_the_note_preview() {
    assert_settings_scroll_is_isolated(ViewMode::View);
}

#[test]
fn settings_scroll_does_not_move_the_note_editor() {
    assert_settings_scroll_is_isolated(ViewMode::Edit);
}

fn scroll_at(window: &mut Window, position: Point<Pixels>, delta: f32, cx: &mut App) {
    window.dispatch_event(
        ScrollWheelEvent {
            position,
            delta: ScrollDelta::Pixels(point(px(0.), px(delta))),
            ..Default::default()
        }
        .to_platform_input(),
        cx,
    );
    window.render_frame(cx);
}

fn open_note(cx: &mut TestAppContext, note: &Path) -> (WindowHandle<Root>, Entity<DatalithView>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        fonts::load_embedded_fonts(cx);
        FontCatalog::init(cx);
        crate::app::themes::load_embedded_themes(cx);
        crate::app::themes::ThemeLibrary::init(cx);
        SettingsView::init_theme_options(cx);
        crate::app::preferences::apply(cx);
        cx.set_global(crate::app::AppState::default());
    });
    let mut app = None;
    let handle = cx.open_window(size(px(1000.), px(800.)), |window, cx| {
        let view = cx.new(|cx| DatalithView::new(false, vec![], window, cx));
        view.update(cx, |view, cx| {
            view.startup_driver = gpui_kit::Task::ready(());
            view.startup = None;
            view.open_file(note.to_path_buf(), true, window, cx);
        });
        app = Some(view.clone());
        Root::new(view, window, cx)
    });
    cx.run_until_parked();
    let app = app.unwrap();
    cx.update(|cx| cx.global_mut::<crate::app::AppState>().view = Some(app.clone()));
    (handle, app)
}

fn offset(origin: Point<Pixels>, x: f32, y: f32) -> Point<Pixels> {
    point(px(f32::from(origin.x) + x), px(f32::from(origin.y) + y))
}

fn assert_settings_scroll_is_isolated(mode: ViewMode) {
    let note = std::env::temp_dir().join(format!(
        "datalith-settings-scroll-{mode:?}-{}.md",
        std::process::id()
    ));
    std::fs::write(&note, "A paragraph in a long note.\n\n".repeat(100)).unwrap();
    let mut cx = TestAppContext::single();
    let (handle, app) = open_note(&mut cx, &note);
    cx.update_window(handle.into(), |_, window, cx| {
        let input = app
            .read(cx)
            .tabs
            .active_handler()
            .unwrap()
            .read(cx)
            .input()
            .unwrap()
            .clone();
        let note_target: ElementId = match mode {
            ViewMode::View => "markdown-title".into(),
            ViewMode::Edit => ("input", input.entity_id()).into(),
        };
        let note_position = |window: &Window, cx: &App| match mode {
            ViewMode::View => window.find("markdown-title").bounds().top(),
            ViewMode::Edit => input.read(cx).scroll_offset().y,
        };
        window.render_frame(cx);
        if mode == ViewMode::View {
            window.click("toggle-mode", cx);
        }
        let initial = note_position(window, cx);
        window.scroll(
            note_target.clone(),
            ScrollDelta::Pixels(point(px(0.), px(-40.))),
            cx,
        );
        let before_settings = note_position(window, cx);
        assert!(
            before_settings < initial,
            "the underlying note must be scrollable"
        );

        app.update(cx, |view, cx| {
            view.settings.open_theme(window, cx);
            cx.notify();
        });
        window.render_frame(cx);
        let close_button = window.find("close-settings").bounds();
        let position = offset(close_button.center(), -120., 250.);
        scroll_at(window, position, -80., cx);
        assert_eq!(
            note_position(window, cx),
            before_settings,
            "scrolling settings must preserve the note's position"
        );

        // Continue scrolling past both ends of the settings content.
        for delta in [-10000., -80., 10000., 80.] {
            scroll_at(window, position, delta, cx);
            assert_eq!(note_position(window, cx), before_settings);
        }

        // The header and the backdrop above the note also isolate wheel input.
        for position in [
            close_button.center(),
            offset(close_button.center(), 60., 250.),
        ] {
            scroll_at(window, position, -80., cx);
            assert_eq!(note_position(window, cx), before_settings);
        }

        window.click("close-settings", cx);
        assert!(!app.read(cx).settings.open);
        assert_eq!(note_position(window, cx), before_settings);
        window.scroll(
            note_target,
            ScrollDelta::Pixels(point(px(0.), px(-40.))),
            cx,
        );
        assert!(
            note_position(window, cx) < before_settings,
            "the note must scroll again after closing settings"
        );
    })
    .unwrap();
    cx.run_until_parked();
    std::fs::remove_file(note).unwrap();
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "Exercises the full native copy dialog from Settings"
)]
fn theme_page_filters_families_and_opens_the_native_copy_dialog() {
    let mut cx = TestAppContext::single();
    let (handle, app) = open_note(&mut cx, Path::new("docs/vault/Welcome.md"));
    let catppuccin = cx.update(|cx| {
        cx.global::<crate::app::themes::ThemeLibrary>()
            .family_named("Catppuccin")
            .unwrap()
            .id()
    });
    cx.update_window(handle.into(), |_, window, cx| {
        app.update(cx, |app, cx| {
            app.settings.open_theme(window, cx);
            app.settings.focus(window, cx);
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.find("import-theme").visible());
        assert!(window.find("theme-appearance").visible());
        window.click("theme-search", cx);
        window.input("Catppuccin", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find(("copy-theme", catppuccin)).visible());
        window.click(("expand-theme", catppuccin), cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("import-theme").visible());
        window.click(("copy-theme", catppuccin), cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("theme-dialog-name").visible());
        assert_eq!(window.find("theme-dialog-name").focused(), Some(true));
        window.press("escape", cx);
    })
    .unwrap();
    cx.run_until_parked();
    std::thread::sleep(std::time::Duration::from_millis(300));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("theme-dialog-name").is_none());
    })
    .unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.click(("copy-theme", catppuccin), cx);
    })
    .unwrap();
    cx.run_until_parked();
    std::thread::sleep(std::time::Duration::from_millis(300));
    cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
        .unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("theme-dialog-name", cx);
        assert_eq!(window.find("theme-dialog-name").focused(), Some(true));
        window.press("secondary-a", cx);
        window.input("Catppuccin", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("theme-dialog-name").value(), Some("Catppuccin"));
    })
    .unwrap();
    cx.update_window(handle.into(), |_, window, cx| window.press("enter", cx))
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(
            cx.global::<crate::app::themes::ThemeLibrary>()
                .family_named("Catppuccin copy")
                .is_none()
        );
    });
    let name = format!("Theme UI copy {:016x}", rand::random::<u64>());
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("theme-dialog-name", cx);
        window.press("secondary-a", cx);
        window.input(&name, cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| window.press("enter", cx))
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let library = cx.global::<crate::app::themes::ThemeLibrary>();
        let copied = library.family_named(&name).unwrap();
        assert_eq!(copied.variants().len(), 4);
        assert!(
            app.read(cx)
                .tabs
                .theme_editor_for(copied.id(), cx)
                .is_some()
        );
        if let crate::app::themes::ThemeSource::Custom(path) = copied.source() {
            std::fs::remove_file(path).unwrap();
        }
    });
}

#[test]
fn theme_page_sets_the_light_slot_from_an_expanded_variant() {
    use crate::app::{settings::ThemeKind, themes::ThemeLibrary};

    let mut cx = TestAppContext::single();
    let (handle, app) = open_note(&mut cx, Path::new("docs/vault/Welcome.md"));
    let (family, variant, old) = cx.update(|cx| {
        let library = cx.global::<ThemeLibrary>();
        let family = library.family_named("Catppuccin").unwrap();
        let variant = family
            .variants()
            .iter()
            .find(|v| v.mode() == ThemeKind::Light.mode())
            .unwrap();
        (
            family.id(),
            variant.id(),
            library.current(ThemeKind::Light).to_owned(),
        )
    });
    cx.update_window(handle.into(), |_, window, cx| {
        app.update(cx, |app, cx| {
            app.settings.open_theme(window, cx);
            cx.notify();
        });
        window.render_frame(cx);
        window.click(("expand-theme", family), cx);
        window.render_frame(cx);
        window.click(("set-current", variant), cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let library = cx.global::<ThemeLibrary>();
        assert_eq!(
            library.current(ThemeKind::Light),
            library.variant_by_id(variant).unwrap().name()
        );
        let previous = library.variant(&old).unwrap().id();
        cx.global_mut::<ThemeLibrary>()
            .set_current(previous, ThemeKind::Light)
            .unwrap();
        crate::app::themes::refresh_current(cx);
    });
}

#[test]
fn system_appearance_resolves_the_current_slot_through_the_production_settings_page() {
    use crate::app::{
        settings::{ThemeKind, ThemePreference},
        themes::ThemeLibrary,
    };

    let mut cx = TestAppContext::single();
    let (handle, app) = open_note(&mut cx, Path::new("docs/vault/Welcome.md"));
    let previous = crate::app::settings::snapshot().theme_preference;
    cx.update_window(handle.into(), |_, window, cx| {
        app.update(cx, |app, cx| {
            app.settings.open_theme(window, cx);
            cx.notify();
        });
        window.render_frame(cx);
        window.click("theme-appearance", cx);
        window.press("home", cx);
        window.press("enter", cx);
        window.render_frame(cx);
        assert_eq!(
            crate::app::settings::snapshot().theme_preference,
            ThemePreference::System
        );
        let effective = ThemePreference::System.resolve(cx.window_appearance());
        let kind = match effective {
            crate::app::settings::ThemeMode::Light => ThemeKind::Light,
            crate::app::settings::ThemeMode::Dark => ThemeKind::Dark,
        };
        let library = cx.global::<ThemeLibrary>();
        let variant = library.variant(library.current(kind)).unwrap();
        assert_eq!(cx.theme().mode, effective.into());
        assert_eq!(
            cx.theme().background,
            library
                .resolved(variant.id(), cx.global::<FontCatalog>())
                .unwrap()
                .theme()
                .background
        );
        crate::app::settings::set_theme_preference(previous).unwrap();
        crate::app::preferences::apply_theme_preference(previous, cx);
    })
    .unwrap();
}

#[test]
fn theme_mode_list_refreshes_when_the_preference_changes_while_closed() {
    use crate::app::settings::ThemePreference;

    let mut cx = TestAppContext::single();
    let (handle, app) = open_note(&mut cx, Path::new("docs/vault/Welcome.md"));
    let previous = crate::app::settings::snapshot().theme_preference;
    cx.update_window(handle.into(), |_, window, cx| {
        app.update(cx, |app, cx| {
            app.settings.open_theme(window, cx);
            cx.notify();
        });
        window.render_frame(cx);
        window.click("close-settings", cx);
        crate::app::settings::set_theme_preference(ThemePreference::Dark).unwrap();
        crate::app::preferences::apply_theme_preference(ThemePreference::Dark, cx);
        app.update(cx, |app, cx| {
            app.settings.open_theme(window, cx);
            cx.notify();
        });
        window.render_frame(cx);
        window.click("theme-appearance", cx);
        window.press("enter", cx);
        assert_eq!(
            crate::app::settings::snapshot().theme_preference,
            ThemePreference::Dark
        );
        crate::app::settings::set_theme_preference(previous).unwrap();
        crate::app::preferences::apply_theme_preference(previous, cx);
    })
    .unwrap();
}

#[test]
fn manage_themes_from_the_keyboard_opens_the_theme_page() {
    let mut cx = TestAppContext::single();
    let (handle, app) = open_note(&mut cx, Path::new("docs/vault/Welcome.md"));
    cx.update_window(handle.into(), |_, window, cx| {
        for key in ["enter", "space"] {
            app.update(cx, |app, cx| {
                app.settings.open();
                app.settings.focus(window, cx);
                cx.notify();
            });
            window.render_frame(cx);
            assert!(window.find("manage-themes").visible());
            for _ in 0..4 {
                window.press("tab", cx);
            }
            assert_eq!(window.find("manage-themes").focused(), Some(true));
            window.press(key, cx);
            window.dispatch_event(
                KeyUpEvent {
                    keystroke: Keystroke::parse(key).unwrap(),
                }
                .to_platform_input(),
                cx,
            );
            window.render_frame(cx);
            let settings = &app.read(cx).settings;
            assert!(settings.theme_open);
            assert!(window.find("import-theme").visible());
            assert!(window.try_find("manage-themes").is_none());
            window.press("tab", cx);
            assert_eq!(window.find("close-settings").focused(), Some(true));
        }
    })
    .unwrap();
}
