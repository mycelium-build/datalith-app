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
    Focusable as _, InputEvent as _, ScrollDelta, ScrollWheelEvent, TestAppContext, WindowHandle,
    point, px, size,
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
            window.click(format!("color-value-{first}-background"), cx);
            window.press("secondary-a", cx);
            window.input("#123456", cx);
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

fn cleanup(cx: &TestAppContext, id: u64) {
    cx.update(|cx| {
        if let Some(family) = cx.global::<ThemeLibrary>().family(id)
            && let ThemeSource::Custom(path) = family.source()
        {
            let _ = std::fs::remove_file(path);
        }
    });
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
        window.click(format!("color-value-{variant}-background"), cx);
        window.press("secondary-a", cx);
        window.input("#123456", cx);
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
    let (handle, app, family, variant, name) = workspace(&mut cx);
    let preference = crate::app::settings::snapshot().theme_preference;
    let (kind, old) = cx.update(|cx| {
        let library = cx.global::<ThemeLibrary>();
        let kind = ThemeKind::from(library.variant_by_id(variant).unwrap().mode());
        (kind, library.current(kind).to_owned())
    });
    cx.update_window(handle.into(), |_, window, cx| {
        app.update(cx, |app, cx| {
            app.settings.open_theme(window, cx);
            cx.notify();
        });
        window.render_frame(cx);
        window.click("theme-appearance", cx);
        window.press("home", cx);
        window.press("down", cx);
        if kind == ThemeKind::Dark {
            window.press("down", cx);
        }
        window.press("enter", cx);
        window.click("theme-search", cx);
        window.input(&name, cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click(("set-current", variant), cx);
        window.click("close-settings", cx);
        window.render_frame(cx);
        window.click(format!("color-value-{variant}-background"), cx);
        window.press("secondary-a", cx);
        window.input("#234567", cx);
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
fn invalid_color_stays_local_and_blur_restores_last_valid_value() {
    let mut cx = TestAppContext::single();
    let (handle, app, family, variant, _) = workspace(&mut cx);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click(format!("color-value-{variant}-background"), cx);
        window.press("secondary-a", cx);
        window.input("#123456", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.press("secondary-a", cx);
        window.input("invalid", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let editor = app.read(cx).tabs.theme_editor_for(family, cx).unwrap();
        assert!(
            editor.read(cx).variants[&variant].colors["background"]
                .error
                .is_some()
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
            editor.read(cx).variants[&variant].colors["background"]
                .controls
                .as_ref()
                .unwrap()
                .input
                .read(cx)
                .value()
                .as_str(),
            "#123456"
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
        window.click(format!("reset-color-{variant}-background"), cx);
        window.click(format!("color-value-{variant}-border"), cx);
        window.press("secondary-a", cx);
        window.input("#123456", cx);
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
        assert!(!picker.read(cx).is_open());
        window.click(format!("color-picker-{variant}-background"), cx);
        assert!(picker.read(cx).is_open());
        let chosen = gpui_kit::component::try_parse_color("#123456").unwrap();
        picker.update(cx, |picker, cx| picker.select_color(chosen, window, cx));
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let chosen = gpui_kit::component::try_parse_color("#123456")
            .unwrap()
            .to_hex();
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
    let (handle, _, family, variant, _) = workspace(&mut cx);
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
        window.click(format!("color-value-{variant}-background"), cx);
        window.press("secondary-a", cx);
        window.input("#123456", cx);
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
        window.click(format!("color-value-{first}-background"), cx);
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
    cx.update_window(handle.into(), |_, window, cx| {
        window.press("secondary-a", cx);
        window.input("#225588", cx);
    })
    .unwrap();
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
    let (handle, _, family, variant, _) = workspace(&mut cx);
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
        window.click(
            format!("color-value-{variant}-highlight:syntax.keyword"),
            cx,
        );
        window.press("secondary-a", cx);
        window.input("#345678", cx);
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
        window.click(("rename-variant", variant), cx);
    })
    .unwrap();
    cx.run_until_parked();
    std::thread::sleep(std::time::Duration::from_millis(260));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("theme-dialog-name", cx);
        window.press("secondary-a", cx);
        window.input("Morning", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| window.click("ok", cx))
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
    })
    .unwrap();
    cx.run_until_parked();
    // The native dialog uses a wall-clock entrance animation.
    std::thread::sleep(std::time::Duration::from_millis(260));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("theme-dialog-name", cx);
        window.press("secondary-a", cx);
        window.input(&renamed, cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| window.click("ok", cx))
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
fn color_swatch_opens_picker_directly_and_hex_input_does_not() {
    let mut cx = TestAppContext::single();
    let (handle, app, family, variant, _) = workspace(&mut cx);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click(format!("color-picker-{variant}-background"), cx);
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
            editor.read(cx).visible_colors.len(),
            editor.read(cx).variants[&variant].colors.len()
        );
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
            vec!["highlight:syntax.tag.doctype"]
        );
        window.click(
            format!("color-value-{variant}-highlight:syntax.tag.doctype"),
            cx,
        );
        window.press("secondary-a", cx);
        window.input("#123456", cx);
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
            editor.visible_colors.len(),
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
