use crate::app::{AppState, fonts, themes};
use crate::ui::{DatalithView, settings::SettingsView};
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AppContext as _, TestAppContext, px, size};

#[test]
fn family_editors_have_independent_tabs_and_reopening_focuses_existing_tab() {
    let mut cx = TestAppContext::single();
    let (first, second) = cx.update(|cx| {
        gpui_kit::init(cx);
        fonts::FontCatalog::init(cx);
        themes::load_embedded_themes(cx);
        themes::ThemeLibrary::init(cx);
        SettingsView::init_theme_options(cx);
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
            app.close_active_tab(window, cx);
            assert_eq!(app.tabs.entries.len(), 1);
            assert!(app.tabs.theme_editor_for(second, cx).is_some());
        });
        Root::new(app, window, cx)
    });
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
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
