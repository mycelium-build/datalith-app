//! `datalith://` deep links: `launch` focuses the app and `open?path=…` reveals a note.
//! URLs arrive through the platform's open-URLs callback
//! and are dispatched to the UI through a queue polled on the main executor.

use std::collections::VecDeque;
use std::path::{Component, Path, PathBuf};
use std::sync::{LazyLock, Mutex, PoisonError};
use std::time::Duration;

use gpui::{App, AsyncApp};
use percent_encoding::percent_decode_str;

use super::state::AppState;

const POLL_INTERVAL: Duration = Duration::from_millis(200);
const SCHEME_PREFIX: &str = "datalith://";

#[derive(Clone, Debug, PartialEq, Eq)]
enum DeepLink {
    Launch,
    OpenNote { path: String },
}

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

fn parse(url: &str) -> Option<DeepLink> {
    let remainder = url.strip_prefix(SCHEME_PREFIX)?;
    let (route, query) = remainder.split_once('?').unwrap_or((remainder, ""));
    match route {
        "launch" => Some(DeepLink::Launch),
        "open" => {
            let path = query.split('&').find_map(|pair| {
                let (key, value) = pair.split_once('=')?;
                (key == "path").then(|| value.to_owned())
            })?;
            Some(DeepLink::OpenNote {
                path: percent_decode_str(&path).decode_utf8_lossy().into_owned(),
            })
        }
        _ => None,
    }
}

/// Resolves a vault-relative note path inside `root`, rejecting anything that escapes the vault.
fn resolve_in_vault(root: &Path, relative: &str) -> Option<PathBuf> {
    let candidate = PathBuf::from(relative.replace('\\', "/"));
    if candidate.is_absolute()
        || candidate.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return None;
    }
    Some(root.join(candidate))
}

fn dispatch(link: DeepLink, cx: &mut AsyncApp) {
    let DeepLink::OpenNote { path } = link else {
        // `launch` needs no work:
        // the OS activates the running app when the scheme fires,
        // and cold starts open the app by themselves.
        return;
    };
    let view = cx.update(|app| {
        app.try_global::<AppState>()
            .and_then(|state| state.view.clone())
    });
    let Some(view) = view else {
        // The window is not up yet (cold start);
        // keep the link queued for the next poll.
        pending_lock().push_back(DeepLink::OpenNote { path });
        return;
    };
    view.update(cx, |view, cx| {
        let Some(root) = view.root_path.clone() else {
            return;
        };
        if let Some(resolved) = resolve_in_vault(&root, &path) {
            view.pending_open = Some(resolved);
            cx.notify();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_launch_and_open_routes() {
        assert_eq!(parse("datalith://launch"), Some(DeepLink::Launch));
        assert_eq!(
            parse("datalith://open?path=Clips%2FMy%20Note.md"),
            Some(DeepLink::OpenNote {
                path: "Clips/My Note.md".into()
            })
        );
        assert_eq!(parse("datalith://open"), None);
        assert_eq!(parse("datalith://unknown"), None);
        assert_eq!(parse("https://example.com"), None);
    }

    #[test]
    fn resolve_keeps_paths_inside_the_vault() {
        let root = Path::new("/vault");
        assert_eq!(
            resolve_in_vault(root, "Clips/Note.md"),
            Some(PathBuf::from("/vault/Clips/Note.md"))
        );
        assert_eq!(resolve_in_vault(root, "/etc/passwd"), None);
        assert_eq!(resolve_in_vault(root, "../outside.md"), None);
        assert_eq!(resolve_in_vault(root, "a/../../b.md"), None);
    }
}
