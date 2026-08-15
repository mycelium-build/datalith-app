use std::path::Path;

pub fn reveal_in_file_manager(target: &Path) -> anyhow::Result<()> {
    let path = if target.is_dir() {
        target
    } else {
        target.parent().unwrap_or(target)
    };
    #[cfg(target_os = "macos")]
    std::process::Command::new("open").arg(path).spawn()?;
    #[cfg(not(target_os = "macos"))]
    std::process::Command::new("xdg-open").arg(path).spawn()?;
    Ok(())
}

pub fn copy_path(target: &Path) -> anyhow::Result<()> {
    let mut clipboard = arboard::Clipboard::new()?;
    clipboard.set_text(target.to_string_lossy())?;
    Ok(())
}

pub fn open_url(url: &str) -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    std::process::Command::new("open").arg(url).spawn()?;

    #[cfg(target_os = "windows")]
    std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn()?;

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    std::process::Command::new("xdg-open").arg(url).spawn()?;

    Ok(())
}
