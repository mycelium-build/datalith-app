use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

const CURRENT_SCHEMA_VERSION: u32 = 1;
const MAX_RECENT_VAULTS: usize = 10;
pub(crate) const DEFAULT_FONT_SCALE: f64 = 1.0;
pub(crate) const MIN_FONT_SCALE: f64 = 0.5;
pub(crate) const MAX_FONT_SCALE: f64 = 3.0;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum ColorMode {
    #[default]
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ThemeKind {
    Light,
    Dark,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ApplicationSettings {
    pub(crate) last_vault: Option<PathBuf>,
    pub(crate) recent_vaults: Vec<PathBuf>,
    pub(crate) color_mode: ColorMode,
    pub(crate) light_theme_name: Option<String>,
    pub(crate) dark_theme_name: Option<String>,
    pub(crate) font_scale: f64,
}

impl Default for ApplicationSettings {
    fn default() -> Self {
        Self {
            last_vault: None,
            recent_vaults: Vec::new(),
            color_mode: ColorMode::default(),
            light_theme_name: None,
            dark_theme_name: None,
            font_scale: DEFAULT_FONT_SCALE,
        }
    }
}

#[derive(Clone, Default, Deserialize, Serialize)]
struct StoredSettings {
    #[serde(default = "schema_version")]
    schema_version: u32,
    // `last_folder` preserves compatibility with the original unversioned schema.
    #[serde(default, alias = "last_vault")]
    last_folder: Option<String>,
    #[serde(default)]
    recent_vaults: Vec<String>,
    #[serde(default)]
    theme_mode: Option<String>,
    #[serde(default)]
    light_theme_name: Option<String>,
    #[serde(default)]
    dark_theme_name: Option<String>,
    #[serde(default)]
    font_size_multiplier: Option<f64>,
}

const fn schema_version() -> u32 {
    CURRENT_SCHEMA_VERSION
}

impl StoredSettings {
    fn normalized(self) -> ApplicationSettings {
        let mut recent_vaults = Vec::new();
        for path in self.recent_vaults.into_iter().map(PathBuf::from) {
            if path.is_dir() && !recent_vaults.contains(&path) {
                recent_vaults.push(path);
            }
            if recent_vaults.len() == MAX_RECENT_VAULTS {
                break;
            }
        }

        ApplicationSettings {
            last_vault: self
                .last_folder
                .map(PathBuf::from)
                .filter(|path| path.is_dir()),
            recent_vaults,
            color_mode: match self.theme_mode.as_deref() {
                Some("dark") => ColorMode::Dark,
                _ => ColorMode::Light,
            },
            light_theme_name: normalize_theme_name(self.light_theme_name),
            dark_theme_name: normalize_theme_name(self.dark_theme_name),
            font_scale: normalize_font_scale(self.font_size_multiplier.unwrap_or_default()),
        }
    }

    fn from_settings(settings: &ApplicationSettings) -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            last_folder: settings
                .last_vault
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned()),
            recent_vaults: settings
                .recent_vaults
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect(),
            theme_mode: Some(
                match settings.color_mode {
                    ColorMode::Light => "light",
                    ColorMode::Dark => "dark",
                }
                .to_owned(),
            ),
            light_theme_name: settings.light_theme_name.clone(),
            dark_theme_name: settings.dark_theme_name.clone(),
            font_size_multiplier: Some(settings.font_scale),
        }
    }
}

fn normalize_theme_name(name: Option<String>) -> Option<String> {
    name.map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
}

fn normalize_font_scale(scale: f64) -> f64 {
    if scale.is_finite() && (MIN_FONT_SCALE..=MAX_FONT_SCALE).contains(&scale) {
        scale
    } else {
        DEFAULT_FONT_SCALE
    }
}

struct SettingsStore {
    file: PathBuf,
    cached: Option<ApplicationSettings>,
}

impl SettingsStore {
    fn new(file: PathBuf) -> Self {
        Self { file, cached: None }
    }

    fn snapshot(&mut self) -> ApplicationSettings {
        self.cached
            .get_or_insert_with(|| {
                fs::read_to_string(&self.file)
                    .ok()
                    .and_then(|json| serde_json::from_str::<StoredSettings>(&json).ok())
                    .unwrap_or_default()
                    .normalized()
            })
            .clone()
    }

    fn update(&mut self, update: impl FnOnce(&mut ApplicationSettings)) -> Result<()> {
        let mut next = self.snapshot();
        update(&mut next);
        self.persist(&next)?;
        self.cached = Some(next);
        Ok(())
    }

    fn persist(&self, settings: &ApplicationSettings) -> Result<()> {
        let parent = self.file.parent().context("Settings path has no parent")?;
        fs::create_dir_all(parent).with_context(|| {
            format!("Failed to create settings directory: {}", parent.display())
        })?;
        let json = serde_json::to_string(&StoredSettings::from_settings(settings))?;
        fs::write(&self.file, json)
            .with_context(|| format!("Failed to write settings: {}", self.file.display()))
    }
}

fn settings_file() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_default()
        .join("datalith")
        .join("config.json")
}

static SETTINGS: LazyLock<Mutex<SettingsStore>> =
    LazyLock::new(|| Mutex::new(SettingsStore::new(settings_file())));

#[must_use]
pub(crate) fn snapshot() -> ApplicationSettings {
    SETTINGS.lock().unwrap().snapshot()
}

pub(crate) fn record_opened_vault(path: &Path) -> Result<()> {
    let path = path.to_path_buf();
    SETTINGS.lock().unwrap().update(|settings| {
        settings.last_vault = Some(path.clone());
        settings.recent_vaults.retain(|recent| recent != &path);
        settings.recent_vaults.insert(0, path);
        settings.recent_vaults.truncate(MAX_RECENT_VAULTS);
    })
}

pub(crate) fn set_color_mode(mode: ColorMode) -> Result<()> {
    SETTINGS
        .lock()
        .unwrap()
        .update(|settings| settings.color_mode = mode)
}

pub(crate) fn select_theme(kind: ThemeKind, name: &str) -> Result<()> {
    let name = name.trim();
    if name.is_empty() {
        bail!("Theme name cannot be empty");
    }
    SETTINGS.lock().unwrap().update(|settings| match kind {
        ThemeKind::Light => settings.light_theme_name = Some(name.to_owned()),
        ThemeKind::Dark => settings.dark_theme_name = Some(name.to_owned()),
    })
}

pub(crate) fn set_font_scale(scale: f64) -> Result<()> {
    if !scale.is_finite() || !(MIN_FONT_SCALE..=MAX_FONT_SCALE).contains(&scale) {
        bail!("Font scale must be between {MIN_FONT_SCALE} and {MAX_FONT_SCALE}");
    }
    SETTINGS
        .lock()
        .unwrap()
        .update(|settings| settings.font_scale = scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_settings_file(test_name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "datalith-settings-{test_name}-{}-{}.json",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ))
    }

    #[test]
    fn invalid_stored_preferences_are_normalized_in_one_snapshot() {
        let file = temp_settings_file("normalization");
        fs::write(
            &file,
            r#"{"theme_mode":"sepia","light_theme_name":"  ","font_size_multiplier":99}"#,
        )
        .unwrap();

        let settings = SettingsStore::new(file.clone()).snapshot();

        assert_eq!(settings.color_mode, ColorMode::Light);
        assert_eq!(settings.light_theme_name, None);
        assert_eq!(settings.font_scale, DEFAULT_FONT_SCALE);
        let _ = fs::remove_file(file);
    }

    #[test]
    fn recording_a_vault_persists_last_and_recent_vault_together() {
        let directory = std::env::temp_dir().join(format!(
            "datalith-vault-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        fs::create_dir_all(&directory).unwrap();
        let file = temp_settings_file("round-trip");
        let mut store = SettingsStore::new(file.clone());

        store
            .update(|settings| {
                settings.last_vault = Some(directory.clone());
                settings.recent_vaults = vec![directory.clone()];
                settings.color_mode = ColorMode::Dark;
            })
            .unwrap();
        let reloaded = SettingsStore::new(file.clone()).snapshot();

        assert_eq!(reloaded.last_vault, Some(directory.clone()));
        assert_eq!(reloaded.recent_vaults, vec![directory.clone()]);
        assert_eq!(reloaded.color_mode, ColorMode::Dark);
        let _ = fs::remove_file(file);
        let _ = fs::remove_dir(directory);
    }
}
