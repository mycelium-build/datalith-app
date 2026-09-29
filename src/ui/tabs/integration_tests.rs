use crate::app::{AppState, fonts, themes};
use crate::ui::DatalithView;
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AppContext as _, TestAppContext, px, size};

#[test]
fn family_editors_have_independent_tabs_and_theme_command_reopens_the_picker() {
    let mut cx = TestAppContext::single();
    let (first, second) = cx.update(|cx| {
        gpui_kit::init(cx);
        fonts::FontCatalog::init(cx);
        themes::load_embedded_themes(cx);
        themes::ThemeLibrary::init(cx);
        cx.set_global(AppState::default());
        let source = cx
            .global::<themes::ThemeLibrary>()
            .family_named("Catppuccin")
            .unwrap()
            .id();
        let first = cx
            .global_mut::<themes::ThemeLibrary>()
            .copy_family(source, &format!("First {:016x}", rand::random::<u64>()))
            .unwrap();
        let second = cx
            .global_mut::<themes::ThemeLibrary>()
            .copy_family(source, &format!("Second {:016x}", rand::random::<u64>()))
            .unwrap();
        (first, second)
    });
    let mut view = None;
    let handle = cx.open_window(size(px(1200.), px(900.)), |window, cx| {
        let app = cx.new(|cx| DatalithView::new(false, vec![], window, cx));
        app.update(cx, |app, cx| {
            app.startup_driver = gpui_kit::Task::ready(());
            app.startup = None;
            app.open_theme_editor_for(first, None, window, cx);
            app.open_theme_editor_for(second, None, window, cx);
            assert_eq!(app.tabs.entries.len(), 2);
            let initial = app.tabs.theme_editor_for(first, cx).unwrap().entity_id();
            app.open_theme_editor_for(first, None, window, cx);
            assert_eq!(app.tabs.entries.len(), 2);
            assert_eq!(app.tabs.active().unwrap().entity_id(), initial);
            app.open_themes(window, cx);
            assert!(app.settings.open);
        });
        view = Some(app.clone());
        Root::new(app, window, cx)
    });
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("close-settings").visible());
        assert!(window.find("theme-appearance").visible());
    })
    .unwrap();
    let view = view.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |app, cx| {
            app.settings.close();
            app.close_active_tab(window, cx);
            assert_eq!(app.tabs.entries.len(), 1);
            assert!(app.tabs.theme_editor_for(second, cx).is_some());
            assert_document_only_restore(app, second, window, cx);
        });
    })
    .unwrap();
    cx.update(|cx| {
        for id in [first, second] {
            if let Some(themes::ThemeSource::Custom(path)) = cx
                .global::<themes::ThemeLibrary>()
                .family(id)
                .map(crate::app::themes::ThemeFamily::source)
            {
                let _ = std::fs::remove_file(path);
            }
        }
    });
}

fn assert_document_only_restore(
    app: &mut DatalithView,
    family: u64,
    window: &mut gpui_kit::Window,
    cx: &mut gpui_kit::Context<DatalithView>,
) {
    let path = std::path::Path::new("docs/vault/Welcome.md")
        .canonicalize()
        .unwrap();
    let theme_entity = app.tabs.theme_editor_for(family, cx).unwrap().entity_id();
    app.open_file(path.clone(), false, window, cx);
    assert_eq!(app.tabs.entries.len(), 2);
    assert_eq!(
        app.tabs.theme_editor_for(family, cx).unwrap().entity_id(),
        theme_entity
    );
    let document_id = app.tabs.active_document_id().unwrap().clone();
    app.open_shortcuts(window, cx);
    assert!(app.tabs.active_handler().is_none());
    assert!(!app.can_go_back());
    app.toggle_editor_mode(cx);
    app.new_empty_tab(cx);
    let empty_id = app.tabs.active_document_id().unwrap().clone();
    assert_ne!(document_id, empty_id);
    assert_eq!(app.tabs.entries.len(), 4);
    let saved = app.tabs.snapshot(cx);
    assert_eq!(saved.0.len(), 2);
    assert_eq!(saved.0[0].path.as_deref(), Some(path.as_path()));
    assert_eq!(saved.0[1].path, None);
    assert_eq!(saved.1.as_ref(), Some(&empty_id));

    app.open_theme_editor_for(family, None, window, cx);
    let from_theme = app.tabs.snapshot(cx);
    assert_eq!(from_theme.0, saved.0);
    assert_eq!(from_theme.1, None);
    app.open_shortcuts(window, cx);
    assert_eq!(app.tabs.snapshot(cx), from_theme);

    app.restore_workspace_tabs(saved.0.clone(), saved.1.as_ref(), window, cx);
    assert_eq!(app.tabs.entries.len(), 2);
    assert!(
        app.tabs
            .entries
            .iter()
            .all(|tab| matches!(tab, super::Tab::Document(_)))
    );
    assert_eq!(app.tabs.snapshot(cx), saved);
    assert_eq!(app.tabs.open_paths(), vec![path]);
    assert!(app.tabs.select_by_id(&document_id));
    app.open_shortcuts(window, cx);
    app.close_active_tab(window, cx);
    assert!(app.tabs.active_handler().is_some());
}
