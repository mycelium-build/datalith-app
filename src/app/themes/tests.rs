use super::*;

struct Sandbox {
    root: PathBuf,
}
impl Sandbox {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "datalith-theme-domain-{:032x}",
            rand::random::<u128>()
        ));
        std::fs::create_dir_all(&root).unwrap();
        Self { root }
    }
    fn library(&self) -> ThemeLibrary {
        let mut library = ThemeLibrary::new(self.root.join("themes"))
            .with_preferences_file(self.root.join("preferences.json"));
        for set in embedded::sets() {
            library.insert_set(set, ThemeSource::Bundled).unwrap();
        }
        library
    }
    fn saved_slots(&self) -> (String, String) {
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(self.root.join("preferences.json")).unwrap())
                .unwrap();
        (
            value["light_theme_name"].as_str().unwrap().into(),
            value["dark_theme_name"].as_str().unwrap().into(),
        )
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn bundled_families_include_every_variant_and_sanitize_ayu() {
    let sandbox = Sandbox::new();
    let library = sandbox.library();
    assert_eq!(library.family_named("Ayu").unwrap().variants().len(), 2);
    assert_eq!(
        library.family_named("Ayu").unwrap().variants()[0].name(),
        "Ayu Light"
    );
    assert_eq!(
        library.family_named("Catppuccin").unwrap().variants().len(),
        4
    );
    assert_eq!(
        library.family_named("Datalith").unwrap().variants().len(),
        2
    );
}

#[test]
fn bundled_chrome_inherits_its_own_surface_when_upstream_omits_tokens() {
    let sandbox = Sandbox::new();
    let library = sandbox.library();
    let latte = library.variant("Catppuccin Latte").unwrap();
    let colors = latte.document().colors().unwrap();
    assert_eq!(colors["popover.background"], colors["background"]);
    assert_eq!(colors["popover.foreground"], colors["foreground"]);
    let datalith = library
        .variant("Datalith Dark")
        .unwrap()
        .document()
        .colors()
        .unwrap();
    assert_eq!(datalith["tab.background"], datalith["tab_bar.background"]);
}

#[test]
fn copy_rename_and_reload_preserve_every_variant_fonts_colors_highlight_and_ids() {
    let sandbox = Sandbox::new();
    let mut library = sandbox.library();
    let original = library.family_named("Catppuccin").unwrap().id();
    let copied = library.copy_family(original, "My cats").unwrap();
    assert_eq!(library.family(copied).unwrap().variants().len(), 4);
    let first = library.family(copied).unwrap().variants()[0].id();
    library
        .update_color(first, "background", Some("#123456".into()))
        .unwrap();
    library
        .update_font(first, FontRole::Reading, Some("Georgia".into()))
        .unwrap();
    library
        .update_highlight(first, "syntax.keyword", Some("#654321".into()))
        .unwrap();
    library.flush_family(copied).unwrap();
    library.set_current(first, ThemeKind::Light).unwrap();
    library.rename_family(copied, "Cats renamed").unwrap();
    assert_eq!(library.family(copied).unwrap().variants()[0].id(), first);
    assert!(
        library
            .variant_by_id(first)
            .unwrap()
            .name()
            .starts_with("Cats renamed ")
    );
    assert_eq!(
        sandbox.saved_slots().0,
        library.variant_by_id(first).unwrap().name()
    );
    let mut reloaded = sandbox.library();
    assert!(reloaded.load().is_empty());
    let family = reloaded.family_named("Cats renamed").unwrap();
    assert_eq!(family.variants().len(), 4);
    assert_eq!(
        family.variants()[0].document().font(FontRole::Reading),
        Some("Georgia")
    );
    assert_eq!(
        family.variants()[0].document().colors().unwrap()["background"],
        Some("#123456".into())
    );
    assert!(
        serde_json::to_string(&family.variants()[0].document().config().highlight)
            .unwrap()
            .contains("#654321")
    );
}

#[test]
fn add_from_non_first_clones_and_name_validation_does_not_mutate_on_failure() {
    let sandbox = Sandbox::new();
    let mut library = sandbox.library();
    let id = library
        .copy_family(library.family_named("Datalith").unwrap().id(), "Working")
        .unwrap();
    let dark = library.family(id).unwrap().variants()[1].id();
    library
        .update_font(dark, FontRole::Code, Some("Menlo".into()))
        .unwrap();
    let before = serde_json::to_value(library.variant_by_id(dark).unwrap().document()).unwrap();
    assert!(library.add_variant(id, dark, "Light", None).is_err());
    assert_eq!(library.family(id).unwrap().variants().len(), 2);
    let added = library.add_variant(id, dark, "Variant 1", None).unwrap();
    let cloned = library.variant_by_id(added).unwrap().document();
    assert_eq!(cloned.mode(), ThemeMode::Dark);
    assert_eq!(cloned.font(FontRole::Code), Some("Menlo"));
    let mut next = serde_json::to_value(cloned).unwrap();
    next["name"] = before["name"].clone();
    assert_eq!(next, before);
    assert_ne!(library.current(ThemeKind::Dark), cloned.name());
}

#[test]
fn remove_falls_back_in_family_then_to_datalith_and_undo_restores_exact_slots() {
    let sandbox = Sandbox::new();
    let mut library = sandbox.library();
    let id = library
        .copy_family(library.family_named("Catppuccin").unwrap().id(), "Working")
        .unwrap();
    let dark = library.family(id).unwrap().variants()[1].id();
    let other_dark = library.family(id).unwrap().variants()[2].name().to_owned();
    library.set_current(dark, ThemeKind::Dark).unwrap();
    let before = sandbox.saved_slots();
    let removed = library.remove_variant(dark).unwrap();
    assert_eq!(library.current(ThemeKind::Dark), other_dark);
    library.undo_delete(removed).unwrap();
    assert_eq!(sandbox.saved_slots(), before);
    let deleted = library.delete_family(id).unwrap();
    assert_eq!(library.current(ThemeKind::Dark), DEFAULT_DARK);
    library.undo_delete(deleted).unwrap();
    assert_eq!(sandbox.saved_slots(), before);
}

#[test]
fn tolerant_load_repairs_missing_tokens_but_skips_unreadable_json() {
    let sandbox = Sandbox::new();
    let mut library = sandbox.library();
    std::fs::create_dir_all(&library.directory).unwrap();
    std::fs::write(library.directory.join("broken.json"), "{").unwrap();
    std::fs::write(library.directory.join("partial.json"),r##"{"name":"Partial","themes":[{"name":"Partial Shade","mode":"dark","colors":{"background":"#113355"}}]}"##).unwrap();
    let errors = library.load();
    assert_eq!(errors.len(), 1);
    let variant = library.variant("Partial Shade").unwrap();
    assert_eq!(
        variant.document().colors().unwrap()["background"],
        Some("#113355".into())
    );
    assert!(
        library
            .resolved_config(variant.name())
            .unwrap()
            .highlight
            .is_some()
    );
}

#[test]
fn import_conflicts_export_round_trip_and_revision_flush_keep_latest() {
    let sandbox = Sandbox::new();
    let mut library = sandbox.library();
    let id = library
        .copy_family(library.family_named("Datalith").unwrap().id(), "Working")
        .unwrap();
    let export = sandbox.root.join("export.json");
    library.export(id, &export).unwrap();
    let exported: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&export).unwrap()).unwrap();
    assert!(exported.get("schema_version").is_none());
    assert_eq!(exported["themes"].as_array().unwrap().len(), 2);
    assert!(library.import(&export, ImportPolicy::Replace).is_ok());
    let copied = library.import(&export, ImportPolicy::Copy).unwrap();
    assert_eq!(library.family(copied).unwrap().name(), "Working copy");
    let variant = library.family(copied).unwrap().variants()[0].id();
    library
        .update_color(variant, "background", Some("#123456".into()))
        .unwrap();
    library
        .update_color(variant, "background", Some("#abcdef".into()))
        .unwrap();
    library.flush_pending();
    let mut reloaded = sandbox.library();
    assert!(reloaded.load().is_empty());
    assert_eq!(
        reloaded.family_named("Working copy").unwrap().variants()[0]
            .document()
            .colors()
            .unwrap()["background"],
        Some("#abcdef".into())
    );
    assert_eq!(library.suggested_copy_name("Working"), "Working copy 2");
}

#[test]
fn resolved_non_current_preview_is_isolated_from_global_appearance_and_slots() {
    let context = gpui_kit::TestAppContext::single();
    context.update(|cx| {
        gpui_kit::init(cx);
        super::super::fonts::FontCatalog::init(cx);
        let sandbox = Sandbox::new();
        let mut library = sandbox.library();
        let copied = library
            .copy_family(
                library.family_named("Datalith").unwrap().id(),
                "Preview only",
            )
            .unwrap();
        let dark = library.family(copied).unwrap().variants()[1].id();
        library
            .update_color(dark, "background", Some("#123456".into()))
            .unwrap();
        library
            .update_font(dark, FontRole::Reading, Some("Unavailable font".into()))
            .unwrap();
        let original = cx.theme().background;
        let slots = [
            library.current(ThemeKind::Light).to_owned(),
            library.current(ThemeKind::Dark).to_owned(),
        ];
        let snapshot = library
            .resolved(dark, cx.global::<super::super::fonts::FontCatalog>())
            .unwrap();
        assert_eq!(
            snapshot.color("background"),
            Some(gpui_kit::component::try_parse_color("#123456").unwrap())
        );
        assert_eq!(
            snapshot.configured_font(FontRole::Reading),
            Some("Unavailable font")
        );
        assert_eq!(cx.theme().background, original);
        assert_eq!(library.current(ThemeKind::Light), slots[0]);
        assert_eq!(library.current(ThemeKind::Dark), slots[1]);
    });
}

#[test]
fn reset_uses_the_effective_default_of_each_variants_mode() {
    let context = gpui_kit::TestAppContext::single();
    context.update(|cx| {
        gpui_kit::init(cx);
        super::super::fonts::FontCatalog::init(cx);
        let sandbox = Sandbox::new();
        let mut library = sandbox.library();
        let family = library
            .copy_family(
                library.family_named("Datalith").unwrap().id(),
                "Reset by mode",
            )
            .unwrap();
        let ids: Vec<_> = library
            .family(family)
            .unwrap()
            .variants()
            .iter()
            .map(ThemeVariant::id)
            .collect();
        for id in &ids {
            library
                .update_color(*id, "background", Some("#123456".into()))
                .unwrap();
            library.update_color(*id, "background", None).unwrap();
        }
        let light = library
            .resolved(ids[0], cx.global::<super::super::fonts::FontCatalog>())
            .unwrap();
        let dark = library
            .resolved(ids[1], cx.global::<super::super::fonts::FontCatalog>())
            .unwrap();
        assert_eq!(light.color("background"), Some(light.theme().background));
        assert_eq!(dark.color("background"), Some(dark.theme().background));
        assert_ne!(light.color("background"), dark.color("background"));
        assert!(
            library
                .family(family)
                .unwrap()
                .variants()
                .iter()
                .all(|variant| variant.document().colors().unwrap()["background"].is_none())
        );
    });
}

#[test]
fn every_bundled_editor_color_has_a_resolved_value() {
    let context = gpui_kit::TestAppContext::single();
    context.update(|cx| {
        gpui_kit::init(cx);
        super::super::fonts::FontCatalog::init(cx);
        let sandbox = Sandbox::new();
        let library = sandbox.library();
        for variant in library.families().flat_map(ThemeFamily::variants) {
            let name = variant.name();
            let resolved = library
                .resolved(
                    variant.id(),
                    cx.global::<super::super::fonts::FontCatalog>(),
                )
                .unwrap();
            for token in variant.document().colors().unwrap().keys() {
                assert!(
                    resolved.color(token).is_some(),
                    "{name}: {token} has no effective color"
                );
            }
        }
    });
}

#[test]
fn mode_transition_falls_back_without_claiming_opposite_slot() {
    let sandbox = Sandbox::new();
    let mut library = sandbox.library();
    let id = library
        .copy_family(library.family_named("Datalith").unwrap().id(), "Working")
        .unwrap();
    let light = library.family(id).unwrap().variants()[0].id();
    library.set_current(light, ThemeKind::Light).unwrap();
    let dark_before = library.current(ThemeKind::Dark).to_owned();
    library.change_mode(light, ThemeMode::Dark).unwrap();
    assert_eq!(library.current(ThemeKind::Light), DEFAULT_LIGHT);
    assert_eq!(library.current(ThemeKind::Dark), dark_before);
    assert_eq!(sandbox.saved_slots(), (DEFAULT_LIGHT.into(), dark_before));
}

#[test]
fn failed_preference_write_cannot_leave_dangling_refs_or_claim_rename_or_delete() {
    let sandbox = Sandbox::new();
    let mut library = sandbox.library();
    let id = library
        .copy_family(library.family_named("Datalith").unwrap().id(), "Working")
        .unwrap();
    let light = library.family(id).unwrap().variants()[0].id();
    library.set_current(light, ThemeKind::Light).unwrap();
    let path = match library.family(id).unwrap().source() {
        ThemeSource::Custom(path) => path.clone(),
        ThemeSource::Bundled => unreachable!(),
    };
    let before = std::fs::read(&path).unwrap();
    let prefs = sandbox.root.join("preferences.json");
    std::fs::remove_file(&prefs).unwrap();
    std::fs::create_dir(&prefs).unwrap();
    assert!(library.rename_family(id, "Renamed").is_err());
    assert!(library.remove_variant(light).is_err());
    assert!(library.set_current(light, ThemeKind::Light).is_err());
    assert_eq!(library.family(id).unwrap().name(), "Working");
    assert_eq!(library.current(ThemeKind::Light), "Working Light");
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn valid_but_damaged_variant_keeps_healthy_tokens_and_unavailable_fonts() {
    let sandbox = Sandbox::new();
    let mut library = sandbox.library();
    std::fs::create_dir_all(&library.directory).unwrap();
    std::fs::write(library.directory.join("damaged.json"), r##"{"name":"Damaged","themes":[{"name":"Damaged","mode":"dark","colors":{"background":"#123456","foreground":42,"border":"not-a-color"},"fonts":{"interface":"Not installed","code":12},"highlight":{"editor.foreground":"#abcdef","editor.background":"bad","syntax":{"keyword":{"color":"invalid"},"string":{"color":"#765432"}}}}]}"##).unwrap();
    let errors = library.load();
    assert!(errors.is_empty(), "{errors:?}");
    let document = library.get("Damaged").unwrap();
    assert_eq!(
        document.colors().unwrap()["background"],
        Some("#123456".into())
    );
    assert_eq!(document.colors().unwrap()["foreground"], None);
    assert_eq!(document.colors().unwrap()["border"], None);
    assert_eq!(document.font(FontRole::Interface), Some("Not installed"));
    assert_eq!(document.font(FontRole::Code), None);
    let resolved = library.resolved_config("Damaged").unwrap();
    assert!(resolved.highlight.is_some());
    let highlight = serde_json::to_value(resolved.highlight).unwrap();
    assert_eq!(highlight["editor.foreground"], "#abcdefff");
    assert_eq!(highlight["syntax"]["string"]["color"], "#765432ff");
    assert_ne!(highlight["syntax"]["keyword"]["color"], "invalid");
}

#[test]
fn import_never_replaces_a_built_in_and_built_in_conflict_copies_whole_family() {
    let sandbox = Sandbox::new();
    let mut library = sandbox.library();
    let file = sandbox.root.join("builtin.json");
    std::fs::write(
        &file,
        include_str!("../../../assets/themes/catppuccin.json"),
    )
    .unwrap();
    assert!(library.import(&file, ImportPolicy::Replace).is_err());
    let id = library.import(&file, ImportPolicy::Copy).unwrap();
    assert_eq!(library.family(id).unwrap().name(), "Catppuccin copy");
    assert_eq!(library.family(id).unwrap().variants().len(), 4);
}

#[test]
fn importing_a_damaged_name_persists_the_normalized_variant() {
    let sandbox = Sandbox::new();
    let mut library = sandbox.library();
    let file = sandbox.root.join("import.json");
    std::fs::write(
        &file,
        r#"{"name":"Imported","themes":[{"name":"Unrelated","mode":"light"}]}"#,
    )
    .unwrap();
    let id = library.import(&file, ImportPolicy::Copy).unwrap();
    assert_eq!(library.family(id).unwrap().variants()[0].name(), "Imported");
    let path = match library.family(id).unwrap().source() {
        ThemeSource::Custom(path) => path,
        ThemeSource::Bundled => unreachable!(),
    };
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(saved["themes"][0]["name"], "Imported");
}

#[test]
fn autosave_failure_keeps_valid_model_and_retry_reports_truthfully() {
    let sandbox = Sandbox::new();
    let mut library = sandbox.library();
    let id = library
        .copy_family(
            library.family_named("Datalith").unwrap().id(),
            "Recoverable",
        )
        .unwrap();
    let variant = library.family(id).unwrap().variants()[0].id();
    let path = match library.family(id).unwrap().source() {
        ThemeSource::Custom(path) => path.clone(),
        ThemeSource::Bundled => unreachable!(),
    };
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    library
        .update_color(variant, "background", Some("#abcdef".into()))
        .unwrap();
    assert!(library.flush_family(id).is_err());
    assert!(matches!(
        library.family(id).unwrap().status(),
        SaveStatus::Failed(_)
    ));
    assert_eq!(library.flush_pending().len(), 1);
    assert_eq!(
        library
            .variant_by_id(variant)
            .unwrap()
            .document()
            .colors()
            .unwrap()["background"],
        Some("#abcdef".into())
    );
    std::fs::remove_dir(&path).unwrap();
    library.retry(id).unwrap();
    assert!(matches!(
        library.family(id).unwrap().status(),
        SaveStatus::Autosaved
    ));
    let mut reloaded = sandbox.library();
    assert!(reloaded.load().is_empty());
    assert_eq!(
        reloaded.family_named("Recoverable").unwrap().variants()[0]
            .document()
            .colors()
            .unwrap()["background"],
        Some("#abcdef".into())
    );
}

#[test]
fn deletion_and_rename_reconcile_pending_edits_without_resurrecting_old_files() {
    let sandbox = Sandbox::new();
    let mut library = sandbox.library();
    let id = library
        .copy_family(library.family_named("Datalith").unwrap().id(), "Pending")
        .unwrap();
    let variant = library.family(id).unwrap().variants()[0].id();
    let path = match library.family(id).unwrap().source() {
        ThemeSource::Custom(path) => path.clone(),
        ThemeSource::Bundled => unreachable!(),
    };
    library
        .update_font(variant, FontRole::Reading, Some("Georgia".into()))
        .unwrap();
    library.rename_family(id, "Renamed pending").unwrap();
    assert!(matches!(
        library.family(id).unwrap().status(),
        SaveStatus::Autosaved
    ));
    assert!(library.flush_pending().is_empty());
    let mut reloaded = sandbox.library();
    assert!(reloaded.load().is_empty());
    assert_eq!(
        reloaded.family_named("Renamed pending").unwrap().variants()[0]
            .document()
            .font(FontRole::Reading),
        Some("Georgia")
    );
    library
        .update_color(variant, "background", Some("#123456".into()))
        .unwrap();
    let deleted = library.delete_family(id).unwrap();
    assert!(!path.exists());
    assert!(library.flush_pending().is_empty());
    assert!(!path.exists());
    library.undo_delete(deleted).unwrap();
    assert_eq!(
        library
            .variant_by_id(variant)
            .unwrap()
            .document()
            .colors()
            .unwrap()["background"],
        Some("#123456".into())
    );
}

#[test]
fn apply_fonts_helper_uses_edited_variant_and_preserves_unset_roles() {
    let sandbox = Sandbox::new();
    let mut library = sandbox.library();
    let id = library
        .copy_family(library.family_named("Datalith").unwrap().id(), "Fonts")
        .unwrap();
    let first = library.family(id).unwrap().variants()[0].id();
    let second = library.family(id).unwrap().variants()[1].id();
    library
        .update_font(first, FontRole::Reading, Some("Old font".into()))
        .unwrap();
    library
        .update_font(second, FontRole::Interface, Some("New font".into()))
        .unwrap();
    library
        .update_font(second, FontRole::Headings, Some("Missing font".into()))
        .unwrap();
    assert_eq!(library.apply_fonts_to_all(second).unwrap(), id);
    assert_eq!(
        library
            .variant_by_id(first)
            .unwrap()
            .document()
            .font(FontRole::Interface),
        Some("New font")
    );
    assert_eq!(
        library
            .variant_by_id(first)
            .unwrap()
            .document()
            .font(FontRole::Reading),
        None
    );
    assert_eq!(
        library
            .variant_by_id(first)
            .unwrap()
            .document()
            .font(FontRole::Headings),
        Some("Missing font")
    );
}

#[test]
fn mono_to_multi_names_both_variants_atomically_and_removal_keeps_suffix() {
    let sandbox = Sandbox::new();
    let mut library = sandbox.library();
    let mut only = library.get(DEFAULT_LIGHT).unwrap().clone();
    only.set_name("Solo");
    let path = library.new_path();
    let set = StoredThemeSet {
        name: "Solo".into(),
        themes: vec![only],
        ..Default::default()
    };
    storage::write(&path, &set).unwrap();
    let family = library.insert_set(set, ThemeSource::Custom(path)).unwrap();
    let original = library.family(family).unwrap().variants()[0].id();
    library.set_current(original, ThemeKind::Light).unwrap();
    let before = sandbox.saved_slots();
    assert!(
        library
            .add_variant(family, original, "Second", None)
            .is_err()
    );
    assert!(
        library
            .add_variant(family, original, "Same", Some("same"))
            .is_err()
    );
    assert_eq!(library.family(family).unwrap().variants().len(), 1);
    assert_eq!(sandbox.saved_slots(), before);
    let path = match library.family(family).unwrap().source() {
        ThemeSource::Custom(path) => path.clone(),
        ThemeSource::Bundled => unreachable!(),
    };
    let saved_theme = std::fs::read(&path).unwrap();
    let prefs = sandbox.root.join("preferences.json");
    let backup = sandbox.root.join("preferences.backup");
    std::fs::rename(&prefs, &backup).unwrap();
    std::fs::create_dir(&prefs).unwrap();
    assert!(
        library
            .add_variant(family, original, "Second", Some("First"))
            .is_err()
    );
    assert_eq!(library.family(family).unwrap().variants().len(), 1);
    assert_eq!(library.variant_by_id(original).unwrap().name(), "Solo");
    assert_eq!(library.current(ThemeKind::Light), "Solo");
    assert_eq!(std::fs::read(&path).unwrap(), saved_theme);
    std::fs::remove_dir(&prefs).unwrap();
    std::fs::rename(&backup, &prefs).unwrap();
    let added = library
        .add_variant(family, original, "Second", Some("First"))
        .unwrap();
    assert_eq!(
        library.variant_by_id(original).unwrap().name(),
        "Solo First"
    );
    assert_eq!(library.variant_by_id(added).unwrap().name(), "Solo Second");
    assert_eq!(library.current(ThemeKind::Light), "Solo First");
    assert!(matches!(
        library.family(family).unwrap().status(),
        SaveStatus::Autosaved
    ));
    library.remove_variant(added).unwrap();
    assert_eq!(
        library.family(family).unwrap().variants()[0].name(),
        "Solo First"
    );
}

#[test]
fn undo_variant_preserves_other_edits_made_after_removal() {
    let sandbox = Sandbox::new();
    let mut library = sandbox.library();
    let id = library
        .copy_family(library.family_named("Datalith").unwrap().id(), "Undo")
        .unwrap();
    let first = library.family(id).unwrap().variants()[0].id();
    let second = library.family(id).unwrap().variants()[1].id();
    let deleted = library.remove_variant(first).unwrap();
    library
        .update_font(second, FontRole::Reading, Some("Later edit".into()))
        .unwrap();
    library.undo_delete(deleted).unwrap();
    assert_eq!(library.family(id).unwrap().variants().len(), 2);
    assert_eq!(
        library
            .variant_by_id(second)
            .unwrap()
            .document()
            .font(FontRole::Reading),
        Some("Later edit")
    );
}

#[test]
fn undo_two_variants_in_either_order_restores_each_exact_id_and_current_slot() {
    for remove_frappe_first in [true, false] {
        for undo_first_removal_first in [true, false] {
            let sandbox = Sandbox::new();
            let mut library = sandbox.library();
            let source = library.family_named("Catppuccin").unwrap().id();
            let family = library.copy_family(source, "Two removals").unwrap();
            let ids: Vec<_> = library
                .family(family)
                .unwrap()
                .variants()
                .iter()
                .map(ThemeVariant::id)
                .collect();
            let latte = ids[0];
            let frappe = ids[1];
            library.set_current(latte, ThemeKind::Light).unwrap();
            let first = library
                .remove_variant(if remove_frappe_first { frappe } else { latte })
                .unwrap();
            let second = library
                .remove_variant(if remove_frappe_first { latte } else { frappe })
                .unwrap();
            assert_eq!(library.current(ThemeKind::Light), DEFAULT_LIGHT);

            let (undo_now, undo_later) = if undo_first_removal_first {
                (first, second)
            } else {
                (second, first)
            };
            library.undo_delete(undo_now).unwrap();
            let restored = if remove_frappe_first == undo_first_removal_first {
                frappe
            } else {
                latte
            };
            let still_missing = if restored == frappe { latte } else { frappe };
            assert!(library.variant_by_id(restored).is_some());
            assert!(library.variant_by_id(still_missing).is_none());
            assert_eq!(
                library.current(ThemeKind::Light),
                if restored == frappe {
                    DEFAULT_LIGHT
                } else {
                    "Two removals Latte"
                }
            );
            assert_eq!(library.current(ThemeKind::Dark), DEFAULT_DARK);

            library.undo_delete(undo_later).unwrap();
            let mut restored_ids: Vec<_> = library
                .family(family)
                .unwrap()
                .variants()
                .iter()
                .map(ThemeVariant::id)
                .collect();
            let mut expected_ids = ids;
            restored_ids.sort_unstable();
            expected_ids.sort_unstable();
            assert_eq!(restored_ids, expected_ids);
            assert_eq!(library.current(ThemeKind::Light), "Two removals Latte");
            assert_eq!(
                sandbox.saved_slots(),
                ("Two removals Latte".into(), DEFAULT_DARK.into())
            );
        }
    }
}

#[test]
fn undo_noncurrent_variant_does_not_restore_a_different_removed_current_variant() {
    let sandbox = Sandbox::new();
    let mut library = sandbox.library();
    let source = library.family_named("Catppuccin").unwrap().id();
    let family = library.copy_family(source, "Dark pair").unwrap();
    let frappe = library.family(family).unwrap().variants()[1].id();
    let macchiato = library.family(family).unwrap().variants()[2].id();
    library.set_current(macchiato, ThemeKind::Dark).unwrap();
    let removed_frappe = library.remove_variant(frappe).unwrap();
    let removed_macchiato = library.remove_variant(macchiato).unwrap();
    assert_eq!(library.current(ThemeKind::Dark), "Dark pair Mocha");

    library.undo_delete(removed_frappe).unwrap();
    assert!(library.variant_by_id(frappe).is_some());
    assert!(library.variant_by_id(macchiato).is_none());
    assert_eq!(library.current(ThemeKind::Dark), "Dark pair Mocha");
    assert_eq!(sandbox.saved_slots().1, "Dark pair Mocha");

    library.undo_delete(removed_macchiato).unwrap();
    assert!(library.variant_by_id(macchiato).is_some());
    assert_eq!(library.current(ThemeKind::Dark), "Dark pair Macchiato");
}

#[test]
fn overlapping_family_prefixes_cannot_make_ambiguous_current_variant_names() {
    let sandbox = Sandbox::new();
    let mut library = sandbox.library();
    let id = library
        .copy_family(library.family_named("Datalith").unwrap().id(), "Project")
        .unwrap();
    let first = library.family(id).unwrap().variants()[0].id();
    library.rename_variant(first, "Light custom").unwrap();
    let overlapping = library.copy_family(
        library.family_named("Datalith").unwrap().id(),
        "Project Light",
    );
    assert!(overlapping.is_ok());
    let other = overlapping.unwrap();
    // A rename to a name already held by the other family is rejected.
    assert!(library.rename_variant(first, "Light Light").is_err());
    assert!(library.family(other).is_some());
}

#[test]
fn replacing_a_current_family_uses_another_imported_variant_of_the_same_mode() {
    let sandbox = Sandbox::new();
    let mut library = sandbox.library();
    let id = library
        .copy_family(library.family_named("Datalith").unwrap().id(), "Replacing")
        .unwrap();
    let first = library.family(id).unwrap().variants()[0].id();
    library.set_current(first, ThemeKind::Light).unwrap();
    let file = sandbox.root.join("replacement.json");
    library.export(id, &file).unwrap();
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    value["themes"][0]["name"] = "Replacing New light".into();
    std::fs::write(&file, serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(library.import(&file, ImportPolicy::Replace).unwrap(), id);
    assert_eq!(library.current(ThemeKind::Light), "Replacing New light");
    assert_eq!(sandbox.saved_slots().0, "Replacing New light");
}

#[test]
fn replacing_with_a_malformed_variant_name_keeps_the_family_prefix_and_current_slot_valid() {
    let sandbox = Sandbox::new();
    let mut library = sandbox.library();
    let id = library
        .copy_family(library.family_named("Datalith").unwrap().id(), "Replacing")
        .unwrap();
    let first = library.family(id).unwrap().variants()[0].id();
    library.set_current(first, ThemeKind::Light).unwrap();
    let file = sandbox.root.join("malformed-replacement.json");
    library.export(id, &file).unwrap();
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    value["themes"][0]["name"] = "Unrelated name".into();
    std::fs::write(&file, serde_json::to_vec(&value).unwrap()).unwrap();
    library.import(&file, ImportPolicy::Replace).unwrap();
    assert_eq!(library.current(ThemeKind::Light), "Replacing Variant 1");
    assert_eq!(sandbox.saved_slots().0, "Replacing Variant 1");
    let path = match library.family(id).unwrap().source() {
        ThemeSource::Custom(path) => path,
        ThemeSource::Bundled => unreachable!(),
    };
    let stored: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(stored["themes"][0]["name"], "Replacing Variant 1");
}

#[test]
fn undo_family_restores_only_its_unchanged_fallback_slots() {
    let sandbox = Sandbox::new();
    let mut library = sandbox.library();
    let source = library.family_named("Datalith").unwrap().id();
    let restored = library.copy_family(source, "Restored").unwrap();
    let unrelated = library.copy_family(source, "Unrelated").unwrap();
    let light = library.family(restored).unwrap().variants()[0].id();
    let unrelated_dark = library.family(unrelated).unwrap().variants()[1].id();
    library.set_current(light, ThemeKind::Light).unwrap();
    library
        .set_current(unrelated_dark, ThemeKind::Dark)
        .unwrap();
    let deleted = library.delete_family(restored).unwrap();
    let default_dark = library.variant(DEFAULT_DARK).unwrap().id();
    library.set_current(default_dark, ThemeKind::Dark).unwrap();
    library.undo_delete(deleted).unwrap();
    assert_eq!(library.current(ThemeKind::Light), "Restored Light");
    assert_eq!(library.current(ThemeKind::Dark), DEFAULT_DARK);
    assert_eq!(
        sandbox.saved_slots(),
        ("Restored Light".into(), DEFAULT_DARK.into())
    );

    library
        .set_current(unrelated_dark, ThemeKind::Dark)
        .unwrap();
    let deleted_again = library.delete_family(restored).unwrap();
    let default_light = library.variant(DEFAULT_LIGHT).unwrap().id();
    library
        .set_current(default_light, ThemeKind::Light)
        .unwrap();
    library.delete_family(unrelated).unwrap();
    library.undo_delete(deleted_again).unwrap();
    assert_eq!(library.current(ThemeKind::Light), DEFAULT_LIGHT);
    assert_eq!(library.current(ThemeKind::Dark), DEFAULT_DARK);
    assert_eq!(
        sandbox.saved_slots(),
        (DEFAULT_LIGHT.into(), DEFAULT_DARK.into())
    );
}

#[test]
fn undo_variant_rejects_a_name_claimed_since_removal_before_writing() {
    let sandbox = Sandbox::new();
    let mut library = sandbox.library();
    let source = library.family_named("Datalith").unwrap().id();
    let family = library.copy_family(source, "Working").unwrap();
    let light = library.family(family).unwrap().variants()[0].id();
    library.set_current(light, ThemeKind::Light).unwrap();
    let deleted = library.remove_variant(light).unwrap();
    let mut claiming = library.get(DEFAULT_LIGHT).unwrap().clone();
    claiming.set_name("Working Light");
    let claiming_path = library.new_path();
    let claiming_set = StoredThemeSet {
        name: "Working Light".into(),
        themes: vec![claiming],
        ..Default::default()
    };
    storage::write(&claiming_path, &claiming_set).unwrap();
    library
        .insert_set(claiming_set, ThemeSource::Custom(claiming_path))
        .unwrap();
    let path = match library.family(family).unwrap().source() {
        ThemeSource::Custom(path) => path.clone(),
        ThemeSource::Bundled => unreachable!(),
    };
    let bytes = std::fs::read(&path).unwrap();
    let slots = sandbox.saved_slots();
    assert!(library.undo_delete(deleted).is_err());
    assert!(library.variant_by_id(light).is_none());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert_eq!(sandbox.saved_slots(), slots);
}

#[test]
fn undo_family_rejects_a_variant_name_claimed_by_another_family_before_writing() {
    let sandbox = Sandbox::new();
    let mut library = sandbox.library();
    let source = library.family_named("Datalith").unwrap().id();
    let family = library.copy_family(source, "Restored").unwrap();
    let path = match library.family(family).unwrap().source() {
        ThemeSource::Custom(path) => path.clone(),
        ThemeSource::Bundled => unreachable!(),
    };
    let deleted = library.delete_family(family).unwrap();
    let mut claiming = library.get(DEFAULT_LIGHT).unwrap().clone();
    claiming.set_name("Restored Light");
    let claiming_path = library.new_path();
    let claiming_set = StoredThemeSet {
        name: "Restored Light".into(),
        themes: vec![claiming],
        ..Default::default()
    };
    storage::write(&claiming_path, &claiming_set).unwrap();
    library
        .insert_set(claiming_set, ThemeSource::Custom(claiming_path))
        .unwrap();
    let slots = sandbox.saved_slots();
    assert!(library.undo_delete(deleted).is_err());
    assert!(library.family(family).is_none());
    assert!(!path.exists());
    assert_eq!(sandbox.saved_slots(), slots);
}

#[test]
fn syntax_color_edit_and_reset_preserve_style_attributes_across_reload() {
    let sandbox = Sandbox::new();
    let mut library = sandbox.library();
    let source = library.family_named("Catppuccin").unwrap().id();
    let family = library.copy_family(source, "Styled").unwrap();
    let variant = library.family(family).unwrap().variants()[0].id();
    let mut raw = serde_json::to_value(library.variant_by_id(variant).unwrap().document()).unwrap();
    raw["highlight"]["syntax"]["emphasis.strong"]["font_style"] = "italic".into();
    let styled: ThemeDocument = serde_json::from_value(raw).unwrap();
    library
        .update_document(variant, |document| {
            *document = styled;
            Ok(())
        })
        .unwrap();
    library
        .update_highlight(variant, "syntax.emphasis.strong", Some("#123456".into()))
        .unwrap();
    let after_set = serde_json::to_value(
        &library
            .variant_by_id(variant)
            .unwrap()
            .document()
            .config()
            .highlight,
    )
    .unwrap();
    assert_eq!(after_set["syntax"]["emphasis.strong"]["font_weight"], 700);
    assert_eq!(
        after_set["syntax"]["emphasis.strong"]["font_style"],
        "italic"
    );
    library
        .update_highlight(variant, "syntax.emphasis.strong", None)
        .unwrap();
    library.flush_family(family).unwrap();
    let mut reloaded = sandbox.library();
    assert!(reloaded.load().is_empty());
    let style = serde_json::to_value(
        &reloaded.family_named("Styled").unwrap().variants()[0]
            .document()
            .config()
            .highlight,
    )
    .unwrap();
    assert_eq!(style["syntax"]["emphasis.strong"]["font_weight"], 700);
    assert_eq!(style["syntax"]["emphasis.strong"]["font_style"], "italic");
}

#[test]
fn multi_variant_sets_and_replacements_assign_suffixes_to_unsuffixed_variants() {
    let sandbox = Sandbox::new();
    let mut library = sandbox.library();
    let source = library.family_named("Datalith").unwrap().id();
    let mut unsuffixed = library.get(DEFAULT_LIGHT).unwrap().clone();
    unsuffixed.set_name("Mixed");
    let mut dark = library.get(DEFAULT_DARK).unwrap().clone();
    dark.set_name("Mixed Dark");
    let path = library.new_path();
    let set = StoredThemeSet {
        name: "Mixed".into(),
        themes: vec![unsuffixed, dark],
        ..Default::default()
    };
    storage::write(&path, &set).unwrap();
    let family = library.insert_set(set, ThemeSource::Custom(path)).unwrap();
    assert_eq!(
        library.family(family).unwrap().variants()[0].name(),
        "Mixed Variant 1"
    );
    assert_eq!(
        library.family(family).unwrap().variants()[1].name(),
        "Mixed Dark"
    );

    let copied = library.copy_family(source, "Replacing").unwrap();
    let file = sandbox.root.join("unsuffixed-replacement.json");
    library.export(copied, &file).unwrap();
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    value["themes"][0]["name"] = "Replacing".into();
    std::fs::write(&file, serde_json::to_vec(&value).unwrap()).unwrap();
    library.import(&file, ImportPolicy::Replace).unwrap();
    assert_eq!(
        library.family(copied).unwrap().variants()[0].name(),
        "Replacing Variant 1"
    );
    let path = match library.family(copied).unwrap().source() {
        ThemeSource::Custom(path) => path,
        ThemeSource::Bundled => unreachable!(),
    };
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(saved["themes"][0]["name"], "Replacing Variant 1");
}
