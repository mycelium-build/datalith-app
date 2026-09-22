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
    use std::time::Duration;

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
            Root::new(view, window, cx)
        });
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.press("escape", cx);
        })
        .unwrap();
        cx.run_until_parked();
        let view = cx.update(|cx| cx.global::<AppState>().view.clone().unwrap());
        cx.update(|cx| {
            if let Some(catalog) = &view.read(cx).vault_catalog {
                catalog.wait_until_ready(Duration::from_secs(5));
            }
        });
        // Let the startup driver and catalog poll publish their completed work.
        cx.executor().advance_clock(Duration::from_millis(200));
        cx.run_until_parked();
        cx.update(|cx| assert!(view.read(cx).startup.is_none()));
        cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
            .unwrap();
        // Indexing notifications cover the mode button until their normal expiry.
        cx.executor().advance_clock(Duration::from_secs(6));
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
            .unwrap();
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
    fn legacy_session_restores_active_note_after_a_file_disappears() {
        let docs = DocsFixture::new();
        let session = serde_json::from_value(serde_json::json!({
            "vault": docs.0,
            "tabs": [
                docs.0.join("deleted.md"),
                docs.0.join("Welcome.md"),
                docs.0.join("Basics.md"),
            ],
            "active_tab": docs.0.join("Basics.md"),
        }))
        .unwrap();
        settings::save_session(session).unwrap();
        settings::reload_from_disk();
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

    #[test]
    fn duplicate_tabs_and_active_occurrence_survive_restart() {
        let docs = DocsFixture::new();
        let mut cx = app();
        let (handle, view) = open(&mut cx, &docs.0);
        let welcome = docs.0.join("Welcome.md");
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |view, cx| {
                view.open_file(docs.0.join("Basics.md"), false, window, cx);
                view.open_file(welcome.clone(), true, window, cx);
                view.tabs.select(0);
                view.go_back(window, cx);
                assert_eq!(view.tabs.open_paths(), vec![welcome.clone(); 2]);
                view.tabs.select(1);
            });
        })
        .unwrap();
        cx.quit();
        drop(view);
        drop(cx);
        settings::reload_from_disk();
        let mut reopened = app();
        let (_, restored) = open(&mut reopened, &docs.0);
        reopened.update(|cx| {
            let restored = restored.read(cx);
            assert_eq!(restored.tabs.open_paths(), vec![welcome; 2]);
            assert_eq!(restored.tabs.active_index(), Some(1));
        });
        let saved = serde_json::to_value(settings::snapshot().session.unwrap()).unwrap();
        assert_eq!(saved["active_tab_index"], 1);
        assert!(saved.get("active_tab").is_none());
    }

    #[test]
    fn active_empty_tab_survives_restart_after_a_file_disappears() {
        let docs = DocsFixture::new();
        let mut cx = app();
        let (_, view) = open(&mut cx, &docs.0);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.new_empty_tab(cx);
                view.new_empty_tab(cx);
                assert_eq!(view.tabs.active_index(), Some(2));
            });
        });
        cx.quit();
        drop(view);
        drop(cx);
        std::fs::remove_file(docs.0.join("Welcome.md")).unwrap();
        settings::reload_from_disk();
        let mut reopened = app();
        let (_, restored) = open(&mut reopened, &docs.0);
        reopened.update(|cx| {
            let restored = restored.read(cx);
            assert_eq!(restored.tabs.open_paths(), vec![PathBuf::new(); 2]);
            assert_eq!(restored.tabs.active_index(), Some(1));
        });
    }

    #[test]
    fn mode_change_survives_settings_write_failure() {
        let docs = DocsFixture::new();
        let mut cx = app();
        let (handle, view) = open(&mut cx, &docs.0);
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |view, cx| {
                view.open_file(docs.0.join("Basics.md"), true, window, cx);
            });
        })
        .unwrap();
        let config = std::env::temp_dir().join(format!(
            "datalith-settings-ui-{}-{}.json",
            std::process::id(),
            std::thread::current().name().unwrap().replace("::", "-")
        ));
        std::fs::remove_file(&config).unwrap();
        std::fs::create_dir(&config).unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.press("secondary-e", cx);
        })
        .unwrap();
        cx.run_until_parked();
        std::fs::remove_dir(&config).unwrap();
        assert_mode(&cx, &view, ViewMode::Edit);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(
                Root::read(window, cx)
                    .notification
                    .read(cx)
                    .notifications()
                    .len(),
                1
            );
            view.update(cx, |view, cx| {
                view.open_file(docs.0.join("Search.md"), true, window, cx);
            });
        })
        .unwrap();
        assert_mode(&cx, &view, ViewMode::Edit);
    }

    #[test]
    fn base_tabs_keep_their_vault_catalog_after_restart() {
        let docs = DocsFixture::new();
        let other = DocsFixture(docs.0.with_extension("other-vault"));
        docs::seed_into(&other.0).unwrap();
        let mut cx = app();
        let (handle, view) = open(&mut cx, &docs.0);
        let bases = [
            docs.0.join("Overview.base"),
            docs.0.join("examples/bases/Library.base"),
        ];
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |view, cx| {
                for path in &bases {
                    view.open_file(path.clone(), true, window, cx);
                }
                view.set_root_path(other.0.clone(), cx);
            });
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            view.read(cx)
                .vault_catalog
                .as_ref()
                .unwrap()
                .wait_until_ready(std::time::Duration::from_secs(5));
            for path in &bases {
                let handler = view.read(cx).tabs.handler_for_path(path).unwrap();
                assert_eq!(handler.read(cx).vault_catalog(cx).unwrap().root(), docs.0);
            }
        });
        cx.quit();
        drop(view);
        drop(cx);
        std::fs::remove_file(docs.0.join("Welcome.md")).unwrap();
        settings::reload_from_disk();

        let mut reopened = app();
        let (_, restored) = open(&mut reopened, &docs.0);
        reopened
            .executor()
            .advance_clock(std::time::Duration::from_secs(1));
        reopened.run_until_parked();
        reopened.update(|cx| {
            assert_eq!(restored.read(cx).root_path.as_ref(), Some(&other.0));
            for path in &bases {
                let handler = restored.read(cx).tabs.handler_for_path(path).unwrap();
                let catalog = handler
                    .read(cx)
                    .vault_catalog(cx)
                    .expect("restored Base must retain its Vault catalog");
                catalog.wait_until_ready(std::time::Duration::from_secs(5));
                assert_eq!(catalog.root(), docs.0);
                assert!(catalog.paths().contains(&docs.0.join("Basics.md")));
            }
            restored.update(cx, |view, cx| view.set_root_path(docs.0.clone(), cx));
        });
        reopened.run_until_parked();
        reopened
            .executor()
            .advance_clock(std::time::Duration::from_secs(1));
        reopened.run_until_parked();
        reopened.update(|cx| {
            let catalog = restored
                .read(cx)
                .vault_catalog
                .as_ref()
                .expect("returning to the Base Vault must reuse its open catalog");
            assert_eq!(catalog.root(), docs.0);
            assert_eq!(catalog.state(), crate::vault::CatalogState::Ready);
        });
    }

    #[test]
    fn pending_base_catalog_survives_saving_and_switching_vaults() {
        let docs = DocsFixture::new();
        let other = DocsFixture(docs.0.with_extension("other-vault"));
        docs::seed_into(&other.0).unwrap();
        let base = docs.0.join("Overview.base");
        let mut session = Session {
            vault: Some(docs.0.clone()),
            tabs: vec![base.clone()],
            ..Session::default()
        };
        session.tab_vaults.insert(0, docs.0.clone());
        let mut cx = app();
        let handle = cx.open_window(size(px(1000.), px(700.)), |window, cx| {
            let view = create_initial_view(false, session, Vec::new(), window, cx);
            view.update(cx, |view, cx| {
                let handler = view.tabs.handler_for_path(&base).unwrap();
                assert!(handler.read(cx).vault_catalog(cx).is_none());
                view.save_session(cx);
                assert_eq!(
                    settings::snapshot().session.unwrap().tab_vaults.get(&0),
                    Some(&docs.0)
                );
                view.set_root_path(other.0.clone(), cx);
                view.set_root_path(docs.0.clone(), cx);
            });
            Root::new(view, window, cx)
        });
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.press("escape", cx);
        })
        .unwrap();
        cx.run_until_parked();
        let view = cx.update(|cx| cx.global::<AppState>().view.clone().unwrap());
        cx.update(|cx| {
            let catalog = view
                .read(cx)
                .vault_catalog
                .as_ref()
                .expect("pending restore must finish after switching away and back");
            catalog.wait_until_ready(std::time::Duration::from_secs(5));
            assert_eq!(catalog.state(), crate::vault::CatalogState::Ready);
            let handler = view.read(cx).tabs.handler_for_path(&base).unwrap();
            assert_eq!(handler.read(cx).vault_catalog(cx).unwrap().root(), docs.0);
        });
        cx.executor()
            .advance_clock(std::time::Duration::from_secs(1));
        cx.run_until_parked();
        cx.update(|cx| assert!(view.read(cx).vault_db_ready_notified));
    }

    #[test]
    fn missing_base_vault_is_preserved_without_recreating_the_folder() {
        let docs = DocsFixture::new();
        let missing = docs.0.join("missing-vault");
        let mut session = Session {
            vault: Some(docs.0.clone()),
            tabs: vec![docs.0.join("Overview.base")],
            ..Session::default()
        };
        session.tab_vaults.insert(0, missing.clone());
        settings::save_session(session).unwrap();
        let mut cx = app();
        let (_, view) = open(&mut cx, &docs.0);
        assert!(!missing.exists());
        cx.update(|cx| {
            let handler = view.read(cx).tabs.active_handler().unwrap();
            assert!(handler.read(cx).vault_catalog(cx).is_none());
            view.read(cx).save_session(cx);
            assert_eq!(
                settings::snapshot().session.unwrap().tab_vaults.get(&0),
                Some(&missing)
            );
        });
    }
}
