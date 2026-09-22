//! Theme documents, user-owned copies, and live appearance previews.

mod embedded;
mod storage;
#[cfg(test)]
mod tests;

use std::{collections::BTreeMap, path::PathBuf, rc::Rc};

use anyhow::{Context as _, Result, bail};
use gpui_kit::component::{ActiveTheme as _, Theme, ThemeConfig, ThemeMode, ThemeRegistry};
use gpui_kit::{App, Global, SharedString};
use serde::{Deserialize, Serialize};

use super::settings::{self, FontRole, FontSettings, ThemeKind, ThemePreference};

pub fn load_embedded_themes(cx: &mut App) -> Vec<gpui_kit::component::notification::Notification> {
    embedded::load(cx)
}

/// A complete, independent appearance. Missing fonts preserve legacy fallbacks.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThemeDocument {
    #[serde(flatten)]
    config: ThemeConfig,
    #[serde(default)]
    fonts: FontSettings,
}

impl ThemeDocument {
    pub fn from_config(config: ThemeConfig) -> Self {
        Self {
            config,
            fonts: FontSettings::default(),
        }
    }

    pub fn name(&self) -> &str {
        self.config.name.as_str()
    }
    pub const fn mode(&self) -> ThemeMode {
        self.config.mode
    }
    pub const fn config(&self) -> &ThemeConfig {
        &self.config
    }

    pub fn set_name(&mut self, name: &str) {
        self.config.name = name.trim().to_owned().into();
    }

    pub fn font(&self, role: FontRole) -> Option<&str> {
        self.fonts.family(role).or(match role {
            FontRole::Interface => self.config.font_family.as_deref(),
            FontRole::Code => self.config.mono_font_family.as_deref(),
            FontRole::Reading | FontRole::Headings => None,
        })
    }

    pub fn set_font(&mut self, role: FontRole, family: Option<String>) {
        // Keep the legacy GPUI fields in sync for portable interface/code fonts.
        match role {
            FontRole::Interface => self.config.font_family = family.clone().map(Into::into),
            FontRole::Code => self.config.mono_font_family = family.clone().map(Into::into),
            FontRole::Reading | FontRole::Headings => {}
        }
        self.fonts.set_family(role, family);
    }

    pub fn has_fonts(&self) -> bool {
        FontRole::ALL
            .into_iter()
            .any(|role| self.font(role).is_some())
    }

    /// All supported color tokens, including those inherited from the mode defaults.
    pub fn colors(&self) -> Result<BTreeMap<String, Option<String>>> {
        Ok(serde_json::from_value(serde_json::to_value(
            &self.config.colors,
        )?)?)
    }

    pub fn set_color(&mut self, token: &str, value: Option<String>) -> Result<()> {
        let mut colors = self.colors()?;
        let color = colors.get_mut(token).context("Unknown theme color")?;
        if let Some(value) = &value {
            gpui_kit::component::try_parse_color(value)?;
        }
        *color = value;
        self.config.colors = serde_json::from_value(serde_json::to_value(colors)?)?;
        Ok(())
    }
}

struct ThemeEntry {
    document: ThemeDocument,
    path: Option<PathBuf>,
}

/// Owns saved themes separately from the transient preview. Built-ins are immutable.
#[derive(Default)]
pub struct ThemeLibrary {
    entries: BTreeMap<String, ThemeEntry>,
    preview: Option<ThemeDocument>,
    directory: PathBuf,
}

impl Global for ThemeLibrary {}

impl ThemeLibrary {
    pub fn init(cx: &mut App) -> Vec<gpui_kit::component::notification::Notification> {
        let mut library = Self::from_registry(ThemeRegistry::global(cx), directory());
        for document in embedded::documents() {
            library.entries.insert(
                document.name().to_owned(),
                ThemeEntry {
                    document,
                    path: None,
                },
            );
        }
        let errors = library.load();
        cx.set_global(library);
        errors
            .into_iter()
            .map(|(name, error)| crate::ui::notifications::theme_load_failed(&name, &error))
            .collect()
    }

    fn from_registry(registry: &ThemeRegistry, directory: PathBuf) -> Self {
        Self {
            entries: registry
                .themes()
                .iter()
                .map(|(name, config)| {
                    (
                        name.to_string(),
                        ThemeEntry {
                            document: ThemeDocument::from_config((**config).clone()),
                            path: None,
                        },
                    )
                })
                .collect(),
            preview: None,
            directory,
        }
    }

    pub fn get(&self, name: &str) -> Option<&ThemeDocument> {
        self.entries.get(name).map(|entry| &entry.document)
    }
    pub fn is_custom(&self, name: &str) -> bool {
        self.entries
            .get(name)
            .is_some_and(|entry| entry.path.is_some())
    }
    pub fn options(&self, mode: ThemeMode) -> Vec<(SharedString, SharedString)> {
        let mut options: Vec<_> = self
            .entries
            .values()
            .filter(|entry| entry.document.mode() == mode)
            .map(|entry| {
                let name: SharedString = entry.document.name().to_owned().into();
                (name.clone(), name)
            })
            .collect();
        options.sort_by_key(|(name, _)| name.to_lowercase());
        options
    }

    pub fn save(&mut self, document: &ThemeDocument, replacing: Option<&str>) -> Result<()> {
        storage::save(self, document, replacing)
    }
}

#[cfg(not(test))]
fn directory() -> PathBuf {
    super::data_dir().join("themes")
}
#[cfg(test)]
fn directory() -> PathBuf {
    std::env::temp_dir().join(format!("datalith-test-themes-{}", std::process::id()))
}

pub fn document(name: &str, cx: &App) -> Option<ThemeDocument> {
    cx.try_global::<ThemeLibrary>()
        .and_then(|library| library.get(name))
        .cloned()
        .or_else(|| {
            ThemeRegistry::global(cx)
                .themes()
                .get(name)
                .map(|config| ThemeDocument::from_config((**config).clone()))
        })
}

pub fn active_document(cx: &App) -> ThemeDocument {
    let config = active_config(cx);
    document(&config.name, cx).unwrap_or_else(|| ThemeDocument::from_config((**config).clone()))
}

fn active_config(cx: &App) -> &Rc<ThemeConfig> {
    if cx.theme().is_dark() {
        &cx.theme().dark_theme
    } else {
        &cx.theme().light_theme
    }
}

pub fn font(role: FontRole, cx: &App) -> Option<&str> {
    let config = active_config(cx);
    cx.try_global::<ThemeLibrary>().and_then(|library| {
        library
            .preview
            .as_ref()
            .filter(|preview| {
                preview.mode() == config.mode && preview.name() == config.name.as_str()
            })
            .or_else(|| library.get(&config.name))
            .and_then(|document| document.font(role))
    })
}

fn apply_document(document: &ThemeDocument, cx: &mut App) {
    let config = Rc::new(document.config.clone());
    let mode = cx.theme().mode;
    match document.mode() {
        ThemeMode::Light => Theme::global_mut(cx).light_theme = config,
        ThemeMode::Dark => Theme::global_mut(cx).dark_theme = config,
    }
    Theme::change(mode, None, cx);
    super::fonts::apply(cx);
    cx.refresh_windows();
}

pub fn preview(document: &ThemeDocument, cx: &mut App) {
    cx.global_mut::<ThemeLibrary>().preview = Some(document.clone());
    apply_document(document, cx);
}

pub fn discard_preview(cx: &mut App) {
    let preview = cx.global_mut::<ThemeLibrary>().preview.take();
    if let Some(saved) = preview.and_then(|preview| document(preview.name(), cx)) {
        apply_document(&saved, cx);
    }
}

/// Persist selection before changing the running appearance. Explicit activation
/// selects the document's mode; choosing a light/dark slot in Settings does not.
pub fn select(name: &str, activate: bool, cx: &mut App) -> Result<()> {
    let document = document(name, cx).context("Theme is unavailable")?;
    if cx
        .try_global::<ThemeLibrary>()
        .is_some_and(|library| library.preview.is_some())
    {
        bail!("Save or discard the theme draft first");
    }
    let (kind, preference) = match document.mode() {
        ThemeMode::Light => (ThemeKind::Light, ThemePreference::Light),
        ThemeMode::Dark => (ThemeKind::Dark, ThemePreference::Dark),
    };
    settings::select_theme_with_preference(kind, name, activate.then_some(preference))?;
    let options = cx.global_mut::<crate::ui::settings::ThemeOptions>();
    match kind {
        ThemeKind::Light => options.light_theme_name = name.to_owned().into(),
        ThemeKind::Dark => options.dark_theme_name = name.to_owned().into(),
    }
    apply_document(&document, cx);
    if activate {
        super::preferences::apply_theme_preference(preference, cx);
    }
    Ok(())
}
