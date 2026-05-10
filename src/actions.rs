use gpui::*;

use crate::app::AppState;

actions!(datalith, [OpenCodex, ToggleSearch, CloseSearch]);

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
