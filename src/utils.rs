use std::path::Path;

use crate::consts::{SUPPORTED_EXTENSIONS, UNKNOWN_NAME};

pub fn file_name_str(path: &Path) -> &str {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(UNKNOWN_NAME)
}

pub fn is_supported_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some(ext) if SUPPORTED_EXTENSIONS.contains(&ext)
    )
}

pub fn open_in_explorer(target: &Path) {
    let path_to_open = if target.is_dir() {
        target.to_path_buf()
    } else {
        target
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| target.to_path_buf())
    };
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(&path_to_open).spawn();
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = std::process::Command::new("xdg-open").arg(&path_to_open).spawn();
    }
}

pub fn copy_path(target: &Path) {
    let path_str = target.to_string_lossy().to_string();
    if let Ok(mut clipboard) = arboard::Clipboard::new() {
        let _ = clipboard.set_text(&path_str);
    }
}
