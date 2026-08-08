use std::path::Path;

const UNKNOWN_NAME: &str = "Unknown";

#[must_use]
pub fn display_name(path: &Path) -> &str {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(UNKNOWN_NAME)
}
