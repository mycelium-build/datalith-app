use super::*;
use crate::{
    app::{
        AppState, fonts, preferences,
        themes::{ThemeLibrary, ThemeSource},
    },
    ui::{DatalithView, settings::SettingsView},
};
use gpui_kit::component::{ActiveTheme as _, Root};
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    Focusable as _, InputEvent as _, ScrollDelta, ScrollWheelEvent, TestAppContext, Window,
    WindowHandle, point, px, size,
};

/// Explicit wall-clock probe of the production editor, excluded from correctness runs.
#[test]
#[ignore = "wall-clock performance probe; run alone with --ignored --nocapture"]
#[allow(
    clippy::too_many_lines,
    reason = "Reports separately timed phases of one production editor workflow"
)]
fn theme_editor_performance() {
    use std::time::{Duration, Instant};
    let mut slowest_open = Duration::ZERO;
    let mut slowest_frame = Duration::ZERO;
    let counts = std::env::var("THEME_PERF_VARIANTS")
        .ok()
        .map_or_else(|| vec![1, 4, 16], |count| vec![count.parse().unwrap()]);
    for count in counts {
        let mut cx = TestAppContext::single();
        let (family, first) = cx.update(|cx| {
            gpui_kit::init(cx);
            FontCatalog::init(cx);
            themes::load_embedded_themes(cx);
            ThemeLibrary::init(cx);
            let library = cx.global_mut::<ThemeLibrary>();
            let source = library.family_named("Catppuccin").unwrap().id();
            let family = library
                .copy_family(source, &format!("Performance {count}"))
                .unwrap();
            let ids: Vec<_> = library
                .family(family)
                .unwrap()
                .variants()
                .iter()
                .map(crate::app::themes::ThemeVariant::id)
                .collect();
            let first = ids[0];
            if count == 1 {
                for id in &ids[1..] {
                    library.remove_variant(*id).unwrap();
                }
            } else {
                for ix in 4..count {
                    library
                        .add_variant(family, first, &format!("Variant {ix}"), None)
                        .unwrap();
                }
            }
            (family, first)
        });
        let mut editor = None;
        let start = Instant::now();
        let handle = cx.open_window(size(px(1600.), px(900.)), |window, cx| {
            let view = cx.new(|cx| ThemeEditor::new(family, Some(first), window, cx));
            editor = Some(view.clone());
            Root::new(view, window, cx)
        });
        cx.run_until_parked();
        let open = start.elapsed();
        let editor = editor.unwrap();
        let start = Instant::now();
        cx.update(|cx| {
            for _ in 0..10 {
                std::hint::black_box(
                    cx.global::<ThemeLibrary>()
                        .resolved(first, cx.global::<FontCatalog>())
                        .unwrap(),
                );
            }
        });
        eprintln!("PERF resolve={:?}", start.elapsed() / 10);
        cx.update_window(handle.into(), |_, window, cx| {
            window.click(format!("token-group-{first}-Advanced"), cx);
            window.render_frame(cx);
        })
        .unwrap();
        let start = Instant::now();
        cx.update_window(handle.into(), |_, window, cx| {
            for _ in 0..5 {
                editor.update(cx, |_, cx| cx.notify());
                window.render_frame(cx);
            }
        })
        .unwrap();
        let frame = start.elapsed() / 5;
        let start = Instant::now();
        cx.update_window(handle.into(), |_, window, cx| {
            choose_editor_color(&editor, first, "background", "#123456", window, cx);
        })
        .unwrap();
        cx.run_until_parked();
        let edit = start.elapsed();
        let start = Instant::now();
        cx.update_window(handle.into(), |_, window, cx| {
            let last = cx
                .global::<ThemeLibrary>()
                .family(family)
                .unwrap()
                .variants()
                .last()
                .unwrap()
                .id();
            editor.update(cx, |editor, cx| editor.select(last, window, cx));
            window.render_frame(cx);
        })
        .unwrap();
        cx.run_until_parked();
        eprintln!(
            "PERF variants={count} open={open:?} frame={frame:?} edit={edit:?} select={:?}",
            start.elapsed()
        );
        slowest_open = slowest_open.max(open);
        slowest_frame = slowest_frame.max(frame);
        cleanup(&cx, family);
    }
    assert!(
        slowest_open < Duration::from_secs(1),
        "opening the editor stalls: {slowest_open:?}"
    );
    assert!(
        slowest_frame < Duration::from_millis(100),
        "ordinary frame stalls: {slowest_frame:?}"
    );
}

fn scroll_controls(window: &mut Window, delta: f32, cx: &mut App) {
    let bounds = window.find("theme-controls-scroll").bounds();
    window.dispatch_event(
        ScrollWheelEvent {
            position: bounds.center(),
            delta: ScrollDelta::Pixels(point(px(0.), px(delta))),
            ..Default::default()
        }
        .to_platform_input(),
        cx,
    );
    window.render_frame(cx);
}

fn workspace(
    cx: &mut TestAppContext,
) -> (WindowHandle<Root>, Entity<DatalithView>, u64, u64, String) {
    let (family_id, variant_id, name) = cx.update(|cx| {
        gpui_kit::init(cx);
        fonts::load_embedded_fonts(cx);
        FontCatalog::init(cx);
        themes::load_embedded_themes(cx);
        ThemeLibrary::init(cx);
        SettingsView::init_theme_options(cx);
        preferences::apply(cx);
        cx.set_global(AppState::default());
        let source = cx
            .global::<ThemeLibrary>()
            .family_named("Catppuccin")
            .unwrap()
            .id();
        let name = format!("UI integration {:016x}", rand::random::<u64>());
        let id = cx
            .global_mut::<ThemeLibrary>()
            .copy_family(source, &name)
            .unwrap();
        let variant = cx.global::<ThemeLibrary>().family(id).unwrap().variants()[0].id();
        (id, variant, name)
    });
    let mut view = None;
    let handle = cx.open_window(size(px(1600.), px(900.)), |window, cx| {
        let app = cx.new(|cx| DatalithView::new(false, vec![], window, cx));
        app.update(cx, |app, cx| {
            app.startup_driver = gpui_kit::Task::ready(());
            app.startup = None;
            app.open_theme_editor_for(family_id, Some(variant_id), window, cx);
        });
        view = Some(app.clone());
        window.activate_window();
        Root::new(app, window, cx)
    });
    cx.run_until_parked();
    let view = view.unwrap();
    cx.update(|cx| cx.global_mut::<AppState>().view = Some(view.clone()));
    (handle, view, family_id, variant_id, name)
}

fn choose_editor_color(
    editor: &Entity<ThemeEditor>,
    variant: u64,
    token: &str,
    value: &str,
    window: &mut Window,
    cx: &mut App,
) {
    editor.update(cx, |editor, cx| {
        editor.edit_color(variant, token, true, window, cx);
    });
    let picker = editor.read(cx).variants[&variant].colors[token]
        .controls
        .as_ref()
        .unwrap()
        .picker
        .clone();
    let color = gpui_kit::component::try_parse_color(value).unwrap();
    picker.update(cx, |picker, cx| picker.select_color(color, window, cx));
}

fn choose_color(
    app: &Entity<DatalithView>,
    family: u64,
    variant: u64,
    token: &str,
    value: &str,
    window: &mut Window,
    cx: &mut App,
) {
    let editor = app
        .read(cx)
        .tabs
        .theme_editor_for(family, cx)
        .unwrap()
        .clone();
    choose_editor_color(&editor, variant, token, value, window, cx);
}

fn cleanup(cx: &TestAppContext, id: u64) {
    cx.update(|cx| {
        if let Some(family) = cx.global::<ThemeLibrary>().family(id)
            && let ThemeSource::Custom(path) = family.source()
        {
            let _ = std::fs::remove_file(path);
        }
    });
}

fn visible_color_count(colors: &[String]) -> usize {
    colors
        .iter()
        .filter(|token| {
            token.as_str() != render::VARIANT_MODE_ROW
                && !token.starts_with(render::GROUP_HEADER_PREFIX)
        })
        .count()
}

fn token_for_origin(editor: &ThemeEditor, origin: colors::ColorOrigin) -> Option<String> {
    let preview = editor.preview.as_ref();
    editor
        .variants
        .get(&editor.edited)?
        .colors
        .keys()
        .find_map(|token| {
            (colors::origin(preview.is_some_and(|appearance| appearance.color_is_defined(token)))
                == origin)
                .then(|| token.clone())
        })
}

#[test]
fn editing_a_non_current_variant_updates_only_its_preview_and_persists_after_close() {
    let mut cx = TestAppContext::single();
    let (handle, app, family, variant, name) = workspace(&mut cx);
    let original = cx.update(|cx| cx.theme().background);
    let slots = cx.update(|cx| {
        [
            cx.global::<ThemeLibrary>()
                .current(crate::app::settings::ThemeKind::Light)
                .to_owned(),
            cx.global::<ThemeLibrary>()
                .current(crate::app::settings::ThemeKind::Dark)
                .to_owned(),
        ]
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        choose_color(&app, family, variant, "background", "#123456", window, cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(
            cx.global::<ThemeLibrary>()
                .variant_by_id(variant)
                .unwrap()
                .document()
                .colors()
                .unwrap()["background"]
                .as_deref(),
            Some("#123456")
        );
        assert_eq!(cx.theme().background, original);
        assert_eq!(
            cx.global::<ThemeLibrary>()
                .current(crate::app::settings::ThemeKind::Light),
            slots[0]
        );
        assert_eq!(
            cx.global::<ThemeLibrary>()
                .current(crate::app::settings::ThemeKind::Dark),
            slots[1]
        );
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        // The editor embeds Datalith's production Markdown document renderer.
        assert!(window.find("markdown-title").visible());
    })
    .unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        let editor = app
            .read(cx)
            .tabs
            .theme_editor_for(family, cx)
            .unwrap()
            .clone();
        window.click(("close-tab", editor.entity_id()), cx);
        assert!(app.read(cx).tabs.theme_editor_for(family, cx).is_none());
        app.update(cx, |app, cx| {
            app.open_theme_editor_for(family, Some(variant), window, cx);
        });
    })
    .unwrap();
    cx.update(|cx| {
        assert_eq!(cx.theme().background, original);
        assert_eq!(
            cx.global::<ThemeLibrary>()
                .variant_by_id(variant)
                .unwrap()
                .document()
                .colors()
                .unwrap()["background"]
                .as_deref(),
            Some("#123456")
        );
    });
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(450));
    cx.run_until_parked();
    cx.update(|cx| {
        let path = match cx.global::<ThemeLibrary>().family(family).unwrap().source() {
            ThemeSource::Custom(path) => path.clone(),
            ThemeSource::Bundled => unreachable!(),
        };
        let saved = std::fs::read_to_string(&path)
            .expect("valid edit must autosave even after closing its tab");
        assert!(saved.contains("#123456"));
        ThemeLibrary::init(cx);
        let reloaded = cx.global::<ThemeLibrary>().family_named(&name).unwrap();
        assert_eq!(
            reloaded.variants()[0].document().colors().unwrap()["background"].as_deref(),
            Some("#123456")
        );
        std::fs::remove_file(path).unwrap();
    });
}

#[test]
fn editing_the_current_variant_repaints_the_application_theme() {
    use crate::app::settings::ThemeKind;

    let mut cx = TestAppContext::single();
    let (handle, app, family, variant, _) = workspace(&mut cx);
    let preference = crate::app::settings::snapshot().theme_preference;
    let (kind, old) = cx.update(|cx| {
        let library = cx.global::<ThemeLibrary>();
        let kind = ThemeKind::from(library.variant_by_id(variant).unwrap().mode());
        (kind, library.current(kind).to_owned())
    });
    cx.update(|cx| crate::ui::themes::set_current(variant, kind, cx));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        choose_color(&app, family, variant, "background", "#234567", window, cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let library = cx.global::<ThemeLibrary>();
        assert_eq!(
            library.current(kind),
            library.variant_by_id(variant).unwrap().name()
        );
        assert_eq!(
            cx.theme().background,
            library
                .resolved(variant, cx.global::<FontCatalog>())
                .unwrap()
                .theme()
                .background
        );
        let old_id = library.variant(&old).unwrap().id();
        cx.global_mut::<ThemeLibrary>()
            .set_current(old_id, kind)
            .unwrap();
        themes::refresh_current(cx);
        crate::app::settings::set_theme_preference(preference).unwrap();
        preferences::apply_theme_preference(preference, cx);
    });
    cleanup(&cx, family);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "covers invalid picker input and the resulting cloned variant in one workflow"
)]
fn invalid_picker_hex_keeps_the_last_color_when_a_variant_is_cloned() {
    let mut cx = TestAppContext::single();
    let (handle, app, family, variant, _) = workspace(&mut cx);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        choose_color(&app, family, variant, "background", "#123456", window, cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        let editor = app.read(cx).tabs.theme_editor_for(family, cx).unwrap();
        let picker = editor.read(cx).variants[&variant].colors["background"]
            .controls
            .as_ref()
            .unwrap()
            .picker
            .clone();
        assert!(
            picker
                .update(cx, |picker, cx| picker.commit_hex("invalid", window, cx))
                .is_none()
        );
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let editor = app.read(cx).tabs.theme_editor_for(family, cx).unwrap();
        assert!(
            editor.read(cx).variants[&variant].colors["background"]
                .error
                .is_none()
        );
        assert_eq!(
            cx.global::<ThemeLibrary>()
                .variant_by_id(variant)
                .unwrap()
                .document()
                .colors()
                .unwrap()["background"]
                .as_deref(),
            Some("#123456")
        );
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("add-variant", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let editor = app.read(cx).tabs.theme_editor_for(family, cx).unwrap();
        assert!(
            editor.read(cx).variants[&variant].colors["background"]
                .error
                .is_none()
        );
        assert_eq!(
            color_hex(
                editor.read(cx).variants[&variant].colors["background"]
                    .controls
                    .as_ref()
                    .unwrap()
                    .picker
                    .read(cx)
                    .value()
                    .unwrap()
            ),
            "#123456".to_owned()
        );
        let editor = editor.read(cx);
        assert_ne!(editor.edited, variant);
        assert_eq!(
            editor.selector.read(cx).selected_value().unwrap().as_str(),
            editor.edited.to_string()
        );
        let appearance = editor.preview.as_ref().unwrap();
        assert_eq!(
            appearance.color("background"),
            gpui_kit::component::try_parse_color("#123456").ok()
        );
        assert_eq!(
            cx.global::<ThemeLibrary>()
                .variant_by_id(editor.edited)
                .unwrap()
                .document()
                .colors()
                .unwrap()["background"]
                .as_deref(),
            Some("#123456")
        );
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let edited = app
            .read(cx)
            .tabs
            .theme_editor_for(family, cx)
            .unwrap()
            .read(cx)
            .edited;
        assert!(
            window.find(("variant-light", edited)).visible(),
            "the clone's controls must be revealed"
        );
    })
    .unwrap();
    cleanup(&cx, family);
}

#[test]
fn changing_mode_refreshes_inherited_inputs_and_pickers_but_keeps_overrides() {
    let mut cx = TestAppContext::single();
    let (handle, app, family, variant, _) = workspace(&mut cx);
    let editor = cx.update(|cx| {
        app.read(cx)
            .tabs
            .theme_editor_for(family, cx)
            .unwrap()
            .clone()
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click(format!("color-value-{variant}-background"), cx);
        window.press("escape", cx);
        window.click(format!("reset-color-{variant}-background"), cx);
        choose_color(&app, family, variant, "border", "#123456", window, cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.click(("variant-dark", variant), cx);
        window.render_frame(cx);
        let expected = cx
            .global::<ThemeLibrary>()
            .resolved(variant, cx.global::<FontCatalog>())
            .unwrap()
            .color("background")
            .unwrap()
            .to_hex();
        let controls = &editor.read(cx).variants[&variant];
        let inherited = &controls.colors["background"];
        assert!(inherited.valid.is_none());
        assert_eq!(
            inherited
                .controls
                .as_ref()
                .unwrap()
                .input
                .read(cx)
                .value()
                .as_str(),
            expected
        );
        assert_eq!(
            inherited
                .controls
                .as_ref()
                .unwrap()
                .picker
                .read(cx)
                .value()
                .unwrap()
                .to_hex(),
            expected
        );
        assert_eq!(
            controls.colors["border"]
                .controls
                .as_ref()
                .unwrap()
                .input
                .read(cx)
                .value()
                .as_str(),
            "#123456"
        );
        assert_eq!(
            controls.colors["border"]
                .controls
                .as_ref()
                .unwrap()
                .picker
                .read(cx)
                .value(),
            gpui_kit::component::try_parse_color("#123456").ok()
        );
    })
    .unwrap();
    cleanup(&cx, family);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "Checks picker, reset and disk state through one complete edit"
)]
fn picker_edit_and_reset_update_model_input_and_picker_across_save() {
    let mut cx = TestAppContext::single();
    let (handle, app, family, variant, _) = workspace(&mut cx);
    let editor = cx.update(|cx| {
        app.read(cx)
            .tabs
            .theme_editor_for(family, cx)
            .unwrap()
            .clone()
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click(format!("color-value-{variant}-background"), cx);
        let picker = editor.read(cx).variants[&variant].colors["background"]
            .controls
            .as_ref()
            .unwrap()
            .picker
            .clone();
        window.render_frame(cx);
        assert!(picker.read(cx).is_open());
        let chosen = gpui_kit::component::try_parse_color("#123456").unwrap();
        picker.update(cx, |picker, cx| picker.select_color(chosen, window, cx));
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let chosen = color_hex(gpui_kit::component::try_parse_color("#123456").unwrap());
        assert_eq!(
            editor.read(cx).variants[&variant].colors["background"]
                .controls
                .as_ref()
                .unwrap()
                .input
                .read(cx)
                .value()
                .as_str(),
            chosen
        );
        assert_eq!(
            cx.global::<ThemeLibrary>()
                .variant_by_id(variant)
                .unwrap()
                .document()
                .colors()
                .unwrap()["background"]
                .as_deref(),
            Some(chosen.as_str())
        );
        window.click(format!("color-value-{variant}-background"), cx);
        window.press("escape", cx);
        window.render_frame(cx);
        window.click(format!("reset-color-{variant}-background"), cx);
        window.render_frame(cx);
        let expected = cx
            .global::<ThemeLibrary>()
            .resolved(variant, cx.global::<FontCatalog>())
            .unwrap()
            .color("background")
            .unwrap()
            .to_hex();
        let row = &editor.read(cx).variants[&variant].colors["background"];
        assert_eq!(
            row.controls
                .as_ref()
                .unwrap()
                .input
                .read(cx)
                .value()
                .as_str(),
            expected
        );
        assert_eq!(
            row.controls
                .as_ref()
                .unwrap()
                .picker
                .read(cx)
                .value()
                .unwrap()
                .to_hex(),
            expected
        );
        assert!(
            cx.global::<ThemeLibrary>()
                .variant_by_id(variant)
                .unwrap()
                .document()
                .colors()
                .unwrap()["background"]
                .is_none()
        );
    })
    .unwrap();
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(450));
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(matches!(
            cx.global::<ThemeLibrary>().family(family).unwrap().status(),
            crate::app::themes::SaveStatus::Autosaved
        ));
        let path = match cx.global::<ThemeLibrary>().family(family).unwrap().source() {
            ThemeSource::Custom(path) => path.clone(),
            ThemeSource::Bundled => unreachable!(),
        };
        let name = cx
            .global::<ThemeLibrary>()
            .family(family)
            .unwrap()
            .name()
            .to_owned();
        ThemeLibrary::init(cx);
        assert!(
            cx.global::<ThemeLibrary>()
                .family_named(&name)
                .unwrap()
                .variants()[0]
                .document()
                .colors()
                .unwrap()["background"]
                .is_none()
        );
        std::fs::remove_file(path).unwrap();
    });
}

#[test]
fn failed_autosave_exposes_retry_and_retries_after_storage_recovers() {
    let mut cx = TestAppContext::single();
    let (handle, app, family, variant, _) = workspace(&mut cx);
    let path = cx.update(
        |cx| match cx.global::<ThemeLibrary>().family(family).unwrap().source() {
            ThemeSource::Custom(path) => path.clone(),
            ThemeSource::Bundled => unreachable!(),
        },
    );
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        choose_color(&app, family, variant, "background", "#123456", window, cx);
    })
    .unwrap();
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(450));
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(matches!(
            cx.global::<ThemeLibrary>().family(family).unwrap().status(),
            crate::app::themes::SaveStatus::Failed(_)
        ));
        assert!(window.find("retry-theme-save").visible());
        std::fs::remove_dir(&path).unwrap();
        window.click("retry-theme-save", cx);
        window.render_frame(cx);
        assert!(window.try_find("retry-theme-save").is_none());
        assert!(matches!(
            cx.global::<ThemeLibrary>().family(family).unwrap().status(),
            crate::app::themes::SaveStatus::Autosaved
        ));
        assert!(std::fs::read_to_string(&path).unwrap().contains("#123456"));
    })
    .unwrap();
    cleanup(&cx, family);
}

#[test]
fn changing_each_font_role_through_native_select_keeps_app_fonts_and_slots_unchanged() {
    let mut cx = TestAppContext::single();
    let (handle, app, family, variant, _) = workspace(&mut cx);
    let before = cx.update(|cx| cx.theme().font_family.clone());
    let slots = cx.update(|cx| {
        [
            cx.global::<ThemeLibrary>()
                .current(crate::app::settings::ThemeKind::Light)
                .to_owned(),
            cx.global::<ThemeLibrary>()
                .current(crate::app::settings::ThemeKind::Dark)
                .to_owned(),
        ]
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click(format!("token-group-{variant}-Fonts"), cx);
    })
    .unwrap();
    for role in FontRole::ALL {
        let picker = cx.update(|cx| {
            app.read(cx)
                .tabs
                .theme_editor_for(family, cx)
                .unwrap()
                .read(cx)
                .variants[&variant]
                .fonts[role.index()]
            .clone()
        });
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.focus(&picker.focus_handle(cx), cx);
            window.press("enter", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| window.input("Arial", cx))
            .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.press("down", cx);
            window.press("enter", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            assert_eq!(
                cx.global::<ThemeLibrary>()
                    .variant_by_id(variant)
                    .unwrap()
                    .document()
                    .font(role),
                Some("Arial")
            );
            assert_eq!(cx.theme().font_family, before);
            assert_eq!(
                cx.global::<ThemeLibrary>()
                    .current(crate::app::settings::ThemeKind::Light),
                slots[0]
            );
            assert_eq!(
                cx.global::<ThemeLibrary>()
                    .current(crate::app::settings::ThemeKind::Dark),
                slots[1]
            );
        });
    }
    cx.update_window(handle.into(), |_, window, cx| {
        scroll_controls(window, 10000., cx);
        window.click(("apply-family-fonts", variant), cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let library = cx.global::<ThemeLibrary>();
        for sibling in library.family(family).unwrap().variants() {
            for role in FontRole::ALL {
                assert_eq!(sibling.document().font(role), Some("Arial"));
            }
        }
        assert_eq!(cx.theme().font_family, before);
    });
    cleanup(&cx, family);
}

#[test]
fn switching_variants_updates_visible_color_controls_and_preview() {
    let mut cx = TestAppContext::single();
    let (handle, app, family, first, _) = workspace(&mut cx);
    let second = cx.update(|cx| {
        cx.global::<ThemeLibrary>()
            .family(family)
            .unwrap()
            .variants()
            .iter()
            .find(|variant| variant.id() != first)
            .unwrap()
            .id()
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        scroll_controls(window, -10000., cx);
        window.click(("select-variant", second), cx);
        window.render_frame(cx);
        assert_eq!(
            app.read(cx)
                .tabs
                .theme_editor_for(family, cx)
                .unwrap()
                .read(cx)
                .edited,
            second
        );
        window.click(("select-variant", first), cx);
        choose_color(&app, family, first, "background", "#225588", window, cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let editor = app.read(cx).tabs.theme_editor_for(family, cx).unwrap();
        assert_eq!(editor.read(cx).edited, first);
        assert_eq!(
            editor
                .read(cx)
                .selector
                .read(cx)
                .selected_value()
                .map(AsRef::as_ref),
            Some(first.to_string().as_str())
        );
    });
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(
            cx.global::<ThemeLibrary>()
                .variant_by_id(first)
                .unwrap()
                .document()
                .colors()
                .unwrap()["background"]
                .as_deref(),
            Some("#225588")
        );
    });
    cleanup(&cx, family);
}

#[test]
fn switching_variants_updates_visible_font_controls_and_preview() {
    let mut cx = TestAppContext::single();
    let (handle, app, family, first, _) = workspace(&mut cx);
    let second = cx.update(|cx| {
        cx.global::<ThemeLibrary>()
            .family(family)
            .unwrap()
            .variants()
            .iter()
            .find(|v| v.id() != first)
            .unwrap()
            .id()
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click(("select-variant", second), cx);
    })
    .unwrap();
    cx.run_until_parked();
    let picker = cx.update(|cx| {
        app.read(cx)
            .tabs
            .theme_editor_for(family, cx)
            .unwrap()
            .read(cx)
            .variants[&second]
            .fonts[FontRole::Interface.index()]
        .clone()
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        scroll_controls(window, -10000., cx);
        window.click(("select-variant", second), cx);
        window.click(format!("token-group-{second}-Fonts"), cx);
        assert!(window.find(("select", picker.entity_id())).visible());
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.focus(&picker.focus_handle(cx), cx);
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(
            app.read(cx)
                .tabs
                .theme_editor_for(family, cx)
                .unwrap()
                .read(cx)
                .edited,
            second
        );
    });
    cleanup(&cx, family);
}

#[test]
fn switching_variants_keeps_mode_controls_keyboard_accessible() {
    let mut cx = TestAppContext::single();
    let (handle, app, family, first, _) = workspace(&mut cx);
    let second = cx.update(|cx| {
        cx.global::<ThemeLibrary>()
            .family(family)
            .unwrap()
            .variants()
            .iter()
            .find(|v| v.id() != first)
            .unwrap()
            .id()
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        scroll_controls(window, -10000., cx);
        window.click(("select-variant", second), cx);
        window.render_frame(cx);
        window.click(("select-variant", first), cx);
        window.click(format!("token-group-{first}-Fonts"), cx);
        window.render_frame(cx);
        assert!(window.try_find(("variant-light", first)).is_none());
        assert!(window.try_find(("variant-dark", first)).is_none());
        window.click(format!("token-group-{first}-Colors"), cx);
        window.render_frame(cx);
        assert!(
            window
                .find(format!("theme-token-{first}-{}", render::VARIANT_MODE_ROW))
                .visible()
        );
        for _ in 0..40 {
            window.press("tab", cx);
            if window.find(("variant-light", first)).focused() == Some(true) {
                break;
            }
        }
        assert_eq!(window.find(("variant-light", first)).focused(), Some(true));
        window.press("tab", cx);
        assert_eq!(window.find(("variant-dark", first)).focused(), Some(true));
        assert_eq!(
            app.read(cx)
                .tabs
                .theme_editor_for(family, cx)
                .unwrap()
                .read(cx)
                .edited,
            first
        );
    })
    .unwrap();
    cleanup(&cx, family);
}

#[test]
fn removing_a_variant_and_using_notification_undo_restores_its_editor_controls() {
    let mut cx = TestAppContext::single();
    let (handle, app, family, variant, _) = workspace(&mut cx);
    let before = cx.update(|cx| {
        cx.global::<ThemeLibrary>()
            .family(family)
            .unwrap()
            .variants()
            .len()
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.hover(("select-variant", variant), cx);
        window.click(("remove-variant", variant), cx);
        window.render_frame(cx);
        assert_eq!(
            cx.global::<ThemeLibrary>()
                .family(family)
                .unwrap()
                .variants()
                .len(),
            before - 1
        );
        let editor = app.read(cx).tabs.theme_editor_for(family, cx).unwrap();
        assert!(!editor.read(cx).variants.contains_key(&variant));
    })
    .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("undo-theme-delete", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(
            cx.global::<ThemeLibrary>()
                .family(family)
                .unwrap()
                .variants()
                .len(),
            before
        );
        let editor = app.read(cx).tabs.theme_editor_for(family, cx).unwrap();
        assert!(cx.global::<ThemeLibrary>().variant_by_id(variant).is_some());
        assert_ne!(editor.read(cx).edited, variant);
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click(("select-variant", variant), cx);
        assert!(
            window
                .find(format!("color-value-{variant}-background"))
                .visible()
        );
    })
    .unwrap();
    cleanup(&cx, family);
}

#[test]
fn syntax_color_edits_use_the_variant_highlight_override() {
    let mut cx = TestAppContext::single();
    let (handle, app, family, variant, _) = workspace(&mut cx);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click(format!("token-group-{variant}-Advanced"), cx);
        assert!(
            window.find("theme-color-search").visible(),
            "Search bounds {:?}",
            window.find("theme-color-search").bounds()
        );
        window.click("theme-color-search", cx);
        window.input("highlight:syntax.keyword", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        choose_color(
            &app,
            family,
            variant,
            "highlight:syntax.keyword",
            "#345678",
            window,
            cx,
        );
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let document = cx
            .global::<ThemeLibrary>()
            .variant_by_id(variant)
            .unwrap()
            .document();
        let highlight = serde_json::to_value(&document.config().highlight).unwrap();
        assert_eq!(
            highlight["syntax"]["keyword"]["color"].as_str(),
            Some("#345678ff")
        );
    });
    cleanup(&cx, family);
}

#[test]
fn narrow_editor_switches_between_independently_scrolling_controls_and_preview() {
    let mut cx = TestAppContext::single();
    let (handle, _, family, variant, _) = workspace(&mut cx);
    cx.simulate_window_resize(handle.into(), size(px(680.), px(720.)));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let editor = window.find("theme-editor").bounds();
        let input = window
            .find(format!("color-value-{variant}-background"))
            .bounds();
        assert!(input.size.width > px(0.));
        assert!(input.left() >= editor.left());
        assert!(input.right() <= editor.right());
        assert!(window.try_find("theme-preview-pane").is_none());
        window.click("toggle-theme-preview", cx);
        window.render_frame(cx);
        let preview = window.find("theme-preview-pane");
        assert!(preview.visible());
        assert!(preview.bounds().size.width <= editor.size.width);
        assert!(
            window
                .try_find(format!("color-value-{variant}-background"))
                .is_none()
        );
        window.click("toggle-theme-preview", cx);
        window.render_frame(cx);
        assert!(
            window
                .find(format!("color-value-{variant}-background"))
                .visible()
        );
    })
    .unwrap();
    cleanup(&cx, family);
}

#[test]
fn importing_a_theme_with_a_custom_name_conflict_creates_a_copy() {
    let mut cx = TestAppContext::single();
    let (handle, _, family, _, name) = workspace(&mut cx);
    let path = cx.update(
        |cx| match cx.global::<ThemeLibrary>().family(family).unwrap().source() {
            ThemeSource::Custom(path) => path.clone(),
            ThemeSource::Bundled => unreachable!(),
        },
    );
    cx.update(|cx| crate::ui::settings::theme::dialogs::import_family(path, cx));
    cx.run_until_parked();
    std::thread::sleep(std::time::Duration::from_millis(300));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("copy-theme-import").visible());
        assert!(window.find("replace-theme-import").visible());
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let library = cx.global::<ThemeLibrary>();
        let copied = library.family_named(&format!("{name} copy")).unwrap();
        assert_eq!(
            copied.variants().len(),
            library.family(family).unwrap().variants().len()
        );
        if let ThemeSource::Custom(path) = copied.source() {
            std::fs::remove_file(path).unwrap();
        }
    });
    cleanup(&cx, family);
}

#[test]
fn replacing_a_custom_theme_refreshes_its_open_editor() {
    let mut cx = TestAppContext::single();
    let (handle, app, family, variant, _) = workspace(&mut cx);
    let source =
        cx.update(
            |cx| match cx.global::<ThemeLibrary>().family(family).unwrap().source() {
                ThemeSource::Custom(path) => path.clone(),
                ThemeSource::Bundled => unreachable!(),
            },
        );
    let mut data: serde_json::Value =
        serde_json::from_slice(&std::fs::read(source).unwrap()).unwrap();
    data["themes"][0]["colors"]["background"] = serde_json::json!("#123456");
    let import = std::env::temp_dir().join(format!(
        "datalith-replace-{:016x}.json",
        rand::random::<u64>()
    ));
    std::fs::write(&import, serde_json::to_vec(&data).unwrap()).unwrap();
    cx.update(|cx| crate::ui::settings::theme::dialogs::import_family(import.clone(), cx));
    cx.run_until_parked();
    std::thread::sleep(std::time::Duration::from_millis(300));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("replace-theme-import", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(
            cx.global::<ThemeLibrary>()
                .variant_by_id(variant)
                .unwrap()
                .document()
                .colors()
                .unwrap()["background"]
                .as_deref(),
            Some("#123456")
        );
        let editor = app.read(cx).tabs.theme_editor_for(family, cx).unwrap();
        assert_eq!(
            editor.read(cx).variants[&variant].colors["background"]
                .display
                .as_str(),
            "#123456"
        );
    });
    std::fs::remove_file(import).unwrap();
    cleanup(&cx, family);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "Exercises cancel, validation and commit in the same native dialog"
)]
fn first_add_names_both_variants_and_cancel_or_duplicate_does_not_mutate() {
    let mut cx = TestAppContext::single();
    let (handle, app, initial_family, _, _) = workspace(&mut cx);
    let (family, first) = cx.update(|cx| {
        let source = cx
            .global::<ThemeLibrary>()
            .family_named("Asciinema")
            .unwrap()
            .id();
        let id = cx
            .global_mut::<ThemeLibrary>()
            .copy_family(source, &format!("Mono {:016x}", rand::random::<u64>()))
            .unwrap();
        (
            id,
            cx.global::<ThemeLibrary>().family(id).unwrap().variants()[0].id(),
        )
    });
    cx.update_window(handle.into(), |_, window, cx| {
        app.update(cx, |app, cx| {
            app.open_theme_editor_for(family, Some(first), window, cx);
        });
        window.render_frame(cx);
        window.click("add-variant", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("new-variant-suffix").visible());
        window.press("escape", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(
            cx.global::<ThemeLibrary>()
                .family(family)
                .unwrap()
                .variants()
                .len(),
            1
        );
    });

    cx.update_window(handle.into(), |_, window, cx| {
        window.click("add-variant", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
    })
    .unwrap();
    // Dialog 0.6.1 uses a 250 ms wall-clock entrance animation. Wait before
    // hit-testing its fields; faster editor frames must not type into the first field.
    std::thread::sleep(std::time::Duration::from_millis(260));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("new-variant-suffix", cx);
        assert_eq!(window.find("new-variant-suffix").focused(), Some(true));
        window.press("secondary-a", cx);
        window.input("Variant 1", cx);
    })
    .unwrap();
    cx.run_until_parked();
    std::thread::sleep(std::time::Duration::from_millis(300));
    cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
        .unwrap();
    cx.update_window(handle.into(), |_, window, cx| window.click("ok", cx))
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(
            cx.global::<ThemeLibrary>()
                .family(family)
                .unwrap()
                .variants()
                .len(),
            1
        );
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("new-variant-suffix", cx);
        window.press("secondary-a", cx);
        window.input("Variant 2", cx);
    })
    .unwrap();
    cx.run_until_parked();
    std::thread::sleep(std::time::Duration::from_millis(300));
    cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
        .unwrap();
    cx.update_window(handle.into(), |_, window, cx| window.click("ok", cx))
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let variants = cx
            .global::<ThemeLibrary>()
            .family(family)
            .unwrap()
            .variants();
        assert_eq!(variants.len(), 2);
        assert!(variants.iter().any(|variant| variant.id() == first));
        assert_ne!(
            app.read(cx)
                .tabs
                .theme_editor_for(family, cx)
                .unwrap()
                .read(cx)
                .edited,
            first
        );
    });
    cleanup(&cx, family);
    cleanup(&cx, initial_family);
}

#[test]
fn variant_sidebar_uses_short_names_and_rename_updates_the_selected_variant() {
    let mut cx = TestAppContext::single();
    let (handle, app, family, variant, _) = workspace(&mut cx);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window
                .find(format!("color-value-{variant}-background"))
                .bounds()
                .right(),
            window
                .find(format!("color-value-{variant}-foreground"))
                .bounds()
                .right()
        );
        assert_eq!(
            window.find(("select-variant", variant)).label(),
            Some("Latte")
        );
        let variant_row = window.find(format!("theme-variant-row-{variant}")).bounds();
        let variant_button = window.find(("select-variant", variant)).bounds();
        assert_eq!(variant_button.left(), variant_row.left());
        assert_eq!(variant_button.right(), variant_row.right());
        let add = window.find("add-variant").bounds();
        let last = cx
            .global::<ThemeLibrary>()
            .family(family)
            .unwrap()
            .variants()
            .last()
            .unwrap()
            .id();
        assert!(add.top() >= window.find(("select-variant", last)).bounds().bottom());
        assert!(!window.find(("rename-variant", variant)).visible());
        assert!(!window.find(("remove-variant", variant)).visible());
        window.hover(("select-variant", variant), cx);
        assert!(window.find(("rename-variant", variant)).visible());
        assert!(window.find(("remove-variant", variant)).visible());
        window.click(("rename-variant", variant), cx);
        window.render_frame(cx);
        assert_eq!(
            window
                .find(format!("variant-name-input-{variant}"))
                .focused(),
            Some(true)
        );
        window.press("secondary-a", cx);
        window.input("Morning", cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find(("select-variant", variant)).label(),
            Some("Morning")
        );
        assert_eq!(
            cx.global::<ThemeLibrary>()
                .family(family)
                .unwrap()
                .suffix(variant),
            Some("Morning")
        );
        let edited = app
            .read(cx)
            .tabs
            .theme_editor_for(family, cx)
            .unwrap()
            .read(cx)
            .edited;
        assert_eq!(edited, variant);
    })
    .unwrap();
    cleanup(&cx, family);
}

#[test]
fn renaming_the_theme_in_its_editor_preserves_variants_and_updates_its_tab() {
    let mut cx = TestAppContext::single();
    let (handle, app, family, variant, original) = workspace(&mut cx);
    let renamed = format!("{original} renamed");
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("rename-theme", cx);
        window.render_frame(cx);
        assert_eq!(window.find("theme-name-input").focused(), Some(true));
        window.press("secondary-a", cx);
        window.input(&renamed, cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let library = cx.global::<ThemeLibrary>();
        let family_state = library.family(family).unwrap();
        assert_eq!(family_state.name(), renamed);
        assert_eq!(family_state.variants().len(), 4);
        assert_eq!(family_state.suffix(variant), Some("Latte"));
        assert_eq!(
            app.read(cx)
                .tabs
                .theme_editor_for(family, cx)
                .unwrap()
                .read(cx)
                .family_name(cx),
            renamed
        );
    });
    cleanup(&cx, family);
}

#[test]
fn deletion_notifications_expire_after_five_seconds_for_variants_and_themes() {
    let mut cx = TestAppContext::single();
    let (handle, _, family, variant, _) = workspace(&mut cx);
    for whole_theme in [false, true] {
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            if whole_theme {
                let deleted = cx
                    .global_mut::<ThemeLibrary>()
                    .delete_family(family)
                    .unwrap();
                crate::ui::settings::theme::show_undo(family, deleted, window, cx);
            } else {
                window.hover(("select-variant", variant), cx);
                window.click(("remove-variant", variant), cx);
            }
        })
        .unwrap();
        cx.run_until_parked();
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(300));
        cx.run_until_parked();
        cx.executor()
            .advance_clock(std::time::Duration::from_secs(4));
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            assert_eq!(
                Root::read(window, cx)
                    .notification
                    .read(cx)
                    .notifications()
                    .len(),
                1
            );
        })
        .unwrap();
        cx.executor()
            .advance_clock(std::time::Duration::from_secs(2));
        cx.run_until_parked();
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(300));
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            assert!(
                Root::read(window, cx)
                    .notification
                    .read(cx)
                    .notifications()
                    .is_empty()
            );
        })
        .unwrap();
    }
}

#[test]
fn color_swatch_and_hex_input_open_picker_directly_and_blur_restores_the_row() {
    let mut cx = TestAppContext::single();
    let (handle, app, family, variant, _) = workspace(&mut cx);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click(format!("color-value-{variant}-background"), cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let editor = app
            .read(cx)
            .tabs
            .theme_editor_for(family, cx)
            .unwrap()
            .read(cx);
        assert!(
            editor.variants[&variant].colors["background"]
                .controls
                .as_ref()
                .unwrap()
                .picker
                .read(cx)
                .is_open()
        );
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.press("escape", cx);
        window.render_frame(cx);
        window.click(format!("color-value-{variant}-foreground"), cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let editor = app
            .read(cx)
            .tabs
            .theme_editor_for(family, cx)
            .unwrap()
            .read(cx);
        assert!(
            editor.variants[&variant].colors["foreground"]
                .controls
                .as_ref()
                .unwrap()
                .picker
                .read(cx)
                .is_open()
        );
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.press("escape", cx);
        window.render_frame(cx);
        window.click(format!("token-group-{variant}-Fonts"), cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let editor = app
            .read(cx)
            .tabs
            .theme_editor_for(family, cx)
            .unwrap()
            .read(cx);
        assert!(editor.active_color.is_none());
        assert!(
            !editor.variants[&variant].colors["foreground"]
                .controls
                .as_ref()
                .unwrap()
                .picker
                .read(cx)
                .is_open()
        );
    });
    cleanup(&cx, family);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "Checks schema completeness, search, editing and reset in one UI workflow"
)]
fn advanced_includes_unset_syntax_and_status_colors_and_clears_filters() {
    let mut cx = TestAppContext::single();
    let (handle, app, family, variant, _) = workspace(&mut cx);
    let editor = cx.update(|cx| {
        app.read(cx)
            .tabs
            .theme_editor_for(family, cx)
            .unwrap()
            .clone()
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click(format!("token-group-{variant}-Advanced"), cx);
        assert_eq!(
            visible_color_count(&editor.read(cx).visible_colors),
            editor.read(cx).variants[&variant].colors.len()
        );
        let headers = editor
            .read(cx)
            .visible_colors
            .iter()
            .filter(|token| token.starts_with(render::GROUP_HEADER_PREFIX))
            .count();
        assert!(headers > 2, "Advanced colors should show category headings");
        assert!(
            editor
                .read(cx)
                .visible_colors
                .iter()
                .any(|key| key == "highlight:editor.gutter.background")
        );
        assert!(
            editor
                .read(cx)
                .visible_colors
                .iter()
                .any(|key| key == "highlight:error.border")
        );
        assert!(
            window.find("theme-color-search").visible(),
            "Search bounds {:?}",
            window.find("theme-color-search").bounds()
        );
        window.click("theme-color-search", cx);
        assert_eq!(
            window.find("theme-color-search").focused(),
            Some(true),
            "Search bounds: {:?}",
            window.find("theme-color-search").bounds()
        );
        window.input("highlight:syntax.tag.doctype", cx);
        assert_eq!(
            editor.read(cx).color_query.read(cx).value().as_str(),
            "highlight:syntax.tag.doctype"
        );
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            editor.read(cx).visible_colors,
            vec![
                render::GROUP_HEADER_PREFIX.to_owned() + "Syntax",
                "highlight:syntax.tag.doctype".to_owned()
            ]
        );
        choose_color(
            &app,
            family,
            variant,
            "highlight:syntax.tag.doctype",
            "#123456",
            window,
            cx,
        );
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        let document = cx
            .global::<ThemeLibrary>()
            .variant_by_id(variant)
            .unwrap()
            .document();
        let highlight = serde_json::to_value(&document.config().highlight).unwrap();
        assert_eq!(highlight["syntax"]["tag.doctype"]["color"], "#123456ff");
        window.click(
            format!("reset-color-{variant}-highlight:syntax.tag.doctype"),
            cx,
        );
        window.click("clear-color-filters", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let editor = editor.read(cx);
        assert_eq!(
            visible_color_count(&editor.visible_colors),
            editor.variants[&variant].colors.len()
        );
        assert!(
            editor.variants[&variant].colors["highlight:syntax.tag.doctype"]
                .valid
                .is_none()
        );
    });
    cleanup(&cx, family);
}

#[test]
fn colors_category_keeps_bases_and_common_overrides_without_search() {
    let mut cx = TestAppContext::single();
    let (handle, app, family, _, _) = workspace(&mut cx);
    let editor = cx.update(|cx| {
        app.read(cx)
            .tabs
            .theme_editor_for(family, cx)
            .unwrap()
            .clone()
    });

    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let visible = &editor.read(cx).visible_colors;
        assert_eq!(visible_color_count(visible), 25);
        assert!(visible.iter().any(|token| token == "list.background"));
        for (token, _, _) in render::ESSENTIAL_COLORS {
            assert!(visible.iter().any(|visible| visible == token), "{token}");
        }
        assert!(
            [
                "button.background",
                "caret",
                "list.even.background",
                "table.background",
                "highlight:syntax.keyword",
            ]
            .iter()
            .all(|token| !visible.iter().any(|visible| visible == token)),
            "less common component details remain in Advanced"
        );
        let base_tokens: Vec<_> = visible
            .iter()
            .filter(|token| {
                token.as_str() != render::VARIANT_MODE_ROW
                    && !token.starts_with(render::GROUP_HEADER_PREFIX)
            })
            .take(6)
            .map(String::as_str)
            .collect();
        assert_eq!(
            base_tokens,
            [
                "background",
                "foreground",
                "muted.background",
                "border",
                "primary.background",
                "secondary.background",
            ]
        );
        assert!(window.try_find("theme-color-search").is_none());
        assert!(window.try_find("clear-color-filters").is_none());
        assert_eq!(
            visible.last().map(String::as_str),
            Some("list.head.background")
        );
        editor.update(cx, |editor, cx| {
            editor.reset_color(editor.edited, "list.active.background", window, cx);
        });
        assert_eq!(visible_color_count(&editor.read(cx).visible_colors), 25);
        assert!(
            editor
                .read(cx)
                .visible_colors
                .iter()
                .any(|token| token == "list.active.background")
        );
    })
    .unwrap();
    cleanup(&cx, family);
}

#[test]
fn colors_common_overrides_are_reachable_by_scrolling() {
    let mut cx = TestAppContext::single();
    let (handle, _, family, variant, _) = workspace(&mut cx);
    let expected = [
        "list.active.background",
        "list.active.border",
        "list.hover.background",
        "list.background",
        "ring",
        "list.head.background",
    ];

    for window_size in [size(px(1600.), px(900.)), size(px(680.), px(720.))] {
        cx.simulate_window_resize(handle.into(), window_size);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            scroll_controls(window, 10000., cx);
            let mut seen = std::collections::HashSet::new();
            for _ in 0..32 {
                let viewport = window.find("theme-controls-scroll").bounds();
                for token in expected {
                    if window
                        .try_find(format!("theme-token-{variant}-{token}"))
                        .is_some_and(|row| {
                            row.visible()
                                && row.bounds().top() >= viewport.top()
                                && row.bounds().bottom() <= viewport.bottom()
                        })
                    {
                        assert!(
                            window
                                .find(format!("theme-token-name-{variant}-{token}"))
                                .visible(),
                            "Colors must show the exact key {token}"
                        );
                        seen.insert(token);
                    }
                }
                if seen.len() == expected.len() {
                    break;
                }
                window.scroll(
                    "theme-controls-scroll",
                    ScrollDelta::Pixels(point(px(0.), -viewport.size.height * 0.5)),
                    cx,
                );
            }
            let missing: Vec<_> = expected
                .into_iter()
                .filter(|token| !seen.contains(token))
                .collect();
            assert!(missing.is_empty(), "Colors never displayed {missing:?}");
        })
        .unwrap();
    }
    cleanup(&cx, family);
}

#[test]
fn switching_color_categories_resets_search_and_group_filter() {
    let mut cx = TestAppContext::single();
    let (handle, app, family, variant, _) = workspace(&mut cx);
    let editor = cx.update(|cx| {
        app.read(cx)
            .tabs
            .theme_editor_for(family, cx)
            .unwrap()
            .clone()
    });

    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("theme-color-search").is_none());

        editor.update(cx, |editor, cx| {
            editor.color_group.update(cx, |group, cx| {
                group.set_selected_value(&"Base colors".into(), window, cx);
            });
            editor
                .color_query
                .update(cx, |query, cx| query.set_value("sidebar", window, cx));
            editor.refresh_color_list(cx);
        });
        assert_eq!(visible_color_count(&editor.read(cx).visible_colors), 25);

        window.click(format!("token-group-{variant}-Advanced"), cx);
        let editor_state = editor.read(cx);
        assert_eq!(editor_state.color_query.read(cx).value().as_str(), "");
        assert_eq!(
            editor_state
                .color_group
                .read(cx)
                .selected_value()
                .map(std::string::ToString::to_string),
            Some("All families".to_owned())
        );
        assert_eq!(
            visible_color_count(&editor_state.visible_colors),
            editor_state.variants[&variant].colors.len()
        );

        editor.update(cx, |editor, cx| {
            editor.color_group.update(cx, |group, cx| {
                group.set_selected_value(&"Tables".into(), window, cx);
            });
            editor.refresh_color_list(cx);
        });
        let table_colors: Vec<_> = editor.read(cx).variants[&variant]
            .colors
            .keys()
            .filter(|token| colors::family(token) == "Tables")
            .cloned()
            .collect();
        assert_eq!(
            editor.read(cx).visible_colors,
            std::iter::once(render::GROUP_HEADER_PREFIX.to_owned() + "Lists & tables")
                .chain(table_colors)
                .collect::<Vec<_>>()
        );
        editor.update(cx, |editor, cx| {
            editor.color_query.update(cx, |query, cx| {
                query.set_value("not-a-color-token", window, cx);
            });
            editor.refresh_color_list(cx);
        });
        assert_eq!(editor.read(cx).visible_colors.len(), 0);
        window.click(format!("token-group-{variant}-Colors"), cx);
        window.render_frame(cx);
        let editor_state = editor.read(cx);
        assert_eq!(editor_state.color_query.read(cx).value().as_str(), "");
        assert_eq!(
            editor_state
                .color_group
                .read(cx)
                .selected_value()
                .map(std::string::ToString::to_string),
            Some("All colors".to_owned())
        );
        assert_eq!(visible_color_count(&editor_state.visible_colors), 25);
        assert!(window.try_find("theme-color-search").is_none());
    })
    .unwrap();
    cleanup(&cx, family);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "Exercises source filtering, an active edit, reset, and clearing in one UI workflow"
)]
fn advanced_origin_checkboxes_filter_live_sources_and_reset_immediately() {
    let mut cx = TestAppContext::single();
    let (handle, app, family, variant, _) = workspace(&mut cx);
    let editor = cx.update(|cx| {
        app.read(cx)
            .tabs
            .theme_editor_for(family, cx)
            .unwrap()
            .clone()
    });
    let (theme_token, component_token) = cx.update(|cx| {
        let editor = editor.read(cx);
        (
            token_for_origin(editor, colors::ColorOrigin::ThemeDefined)
                .expect("theme fixture should define colors"),
            token_for_origin(editor, colors::ColorOrigin::ComponentDefault)
                .expect("component defaults should provide colors"),
        )
    });

    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click(format!("token-group-{variant}-Advanced"), cx);
        window.render_frame(cx);
        assert!(editor.read(cx).color_origins.all_selected());
        assert!(window.try_find("color-origin-datalith-default").is_none());
        for origin in colors::ColorOrigin::ALL {
            let checkbox = window.find(format!("color-origin-{}", origin.id()));
            assert_eq!(checkbox.role(), Some(gpui_kit::Role::CheckBox));
            assert_eq!(checkbox.checked(), Some(true));
        }
        assert_eq!(
            visible_color_count(&editor.read(cx).visible_colors),
            editor.read(cx).variants[&variant].colors.len()
        );

        window.click("color-origin-theme-defined", cx);
        let visible = &editor.read(cx).visible_colors;
        assert!(visible.iter().any(|token| token == &component_token));
        assert!(!visible.iter().any(|token| token == &theme_token));

        editor.update(cx, |editor, cx| {
            editor.edit_color(variant, &component_token, true, window, cx);
        });
        let picker = editor.read(cx).variants[&variant].colors[&component_token]
            .controls
            .as_ref()
            .unwrap()
            .picker
            .clone();
        // A slider edit commits without closing the picker; selecting a palette
        // swatch would finish the edit and correctly remove this filtered row.
        let chosen = gpui_kit::component::try_parse_color("#123456").unwrap();
        picker.update(cx, |picker, cx| picker.update_color(chosen, window, cx));
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle.into(), |_, window, cx| {
        assert!(
            editor
                .read(cx)
                .visible_colors
                .iter()
                .any(|token| token == &component_token),
            "the active row must remain available while its origin changes"
        );
        window.click("color-origin-component-default", cx);
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle.into(), |_, window, cx| {
        window.click("color-origin-theme-defined", cx);
        let editor_state = editor.read(cx);
        assert!(editor_state.active_color.is_none());
        assert!(
            !editor_state.variants[&variant].colors[&component_token]
                .controls
                .as_ref()
                .unwrap()
                .picker
                .read(cx)
                .is_open()
        );
        let visible = &editor.read(cx).visible_colors;
        assert!(visible.iter().any(|token| token == &theme_token));
        assert!(visible.iter().any(|token| token == &component_token));
        editor.update(cx, |editor, cx| {
            editor.reset_color(variant, &component_token, window, cx);
        });
        assert!(
            !editor
                .read(cx)
                .visible_colors
                .iter()
                .any(|token| token == &component_token),
            "reset should immediately restore the component-default origin"
        );
        window.click("color-origin-theme-defined", cx);
        assert!(editor.read(cx).visible_colors.is_empty());
        window.click("clear-color-filters", cx);
        assert!(editor.read(cx).color_origins.all_selected());
        assert_eq!(
            visible_color_count(&editor.read(cx).visible_colors),
            editor.read(cx).variants[&variant].colors.len()
        );
    })
    .unwrap();
    cleanup(&cx, family);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "Checks virtualized rendering, resizing, scrolling and edit-close position in one UI workflow"
)]
fn advanced_color_list_is_virtualized_and_scrolls_to_the_last_schema_token() {
    let mut cx = TestAppContext::single();
    let (handle, app, family, variant, _) = workspace(&mut cx);
    let editor = cx.update(|cx| {
        app.read(cx)
            .tabs
            .theme_editor_for(family, cx)
            .unwrap()
            .clone()
    });

    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click(format!("token-group-{variant}-Advanced"), cx);
        window.render_frame(cx);
    })
    .unwrap();
    cx.simulate_window_resize(handle.into(), size(px(1400.), px(820.)));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);

        let (all_tokens, last_token, viewport) = {
            let editor_state = editor.read(cx);
            let all_tokens: Vec<_> = editor_state.variants[&variant]
                .colors
                .keys()
                .cloned()
                .collect();
            let last_token = editor_state
                .visible_colors
                .iter()
                .rev()
                .find(|token| {
                    token.as_str() != render::VARIANT_MODE_ROW
                        && !token.starts_with(render::GROUP_HEADER_PREFIX)
                })
                .unwrap()
                .clone();
            (
                all_tokens,
                last_token,
                editor_state.color_list.viewport_bounds(),
            )
        };
        assert_eq!(visible_color_count(&editor.read(cx).visible_colors), 203);
        assert!(viewport.size.height > px(0.));
        assert!(viewport.size.height < window.viewport_size().height);

        let rendered_rows = all_tokens
            .iter()
            .filter(|token| {
                window
                    .try_find(format!("theme-token-{variant}-{token}"))
                    .is_some()
            })
            .count();
        assert!(rendered_rows > 0);
        assert!(
            rendered_rows < all_tokens.len(),
            "only viewport rows should render"
        );
        assert!(
            window
                .try_find(format!("theme-token-{variant}-{last_token}"))
                .is_none(),
            "the last schema item must begin outside the viewport"
        );

        let wheel_delta = px(-viewport.size.height.as_f32() * 0.8);
        for _ in 0..64 {
            window.scroll(
                "theme-controls-scroll",
                ScrollDelta::Pixels(point(px(0.), wheel_delta)),
                cx,
            );
            if window
                .try_find(format!("theme-token-{variant}-{last_token}"))
                .is_some_and(|row| {
                    row.visible()
                        && row.bounds().top() >= viewport.top()
                        && row.bounds().bottom() <= viewport.bottom()
                })
            {
                break;
            }
        }
        assert!(editor.read(cx).color_list.logical_scroll_top().item_ix > 0);
        let last_row = window.find(format!("theme-token-{variant}-{last_token}"));
        assert!(
            last_row.visible(),
            "the native list should scroll to the tail"
        );
        let viewport = editor.read(cx).color_list.viewport_bounds();
        assert!(last_row.bounds().top() >= viewport.top());
        assert!(last_row.bounds().bottom() <= viewport.bottom());

        editor.update(cx, |editor, cx| {
            editor.edit_color(variant, &last_token, false, window, cx);
            editor.finish_color_edit(variant, &last_token, cx);
        });
        window.render_frame(cx);
        assert!(
            window
                .find(format!("theme-token-{variant}-{last_token}"))
                .visible(),
            "closing an edit should preserve the current scroll position"
        );
    })
    .unwrap();
    cleanup(&cx, family);
}

#[test]
fn default_font_labels_follow_each_roles_effective_fallback() {
    let mut cx = TestAppContext::single();
    let (_, _, family, variant, _) = workspace(&mut cx);
    cx.update(|cx| {
        let mut document = cx
            .global::<ThemeLibrary>()
            .variant_by_id(variant)
            .unwrap()
            .document()
            .clone();
        document.set_font(FontRole::Interface, Some("Arial".into()));
        for role in FontRole::ALL {
            let choices = font_choices(&document, role, cx);
            let mut default = document.clone();
            default.set_font(role, None);
            let font = cx.global::<FontCatalog>().resolve_roles(&default)[role.index()].clone();
            assert_eq!(choices[0].label, format!("Default ({font})"));
            assert_eq!(choices[0].value.as_str(), "");
        }
        assert_eq!(
            font_choices(&document, FontRole::Reading, cx)[0]
                .label
                .as_str(),
            "Default (Arial)"
        );
    });
    cleanup(&cx, family);
}
