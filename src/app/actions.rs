// The gpui `actions!` macro generates unit structs deriving `PartialEq` without `Eq`;
// that lint can only be suppressed here, not in the macro itself.
#![allow(clippy::derive_partial_eq_without_eq)]

use gpui::{App, AppContext, PathPromptOptions, SharedString, actions};
use gpui_component::{Theme, ThemeMode};
use std::path::{Path, PathBuf};

use crate::app::{
    AppState, preferences,
    settings::{self, ThemePreference},
    system,
};
use crate::document::handler::FileHandlerEvent;
use crate::ui::notifications;
use crate::ui::palette::PaletteKind;
use crate::ui::tabs::NavigationAction;
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
        OpenShortcuts,
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
    cx.on_action(open_shortcuts);
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
        let tree_entry = view
            .tree_state
            .read(cx)
            .selected_entry()
            .map(|e| PathBuf::from(e.item().id.to_string()));
        let target = tree_entry
            .or_else(|| view.tabs.active_path().map(Path::to_path_buf))
            .or_else(|| view.last_sidebar_selection.clone());
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
        if let Some(target) = target {
            let _ = system::reveal_in_file_manager(&target);
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
        if let Some(target) = target {
            let _ = system::copy_path(&target);
        }
        cx.notify();
    });
}

pub fn handle_close_tab(_: &CloseTab, cx: &mut App) {
    with_view!(cx, |view, cx| {
        view.close_active_tab(cx);
        cx.notify();
    });
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
    if let Err(error) = settings::set_theme_preference(preference) {
        notifications::push_window_notification(
            cx,
            notifications::settings_save_failed("theme mode", &error),
        );
    }
    preferences::apply_theme_preference(preference, cx);
}

pub fn open_settings(_: &OpenSettings, cx: &mut App) {
    with_view!(cx, |view, cx| {
        view.settings.open();
        cx.notify();
    });
}

pub fn open_shortcuts(_: &OpenShortcuts, cx: &mut App) {
    with_view!(cx, |view, cx| {
        view.settings.open_shortcuts();
        cx.notify();
    });
}

pub fn open_about(_: &OpenAbout, cx: &mut App) {
    with_view!(cx, |view, cx| {
        view.settings.open_about();
        cx.notify();
    });
}

pub fn open_licenses(_: &OpenLicenses, cx: &mut App) {
    with_view!(cx, |view, cx| {
        view.licenses.open();
        cx.notify();
    });
}

pub fn open_source(_: &OpenSource, _cx: &mut App) {
    let url = crate::ui::licenses::corresponding_source_url();
    let _ = system::open_url(&url);
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
            let handler = file_handler.read(cx);
            let link_info = handler.editor.as_ref().and_then(|editor| {
                let md = editor.as_markdown()?;
                let input = md.input().read(cx);
                let offset = input.cursor();
                let text = input.value().to_string();
                crate::document::markdown::find_link_at_offset(&text, offset).map(|url| (url, true))
            });
            let _ = handler;
            if let Some((url, new_tab)) = link_info {
                file_handler.update(cx, |_handler, cx| {
                    cx.emit(FileHandlerEvent::LinkClicked(url, new_tab));
                });
            }
        }
    });
}
