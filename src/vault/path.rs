use std::path::{Path, PathBuf};

const UNKNOWN_NAME: &str = "Unknown";

#[must_use]
pub fn display_name(path: &Path) -> &str {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(UNKNOWN_NAME)
}

/// Resolves a vault id:
/// an absolute path, or the name of a vault folder to its path,
/// matching paths first and taking the first match.
pub fn resolve_vault_id<'a>(
    id: &str,
    paths: impl IntoIterator<Item = &'a PathBuf>,
) -> Option<PathBuf> {
    let mut by_name = None;
    for path in paths {
        if path.to_string_lossy() == id {
            return Some(path.clone());
        }
        if by_name.is_none()
            && path
                .file_name()
                .is_some_and(|name| name.to_string_lossy() == id)
        {
            by_name = Some(path.clone());
        }
    }
    by_name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_paths_then_names() {
        let paths = [
            PathBuf::from("/vaults/First"),
            PathBuf::from("/other/Second"),
        ];
        assert_eq!(
            resolve_vault_id("/other/Second", &paths),
            Some(PathBuf::from("/other/Second"))
        );
        assert_eq!(
            resolve_vault_id("Second", &paths),
            Some(PathBuf::from("/other/Second"))
        );
        assert_eq!(resolve_vault_id("first", &paths), None);
        assert_eq!(resolve_vault_id("/nowhere/Missing", &paths), None);
    }
}
