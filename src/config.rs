use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Default)]
pub struct Config {
    pub last_folder: Option<String>,
    #[serde(default)]
    pub recent_vaults: Vec<String>,
}

fn config_file() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_default()
        .join("datalith")
        .join("config.json")
}

fn save_config(config: &Config) {
    let file = config_file();
    let _ = fs::create_dir_all(file.parent().unwrap());
    let _ = fs::write(&file, serde_json::to_string(config).unwrap_or_default());
}

fn load_config() -> Config {
    fs::read_to_string(config_file())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_last_folder(path: &Path) {
    let mut config = load_config();
    config.last_folder = Some(path.to_string_lossy().to_string());
    save_config(&config);
}

pub fn load_last_folder() -> Option<PathBuf> {
    let config = load_config();
    config.last_folder.map(PathBuf::from).filter(|p| p.is_dir())
}

pub fn add_recent_vault(path: &Path) {
    let mut config = load_config();
    let path_str = path.to_string_lossy().to_string();
    config.recent_vaults.retain(|v| v != &path_str);
    config.recent_vaults.insert(0, path_str);
    config.recent_vaults.truncate(10);
    save_config(&config);
}

pub fn load_recent_vaults() -> Vec<PathBuf> {
    let config = load_config();
    config
        .recent_vaults
        .iter()
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .collect()
}
