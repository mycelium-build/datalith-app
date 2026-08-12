use std::path::PathBuf;

use gpui::{App, AppContext, BorrowAppContext, WindowOptions};
use gpui_component::Root;
use gpui_component::notification::Notification;

use crate::app::AppState;
use crate::ui::DatalithView;

pub fn open_initial(
    cx: &App,
    first_startup: bool,
    initial_vault: Option<PathBuf>,
    initial_tabs: Vec<PathBuf>,
    pending_notifications: Vec<Notification>,
) {
    cx.spawn(async move |cx| {
        if let Err(error) = cx.open_window(WindowOptions::default(), |window, cx| {
            let view =
                cx.new(|cx| DatalithView::new(first_startup, pending_notifications, window, cx));
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
                    view.focus_active_tab(window, cx);
                });
            }
            cx.new(|cx| Root::new(view, window, cx))
        }) {
            eprintln!("Failed to open window: {error}");
        }
    })
    .detach();
}
