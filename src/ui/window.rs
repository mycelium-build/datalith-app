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
