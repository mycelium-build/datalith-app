use super::*;
use crate::app::{AppState, actions, fonts, keymap, preferences, themes};
use crate::ui::{DatalithView, settings::SettingsView};
use gpui_kit::component::{ActiveTheme as _, Root};
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AppContext as _, TestAppContext, WindowHandle, px, size};

fn workspace(cx: &mut TestAppContext) -> (WindowHandle<Root>, Entity<DatalithView>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        fonts::FontCatalog::init(cx);
        themes::load_embedded_themes(cx);
        themes::ThemeLibrary::init(cx);
        SettingsView::init_theme_options(cx);
        preferences::apply(cx);
        cx.set_global(AppState::default());
        actions::register(cx);
        keymap::register(cx);
    });
    let mut app = None;
    let handle = cx.open_window(size(px(1200.), px(900.)), |window, cx| {
        let view = cx.new(|cx| DatalithView::new(false, vec![], window, cx));
        view.update(cx, |view, cx| {
            view.startup_driver = gpui_kit::Task::ready(());
            view.startup = None;
            view.open_theme_editor(window, cx);
        });
        cx.global_mut::<AppState>().view = Some(view.clone());
        app = Some(view.clone());
        window.activate_window();
        Root::new(view, window, cx)
    });
    cx.run_until_parked();
    (handle, app.unwrap())
}

#[test]
fn editor_tabs_retain_drafts_and_do_not_replace_or_pollute_documents() {
    let mut cx = TestAppContext::single();
    let (handle, app) = workspace(&mut cx);
    let before = cx.update(|cx| cx.theme().background);
    let editor = cx.update(|cx| app.read(cx).tabs.theme_editor().unwrap().clone());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("theme-color-value", cx);
        window.press("secondary-a", cx);
        window.input("#123456", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        app.update(cx, |view, cx| {
            view.open_file(PathBuf::from("docs/vault/Welcome.md"), false, window, cx);
            assert_eq!(view.tabs.entries.len(), 2);
            assert!(view.tabs.active_handler().is_some());
            view.open_shortcuts(window, cx);
            let shortcuts = view.tabs.active().unwrap().entity_id();
            view.open_shortcuts(window, cx);
            assert_eq!(view.tabs.active().unwrap().entity_id(), shortcuts);
            assert_eq!(view.tabs.entries.len(), 3);
            assert_eq!(
                view.tabs.open_paths(),
                [PathBuf::from("docs/vault/Welcome.md")]
            );
            assert!(view.tabs.active_path().is_none());
            assert!(view.tabs.active_handler().is_none());
            assert!(!view.can_go_back() && !view.can_go_forward());
            assert!(!view.settings.open);
        });
        window.render_frame(cx);
        assert!(window.find("shortcuts-editor").visible());
        assert!(window.try_find("save-theme").is_none());
        window.press("secondary-1", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("save-theme").visible());
        assert!(editor.read(cx).has_unsaved_changes());
        assert_eq!(
            cx.theme().background,
            gpui_kit::component::try_parse_color("#123456").unwrap()
        );
        app.update(cx, |view, cx| {
            view.open_theme_editor(window, cx);
            assert_eq!(view.tabs.entries.len(), 3);
            assert_eq!(view.tabs.theme_editor(), Some(&editor));
        });
        window.press("secondary-2", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(app.read(cx).tabs.active_handler().is_some());
        window.click(("close-tab", editor.entity_id()), cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("theme-continue").visible());
        window.click("theme-discard", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(app.read(cx).tabs.theme_editor().is_none());
        assert_eq!(cx.theme().background, before);
        assert!(app.read(cx).tabs.active_handler().is_some());
        window.press("secondary-9", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("shortcuts-editor").visible());
        window.press("escape", cx);
        assert_eq!(app.read(cx).tabs.entries.len(), 2);
        window.press("secondary-w", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(app.read(cx).tabs.entries.len(), 1);
        assert!(app.read(cx).tabs.active_handler().is_some());
    });
}

#[test]
fn vault_changes_and_closing_an_inactive_editor_preserve_the_selected_tab() {
    let mut cx = TestAppContext::single();
    let (handle, app) = workspace(&mut cx);
    cx.update_window(handle.into(), |_, window, cx| {
        app.update(cx, |view, cx| {
            view.open_file(PathBuf::from("docs/vault/Welcome.md"), false, window, cx);
            view.open_shortcuts(window, cx);
            let shortcuts = view.tabs.active().unwrap().entity_id();
            view.tabs
                .rename_path(Path::new("docs/vault"), Path::new("other"));
            assert_eq!(view.tabs.open_paths(), [PathBuf::from("other/Welcome.md")]);
            view.close_tabs_under(Path::new("other"), cx);
            assert_eq!(view.tabs.entries.len(), 2);
            assert_eq!(view.tabs.active().unwrap().entity_id(), shortcuts);
            view.close_tab(0, window, cx);
        });
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(app.read(cx).tabs.entries.len(), 1);
        assert!(window.find("shortcuts-editor").visible());
    })
    .unwrap();
}
