use gpui::App;
use gpui_component::WindowExt;
use gpui_component::notification::Notification;

pub fn push_window_notification(cx: &mut App, notification: Notification) {
    if let Some(window) = cx.windows().first().copied() {
        let _ = window.update(cx, |_, window, cx| {
            window.push_notification(notification, cx);
        });
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

pub fn create_file_failed(base_name: &str, error: &anyhow::Error) -> Notification {
    Notification::error(format!("Failed to create {base_name}: {error}")).autohide(false)
}

pub fn theme_fallback(saved: &str, fallback: &str) -> Notification {
    Notification::warning(format!(
        "Saved theme \"{saved}\" is not available, using \"{fallback}\""
    ))
    .autohide(false)
}
