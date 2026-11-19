//! Persist just the edited document before attempting a provider write. The
//! original version travels with the draft so reopening cannot mask a conflict.
use crate::source;
use anyhow::Context as _;
use serde::{Deserialize, Serialize};
use std::{
    io::Write as _,
    path::{Path, PathBuf},
};

#[derive(Serialize, Deserialize)]
struct Pending {
    path: PathBuf,
    base: String,
    text: String,
}

#[must_use]
pub fn storage_key(bytes: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};
    use std::fmt::Write as _;
    let mut key = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        let _ = write!(key, "{byte:02x}");
    }
    key
}

/// Recoverable draft identities, including documents since removed by a provider.
///
/// # Errors
/// Returns an error when the private draft directory cannot be read.
pub fn pending_paths() -> anyhow::Result<Vec<PathBuf>> {
    let directory = crate::environment::app_data_dir()?.join("drafts");
    if !directory.exists() {
        return Ok(Vec::new());
    }
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(directory)? {
        let file = entry?.path();
        if let Ok(bytes) = std::fs::read(&file)
            && let Ok(draft) = serde_json::from_slice::<Pending>(&bytes)
            && file.file_name().and_then(|n| n.to_str())
                == Some(&storage_key(draft.path.as_os_str().as_encoded_bytes()))
        {
            paths.push(draft.path);
        }
    }
    paths.sort();
    Ok(paths)
}

pub struct Edit {
    path: PathBuf,
    file: PathBuf,
    base: String,
    pub pending: Option<String>,
}

pub struct WriteRequest {
    path: PathBuf,
    base: String,
    text: String,
}
impl WriteRequest {
    /// Writes the submitted version, including retries after an interrupted acknowledgment.
    ///
    /// # Errors
    /// Returns an error if the file is unavailable, changed concurrently or cannot be written.
    pub fn commit(self) -> anyhow::Result<String> {
        if source::read(&self.path)?.as_ref() != self.text.as_bytes() {
            source::save_version(&self.path, Some(self.base.as_bytes()), self.text.as_bytes())?;
        }
        Ok(self.text)
    }
}

impl Edit {
    /// # Errors
    /// Returns an error if private draft storage is unavailable or a saved draft cannot be decoded.
    pub fn open(path: &Path, base: String) -> anyhow::Result<Self> {
        let directory = crate::environment::app_data_dir()?.join("drafts");
        std::fs::create_dir_all(&directory)?;
        Self::from_file(
            path,
            base,
            directory.join(storage_key(path.as_os_str().as_encoded_bytes())),
        )
    }
    fn from_file(path: &Path, base: String, file: PathBuf) -> anyhow::Result<Self> {
        let mut edit = Self {
            path: path.into(),
            file,
            base,
            pending: None,
        };
        match std::fs::read(&edit.file) {
            Ok(bytes) => {
                let saved: Pending = serde_json::from_slice(&bytes)?;
                anyhow::ensure!(saved.path == path, "Draft belongs to another document");
                edit.base = saved.base;
                edit.pending = Some(saved.text);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        Ok(edit)
    }
    /// # Errors
    /// Returns an error if persistence fails or the document changed externally.
    pub fn save(&mut self, text: &str) -> anyhow::Result<()> {
        if text == self.base && self.pending.is_none() {
            return Ok(());
        }
        self.stage(text)?;
        if let Some(write) = self.write_request() {
            let saved = write.commit()?;
            self.accept(saved)?;
        }
        Ok(())
    }
    /// Persist the user's newest input before scheduling provider I/O.
    ///
    /// # Errors
    /// Returns an error when private storage is unavailable.
    pub fn stage(&mut self, text: &str) -> anyhow::Result<()> {
        if self.pending.is_none() && text == self.base {
            return Ok(());
        }
        self.pending = Some(text.to_owned());
        let mut tmp = tempfile::NamedTempFile::new_in(
            self.file.parent().context("Missing draft directory")?,
        )?;
        serde_json::to_writer(
            &mut tmp,
            &Pending {
                path: self.path.clone(),
                base: self.base.clone(),
                text: text.into(),
            },
        )?;
        tmp.flush()?;
        tmp.as_file().sync_all()?;
        tmp.persist(&self.file)?;
        Ok(())
    }
    #[must_use]
    pub fn write_request(&self) -> Option<WriteRequest> {
        self.pending.as_ref().map(|text| WriteRequest {
            path: self.path.clone(),
            base: self.base.clone(),
            text: text.clone(),
        })
    }
    /// Acknowledge only the submitted version. Keep any edits typed during the write.
    ///
    /// # Errors
    /// Returns an error when updating the private draft fails.
    pub fn accept(&mut self, saved: String) -> anyhow::Result<()> {
        self.base = saved;
        if self.pending.as_ref() == Some(&self.base) {
            self.clear()
        } else if let Some(text) = self.pending.clone() {
            self.stage(&text)
        } else {
            Ok(())
        }
    }
    fn clear(&mut self) -> anyhow::Result<()> {
        std::fs::remove_file(&self.file)?;
        self.pending = None;
        Ok(())
    }
    /// # Errors
    /// Returns an error when a separate file cannot be created or draft cleanup fails.
    pub fn keep_both(&mut self) -> anyhow::Result<PathBuf> {
        let directory = self.path.parent().context("Missing parent")?.to_owned();
        self.copy_to(&directory)
    }
    /// Exports a recovered draft to a writable folder, including when its original grant expired.
    ///
    /// # Errors
    /// Returns an error if the copy or private draft cleanup fails.
    pub fn copy_to(&mut self, directory: &Path) -> anyhow::Result<PathBuf> {
        let text = self.pending.as_ref().context("No pending draft")?;
        let stem = self.path.file_stem().unwrap_or_default().to_string_lossy();
        let ext = self
            .path
            .extension()
            .map_or_else(String::new, |e| format!(".{}", e.to_string_lossy()));
        let copy = crate::path::unique_path(directory, &format!("{stem} (local edits){ext}"));
        source::save_version(&copy, None, text.as_bytes())?;
        self.clear()?;
        Ok(copy)
    }
    /// # Errors
    /// Returns an error if the current document is unavailable.
    pub fn refresh(&mut self) -> anyhow::Result<Option<String>> {
        if self.pending.is_some() {
            return Ok(None);
        }
        let disk = source::read_to_string(&self.path)?;
        if disk == self.base {
            return Ok(None);
        }
        self.base.clone_from(&disk);
        Ok(Some(disk))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn acknowledging_an_older_write_preserves_newer_keystrokes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Note.md");
        let draft = dir.path().join("draft");
        std::fs::write(&path, "zero").unwrap();
        let mut edit = Edit::from_file(&path, "zero".into(), draft.clone()).unwrap();
        edit.stage("one").unwrap();
        let in_flight = edit.write_request().unwrap();
        edit.stage("two").unwrap();
        edit.accept(in_flight.commit().unwrap()).unwrap();
        drop(edit);
        let mut reopened = Edit::from_file(&path, "one".into(), draft.clone()).unwrap();
        assert_eq!(reopened.pending.as_deref(), Some("two"));
        reopened.save("two").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "two");
        assert!(!draft.exists());
    }
    #[test]
    fn external_change_and_relaunch_preserve_both_versions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Courses.md");
        let draft = dir.path().join("draft");
        std::fs::write(&path, "milk").unwrap();
        let mut edit = Edit::from_file(&path, "milk".into(), draft.clone()).unwrap();
        std::fs::write(&path, "milk, coffee").unwrap();
        assert!(edit.save("milk, bread").is_err());
        drop(edit);
        let mut reopened = Edit::from_file(&path, "milk, coffee".into(), draft.clone()).unwrap();
        assert!(reopened.save("milk, bread").is_err());
        let second = reopened.keep_both().unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "milk, coffee");
        assert_eq!(std::fs::read_to_string(second).unwrap(), "milk, bread");
        assert!(!draft.exists());
    }
}
