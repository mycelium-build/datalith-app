use gpui_component::notification::Notification;

pub(crate) fn vault_db_ready() -> Notification {
    Notification::info("Vault DB ready")
}

pub(crate) fn vault_db_failed_to_load() -> Notification {
    Notification::error("Vault DB failed to load").autohide(false)
}

pub(crate) fn catalog_loading() -> Notification {
    Notification::info("Vault DB loading, some features degraded")
}

pub(crate) fn rename_while_loading() -> Notification {
    Notification::warning("Vault DB still loading, rename unavailable")
}

pub(crate) fn rename_completed(updated: usize) -> Notification {
    let msg = if updated == 0 {
        "Rename completed".to_string()
    } else {
        format!("Rename completed, {updated} files updated")
    };
    Notification::success(&msg)
}

pub(crate) fn rename_completed_partial(updated: usize, total: usize) -> Notification {
    Notification::warning(format!(
        "Rename completed partially, {updated}/{total} files updated",
    ))
    .autohide(false)
}

pub(crate) fn rename_failed() -> Notification {
    Notification::error("Rename failed").autohide(false)
}
