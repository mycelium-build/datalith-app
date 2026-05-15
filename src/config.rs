use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Clone)]
pub(crate) struct Config {
    pub(crate) last_folder: Option<String>,
    #[serde(default)]
    pub(crate) recent_vaults: Vec<String>,
}

fn config_file() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_default()
        .join("datalith")
        .join("config.json")
}

static CONFIG_CACHE: Mutex<Option<Config>> = Mutex::new(None);

fn get_cached() -> Option<Config> {
    CONFIG_CACHE.lock().unwrap().clone()
}

fn set_cached(config: &Config) {
    *CONFIG_CACHE.lock().unwrap() = Some(config.clone());
}

fn flush_to_disk(config: &Config) -> Result<()> {
    let file = config_file();
    fs::create_dir_all(file.parent().context("Config path has no parent")?).with_context(|| {
        format!(
            "Failed to create config directory: {}",
            file.parent().unwrap().display()
        )
    })?;
    fs::write(&file, serde_json::to_string(config)?)
        .with_context(|| format!("Failed to write config: {}", file.display()))?;
    Ok(())
}

fn load_from_disk() -> Config {
    fs::read_to_string(config_file())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn get_config() -> Config {
    if let Some(cached) = get_cached() {
        return cached;
    }
    let config = load_from_disk();
    set_cached(&config);
    config
}

fn save_config(config: &Config) -> Result<()> {
    set_cached(config);
    flush_to_disk(config)
}

pub(crate) fn save_last_folder(path: &Path) -> Result<()> {
    let mut config = get_config();
    config.last_folder = Some(path.to_string_lossy().to_string());
    save_config(&config)
}

pub(crate) fn load_last_folder() -> Option<PathBuf> {
    let config = get_config();
    config.last_folder.map(PathBuf::from).filter(|p| p.is_dir())
}

pub(crate) fn add_recent_vault(path: &Path) -> Result<()> {
    let mut config = get_config();
    let path_str = path.to_string_lossy().to_string();
    config.recent_vaults.retain(|v| v != &path_str);
    config.recent_vaults.insert(0, path_str);
    config.recent_vaults.truncate(10);
    save_config(&config)
}

pub(crate) fn load_recent_vaults() -> Vec<PathBuf> {
    let config = get_config();
    config
        .recent_vaults
        .iter()
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .collect()
}
