//! Crash-safe file replacement shared by settings, themes, and workspaces.

use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::Path;

use anyhow::{Context as _, Result};

/// Write through a same-directory temporary file, so readers see either the
/// old or the new contents, then flush the rename itself on Unix.
pub fn write(path: &Path, contents: &[u8]) -> Result<()> {
    let parent = path.parent().context("Path has no parent")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    let file_name = path
        .file_name()
        .context("Path has no file name")?
        .to_string_lossy();
    let temporary = parent.join(format!(".{file_name}.{:032x}.tmp", rand::random::<u128>()));
    let result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .with_context(|| format!("Failed to create {}", temporary.display()))?;
        file.write_all(contents)
            .with_context(|| format!("Failed to write {}", temporary.display()))?;
        file.sync_all()
            .with_context(|| format!("Failed to flush {}", temporary.display()))?;
        drop(file);
        fs::rename(&temporary, path)
            .with_context(|| format!("Failed to replace {}", path.display()))?;
        #[cfg(unix)]
        fs::File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}
