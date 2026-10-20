//! Theme families, stable variant identity, selection, and isolated appearances.

mod embedded;
mod storage;
#[cfg(test)]
mod tests;

use std::{
    path::PathBuf,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};

use anyhow::{Context as _, Result, bail, ensure};
use gpui_kit::component::{
    Theme, ThemeConfig, ThemeMode,
    highlighter::{HighlightTheme, HighlightThemeStyle},
    notification::Notification,
};
use gpui_kit::{App, Global, SharedString, Subscription};
use serde::{Deserialize, Serialize};

use super::settings::{self, FontRole, FontSettings, ThemeKind};
use storage::{LoadFailure, StoredThemeSet};

const DEFAULT_LIGHT: &str = "Datalith Light";
const DEFAULT_DARK: &str = "Datalith Dark";
static ID: AtomicU64 = AtomicU64::new(1);
fn next_id() -> u64 {
    ID.fetch_add(1, Ordering::Relaxed)
}

/// The first generated variant name that is not taken.
fn free_variant_name(family: &str, multi: bool, taken: impl Fn(&str) -> bool) -> String {
    let mut n = 1u64;
    loop {
        let candidate = if multi || n > 1 {
            format!("{family} Variant {n}")
        } else {
            family.to_owned()
        };
        if !taken(&candidate) {
            return candidate;
        }
        n = n.saturating_add(1);
    }
}

fn valid_variant_name(family: &str, variant: &str, multi: bool) -> bool {
    (!multi && variant == family)
        || variant
            .strip_prefix(family)
            .and_then(|tail| tail.strip_prefix(' '))
            .is_some_and(|suffix| !suffix.trim().is_empty())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThemeDocument {
    #[serde(flatten)]
    config: ThemeConfig,
    #[serde(default, skip_serializing_if = "FontSettings::is_empty")]
    fonts: FontSettings,
}

impl ThemeDocument {
    #[cfg(test)]
    pub fn from_config(config: ThemeConfig) -> Self {
        Self {
            config,
            fonts: FontSettings::default(),
        }
    }
    pub fn name(&self) -> &str {
        &self.config.name
    }
    pub const fn mode(&self) -> ThemeMode {
        self.config.mode
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
        self.fonts.set_family(role, family);
        let family = self
            .fonts
            .family(role)
            .map(|family| SharedString::from(family.to_owned()));
        match role {
            FontRole::Interface => self.config.font_family = family,
            FontRole::Code => self.config.mono_font_family = family,
            FontRole::Reading | FontRole::Headings => {}
        }
    }
    fn normalize_fonts(&mut self) {
        for role in FontRole::ALL {
            self.fonts
                .set_family(role, self.fonts.family(role).map(str::to_owned));
            let family = self.font(role).map(str::to_owned);
            self.set_font(role, family);
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ThemeSource {
    Bundled,
    Custom(PathBuf),
}

#[derive(Clone, Debug)]
pub struct ThemeVariant {
    id: u64,
    document: ThemeDocument,
}
impl ThemeVariant {
    pub const fn id(&self) -> u64 {
        self.id
    }
    pub fn name(&self) -> &str {
        self.document.name()
    }
    pub const fn mode(&self) -> ThemeMode {
        self.document.mode()
    }
    pub const fn document(&self) -> &ThemeDocument {
        &self.document
    }
}

#[derive(Clone, Debug)]
pub enum SaveStatus {
    Autosaved,
    Failed,
}

#[derive(Clone, Debug)]
pub struct ThemeFamily {
    id: u64,
    name: String,
    author: Option<String>,
    url: Option<String>,
    variants: Vec<ThemeVariant>,
    source: ThemeSource,
    status: SaveStatus,
}
impl ThemeFamily {
    pub const fn id(&self) -> u64 {
        self.id
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn variants(&self) -> &[ThemeVariant] {
        &self.variants
    }
    pub fn suffix(&self, variant: u64) -> Option<&str> {
        self.variants.iter().find(|v| v.id == variant).map(|v| {
            v.name()
                .strip_prefix(&format!("{} ", self.name))
                .unwrap_or_default()
        })
    }
    fn set(&self) -> StoredThemeSet {
        StoredThemeSet {
            name: self.name.clone().into(),
            author: self.author.clone().map(Into::into),
            url: self.url.clone().map(Into::into),
            themes: self.variants.iter().map(|v| v.document.clone()).collect(),
        }
    }
    pub const fn is_custom(&self) -> bool {
        matches!(self.source, ThemeSource::Custom(_))
    }
}

/// A snapshot that may be rendered without modifying any application global.
#[derive(Clone)]
pub struct ResolvedAppearance {
    theme: Theme,
    fonts: [SharedString; 4],
}
impl ResolvedAppearance {
    pub const fn theme(&self) -> &Theme {
        &self.theme
    }
    pub const fn font(&self, role: FontRole) -> &SharedString {
        let [interface, reading, headings, code] = &self.fonts;
        match role {
            FontRole::Interface => interface,
            FontRole::Reading => reading,
            FontRole::Headings => headings,
            FontRole::Code => code,
        }
    }
}

/// Owns persisted families and current slots; views retain only IDs and input state.
pub struct ThemeLibrary {
    families: Vec<ThemeFamily>,
    defaults: [ThemeDocument; 2],
    directory: PathBuf,
    slots: [String; 2],
    quit_subscription: Option<Subscription>,
}
impl Global for ThemeLibrary {}

impl ThemeLibrary {
    pub fn init(cx: &mut App) -> Vec<Notification> {
        let mut library = Self::new(super::data_dir().join("themes"));
        for set in embedded::sets() {
            let _ = library.insert_set(set, ThemeSource::Bundled);
        }
        let mut pending: Vec<_> = library
            .load()
            .into_iter()
            .map(|failure| match failure {
                LoadFailure::Read { theme, error } => {
                    crate::ui::notifications::theme_load_failed(&theme, &error)
                }
                LoadFailure::Normalization { theme, error } => {
                    crate::ui::notifications::settings_save_failed(
                        &format!(
                            "normalized changes to the {theme} theme (loaded for this session)"
                        ),
                        &error,
                    )
                }
            })
            .collect();
        let prefs = settings::snapshot();
        let saved = [prefs.light_theme_name, prefs.dark_theme_name];
        for (kind, saved) in ThemeKind::ALL.into_iter().zip(saved.clone()) {
            if let Some(name) = saved {
                if library
                    .variant(&name)
                    .is_some_and(|v| v.mode() == kind.mode())
                {
                    *kind.select_mut(&mut library.slots) = name;
                } else {
                    pending.push(crate::ui::notifications::theme_fallback(
                        &name,
                        library.current(kind),
                    ));
                }
            }
        }
        // Avoid rewriting the settings file on every launch.
        let preference_error = (saved != library.slots.clone().map(Some))
            .then(|| settings::replace_theme_slots(&library.slots).err())
            .flatten();
        cx.set_global(library);
        let quit_subscription = cx.on_app_quit(|cx| {
            for (id, error) in cx.global_mut::<Self>().flush_pending() {
                eprintln!("Could not save theme {id} on exit: {error}");
            }
            async {}
        });
        cx.global_mut::<Self>().quit_subscription = Some(quit_subscription);
        refresh_current(cx);
        if let Some(error) = preference_error {
            pending.push(crate::ui::notifications::theme_load_failed(
                "Current themes",
                &error,
            ));
        }
        pending
    }

    pub fn new(directory: PathBuf) -> Self {
        let defaults = embedded::defaults();
        Self {
            families: vec![],
            defaults,
            directory,
            slots: [DEFAULT_LIGHT.into(), DEFAULT_DARK.into()],
            quit_subscription: None,
        }
    }
    pub fn families(&self) -> impl Iterator<Item = &ThemeFamily> {
        self.families.iter()
    }
    pub fn family(&self, id: u64) -> Option<&ThemeFamily> {
        self.families.iter().find(|family| family.id == id)
    }
    pub fn variant(&self, name: &str) -> Option<&ThemeVariant> {
        self.families
            .iter()
            .flat_map(|family| &family.variants)
            .find(|variant| variant.name() == name)
    }
    pub fn variant_by_id(&self, id: u64) -> Option<&ThemeVariant> {
        self.families
            .iter()
            .flat_map(|family| &family.variants)
            .find(|variant| variant.id == id)
    }
    pub fn current(&self, kind: ThemeKind) -> &str {
        kind.select(&self.slots)
    }
    /// Resolve the current selection, falling back to the built-in document
    /// for its mode if the selected variant is unavailable.
    pub fn current_document(&self, kind: ThemeKind) -> &ThemeDocument {
        self.get(self.current(kind))
            .unwrap_or_else(|| kind.select(&self.defaults))
    }
    pub fn get(&self, name: &str) -> Option<&ThemeDocument> {
        self.variant(name).map(ThemeVariant::document)
    }
    fn unique_name(&self, name: &str, except: Option<u64>) -> Result<()> {
        ensure!(!name.trim().is_empty(), "Enter a theme name");
        ensure!(
            !self
                .families
                .iter()
                .any(|f| Some(f.id) != except && f.name.eq_ignore_ascii_case(name)),
            "A theme with this name already exists"
        );
        Ok(())
    }
    fn replace_slots(&mut self, slots: [String; 2]) -> Result<()> {
        settings::replace_theme_slots(&slots)?;
        self.slots = slots;
        Ok(())
    }
    pub fn set_current(&mut self, id: u64, kind: ThemeKind) -> Result<()> {
        let variant = self
            .variant_by_id(id)
            .context("Theme variant is unavailable")?;
        ensure!(
            variant.mode() == kind.mode(),
            "Theme variant has the wrong mode"
        );
        let mut slots = self.slots.clone();
        *kind.select_mut(&mut slots) = variant.name().into();
        self.replace_slots(slots)?;
        Ok(())
    }
    /// The first free "{name} - Copy" name, then "{name} - Copy 2", and so on.
    pub fn suggested_copy_name(&self, name: &str) -> String {
        let mut candidate = format!("{name} - Copy");
        let mut n = 2u64;
        while self
            .families
            .iter()
            .any(|family| family.name.eq_ignore_ascii_case(&candidate))
        {
            candidate = format!("{name} - Copy {n}");
            n = n.saturating_add(1);
        }
        candidate
    }
    /// Flush the latest revision for one family. A failed write retains the model and exposes Retry.
    pub fn flush_family(&mut self, id: u64) -> Result<()> {
        let family = self
            .families
            .iter_mut()
            .find(|family| family.id == id)
            .context("Theme is unavailable")?;
        let ThemeSource::Custom(ref path) = family.source else {
            bail!("Built-in themes cannot be saved")
        };
        let result = storage::write(path, &family.set());
        family.status = match &result {
            Ok(()) => SaveStatus::Autosaved,
            Err(_) => SaveStatus::Failed,
        };
        result
    }
    pub fn flush_pending(&mut self) -> Vec<(u64, anyhow::Error)> {
        let mut errors = Vec::new();
        for ix in 0..self.families.len() {
            if let Some(family) = self.families.get(ix)
                && matches!(family.status, SaveStatus::Failed)
            {
                let id = family.id;
                if let Err(error) = self.flush_family(id) {
                    errors.push((id, error));
                }
            }
        }
        errors
    }
    pub fn resolved(
        &self,
        id: u64,
        fonts: &super::fonts::FontCatalog,
    ) -> Result<ResolvedAppearance> {
        let document = &self
            .variant_by_id(id)
            .context("Theme variant is unavailable")?
            .document;
        Ok(resolve(document, &self.defaults, fonts))
    }
    pub fn resolved_config(&self, name: &str) -> Option<ThemeConfig> {
        let document = self.get(name)?;
        Some(merged_document(document, &self.defaults).config)
    }
}

fn resolve(
    document: &ThemeDocument,
    defaults: &[ThemeDocument; 2],
    fonts: &super::fonts::FontCatalog,
) -> ResolvedAppearance {
    let resolved = merged_document(document, defaults);
    let families = fonts.resolve_roles(&resolved);
    let mut theme = Theme::default();
    theme.apply_config(&Rc::new(resolved.config));
    let [interface, _, _, code] = &families;
    theme.font_family = interface.clone();
    theme.mono_font_family = code.clone();
    ResolvedAppearance {
        theme,
        fonts: families,
    }
}

fn merged_document(document: &ThemeDocument, defaults: &[ThemeDocument; 2]) -> ThemeDocument {
    let default = ThemeKind::from(document.mode()).select(defaults);
    let explicit_colors = document.config.colors.clone();
    let explicit_highlight = document.config.highlight.clone();
    let mut merged = serde_json::to_value(default).unwrap_or_default();
    if let Ok(mut value) = serde_json::to_value(document) {
        storage::merge_missing(&mut value, &merged);
        merged = value;
    }
    let mut merged: ThemeDocument =
        serde_json::from_value(merged).unwrap_or_else(|_| default.clone());

    // Datalith supplies inherited presentation settings and font roles, but
    // colors are sparse overrides: GPUI owns their mode-aware fallback graph.
    merged.config.colors = explicit_colors;

    // Keep explicit syntax colors and styles, filling the rest from GPUI's
    // built-in mode-specific highlight theme instead of Datalith's palette.
    let default_highlight = gpui_highlight_fallback(document.mode());
    let mut highlight = explicit_highlight
        .as_ref()
        .and_then(|highlight| serde_json::to_value(highlight).ok())
        .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));
    if let Ok(default_highlight) = serde_json::to_value(&default_highlight) {
        storage::merge_missing(&mut highlight, &default_highlight);
    }
    merged.config.highlight = serde_json::from_value(highlight)
        .ok()
        .or(Some(default_highlight));
    merged
}

fn gpui_highlight_fallback(mode: ThemeMode) -> HighlightThemeStyle {
    let theme = if mode == ThemeMode::Dark {
        HighlightTheme::default_dark()
    } else {
        HighlightTheme::default_light()
    };
    theme.style.clone()
}

/// Applies the library's validated slots and the session's appearance preference.
pub fn refresh_current(cx: &mut App) {
    let library = cx.global::<ThemeLibrary>();
    let light = library
        .resolved_config(library.current(ThemeKind::Light))
        .map(Rc::new);
    let dark = library
        .resolved_config(library.current(ThemeKind::Dark))
        .map(Rc::new);
    if let Some(light) = light {
        Theme::global_mut(cx).light_theme = light;
    }
    if let Some(dark) = dark {
        Theme::global_mut(cx).dark_theme = dark;
    }
    super::preferences::apply_theme_preference(settings::snapshot().theme_preference, cx);
}
