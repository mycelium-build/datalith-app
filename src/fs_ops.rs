use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::consts::{DEFAULT_FILE_NAME, DEFAULT_FOLDER_NAME};

pub fn parent_dir_for_target(target: &Path) -> PathBuf {
    if target.is_dir() {
        target.to_path_buf()
    } else {
        target
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("/"))
    }
}

pub fn unique_name(base_dir: &Path, name: &str) -> PathBuf {
    let (stem, ext) = if let Some(dot) = name.rfind('.') {
        (&name[..dot], &name[dot..])
    } else {
        (name, "")
    };
    let mut candidate = base_dir.join(name);
    let mut counter = 1;
    while candidate.exists() {
        candidate = base_dir.join(format!("{stem} {counter}{ext}"));
        counter += 1;
    }
    candidate
}

pub fn new_file_from_target(target: &Path) -> Result<PathBuf> {
    let dir = parent_dir_for_target(target);
    let path = unique_name(&dir, DEFAULT_FILE_NAME);
    fs::write(&path, "").with_context(|| format!("Failed to create file {:?}", path))?;
    Ok(path)
}

pub fn new_folder_from_target(target: &Path) -> Result<PathBuf> {
    let dir = parent_dir_for_target(target);
    let path = unique_name(&dir, DEFAULT_FOLDER_NAME);
    fs::create_dir(&path).with_context(|| format!("Failed to create folder {:?}", path))?;
    Ok(path)
}

pub fn delete_target(target: &Path) -> Result<()> {
    if target.is_dir() {
        fs::remove_dir_all(target).with_context(|| format!("Failed to delete directory {:?}", target))?;
    } else {
        fs::remove_file(target).with_context(|| format!("Failed to delete file {:?}", target))?;
    }
    Ok(())
}

pub fn duplicate_target(target: &Path) -> Result<PathBuf> {
    if target.is_dir() {
        let parent = target.parent().unwrap_or_else(|| Path::new("/"));
        let name = target.file_name().and_then(|n| n.to_str()).unwrap_or("copy");
        let new_path = unique_name(parent, name);
        copy_dir(target, &new_path).with_context(|| format!("Failed to duplicate dir {:?}", target))?;
        Ok(new_path)
    } else {
        let parent = target.parent().unwrap_or_else(|| Path::new("/"));
        let name = target.file_name().and_then(|n| n.to_str()).unwrap_or("copy");
        let new_path = unique_name(parent, name);
        fs::copy(target, &new_path).with_context(|| format!("Failed to duplicate file {:?}", target))?;
        Ok(new_path)
    }
}

fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let dest = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir(&path, &dest)?;
        } else {
            fs::copy(&path, &dest)?;
        }
    }
    Ok(())
}
