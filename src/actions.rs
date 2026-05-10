use gpui::*;

use crate::app::AppState;

actions!(datalith, [
    OpenCodex,
    ToggleSearch,
    CloseSearch,
    NewFile,
    NewFolder,
    Rename,
    Delete,
    Duplicate,
    OpenInExplorer,
    CopyPath
]);

pub fn open_codex(_: &OpenCodex, cx: &mut App) {
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
            view.search_open = !view.search_open;
            if view.search_open {
                view.needs_search_focus = true;
                let query = view.search_input.read(cx).value();
                if !query.trim().is_empty() {
                    view.search(query);
                }
            }
            cx.notify();
        });
    }
}

pub fn close_search(_: &CloseSearch, cx: &mut App) {
    if let Some(view) = cx.read_global(|state: &AppState, _| state.view.clone()) {
        view.update(cx, |view, cx| {
            if view.search_open {
                view.search_open = false;
                cx.notify();
            }
        });
    }
}

pub fn handle_new_file(_: &NewFile, cx: &mut App) {
    if let Some(view) = cx.read_global(|state: &AppState, _| state.view.clone()) {
        let target = view.read(cx).context_menu_target.clone();
        if let Some(target) = target {
            view.update(cx, |view, cx| {
                view.new_file_from_target(&target);
                view.context_menu_target = None;
                view.refresh_tree(cx);
            });
        }
    }
}

pub fn handle_new_folder(_: &NewFolder, cx: &mut App) {
    if let Some(view) = cx.read_global(|state: &AppState, _| state.view.clone()) {
        let target = view.read(cx).context_menu_target.clone();
        if let Some(target) = target {
            view.update(cx, |view, cx| {
                view.new_folder_from_target(&target);
                view.context_menu_target = None;
                view.refresh_tree(cx);
            });
        }
    }
}

pub fn handle_rename(_: &Rename, cx: &mut App) {
    if let Some(view) = cx.read_global(|state: &AppState, _| state.view.clone()) {
        let target = view.read(cx).context_menu_target.clone();
        if let Some(target) = target {
            view.update(cx, |view, cx| {
                view.context_menu_target = None;
                view.rename_target = Some(target);
                cx.notify();
            });
        }
    }
}

pub fn handle_delete(_: &Delete, cx: &mut App) {
    if let Some(view) = cx.read_global(|state: &AppState, _| state.view.clone()) {
        let target = view.read(cx).context_menu_target.clone();
        if let Some(target) = target {
            view.update(cx, |view, cx| {
                view.delete_target(&target);
                view.context_menu_target = None;
                view.refresh_tree(cx);
            });
        }
    }
}

pub fn handle_duplicate(_: &Duplicate, cx: &mut App) {
    if let Some(view) = cx.read_global(|state: &AppState, _| state.view.clone()) {
        let target = view.read(cx).context_menu_target.clone();
        if let Some(target) = target {
            view.update(cx, |view, cx| {
                view.duplicate_target(&target);
                view.context_menu_target = None;
                view.refresh_tree(cx);
            });
        }
    }
}

pub fn handle_open_in_explorer(_: &OpenInExplorer, cx: &mut App) {
    if let Some(view) = cx.read_global(|state: &AppState, _| state.view.clone()) {
        let target = view.read(cx).context_menu_target.clone();
        if let Some(target) = target {
            crate::view::DatalithView::open_in_explorer(&target);
            view.update(cx, |view, cx| {
                view.context_menu_target = None;
                cx.notify();
            });
        }
    }
}

pub fn handle_copy_path(_: &CopyPath, cx: &mut App) {
    if let Some(view) = cx.read_global(|state: &AppState, _| state.view.clone()) {
        let target = view.read(cx).context_menu_target.clone();
        if let Some(target) = target {
            crate::view::DatalithView::copy_path(&target);
            view.update(cx, |view, cx| {
                view.context_menu_target = None;
                cx.notify();
            });
        }
    }
}
