use super::*;
use gpui_kit::component::ThemeSet;

fn library(test: &str) -> ThemeLibrary {
    let directory = std::env::temp_dir().join(format!(
        "datalith-themes-{test}-{:032x}",
        rand::random::<u128>()
    ));
    let mut library = ThemeLibrary {
        directory,
        ..Default::default()
    };
    let set: ThemeSet =
        serde_json::from_str(include_str!("../../../assets/themes/datalith.json")).unwrap();
    for config in set.themes {
        library.entries.insert(
            config.name.to_string(),
            ThemeEntry {
                document: ThemeDocument::from_config(config),
                path: None,
            },
        );
    }
    library
}

#[test]
fn custom_themes_round_trip_all_fonts_and_colors_without_modifying_the_base() {
    let mut library = library("round-trip");
    let base = library.get("Datalith Dark").unwrap().clone();
    let mut draft = base.clone();
    draft.set_name("My midnight theme");
    for (role, font) in
        FontRole::ALL
            .into_iter()
            .zip(["Arial", "Georgia", "Pixeloid Sans", "Menlo"])
    {
        draft.set_font(role, Some(font.into()));
    }
    draft
        .set_color("background", Some("#102030".into()))
        .unwrap();
    library.save(&draft, None).unwrap();
    let mut reloaded = ThemeLibrary {
        directory: library.directory.clone(),
        ..Default::default()
    };
    assert!(reloaded.load().is_empty());
    let saved = reloaded.get(draft.name()).unwrap();
    assert_eq!(
        serde_json::to_value(saved).unwrap(),
        serde_json::to_value(&draft).unwrap()
    );
    assert_eq!(
        serde_json::to_value(library.get(base.name())).unwrap(),
        serde_json::to_value(Some(&base)).unwrap()
    );
    assert!(library.is_custom(draft.name()));
    assert!(!library.is_custom(base.name()));
    std::fs::remove_dir_all(library.directory).unwrap();
}

#[test]
fn saves_protect_builtins_duplicates_and_paths_and_keep_the_old_file_on_failure() {
    let mut library = library("protection");
    let mut draft = library.get("Datalith Dark").unwrap().clone();
    assert!(library.save(&draft, Some(draft.name())).is_err());
    assert!(library.save(&draft, None).is_err());
    draft.set_name("../outside");
    library.save(&draft, None).unwrap();
    let path = library
        .entries
        .get(draft.name())
        .unwrap()
        .path
        .clone()
        .unwrap();
    assert_eq!(path.parent(), Some(library.directory.as_path()));
    let before = std::fs::read(&path).unwrap();
    let mut conflicting = draft.clone();
    conflicting.set_name("datalith dark");
    assert!(library.save(&conflicting, Some(draft.name())).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    draft.set_font(FontRole::Reading, Some("Georgia".into()));
    library.save(&draft, Some(draft.name())).unwrap();
    assert_eq!(
        library.entries.get(draft.name()).unwrap().path.as_ref(),
        Some(&path)
    );
    std::fs::remove_dir_all(library.directory).unwrap();
}

#[test]
fn malformed_and_future_files_do_not_hide_valid_custom_themes() {
    let mut library = library("partial-load");
    let mut draft = library.get("Datalith Dark").unwrap().clone();
    draft.set_name("Valid custom");
    library.save(&draft, None).unwrap();
    std::fs::write(library.directory.join("broken.json"), "{").unwrap();
    std::fs::write(
        library.directory.join("future.json"),
        serde_json::json!({"schema_version": 999, "theme": draft}).to_string(),
    )
    .unwrap();
    let mut reloaded = ThemeLibrary {
        directory: library.directory.clone(),
        ..Default::default()
    };
    assert_eq!(reloaded.load().len(), 2);
    assert!(reloaded.get("Valid custom").is_some());
    std::fs::remove_dir_all(library.directory).unwrap();
}

#[test]
fn preview_changes_the_workspace_and_discard_restores_the_saved_theme() {
    let cx = gpui_kit::TestAppContext::single();
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::app::fonts::FontCatalog::init(cx);
        crate::app::themes::load_embedded_themes(cx);
        cx.set_global(library("preview"));
        let saved = document("Datalith Dark", cx).unwrap();
        Theme::global_mut(cx).mode = ThemeMode::Dark;
        apply_document(&saved, cx);
        let before = cx.theme().background;
        let mut draft = saved.clone();
        draft
            .set_color("background", Some("#123456".into()))
            .unwrap();
        draft.set_font(FontRole::Reading, Some("Arial".into()));
        preview(&draft, cx);
        assert_eq!(
            cx.theme().background,
            gpui_kit::component::try_parse_color("#123456").unwrap()
        );
        assert_eq!(super::font(FontRole::Reading, cx), Some("Arial"));
        assert!(
            cx.global::<ThemeLibrary>()
                .get(saved.name())
                .unwrap()
                .font(FontRole::Reading)
                .is_none()
        );
        discard_preview(cx);
        assert_eq!(cx.theme().background, before);
        assert_eq!(super::font(FontRole::Reading, cx), None);
    });
}
