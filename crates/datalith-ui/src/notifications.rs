use gpui_kit::component::notification::Notification;

#[must_use]
pub fn graph_link_open_failed(error: &anyhow::Error) -> Notification {
    Notification::error(format!("Failed to open Graph View link: {error}")).autohide(false)
}

#[must_use]
pub fn todo_task_failed(action: &str, error: &anyhow::Error) -> Notification {
    Notification::error(format!("Failed to {action}: {error}")).autohide(false)
}
