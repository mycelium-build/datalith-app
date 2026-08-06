use std::path::PathBuf;

use gpui::*;
use gpui_component::Root;

use crate::app::AppState;
use crate::ui::DatalithView;

pub(crate) fn open_initial(
    cx: &mut App,
    initial_vault: Option<PathBuf>,
    initial_tabs: Vec<PathBuf>,
) {
    cx.spawn(async move |cx| {
        cx.open_window(WindowOptions::default(), |window, cx| {
            let view = cx.new(|cx| DatalithView::new(window, cx));
            cx.update_global(|state: &mut AppState, _| {
                state.view = Some(view.clone());
            });
            if let Some(path) = initial_vault {
                view.update(cx, |view, cx| {
                    view.set_root_path(path, cx);
                    for tab in initial_tabs {
                        view.open_file(tab, true, window, cx);
                    }
                    view.tabs.select(0);
                });
            }
            cx.new(|cx| Root::new(view, window, cx))
        })
        .expect("Failed to open window");
    })
    .detach();
}
