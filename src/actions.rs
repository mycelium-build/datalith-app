use gpui::*;
use gpui_component::{Theme, ThemeMode, ThemeRegistry};
use std::path::PathBuf;

use crate::app::AppState;
use crate::fs_ops;
use crate::utils;
use crate::view::palette::PaletteKind;

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
        SelectTab1,
        SelectTab2,
        SelectTab3,
        SelectTab4,
        SelectTab5,
        SelectTab6,
        SelectTab7,
        SelectTab8,
        SelectTab9,
    ]
);

macro_rules! with_view {
    ($cx:expr, |$view:ident, $cx2:ident| $body:block) => {
        if let Some($view) = $cx.read_global(|state: &AppState, _| state.view.clone()) {
            $view.update($cx, |$view, $cx2| $body);
        }
    };
}

pub(crate) fn open_vault(_: &OpenVault, cx: &mut App) {
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

pub(crate) fn toggle_search(_: &ToggleSearch, cx: &mut App) {
    with_view!(cx, |view, cx| {
        if view.palette.open {
            view.palette.close();
        } else {
            view.palette.open_as(PaletteKind::Search);
            let query = view.palette.search_query.clone();
            if !query.trim().is_empty() {
                view.palette.search(view.search_engine.as_ref(), query);
            }
        }
        cx.notify();
    });
}

pub(crate) fn toggle_quick_switcher(_: &ToggleQuickSwitcher, cx: &mut App) {
    with_view!(cx, |view, cx| {
        if view.palette.open {
            view.palette.close();
        } else {
            view.palette.open_as(PaletteKind::QuickSwitcher);
            let open: Vec<PathBuf> = view.open_files.iter().map(|f| f.path.clone()).collect();
            let query = view.palette.qs_query.clone();
            if query.trim().is_empty() {
                view.palette
                    .refresh_quick_switcher(view.search_engine.as_ref(), &open);
            } else {
                view.palette.filter_quick_switcher(&open, query);
            }
        }
        cx.notify();
    });
}

pub(crate) fn close_palette(_: &ClosePalette, cx: &mut App) {
    with_view!(cx, |view, cx| {
        view.palette.close();
        cx.notify();
    });
}

pub(crate) fn handle_new_file(_: &NewFile, cx: &mut App) {
    with_view!(cx, |view, cx| {
        view.commit_rename(cx);
        let target = view
            .context_menu_target
            .take()
            .or_else(|| view.resolve_target(cx))
            .or_else(|| view.root_path.clone());
        if let Some(target) = target {
            if let Ok(created) = fs_ops::new_file_from_target(&target) {
                view.track_new_file(&created);
                if target.is_dir() {
                    let id: SharedString = target.to_string_lossy().to_string().into();
                    view.tree_state.update(cx, |state, cx| {
                        state.expand_by_id(&id, cx);
                    });
                }
                view.refresh_tree(cx);
                view.rename_target = Some(created.clone());
                view.pending_open = Some(created);
            }
        }
        cx.notify();
    });
}

pub(crate) fn handle_new_folder(_: &NewFolder, cx: &mut App) {
    with_view!(cx, |view, cx| {
        view.commit_rename(cx);
        let target = view
            .context_menu_target
            .take()
            .or_else(|| view.resolve_target(cx))
            .or_else(|| view.root_path.clone());
        if let Some(target) = target {
            if let Ok(created) = fs_ops::new_folder_from_target(&target) {
                view.refresh_tree(cx);
                view.rename_target = Some(created);
            }
        }
        cx.notify();
    });
}

pub(crate) fn handle_rename(_: &Rename, cx: &mut App) {
    with_view!(cx, |view, cx| {
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

pub(crate) fn handle_delete(_: &Delete, cx: &mut App) {
    with_view!(cx, |view, cx| {
        let target_index = view.tree_state.read(cx).selected_index();
        let tree_entry = view
            .tree_state
            .read(cx)
            .selected_entry()
            .map(|e| PathBuf::from(e.item().id.to_string()));
        let target = tree_entry
            .or_else(|| {
                let active = view.active_tab.min(view.open_files.len().saturating_sub(1));
                view.open_files.get(active).map(|f| f.path.clone())
            })
            .or_else(|| view.last_sidebar_selection.clone());
        view.commit_rename(cx);
        if let Some(target) = target {
            view.track_file_delete(&target);
            if let Err(e) = fs_ops::delete_target(&target) {
                eprintln!("{e}");
            }
            if target.is_dir() {
                view.close_tabs_under(&target, cx);
            } else {
                view.close_tab_for_file(&target, cx);
            }
            view.refresh_tree(cx);
            let count = view.tree_state.read(cx).entry_count();
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

pub(crate) fn handle_duplicate(_: &Duplicate, cx: &mut App) {
    with_view!(cx, |view, cx| {
        view.commit_rename(cx);
        let target = view
            .context_menu_target
            .take()
            .or_else(|| view.resolve_target(cx));
        if let Some(target) = target {
            if let Ok(duplicated) = fs_ops::duplicate_target(&target) {
                view.track_new_file(&duplicated);
                view.pending_open = Some(duplicated);
            }
            view.refresh_tree(cx);
        }
        cx.notify();
    });
}

pub(crate) fn handle_open_in_explorer(_: &OpenInExplorer, cx: &mut App) {
    with_view!(cx, |view, cx| {
        let target = view
            .context_menu_target
            .take()
            .or_else(|| view.resolve_target(cx));
        if let Some(target) = target {
            utils::open_in_explorer(&target);
        }
        cx.notify();
    });
}

pub(crate) fn handle_copy_path(_: &CopyPath, cx: &mut App) {
    with_view!(cx, |view, cx| {
        let target = view
            .context_menu_target
            .take()
            .or_else(|| view.resolve_target(cx));
        if let Some(target) = target {
            utils::copy_path(&target);
        }
        cx.notify();
    });
}

pub(crate) fn handle_close_tab(_: &CloseTab, cx: &mut App) {
    with_view!(cx, |view, cx| {
        view.close_active_tab(cx);
        cx.notify();
    });
}

pub(crate) fn handle_new_tab(_: &NewTab, cx: &mut App) {
    with_view!(cx, |view, cx| {
        view.new_empty_tab(cx);
        cx.notify();
    });
}

pub(crate) fn handle_focus_sidebar(_: &FocusSidebar, cx: &mut App) {
    with_view!(cx, |view, cx| {
        view.ensure_sidebar_selection(cx);
        view.focus_sidebar_requested = true;
        cx.notify();
    });
}

pub(crate) fn toggle_theme(_: &ToggleTheme, cx: &mut App) {
    let current_mode = Theme::global(cx).mode;
    let new_mode = match current_mode {
        ThemeMode::Light => ThemeMode::Dark,
        ThemeMode::Dark => ThemeMode::Light,
    };
    let registry = ThemeRegistry::global(cx);
    let saved_name = match new_mode {
        ThemeMode::Light => crate::config::load_light_theme_name(),
        ThemeMode::Dark => crate::config::load_dark_theme_name(),
    };
    if let Some(name) = saved_name {
        if let Some(theme_config) = registry.themes().get(name.as_str()) {
            if new_mode == ThemeMode::Light {
                Theme::global_mut(cx).light_theme = theme_config.clone();
            } else {
                Theme::global_mut(cx).dark_theme = theme_config.clone();
            }
        }
    }
    let _ = crate::config::save_theme_mode(new_mode);
    Theme::change(new_mode, None, cx);
    cx.refresh_windows();
}

pub(crate) fn open_settings(_: &OpenSettings, cx: &mut App) {
    with_view!(cx, |view, cx| {
        view.settings.open();
        cx.notify();
    });
}

fn select_tab_index(index: usize, cx: &mut App) {
    with_view!(cx, |view, cx| {
        if index < view.open_files.len() {
            view.active_tab = index;
            view.focus_editor_requested = true;
            cx.notify();
        }
    });
}

macro_rules! define_tab_handlers {
    ($($handler:ident => $action:ty => $index:expr),* $(,)?) => {
        $(
            pub(crate) fn $handler(_: &$action, cx: &mut App) {
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

pub(crate) fn handle_select_tab_9(_: &SelectTab9, cx: &mut App) {
    with_view!(cx, |view, cx| {
        let index = view.open_files.len().saturating_sub(1);
        if !view.open_files.is_empty() {
            view.active_tab = index;
            view.focus_editor_requested = true;
            cx.notify();
        }
    });
}
