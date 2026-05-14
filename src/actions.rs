use gpui::*;
use std::path::PathBuf;

use crate::app::AppState;
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
        NewTab
    ]
);

pub fn open_vault(_: &OpenVault, cx: &mut App) {
    let rx = cx.prompt_for_paths(PathPromptOptions {
        files: false,
        directories: true,
        multiple: false,
        prompt: Some("Select a folder".into()),
    });
    cx.spawn(async move |cx| {
        if let Ok(Ok(Some(paths))) = rx.await {
            if let Some(path) = paths.into_iter().next() {
                let view_opt = cx.read_global(|state: &AppState, _| state.view.clone());
                if let Some(view) = view_opt {
                    cx.update_entity(&view, |view, cx| {
                        view.set_root_path(path, cx);
                    });
                }
            }
        }
    })
    .detach();
}

pub fn toggle_search(_: &ToggleSearch, cx: &mut App) {
    if let Some(view) = cx.read_global(|state: &AppState, _| state.view.clone()) {
        view.update(cx, |view, cx| {
            if view.palette.open {
                view.palette.close();
            } else {
                view.palette.open_as(PaletteKind::Search);
                let query = view.palette.search_query.clone();
                if !query.trim().is_empty() {
                    let engine = view.search_engine.clone();
                    view.palette.search(&engine, query);
                }
            }
            cx.notify();
        });
    }
}

pub fn toggle_quick_switcher(_: &ToggleQuickSwitcher, cx: &mut App) {
    if let Some(view) = cx.read_global(|state: &AppState, _| state.view.clone()) {
        view.update(cx, |view, cx| {
            if view.palette.open {
                view.palette.close();
            } else {
                view.palette.open_as(PaletteKind::QuickSwitcher);
                if let Some(ref root) = view.root_path {
                    let open: Vec<PathBuf> =
                        view.open_files.iter().map(|f| f.path.clone()).collect();
                    let query = view.palette.qs_query.clone();
                    if query.trim().is_empty() {
                        view.palette.refresh_quick_switcher(root, &open);
                    } else {
                        view.palette.filter_quick_switcher(&open, query);
                    }
                }
            }
            cx.notify();
        });
    }
}

pub fn close_palette(_: &ClosePalette, cx: &mut App) {
    if let Some(view) = cx.read_global(|state: &AppState, _| state.view.clone()) {
        view.update(cx, |view, cx| {
            view.palette.close();
            cx.notify();
        });
    }
}

pub fn handle_new_file(_: &NewFile, cx: &mut App) {
    if let Some(view) = cx.read_global(|state: &AppState, _| state.view.clone()) {
        view.update(cx, |view, cx| {
            let target = view
                .context_menu_target
                .take()
                .or_else(|| view.resolve_target(cx))
                .or_else(|| view.root_path.clone());
            if let Some(target) = target {
                let created = view.new_file_from_target(&target);
                view.refresh_tree(cx);
                if let Some(path) = created {
                    view.rename_target = Some(path);
                }
            }
            cx.notify();
        });
    }
}

pub fn handle_new_folder(_: &NewFolder, cx: &mut App) {
    if let Some(view) = cx.read_global(|state: &AppState, _| state.view.clone()) {
        view.update(cx, |view, cx| {
            let target = view
                .context_menu_target
                .take()
                .or_else(|| view.resolve_target(cx))
                .or_else(|| view.root_path.clone());
            if let Some(target) = target {
                let created = view.new_folder_from_target(&target);
                view.refresh_tree(cx);
                if let Some(path) = created {
                    view.rename_target = Some(path);
                }
            }
            cx.notify();
        });
    }
}

pub fn handle_rename(_: &Rename, cx: &mut App) {
    if let Some(view) = cx.read_global(|state: &AppState, _| state.view.clone()) {
        view.update(cx, |view, cx| {
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
}

pub fn handle_delete(_: &Delete, cx: &mut App) {
    if let Some(view) = cx.read_global(|state: &AppState, _| state.view.clone()) {
        view.update(cx, |view, cx| {
            let target = view
                .context_menu_target
                .take()
                .or_else(|| view.resolve_target(cx));
            if let Some(target) = target {
                view.delete_target(&target, cx);
                view.refresh_tree(cx);
            }
            cx.notify();
        });
    }
}

pub fn handle_duplicate(_: &Duplicate, cx: &mut App) {
    if let Some(view) = cx.read_global(|state: &AppState, _| state.view.clone()) {
        view.update(cx, |view, cx| {
            let target = view
                .context_menu_target
                .take()
                .or_else(|| view.resolve_target(cx));
            if let Some(target) = target {
                view.duplicate_target(&target);
                view.refresh_tree(cx);
            }
            cx.notify();
        });
    }
}

pub fn handle_open_in_explorer(_: &OpenInExplorer, cx: &mut App) {
    if let Some(view) = cx.read_global(|state: &AppState, _| state.view.clone()) {
        view.update(cx, |view, cx| {
            let target = view
                .context_menu_target
                .take()
                .or_else(|| view.resolve_target(cx));
            if let Some(target) = target {
                crate::view::DatalithView::open_in_explorer(&target);
            }
            cx.notify();
        });
    }
}

pub fn handle_copy_path(_: &CopyPath, cx: &mut App) {
    if let Some(view) = cx.read_global(|state: &AppState, _| state.view.clone()) {
        view.update(cx, |view, cx| {
            let target = view
                .context_menu_target
                .take()
                .or_else(|| view.resolve_target(cx));
            if let Some(target) = target {
                crate::view::DatalithView::copy_path(&target);
            }
            cx.notify();
        });
    }
}

pub fn handle_close_tab(_: &CloseTab, cx: &mut App) {
    if let Some(view) = cx.read_global(|state: &AppState, _| state.view.clone()) {
        view.update(cx, |view, cx| {
            view.close_active_tab(cx);
            cx.notify();
        });
    }
}

pub fn handle_new_tab(_: &NewTab, cx: &mut App) {
    if let Some(view) = cx.read_global(|state: &AppState, _| state.view.clone()) {
        view.update(cx, |view, cx| {
            view.new_empty_tab(cx);
            cx.notify();
        });
    }
}
