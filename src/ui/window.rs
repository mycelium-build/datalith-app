use gpui_kit::component::notification::Notification;
use gpui_kit::component::{Root, TitleBar};
use gpui_kit::{
    App, AppContext, BorrowAppContext, Bounds, Entity, Window, WindowBounds, WindowDecorations,
    WindowOptions, point, px, size,
};

use crate::app::AppState;
use crate::app::session::Session;
use crate::ui::DatalithView;

pub fn open_initial(
    cx: &App,
    first_startup: bool,
    session: Session,
    pending_notifications: Vec<Notification>,
) {
    cx.spawn(async move |cx| {
        let options = WindowOptions {
            app_id: Some(crate::channel::Channel::current().stem().into()),
            window_bounds: Some(WindowBounds::Maximized(Bounds::new(
                point(px(0.0), px(0.0)),
                size(px(1440.0), px(900.0)),
            ))),
            window_min_size: Some(size(px(800.0), px(480.0))),
            window_decorations: Some(WindowDecorations::Client),
            ..TitleBar::window_options()
        };
        if let Err(error) = cx.open_window(options, |window, cx| {
            window.set_window_title(crate::channel::Channel::current().product_name());
            let view =
                create_initial_view(first_startup, session, pending_notifications, window, cx);
            cx.new(|cx| Root::new(view, window, cx))
        }) {
            eprintln!("Failed to open window: {error}");
        }
    })
    .detach();
}

fn create_initial_view(
    first_startup: bool,
    session: Session,
    pending_notifications: Vec<Notification>,
    window: &mut Window,
    cx: &mut App,
) -> Entity<DatalithView> {
    let view = cx.new(|cx| DatalithView::new(first_startup, pending_notifications, window, cx));
    cx.update_global(|state: &mut AppState, _| state.view = Some(view.clone()));
    view.update(cx, |view, cx| view.restore_session(session, window, cx));
    let closing_view = view.downgrade();
    let window_id = window.window_handle().window_id();
    // Client-side title-bar controls remove the window directly, bypassing should-close.
    cx.on_window_closed(move |cx, closed_id| {
        if closed_id == window_id
            && let Some(view) = closing_view.upgrade()
        {
            view.read(cx).save_session(cx);
        }
    })
    .detach();
    let quitting_view = view.downgrade();
    cx.on_app_quit(move |cx| {
        if let Some(view) = quitting_view.upgrade() {
            view.read(cx).save_session(cx);
        }
        async {}
    })
    .detach();
    view
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use gpui_kit::test::TestWindowExt as _;
    use gpui_kit::{TestAppContext, WindowHandle};

    use super::*;
    use crate::app::{actions, docs, settings};
    use crate::document::handler::ViewMode;

    struct DocsFixture(PathBuf);

    impl DocsFixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "datalith-session-{}-{}",
                std::process::id(),
                std::thread::current().name().unwrap().replace("::", "-")
            ));
            let _ = std::fs::remove_dir_all(&root);
            docs::seed_into(&root).unwrap();
            Self(root)
        }
    }

    impl Drop for DocsFixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn app() -> TestAppContext {
        let cx = TestAppContext::single();
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::settings::SettingsView::init_theme_options(cx);
            cx.set_global(AppState::default());
            actions::register(cx);
            crate::app::keymap::register(cx);
        });
        cx
    }

    fn open(cx: &mut TestAppContext, docs: &Path) -> (WindowHandle<Root>, Entity<DatalithView>) {
        let (first, session) = Session::initial(&settings::snapshot(), Some(docs.to_owned()));
        let handle = cx.open_window(size(px(1000.), px(700.)), |window, cx| {
            let view = create_initial_view(first, session, Vec::new(), window, cx);
            // Hold the startup overlay until indexing is ready, then test the workspace.
            view.update(cx, |view, _| {
                view.startup_driver = gpui_kit::Task::ready(());
            });
            Root::new(view, window, cx)
        });
        cx.run_until_parked();
        let view = cx.update(|cx| cx.global::<AppState>().view.clone().unwrap());
        cx.update(|cx| {
            if let Some(catalog) = &view.read(cx).vault_catalog {
                catalog.wait_until_ready(std::time::Duration::from_secs(5));
            }
            // Indexing notifications can cover the mode button during these short tests.
            view.update(cx, |view, _| {
                view.catalog_poll_task = gpui_kit::Task::ready(());
                view.pending_notifications.clear();
                view.startup = None;
                view.focus_editor_requested = true;
            });
        });
        (handle, view)
    }

    fn assert_mode(cx: &TestAppContext, view: &Entity<DatalithView>, mode: ViewMode) {
        cx.update(|cx| {
            let view = view.read(cx);
            assert!(!view.tabs.is_empty());
            for (_, _, handler) in view.tabs.iter() {
                assert_eq!(handler.read(cx).mode, mode);
            }
        });
    }

    #[test]
    fn first_startup_opens_welcome_in_reading_mode() {
        let docs = DocsFixture::new();
        let mut cx = app();
        let (_, view) = open(&mut cx, &docs.0);
        cx.update(|cx| {
            let view = view.read(cx);
            assert_eq!(view.root_path.as_ref(), Some(&docs.0));
            assert_eq!(view.tabs.open_paths(), vec![docs.0.join("Welcome.md")]);
            assert_eq!(
                view.tabs.active_path(),
                Some(docs.0.join("Welcome.md").as_path())
            );
        });
        assert_mode(&cx, &view, ViewMode::View);
    }

    #[test]
    fn closing_and_reopening_restores_workspace_and_global_mode() {
        let docs = DocsFixture::new();
        let mut cx = app();
        let (handle, view) = open(&mut cx, &docs.0);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click("toggle-mode", cx);
        })
        .unwrap();
        cx.run_until_parked();
        assert_mode(&cx, &view, ViewMode::Edit);
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |view, cx| {
                view.open_file(docs.0.join("Basics.md"), true, window, cx);
                view.open_file(docs.0.join("Search.md"), true, window, cx);
                view.tabs.select(1);
                let folder = docs.0.join("formats").to_string_lossy().into_owned().into();
                view.expand_tree_item(&folder, cx);
            });
        })
        .unwrap();
        assert_mode(&cx, &view, ViewMode::Edit);
        // The custom title-bar close button uses this same window lifecycle.
        cx.update_window(handle.into(), |_, window, _| window.remove_window())
            .unwrap();
        drop(view);
        drop(cx);
        settings::reload_from_disk();
        assert_eq!(settings::snapshot().document_mode, ViewMode::Edit);
        let mut reopened = app();
        let (_, restored) = open(&mut reopened, &docs.0);
        reopened.update(|cx| {
            let restored = restored.read(cx);
            assert_eq!(restored.root_path.as_ref(), Some(&docs.0));
            assert_eq!(
                restored.tabs.open_paths(),
                vec![
                    docs.0.join("Welcome.md"),
                    docs.0.join("Basics.md"),
                    docs.0.join("Search.md")
                ]
            );
            assert_eq!(
                restored.tabs.active_path(),
                Some(docs.0.join("Basics.md").as_path())
            );
            assert!(
                restored
                    .expanded_tree_ids
                    .iter()
                    .any(|id| Path::new(id.as_str()) == docs.0.join("formats"))
            );
        });
        assert_mode(&reopened, &restored, ViewMode::Edit);
    }

    #[test]
    fn keyboard_mode_change_applies_to_other_tabs_and_future_files() {
        let docs = DocsFixture::new();
        let mut cx = app();
        let (handle, view) = open(&mut cx, &docs.0);
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |view, cx| {
                view.open_file(docs.0.join("Basics.md"), true, window, cx);
            });
            window.render_frame(cx);
            window.press("secondary-e", cx);
        })
        .unwrap();
        cx.run_until_parked();
        assert_mode(&cx, &view, ViewMode::Edit);
        cx.update_window(handle.into(), |_, window, cx| {
            window.press("secondary-e", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |view, cx| {
                view.open_file(docs.0.join("Search.md"), true, window, cx);
            });
        })
        .unwrap();
        assert_mode(&cx, &view, ViewMode::View);
        settings::reload_from_disk();
        assert_eq!(settings::snapshot().document_mode, ViewMode::View);
    }

    #[test]
    fn quitting_with_no_tabs_restores_an_empty_workspace() {
        let docs = DocsFixture::new();
        let mut cx = app();
        let (_, view) = open(&mut cx, &docs.0);
        cx.update(|cx| view.update(cx, DatalithView::close_active_tab));
        cx.quit();
        drop(view);
        drop(cx);
        settings::reload_from_disk();
        assert_eq!(
            settings::snapshot()
                .session
                .as_ref()
                .map(|session| session.tabs.len()),
            Some(0)
        );
        let (first, session) = Session::initial(&settings::snapshot(), Some(docs.0.clone()));
        assert!(!first);
        assert!(session.tabs.is_empty());
        let mut reopened = app();
        let (_, restored) = open(&mut reopened, &docs.0);
        reopened.update(|cx| assert!(restored.read(cx).tabs.is_empty()));
    }

    #[test]
    fn missing_files_do_not_change_the_restored_active_note() {
        let docs = DocsFixture::new();
        settings::save_session(Session {
            vault: Some(docs.0.clone()),
            tabs: vec![
                docs.0.join("deleted.md"),
                docs.0.join("Welcome.md"),
                docs.0.join("Basics.md"),
            ],
            active_tab: Some(docs.0.join("Basics.md")),
            ..Session::default()
        })
        .unwrap();
        let mut cx = app();
        let (_, view) = open(&mut cx, &docs.0);
        cx.update(|cx| {
            let view = view.read(cx);
            assert_eq!(
                view.tabs.open_paths(),
                vec![docs.0.join("Welcome.md"), docs.0.join("Basics.md")]
            );
            assert_eq!(
                view.tabs.active_path(),
                Some(docs.0.join("Basics.md").as_path())
            );
        });
    }
}
