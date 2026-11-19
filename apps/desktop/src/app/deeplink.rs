//! `datalith://` deep links: `launch` focuses the app and
//! `open?vault=…&path=…` reveals a note in a vault.
//! URLs arrive through the platform's open-URLs callback
//! and are dispatched to the UI through a queue polled on the main executor.

use std::collections::VecDeque;
use std::sync::{LazyLock, Mutex, PoisonError};
use std::time::Duration;

use gpui_kit::{App, AsyncApp};

use crate::ui::notifications;
use datalith_vault::path::resolve_vault_id;

use super::settings;
use super::state::AppState;

const POLL_INTERVAL: Duration = Duration::from_millis(200);
use datalith_core::deeplink::{DeepLink, parse, resolve_in_vault};

static PENDING: LazyLock<Mutex<VecDeque<DeepLink>>> = LazyLock::new(|| Mutex::new(VecDeque::new()));

fn pending_lock() -> std::sync::MutexGuard<'static, VecDeque<DeepLink>> {
    PENDING.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Captures URLs delivered by the platform.
/// Registered on the GPUI application builder, before the event loop starts.
pub fn capture(urls: Vec<String>) {
    let mut pending = pending_lock();
    for url in urls {
        if let Some(link) = parse(&url) {
            pending.push_back(link);
        }
    }
}

/// Starts the polling loop that dispatches queued links into the UI.
pub fn start(cx: &App) {
    cx.spawn(async move |cx: &mut AsyncApp| {
        loop {
            cx.background_executor().timer(POLL_INTERVAL).await;
            let links: Vec<DeepLink> = pending_lock().drain(..).collect();
            for link in links {
                dispatch(link, cx);
            }
        }
    })
    .detach();
}

fn dispatch(link: DeepLink, cx: &AsyncApp) {
    let DeepLink::OpenNote { vault, path } = link else {
        // `launch` needs no work:
        // the OS activates the running app when the scheme fires,
        // and cold starts open the app by themselves.
        return;
    };
    let view = cx.update(|app| {
        app.try_global::<AppState>()
            .and_then(|state| state.view.clone())
    });
    let Some(_) = view else {
        // The window is not up yet (cold start);
        // keep the link queued for the next poll.
        pending_lock().push_back(DeepLink::OpenNote { vault, path });
        return;
    };
    // Resolve before scheduling the switch; opening is deferred until that switch succeeds.
    cx.update(|cx| {
        let Some(vault_path) = resolve_vault_id(&vault, settings::known_vault_paths()) else {
            notifications::push_window_notification(
                cx,
                notifications::deeplink_vault_not_found(&vault),
            );
            return;
        };
        if let Some(resolved) = resolve_in_vault(&vault_path, &path) {
            super::actions::open_vault_path(
                vault_path,
                Some(crate::ui::PendingOpen::Open(resolved)),
                cx,
            );
        }
    });
}
