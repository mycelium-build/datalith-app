// The gpui `actions!` macro generates unit structs deriving `PartialEq` without `Eq`;
// that lint can only be suppressed here, not in the macro itself.
#![allow(clippy::derive_partial_eq_without_eq)]

use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{App, AppContext, PathPromptOptions, SharedString, actions};
use std::path::PathBuf;

use crate::app::{AppState, settings::ThemePreference, system};
use crate::document::handler::FileHandlerEvent;
use crate::ui::palette::PaletteKind;
use crate::ui::tabs::NavigationAction;
use crate::ui::{notifications, settings::DOCS_URL};
use crate::vault::CatalogState;
use crate::vault::file_ops;

actions!(
    datalith,
    [
        OpenVault,
        ToggleSearch,
        ClosePalette,
        ToggleQuickSwitcher,
        NewFile,
        NewFolder,
        Rename,
        Delete,
        Duplicate,
        OpenInExplorer,
        CopyPath,
        CloseTab,
        NewTab,
        FocusSidebar,
        ToggleTheme,
        OpenSettings,
        OpenThemeEditor,
        CheckForUpdates,
        OpenShortcuts,
        OpenDocumentation,
        OpenAbout,
        OpenLicenses,
        OpenSource,
        SelectTab1,
        SelectTab2,
        SelectTab3,
        SelectTab4,
        SelectTab5,
        SelectTab6,
        SelectTab7,
        SelectTab8,
        SelectLastTab,
        ToggleEditorMode,
        GoBack,
        GoForward,
        OpenLink,
        Quit,
    ]
);

macro_rules! with_view {
    ($cx:expr, |$view:ident, $cx2:ident| $body:block) => {
        if let Some($view) = $cx.read_global(|state: &AppState, _| state.view.clone()) {
            $view.update($cx, |$view, $cx2| $body);
        }
    };
}

pub fn register(cx: &mut App) {
    cx.on_action(quit);
    cx.on_action(check_for_updates);
    cx.on_action(open_vault);
    cx.on_action(toggle_search);
    cx.on_action(toggle_quick_switcher);
    cx.on_action(close_palette);
    cx.on_action(handle_new_file);
    cx.on_action(handle_new_folder);
    cx.on_action(handle_rename);
    cx.on_action(handle_delete);
    cx.on_action(handle_duplicate);
    cx.on_action(handle_open_in_explorer);
    cx.on_action(handle_copy_path);
    cx.on_action(handle_close_tab);
    cx.on_action(handle_new_tab);
    cx.on_action(handle_focus_sidebar);
    cx.on_action(toggle_theme);
    cx.on_action(handle_select_tab_1);
    cx.on_action(handle_select_tab_2);
    cx.on_action(handle_select_tab_3);
    cx.on_action(handle_select_tab_4);
    cx.on_action(handle_select_tab_5);
    cx.on_action(handle_select_tab_6);
    cx.on_action(handle_select_tab_7);
    cx.on_action(handle_select_tab_8);
    cx.on_action(handle_select_last_tab);
    cx.on_action(open_settings);
    cx.on_action(open_theme_editor);
    cx.on_action(open_shortcuts);
    cx.on_action(open_documentation);
    cx.on_action(open_about);
    cx.on_action(open_licenses);
    cx.on_action(open_source);
    cx.on_action(toggle_editor_mode);
    cx.on_action(go_back);
    cx.on_action(go_forward);
    cx.on_action(handle_open_link);
}

fn quit(_: &Quit, cx: &mut App) {
    cx.quit();
}

pub fn open_vault(_: &OpenVault, cx: &mut App) {
    let rx = cx.prompt_for_paths(PathPromptOptions {
        files: false,
        directories: true,
        multiple: false,
        prompt: Some("Select a folder".into()),
    });
    cx.spawn(async move |cx| {
        if let Ok(Ok(Some(paths))) = rx.await
            && let Some(path) = paths.into_iter().next()
        {
            let view_opt = cx.read_global(|state: &AppState, _| state.view.clone());
            if let Some(view) = view_opt {
                cx.update_entity(&view, |view, cx| {
                    view.set_root_path(path, cx);
                });
            }
        }
    })
    .detach();
}

pub fn toggle_search(_: &ToggleSearch, cx: &mut App) {
    with_view!(cx, |view, cx| {
        if view.palette.open {
            view.palette.close();
        } else {
            view.palette.open_as(PaletteKind::Search);
            let query = view.palette.search_query.clone();
            if !query.trim().is_empty() {
                view.palette.search(view.vault_catalog.as_ref(), &query);
            }
        }
        cx.notify();
    });
}

pub fn toggle_quick_switcher(_: &ToggleQuickSwitcher, cx: &mut App) {
    with_view!(cx, |view, cx| {
        if view.palette.open {
            view.palette.close();
        } else {
            view.palette.open_as(PaletteKind::QuickSwitcher);
            let open = view.tabs.open_paths();
            let query = view.palette.qs_query.clone();
            view.palette
                .refresh_quick_switcher(view.vault_catalog.as_ref(), &open, &query);
        }
        cx.notify();
    });
}

pub fn close_palette(_: &ClosePalette, cx: &mut App) {
    with_view!(cx, |view, cx| {
        view.palette.close();
        cx.notify();
    });
}

pub fn handle_new_file(_: &NewFile, cx: &mut App) {
    with_view!(cx, |view, cx| {
        view.commit_rename(cx);
        let target = view
            .context_menu_target
            .take()
            .or_else(|| view.resolve_target(cx))
            .or_else(|| view.root_path.clone());
        if let Some(target) = target
            && let Ok(created) = file_ops::create(&target)
        {
            if target.is_dir() {
                let id: SharedString = target.to_string_lossy().to_string().into();
                view.mark_tree_item_expanded(&id, true);
            }
            view.refresh_tree(cx);
            view.rename_target = Some(created.clone());
            view.pending_open = Some(created);
        }
        cx.notify();
    });
}

pub fn handle_new_folder(_: &NewFolder, cx: &mut App) {
    with_view!(cx, |view, cx| {
        view.commit_rename(cx);
        let target = view
            .context_menu_target
            .take()
            .or_else(|| view.resolve_target(cx))
            .or_else(|| view.root_path.clone());
        if let Some(target) = target
            && let Ok(created) = file_ops::create_folder(&target)
        {
            view.refresh_tree(cx);
            view.rename_target = Some(created);
        }
        cx.notify();
    });
}

pub fn handle_rename(_: &Rename, cx: &mut App) {
    with_view!(cx, |view, cx| {
        let catalog_blocked = view
            .vault_catalog
            .as_ref()
            .is_none_or(|c| c.state() != CatalogState::Ready);
        if catalog_blocked {
            view.pending_notifications
                .push(notifications::rename_while_loading());
            cx.notify();
            return;
        }
        view.commit_rename(cx);
        let target = view
            .context_menu_target
            .take()
            .or_else(|| view.resolve_target(cx));
        if let Some(target) = target {
            view.rename_target = Some(target);
        }
        cx.notify();
    });
}

pub fn handle_delete(_: &Delete, cx: &mut App) {
    with_view!(cx, |view, cx| {
        let target_index = view.tree_state.read(cx).selected_index();
        let target = view
            .context_menu_target
            .take()
            .or_else(|| view.resolve_target(cx));
        view.commit_rename(cx);
        if let Some(target) = target {
            if let Err(e) = file_ops::delete(&target) {
                eprintln!("{e}");
                cx.notify();
                return;
            }
            view.close_tabs_under(&target, cx);
            view.refresh_tree(cx);
            let count = view.visible_tree_entry_count();
            if count > 0 {
                let new_ix = target_index.unwrap_or(0).min(count.saturating_sub(1));
                view.tree_state.update(cx, |state, cx| {
                    state.set_selected_index(Some(new_ix), cx);
                });
                if let Some(entry) = view.tree_state.read(cx).selected_entry() {
                    view.last_sidebar_selection = Some(PathBuf::from(entry.item().id.to_string()));
                }
            }
            view.focus_sidebar_requested = true;
        }
        cx.notify();
    });
}

pub fn handle_duplicate(_: &Duplicate, cx: &mut App) {
    with_view!(cx, |view, cx| {
        view.commit_rename(cx);
        let target = view
            .context_menu_target
            .take()
            .or_else(|| view.resolve_target(cx));
        if let Some(target) = target {
            if let Ok(duplicated) = file_ops::duplicate(&target)
                && duplicated.is_file()
            {
                view.pending_open = Some(duplicated);
            }
            view.refresh_tree(cx);
        }
        cx.notify();
    });
}

pub fn handle_open_in_explorer(_: &OpenInExplorer, cx: &mut App) {
    with_view!(cx, |view, cx| {
        let target = view
            .context_menu_target
            .take()
            .or_else(|| view.resolve_target(cx));
        if let Some(target) = target
            && let Err(error) = system::reveal_in_file_manager(&target)
        {
            notifications::push_window_notification(
                cx,
                notifications::reveal_in_file_manager_failed(&target, &error),
            );
        }
        cx.notify();
    });
}

pub fn handle_copy_path(_: &CopyPath, cx: &mut App) {
    with_view!(cx, |view, cx| {
        let target = view
            .context_menu_target
            .take()
            .or_else(|| view.resolve_target(cx));
        if let Some(target) = target
            && let Err(error) = system::copy_path(&target)
        {
            notifications::push_window_notification(
                cx,
                notifications::copy_path_failed(&target, &error),
            );
        }
        cx.notify();
    });
}

pub fn handle_close_tab(_: &CloseTab, cx: &mut App) {
    with_workspace_window(crate::ui::DatalithView::close_active_tab, cx);
}

pub fn handle_new_tab(_: &NewTab, cx: &mut App) {
    with_view!(cx, |view, cx| {
        view.new_empty_tab(cx);
        cx.notify();
    });
}

pub fn toggle_editor_mode(_: &ToggleEditorMode, cx: &mut App) {
    with_view!(cx, |view, cx| {
        if let Some(handler) = view.tabs.active_handler().cloned() {
            handler.update(cx, |handler, cx| {
                handler.toggle_editing(cx);
            });
            view.focus_editor_requested = true;
        }
        cx.notify();
    });
}

pub fn handle_focus_sidebar(_: &FocusSidebar, cx: &mut App) {
    with_view!(cx, |view, cx| {
        view.ensure_sidebar_selection(cx);
        view.focus_sidebar_requested = true;
        cx.notify();
    });
}

pub fn toggle_theme(_: &ToggleTheme, cx: &mut App) {
    let current_mode = Theme::global(cx).mode;
    let new_mode = match current_mode {
        ThemeMode::Light => ThemeMode::Dark,
        ThemeMode::Dark => ThemeMode::Light,
    };
    let preference = match new_mode {
        ThemeMode::Light => ThemePreference::Light,
        ThemeMode::Dark => ThemePreference::Dark,
    };
    crate::ui::themes::change_mode(preference, cx);
}

pub fn open_theme_editor(_: &OpenThemeEditor, cx: &mut App) {
    with_workspace_window(crate::ui::DatalithView::open_theme_editor, cx);
}

fn with_workspace_window(
    update: fn(
        &mut crate::ui::DatalithView,
        &mut gpui_kit::Window,
        &mut gpui_kit::Context<crate::ui::DatalithView>,
    ),
    cx: &mut App,
) {
    if let Some(window) = cx.active_window() {
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, cx| {
                with_view!(cx, |view, cx| {
                    update(view, window, cx);
                });
            });
        });
    }
}

fn open_preferences(open: fn(&mut crate::ui::settings::SettingsView), cx: &mut App) {
    if let Some(window) = cx.active_window() {
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, cx| {
                with_view!(cx, |view, cx| {
                    open(&mut view.settings);
                    view.settings.focus(window, cx);
                    cx.notify();
                });
            });
        });
    }
}

pub fn open_settings(_: &OpenSettings, cx: &mut App) {
    open_preferences(crate::ui::settings::SettingsView::open, cx);
}

pub fn open_shortcuts(_: &OpenShortcuts, cx: &mut App) {
    with_workspace_window(crate::ui::DatalithView::open_shortcuts, cx);
}

pub fn open_documentation(_: &OpenDocumentation, cx: &mut App) {
    if let Err(error) = system::open_url(DOCS_URL) {
        notifications::push_window_notification(
            cx,
            notifications::documentation_open_failed(&error),
        );
    }
}

pub fn open_about(_: &OpenAbout, cx: &mut App) {
    open_preferences(crate::ui::settings::SettingsView::open_about, cx);
}

pub fn open_licenses(_: &OpenLicenses, cx: &mut App) {
    with_view!(cx, |view, cx| {
        view.licenses.open();
        cx.notify();
    });
}

pub fn open_source(_: &OpenSource, cx: &mut App) {
    let url = crate::ui::licenses::corresponding_source_url();
    if let Err(error) = system::open_url(&url) {
        notifications::push_window_notification(cx, notifications::open_url_failed(&url, &error));
    }
}

fn select_tab_index(index: usize, cx: &mut App) {
    with_view!(cx, |view, cx| {
        if view.tabs.select(index) {
            view.focus_editor_requested = true;
            cx.notify();
        }
    });
}

macro_rules! define_tab_handlers {
    ($($handler:ident => $action:ty => $index:expr),* $(,)?) => {
        $(
            pub fn $handler(_: &$action, cx: &mut App) {
                select_tab_index($index, cx);
            }
        )*
    };
}

define_tab_handlers!(
    handle_select_tab_1 => SelectTab1 => 0,
    handle_select_tab_2 => SelectTab2 => 1,
    handle_select_tab_3 => SelectTab3 => 2,
    handle_select_tab_4 => SelectTab4 => 3,
    handle_select_tab_5 => SelectTab5 => 4,
    handle_select_tab_6 => SelectTab6 => 5,
    handle_select_tab_7 => SelectTab7 => 6,
    handle_select_tab_8 => SelectTab8 => 7,
);

pub fn handle_select_last_tab(_: &SelectLastTab, cx: &mut App) {
    with_view!(cx, |view, cx| {
        if view.tabs.select_last() {
            view.focus_editor_requested = true;
            cx.notify();
        }
    });
}

pub fn go_back(_: &GoBack, cx: &mut App) {
    with_view!(cx, |view, cx| {
        view.pending_navigation = Some(NavigationAction::GoBack);
        cx.notify();
    });
}

pub fn go_forward(_: &GoForward, cx: &mut App) {
    with_view!(cx, |view, cx| {
        view.pending_navigation = Some(NavigationAction::GoForward);
        cx.notify();
    });
}

pub fn handle_open_link(_: &OpenLink, cx: &mut App) {
    with_view!(cx, |view, cx| {
        if let Some(file_handler) = view.tabs.active_handler().cloned() {
            let link_info = {
                let handler = file_handler.read(cx);
                handler.editor.as_ref().and_then(|editor| {
                    let md = editor.as_markdown()?;
                    let input = md.input().read(cx);
                    let offset = input.cursor();
                    let text = input.value().to_string();
                    crate::document::markdown::find_link_at_offset(&text, offset)
                        .map(|url| (url, true))
                })
            };
            if let Some((url, new_tab)) = link_info {
                file_handler.update(cx, |_handler, cx| {
                    cx.emit(FileHandlerEvent::LinkClicked(url, new_tab));
                });
            }
        }
    });
}

fn check_for_updates(_: &CheckForUpdates, cx: &mut App) {
    if let Some(updater) = super::update::Updater::get(cx) {
        updater.update(cx, super::update::Updater::check_now);
    }
}
