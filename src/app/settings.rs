use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use anyhow::{Context, Result, bail};
use gpui::WindowAppearance;
use serde::{Deserialize, Serialize};

const CURRENT_SCHEMA_VERSION: u32 = 1;
const MAX_RECENT_VAULTS: usize = 10;
pub const DEFAULT_FONT_SCALE: f64 = 1.0;
pub const MIN_FONT_SCALE: f64 = 0.5;
pub const MAX_FONT_SCALE: f64 = 3.0;

/// The effective theme mode currently in use, derived from a [`ThemePreference`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ThemeMode {
    #[default]
    Light,
    Dark,
}

/// The user's theme preference. `System` follows the OS light/dark setting.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ThemePreference {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemePreference {
    pub const fn name(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "system" => Some(Self::System),
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            _ => None,
        }
    }

    /// The theme mode that `self` resolves to given the OS appearance.
    pub const fn resolve(self, appearance: WindowAppearance) -> ThemeMode {
        match self {
            Self::Light => ThemeMode::Light,
            Self::Dark => ThemeMode::Dark,
            Self::System => match appearance {
                WindowAppearance::Dark | WindowAppearance::VibrantDark => ThemeMode::Dark,
                WindowAppearance::Light | WindowAppearance::VibrantLight => ThemeMode::Light,
            },
        }
    }

    /// The window appearance to force for explicit preferences, or `None` for `System` (follow the OS appearance).
    pub fn to_window_appearance(self) -> Option<WindowAppearance> {
        match self {
            Self::System => None,
            Self::Light => Some(WindowAppearance::Light),
            Self::Dark => Some(WindowAppearance::Dark),
        }
    }
}

impl ThemeMode {
    pub const fn window_appearance(self) -> WindowAppearance {
        match self {
            Self::Light => WindowAppearance::Light,
            Self::Dark => WindowAppearance::Dark,
        }
    }
}

impl From<ThemeMode> for gpui_component::ThemeMode {
    fn from(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Light => Self::Light,
            ThemeMode::Dark => Self::Dark,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThemeKind {
    Light,
    Dark,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ApplicationSettings {
    pub last_vault: Option<PathBuf>,
    pub recent_vaults: Vec<PathBuf>,
    pub theme_preference: ThemePreference,
    pub light_theme_name: Option<String>,
    pub dark_theme_name: Option<String>,
    pub font_scale: f64,
}

impl Default for ApplicationSettings {
    fn default() -> Self {
        Self {
            last_vault: None,
            recent_vaults: Vec::new(),
            theme_preference: ThemePreference::default(),
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
    #[serde(default)]
    last_vault: Option<String>,
    #[serde(default)]
    recent_vaults: Vec<String>,
    #[serde(default)]
    theme_preference: Option<String>,
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
                .last_vault
                .map(PathBuf::from)
                .filter(|path| path.is_dir()),
            recent_vaults,
            theme_preference: match self.theme_preference.as_deref() {
                Some("light") => ThemePreference::Light,
                Some("dark") => ThemePreference::Dark,
                _ => ThemePreference::System,
            },
            light_theme_name: normalize_theme_name(self.light_theme_name),
            dark_theme_name: normalize_theme_name(self.dark_theme_name),
            font_scale: normalize_font_scale(self.font_size_multiplier.unwrap_or_default()),
        }
    }

    fn from_settings(settings: &ApplicationSettings) -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            last_vault: settings
                .last_vault
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned()),
            recent_vaults: settings
                .recent_vaults
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect(),
            theme_preference: Some(settings.theme_preference.name().to_owned()),
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
    const fn new(file: PathBuf) -> Self {
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
    super::data_dir().join("config.json")
}

static SETTINGS: LazyLock<Mutex<SettingsStore>> =
    LazyLock::new(|| Mutex::new(SettingsStore::new(settings_file())));

fn settings_lock() -> std::sync::MutexGuard<'static, SettingsStore> {
    // Return the value anyway even if maybe poisoned (mid updating)
    SETTINGS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[must_use]
pub fn snapshot() -> ApplicationSettings {
    settings_lock().snapshot()
}

pub fn record_opened_vault(path: &Path) -> Result<()> {
    let path = path.to_path_buf();
    settings_lock().update(|settings| {
        settings.last_vault = Some(path.clone());
        settings.recent_vaults.retain(|recent| recent != &path);
        settings.recent_vaults.insert(0, path);
        settings.recent_vaults.truncate(MAX_RECENT_VAULTS);
    })
}

pub fn set_theme_preference(preference: ThemePreference) -> Result<()> {
    settings_lock().update(|settings| settings.theme_preference = preference)
}

pub fn select_theme(kind: ThemeKind, name: &str) -> Result<()> {
    let name = name.trim();
    if name.is_empty() {
        bail!("Theme name cannot be empty");
    }
    settings_lock().update(|settings| match kind {
        ThemeKind::Light => settings.light_theme_name = Some(name.to_owned()),
        ThemeKind::Dark => settings.dark_theme_name = Some(name.to_owned()),
    })
}

pub fn set_font_scale(scale: f64) -> Result<()> {
    if !scale.is_finite() || !(MIN_FONT_SCALE..=MAX_FONT_SCALE).contains(&scale) {
        bail!("Font scale must be between {MIN_FONT_SCALE} and {MAX_FONT_SCALE}");
    }
    settings_lock().update(|settings| settings.font_scale = scale)
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
            r#"{"theme_preference":"sepia","light_theme_name":"  ","font_size_multiplier":99}"#,
        )
        .unwrap();

        let settings = SettingsStore::new(file.clone()).snapshot();

        assert_eq!(settings.theme_preference, ThemePreference::System);
        assert_eq!(settings.light_theme_name, None);
        assert!(
            (settings.font_scale - DEFAULT_FONT_SCALE).abs() <= f64::EPSILON,
            "font_scale should normalize to DEFAULT_FONT_SCALE"
        );
        let _ = fs::remove_file(file);
    }

    #[test]
    fn legacy_theme_mode_key_is_ignored_and_defaults_to_system() {
        let file = temp_settings_file("legacy-theme-mode");
        fs::write(&file, r#"{"theme_mode":"dark"}"#).unwrap();

        let settings = SettingsStore::new(file.clone()).snapshot();

        assert_eq!(settings.theme_preference, ThemePreference::System);
        let _ = fs::remove_file(file);
    }

    #[test]
    fn theme_preference_strings_map_to_preferences() {
        let file = temp_settings_file("theme-pref-strings");

        fs::write(&file, r#"{"theme_preference":"light"}"#).unwrap();
        assert_eq!(
            SettingsStore::new(file.clone()).snapshot().theme_preference,
            ThemePreference::Light
        );
        fs::write(&file, r#"{"theme_preference":"dark"}"#).unwrap();
        assert_eq!(
            SettingsStore::new(file.clone()).snapshot().theme_preference,
            ThemePreference::Dark
        );
        fs::write(&file, r#"{"theme_preference":"system"}"#).unwrap();
        assert_eq!(
            SettingsStore::new(file.clone()).snapshot().theme_preference,
            ThemePreference::System
        );
        let _ = fs::remove_file(file);
    }

    #[test]
    fn system_preference_round_trips_through_persistence() {
        let file = temp_settings_file("system-round-trip");
        let mut store = SettingsStore::new(file.clone());

        store
            .update(|settings| settings.theme_preference = ThemePreference::System)
            .unwrap();

        assert_eq!(store.snapshot().theme_preference, ThemePreference::System);
        assert_eq!(
            SettingsStore::new(file.clone()).snapshot().theme_preference,
            ThemePreference::System
        );
        let _ = fs::remove_file(file);
    }

    #[test]
    fn resolve_derives_theme_mode_from_preference_and_appearance() {
        assert_eq!(
            ThemePreference::Light.resolve(WindowAppearance::Dark),
            ThemeMode::Light
        );
        assert_eq!(
            ThemePreference::Dark.resolve(WindowAppearance::Light),
            ThemeMode::Dark
        );
        assert_eq!(
            ThemePreference::System.resolve(WindowAppearance::Dark),
            ThemeMode::Dark
        );
        assert_eq!(
            ThemePreference::System.resolve(WindowAppearance::VibrantDark),
            ThemeMode::Dark
        );
        assert_eq!(
            ThemePreference::System.resolve(WindowAppearance::Light),
            ThemeMode::Light
        );
        assert_eq!(
            ThemePreference::System.resolve(WindowAppearance::VibrantLight),
            ThemeMode::Light
        );
        assert_eq!(
            ThemePreference::Light.to_window_appearance(),
            Some(WindowAppearance::Light)
        );
        assert_eq!(
            ThemePreference::Dark.to_window_appearance(),
            Some(WindowAppearance::Dark)
        );
        assert_eq!(ThemePreference::System.to_window_appearance(), None);
        assert_eq!(
            gpui_component::ThemeMode::from(ThemeMode::Light),
            gpui_component::ThemeMode::Light
        );
        assert_eq!(
            gpui_component::ThemeMode::from(ThemeMode::Dark),
            gpui_component::ThemeMode::Dark
        );
        assert_eq!(
            ThemeMode::Light.window_appearance(),
            WindowAppearance::Light
        );
        assert_eq!(ThemeMode::Dark.window_appearance(), WindowAppearance::Dark);
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
                settings.theme_preference = ThemePreference::Dark;
            })
            .unwrap();
        let reloaded = SettingsStore::new(file.clone()).snapshot();

        assert_eq!(reloaded.last_vault, Some(directory.clone()));
        assert_eq!(reloaded.recent_vaults, vec![directory.clone()]);
        assert_eq!(reloaded.theme_preference, ThemePreference::Dark);
        let _ = fs::remove_file(file);
        let _ = fs::remove_dir(directory);
    }
}
