use std::path::Path;

#[must_use]
pub(crate) fn display_name(path: &Path) -> &str {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(crate::consts::UNKNOWN_NAME)
}
