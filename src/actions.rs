use gpui::*;
use gpui_component::{Theme, ThemeMode};
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
            let open: Vec<PathBuf> =
                view.open_files.iter().map(|f| f.path.clone()).collect();
            let query = view.palette.qs_query.clone();
            if query.trim().is_empty() {
                view.palette.refresh_quick_switcher(view.search_engine.as_ref(), &open);
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
        let target = view
            .context_menu_target
            .take()
            .or_else(|| view.resolve_target(cx))
            .or_else(|| view.root_path.clone());
        if let Some(target) = target {
            if let Ok(created) = fs_ops::new_file_from_target(&target) {
                view.track_new_file(&created);
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
        let target = view
            .context_menu_target
            .take()
            .or_else(|| view.resolve_target(cx));
        if let Some(target) = target {
            view.track_file_delete(&target);
            let selected_index = view.tree_state.read(cx).selected_index();
            if let Err(e) = fs_ops::delete_target(&target) {
                eprintln!("{e}");
            }
            view.refresh_tree(cx);
            if let Some(ix) = selected_index {
                let count = view.tree_state.read(cx).entry_count();
                if count > 0 {
                    let new_ix = ix.min(count.saturating_sub(1));
                    view.tree_state.update(cx, |state, cx| {
                        state.set_selected_index(Some(new_ix), cx);
                    });
                }
            }
        }
        cx.notify();
    });
}

pub(crate) fn handle_duplicate(_: &Duplicate, cx: &mut App) {
    with_view!(cx, |view, cx| {
        let target = view
            .context_menu_target
            .take()
            .or_else(|| view.resolve_target(cx));
        if let Some(target) = target {
            if let Ok(duplicated) = fs_ops::duplicate_target(&target) {
                view.track_new_file(&duplicated);
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
    let _ = crate::config::save_theme_mode(new_mode);
    Theme::change(new_mode, None, cx);
    cx.refresh_windows();
}
