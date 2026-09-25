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
    use crate::app::{actions, docs, settings};
    use crate::document::handler::ViewMode;

    // Keep window-level coverage focused on first launch and menu navigation,
    // workspace restore, per-tab keyboard mode, and transition/close failures.
    struct VaultFixture(PathBuf);

    impl VaultFixture {
        fn new(label: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "datalith-window-{label}-{}-{}",
                std::process::id(),
                std::thread::current().name().unwrap().replace("::", "-")
            ));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).unwrap();
            std::fs::write(root.join("Welcome.md"), "# Welcome\n").unwrap();
            std::fs::write(root.join("Basics.md"), "# Basics\n").unwrap();
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
    fn first_startup_opens_embedded_welcome_in_reading_mode() {
        let vault = docs::docs_vault_path();
        let mut cx = app();
        settings::set_open_new_tab_mode(ViewMode::Edit).unwrap();
        let (handle, view) = open(&mut cx, &vault, true);
        cx.update(|cx| {
            let view = view.read(cx);
            assert_eq!(view.root_path.as_ref(), Some(&vault));
            assert_eq!(view.tabs.open_paths(), vec![vault.join(docs::WELCOME_NOTE)]);
            assert_eq!(
                view.tabs.active_path(),
                Some(vault.join(docs::WELCOME_NOTE).as_path())
            );
            assert!(settings::snapshot().onboarding_complete);
        });
        assert!(!vault.exists());
        assert_modes(&cx, &view, &[ViewMode::View]);
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |view, cx| {
                view.open_file(vault.join("Basics.md"), true, window, cx);
            });
        })
        .unwrap();
        cx.update(|cx| {
            let view = view.read(cx);
            assert!(
                !view
                    .tabs
                    .active_handler()
                    .is_some_and(|handler| handler.read(cx).can_toggle_mode())
            );
        });
        assert_modes(&cx, &view, &[ViewMode::View, ViewMode::View]);

        let personal = VaultFixture::new("documentation-menu-navigation");
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |view, cx| {
                assert!(view.set_root_path(personal.0.clone(), None, window, cx));
            });
        })
        .unwrap();
        wait_for_root(&cx, &view, &personal.0);

        cx.update_window(handle.into(), |_, window, cx| {
            window.click("vault-selector", cx);
            window.render_frame(cx);
            let mut menu = window.within("popup-menu");
            assert!(menu.try_find(0_usize).is_some());
            menu.click(0_usize, cx);
            window.render_frame(cx);
        })
        .unwrap();
        wait_for_root(&cx, &view, &vault);
        cx.update(|cx| {
            let view = view.read(cx);
            assert_eq!(view.root_path.as_ref(), Some(&vault));
            assert_eq!(
                view.tabs.open_paths(),
                vec![vault.join(docs::WELCOME_NOTE), vault.join("Basics.md")]
            );
        });
        assert_modes(&cx, &view, &[ViewMode::View, ViewMode::View]);
    }

    #[test]
    fn workspace_survives_vault_switch_and_window_restore_without_history() {
        let first = VaultFixture::new("switch-first");
        let second = VaultFixture(first.0.with_extension("second-vault"));
        std::fs::create_dir_all(&second.0).unwrap();
        std::fs::write(second.0.join("Welcome.md"), "# Welcome\n").unwrap();
        std::fs::write(second.0.join("Basics.md"), "# Basics\n").unwrap();
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
    fn removing_vaults_preserves_files_and_active_workspace_on_save_failure() {
        let active = VaultFixture::new("close-active");
        let inactive = VaultFixture::new("close-inactive");
        let inactive_saved_state = inactive.0.join(".datalith/workspace/preserve.json");
        std::fs::create_dir_all(inactive_saved_state.parent().unwrap()).unwrap();
        std::fs::write(&inactive_saved_state, "keep this workspace").unwrap();

        let mut cx = app();
        let (handle, view) = open(&mut cx, &active.0, true);
        settings::record_opened_vault(&inactive.0).unwrap();
        settings::record_opened_vault(&active.0).unwrap();
        let before = cx.update(|cx| view.read(cx).tabs.snapshot(cx));

        let remove_inactive_id = format!("remove-vault-{}", inactive.0.display());
        cx.update_window(handle.into(), |_, window, cx| {
            window.click("vault-selector", cx);
            window.render_frame(cx);
            assert_close_button_in_menu(window, remove_inactive_id.clone());
            window.click(remove_inactive_id.clone(), cx);
            window.render_frame(cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            let view = view.read(cx);
            assert_eq!(view.root_path.as_ref(), Some(&active.0));
            assert_eq!(view.tabs.snapshot(cx), before);
            assert_eq!(settings::snapshot().recent_vaults, vec![active.0.clone()]);
        });
        assert!(inactive.0.is_dir());
        assert_eq!(
            std::fs::read_to_string(&inactive_saved_state).unwrap(),
            "keep this workspace"
        );

        let machine_id = crate::app::workspace::machine_id().unwrap();
        let workspace_dir = active.0.join(".datalith/workspace");
        let preserved_dir = active.0.join(".datalith/workspace-preserved");
        let workspace_file = workspace_dir.join(format!("{machine_id}.json"));
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |view, cx| {
                assert!(view.set_root_path(inactive.0.clone(), None, window, cx));
            });
        })
        .unwrap();

        let saved_bytes = std::fs::read(&workspace_file).unwrap();
        std::fs::rename(&workspace_dir, &preserved_dir).unwrap();
        std::fs::write(&workspace_dir, "block workspace writes").unwrap();
        assert!(
            !cx.update(
                |cx| view.update(cx, |view, cx| { view.close_personal_vault(&active.0, cx) })
            )
        );
        cx.run_until_parked();
        cx.update(|cx| {
            let view = view.read(cx);
            assert_eq!(view.root_path.as_ref(), Some(&active.0));
            assert_eq!(view.tabs.snapshot(cx), before);
            assert_eq!(settings::snapshot().recent_vaults, vec![active.0.clone()]);
        });
        assert_eq!(
            std::fs::read(preserved_dir.join(format!("{machine_id}.json"))).unwrap(),
            saved_bytes
        );

        std::fs::remove_file(&workspace_dir).unwrap();
        std::fs::rename(&preserved_dir, &workspace_dir).unwrap();
        let remove_active_id = format!("remove-vault-{}", active.0.display());
        cx.update_window(handle.into(), |_, window, cx| {
            window.click("vault-selector", cx);
            window.render_frame(cx);
            window.press("tab", cx);
            window.render_frame(cx);
            assert_eq!(window.find(remove_active_id.clone()).focused(), Some(true));
        })
        .unwrap();
        cx.run_until_parked();
        activate_close_button_with_keyboard(&cx, handle);
        cx.update_window(handle.into(), |_, window, cx| {
            assert!(
                view.read(cx).root_path.is_none(),
                "keyboard did not remove active vault"
            );
            window.render_frame(cx);
            assert!(view.read(cx).sidebar_focus_handle.is_focused(window));
            assert!(
                !window
                    .try_find("popup-menu")
                    .is_some_and(|popup| popup.visible())
            );
        })
        .unwrap();
        cx.update(|cx| {
            let view = view.read(cx);
            assert_eq!(view.root_path, None);
            assert!(view.tabs.is_empty());
            assert!(settings::snapshot().recent_vaults.is_empty());
            assert_eq!(settings::snapshot().last_vault, None);
            assert!(settings::snapshot().onboarding_complete);
        });
        assert!(workspace_file.is_file());
    }

    fn assert_close_button_in_menu(window: &Window, button_id: String) {
        let menu_bounds = window.find("popup-menu").bounds();
        let close_bounds = window.find(button_id).bounds();
        assert!(
            close_bounds.origin.x >= menu_bounds.origin.x
                && close_bounds.origin.y >= menu_bounds.origin.y
                && close_bounds.right() <= menu_bounds.right()
                && close_bounds.bottom() <= menu_bounds.bottom(),
            "close button bounds {close_bounds:?} escaped popup bounds {menu_bounds:?}"
        );
    }

    fn activate_close_button_with_keyboard(cx: &TestAppContext, handle: WindowHandle<Root>) {
        let mut visual = gpui_kit::VisualTestContext::from_window(handle.into(), cx);
        let keystroke = gpui_kit::Keystroke::parse("space").unwrap();
        visual.simulate_event(gpui_kit::KeyDownEvent {
            keystroke: keystroke.clone(),
            is_held: false,
            prefer_character_input: false,
        });
        visual.simulate_event(gpui_kit::KeyUpEvent { keystroke });
    }

    #[test]
    fn failed_newer_save_preserves_active_workspace_and_cancels_pending_transition() {
        let current = VaultFixture::new("stale-current");
        let target = VaultFixture(current.0.with_extension("stale-target"));
        std::fs::create_dir_all(&target.0).unwrap();
        std::fs::write(target.0.join("Welcome.md"), "# Welcome\n").unwrap();
        std::fs::write(target.0.join("Basics.md"), "# Basics\n").unwrap();
        let newer = VaultFixture(current.0.with_extension("stale-newer"));
        std::fs::create_dir_all(&newer.0).unwrap();
        std::fs::write(newer.0.join("Welcome.md"), "# Welcome\n").unwrap();
        std::fs::write(newer.0.join("Basics.md"), "# Basics\n").unwrap();
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
