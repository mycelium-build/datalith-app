use gpui::App;
use gpui_component::WindowExt;
use gpui_component::notification::Notification;
use std::path::Path;

pub fn push_window_notification(cx: &mut App, notification: Notification) {
    if let Some(window) = cx.windows().first().copied()
        && let Err(error) = window.update(cx, |_, window, cx| {
            window.push_notification(notification, cx);
        })
    {
        eprintln!("Failed to push notification: {error}");
    }
}

pub fn vault_db_ready() -> Notification {
    Notification::info("Vault DB ready")
}

pub fn vault_db_failed_to_load() -> Notification {
    Notification::error("Vault DB failed to load").autohide(false)
}

pub fn catalog_loading() -> Notification {
    Notification::info("Vault DB loading, some features degraded")
}

pub fn rename_while_loading() -> Notification {
    Notification::warning("Vault DB still loading, rename unavailable")
}

pub fn rename_completed(updated: usize) -> Notification {
    let msg = if updated == 0 {
        "Rename completed".to_string()
    } else {
        format!("Rename completed, {updated} files updated")
    };
    Notification::success(&msg)
}

pub fn rename_completed_partial(updated: usize, total: usize) -> Notification {
    Notification::warning(format!(
        "Rename completed partially, {updated}/{total} files updated",
    ))
    .autohide(false)
}

pub fn rename_failed() -> Notification {
    Notification::error("Rename failed").autohide(false)
}

pub fn settings_save_failed(action: &str, error: &anyhow::Error) -> Notification {
    Notification::error(format!("Failed to save {action}: {error}")).autohide(false)
}

pub fn documentation_open_failed(error: &anyhow::Error) -> Notification {
    Notification::error(format!("Failed to open documentation: {error}")).autohide(false)
}

pub fn reveal_in_file_manager_failed(target: &Path, error: &anyhow::Error) -> Notification {
    Notification::error(format!(
        "Failed to reveal {} in the file manager: {error}",
        target.display()
    ))
    .autohide(false)
}

pub fn copy_path_failed(target: &Path, error: &anyhow::Error) -> Notification {
    Notification::error(format!(
        "Failed to copy {} to the clipboard: {error}",
        target.display()
    ))
    .autohide(false)
}

pub fn open_url_failed(url: &str, error: &anyhow::Error) -> Notification {
    Notification::error(format!("Failed to open {url}: {error}")).autohide(false)
}

pub fn save_file_failed(path: &Path, error: &anyhow::Error) -> Notification {
    Notification::error(format!("Failed to save {}: {error}", path.display())).autohide(false)
}

pub fn todo_task_failed(action: &str, error: &anyhow::Error) -> Notification {
    Notification::error(format!("Failed to {action}: {error}")).autohide(false)
}

pub fn graph_link_open_failed(error: &anyhow::Error) -> Notification {
    Notification::error(format!("Failed to open Graph View link: {error}")).autohide(false)
}

pub fn theme_load_failed(theme: &str, error: &anyhow::Error) -> Notification {
    Notification::error(format!("Failed to load the {theme} theme: {error}")).autohide(false)
}

pub fn create_file_failed(base_name: &str, error: &anyhow::Error) -> Notification {
    Notification::error(format!("Failed to create {base_name}: {error}")).autohide(false)
}

pub fn theme_fallback(saved: &str, fallback: &str) -> Notification {
    Notification::warning(format!(
        "Saved theme \"{saved}\" is not available, using \"{fallback}\""
    ))
    .autohide(false)
}

pub fn font_load_failed(error: &anyhow::Error) -> Notification {
    Notification::error(format!("Failed to load the bundled Pixeloid font: {error}"))
        .autohide(false)
}
