use std::path::PathBuf;

use gpui_kit::component::notification::Notification;
use gpui_kit::component::{Root, TitleBar};
use gpui_kit::{
    App, AppContext, BorrowAppContext, Bounds, Entity, Window, WindowBounds, WindowDecorations,
    WindowOptions, point, px, size,
};

use crate::app::AppState;
use crate::ui::DatalithView;

pub fn open_initial(
    cx: &App,
    first_startup: bool,
    initial_vault: Option<PathBuf>,
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
            let view = create_initial_view(
                first_startup,
                initial_vault,
                pending_notifications,
                window,
                cx,
            );
            cx.new(|cx| Root::new(view, window, cx))
        }) {
            eprintln!("Failed to open window: {error}");
        }
    })
    .detach();
}

fn create_initial_view(
    first_startup: bool,
    initial_vault: Option<PathBuf>,
    pending_notifications: Vec<Notification>,
    window: &mut Window,
    cx: &mut App,
) -> Entity<DatalithView> {
    let view = cx.new(|cx| DatalithView::new(first_startup, pending_notifications, window, cx));
    cx.update_global(|state: &mut AppState, _| state.view = Some(view.clone()));
    view.update(cx, |view, cx| {
        view.restore_initial_workspace(initial_vault, first_startup, window, cx);
    });
    let closing_view = view.downgrade();
    let window_id = window.window_handle().window_id();
    // Client-side title-bar controls remove the window directly, bypassing should-close.
    cx.on_window_closed(move |cx, closed_id| {
        if closed_id == window_id
            && let Some(view) = closing_view.upgrade()
            && let Err(error) = view.read(cx).save_workspace(cx)
        {
            eprintln!("Failed to save workspace when closing window: {error:#}");
        }
    })
    .detach();
    let quitting_view = view.downgrade();
    cx.on_app_quit(move |cx| {
        if let Some(view) = quitting_view.upgrade()
            && let Err(error) = view.read(cx).save_workspace(cx)
        {
            eprintln!("Failed to save workspace when quitting: {error:#}");
        }
        async {}
    })
    .detach();
    view
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    use gpui_kit::test::TestWindowExt as _;
    use gpui_kit::{TestAppContext, WindowHandle};

    use super::*;
    use crate::app::{actions, docs};
    use crate::document::handler::ViewMode;

    // Keep window-level coverage to the four paths that need real window state:
    // first launch, close/restore across vaults, per-tab keyboard mode, and a
    // failed save invalidating an older pending transition.
    struct VaultFixture(PathBuf);

    impl VaultFixture {
        fn new(label: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "datalith-window-{label}-{}-{}",
                std::process::id(),
                std::thread::current().name().unwrap().replace("::", "-")
            ));
            let _ = std::fs::remove_dir_all(&root);
            docs::seed_into(&root).unwrap();
            Self(root)
        }
    }

    impl Drop for VaultFixture {
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

    fn open(
        cx: &mut TestAppContext,
        root: &Path,
        first_startup: bool,
    ) -> (WindowHandle<Root>, Entity<DatalithView>) {
        let handle = cx.open_window(size(px(1000.), px(700.)), |window, cx| {
            let view =
                create_initial_view(first_startup, Some(root.to_owned()), Vec::new(), window, cx);
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
        cx.executor().advance_clock(Duration::from_millis(200));
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
            .unwrap();
        cx.executor().advance_clock(Duration::from_secs(6));
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
            .unwrap();
        (handle, view)
    }

    fn assert_modes(cx: &TestAppContext, view: &Entity<DatalithView>, modes: &[ViewMode]) {
        cx.update(|cx| {
            let view = view.read(cx);
            let actual = view
                .tabs
                .iter()
                .map(|(_, _, handler)| handler.read(cx).mode)
                .collect::<Vec<_>>();
            assert_eq!(actual, modes);
        });
    }

    fn wait_for_root(cx: &TestAppContext, view: &Entity<DatalithView>, expected: &Path) {
        let started = Instant::now();
        loop {
            cx.run_until_parked();
            if cx.update(|cx| view.read(cx).root_path.as_deref() == Some(expected)) {
                return;
            }
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "vault transition timed out"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn first_startup_opens_welcome_in_reading_mode() {
        let vault = VaultFixture::new("first-startup");
        let mut cx = app();
        let (_, view) = open(&mut cx, &vault.0, true);
        cx.update(|cx| {
            let view = view.read(cx);
            assert_eq!(view.root_path.as_ref(), Some(&vault.0));
            assert_eq!(view.tabs.open_paths(), vec![vault.0.join("Welcome.md")]);
            assert_eq!(
                view.tabs.active_path(),
                Some(vault.0.join("Welcome.md").as_path())
            );
        });
        assert_modes(&cx, &view, &[ViewMode::View]);
    }

    #[test]
    fn workspace_survives_vault_switch_and_window_restore_without_history() {
        let first = VaultFixture::new("switch-first");
        let second = VaultFixture(first.0.with_extension("second-vault"));
        docs::seed_into(&second.0).unwrap();
        let mut cx = app();
        let (handle, view) = open(&mut cx, &first.0, true);
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |view, cx| {
                view.open_file(first.0.join("Basics.md"), true, window, cx);
            });
        })
        .unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.click("toggle-mode", cx);
        })
        .unwrap();
        assert_modes(&cx, &view, &[ViewMode::View, ViewMode::Edit]);

        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |view, cx| {
                assert!(view.set_root_path(second.0.clone(), None, window, cx));
            });
        })
        .unwrap();
        wait_for_root(&cx, &view, &second.0);
        cx.update(|cx| assert!(view.read(cx).tabs.is_empty()));

        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |view, cx| {
                assert!(view.set_root_path(first.0.clone(), None, window, cx));
            });
        })
        .unwrap();
        wait_for_root(&cx, &view, &first.0);
        cx.update(|cx| {
            let view = view.read(cx);
            assert_eq!(
                view.tabs.open_paths(),
                vec![first.0.join("Welcome.md"), first.0.join("Basics.md")]
            );
            assert_eq!(view.tabs.active_index(), Some(1));
        });
        assert_modes(&cx, &view, &[ViewMode::View, ViewMode::Edit]);

        cx.update_window(handle.into(), |_, window, _| window.remove_window())
            .unwrap();
        drop(view);
        drop(cx);

        let mut reopened = app();
        let (_, restored) = open(&mut reopened, &first.0, false);
        reopened.update(|cx| {
            let restored = restored.read(cx);
            assert_eq!(
                restored.tabs.open_paths(),
                vec![first.0.join("Welcome.md"), first.0.join("Basics.md")]
            );
            assert_eq!(restored.tabs.active_index(), Some(1));
            assert!(!restored.can_go_back());
        });
        assert_modes(&reopened, &restored, &[ViewMode::View, ViewMode::Edit]);
    }

    #[test]
    fn keyboard_mode_change_only_changes_the_active_tab() {
        let vault = VaultFixture::new("keyboard-mode");
        let mut cx = app();
        let (handle, view) = open(&mut cx, &vault.0, true);
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |view, cx| {
                view.open_file(vault.0.join("Basics.md"), true, window, cx);
            });
            window.render_frame(cx);
            window.press("secondary-e", cx);
        })
        .unwrap();
        cx.run_until_parked();
        assert_modes(&cx, &view, &[ViewMode::View, ViewMode::Edit]);
    }

    #[test]
    fn failed_newer_save_preserves_active_workspace_and_cancels_pending_transition() {
        let current = VaultFixture::new("stale-current");
        let target = VaultFixture(current.0.with_extension("stale-target"));
        docs::seed_into(&target.0).unwrap();
        let newer = VaultFixture(current.0.with_extension("stale-newer"));
        docs::seed_into(&newer.0).unwrap();
        let mut cx = app();
        let (handle, view) = open(&mut cx, &current.0, true);
        let machine_id = crate::app::workspace::machine_id().unwrap();
        let workspace_dir = current.0.join(".datalith/workspace");
        let saved_workspace_dir = current.0.join(".datalith/workspace-preserved");
        let workspace_file = workspace_dir.join(format!("{machine_id}.json"));
        let before = cx.update(|cx| view.read(cx).tabs.snapshot(cx));
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |view, cx| {
                assert!(view.set_root_path(target.0.clone(), None, window, cx));
                let notifications_before = view.pending_notifications.len();
                assert!(workspace_file.is_file());
                let saved_bytes = std::fs::read(&workspace_file).unwrap();
                std::fs::rename(&workspace_dir, &saved_workspace_dir).unwrap();
                std::fs::write(&workspace_dir, "block workspace writes").unwrap();
                assert!(!view.set_root_path(newer.0.clone(), None, window, cx));
                assert_eq!(view.pending_notifications.len(), notifications_before + 1);
                assert_eq!(
                    std::fs::read(saved_workspace_dir.join(format!("{machine_id}.json"))).unwrap(),
                    saved_bytes
                );
            });
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            let view = view.read(cx);
            assert_eq!(view.root_path.as_ref(), Some(&current.0));
            assert_eq!(view.tabs.open_paths(), vec![current.0.join("Welcome.md")]);
            assert_eq!(view.tabs.snapshot(cx), before);
        });
        std::fs::remove_file(&workspace_dir).unwrap();
        std::fs::rename(&saved_workspace_dir, &workspace_dir).unwrap();
    }
}
