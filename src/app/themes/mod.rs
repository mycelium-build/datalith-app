//! Theme families, stable variant identity, selection, and isolated appearances.

mod embedded;
mod storage;
#[cfg(test)]
mod tests;

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use anyhow::{Context as _, Result, bail, ensure};
use gpui_kit::component::{ActiveTheme as _, Theme, ThemeConfig, ThemeMode, ThemeRegistry};
use gpui_kit::{App, Global, SharedString, Subscription};
use serde::{Deserialize, Serialize};

use super::settings::{self, FontRole, FontSettings, ThemeKind};
use storage::StoredThemeSet;

const DEFAULT_LIGHT: &str = "Datalith Light";
const DEFAULT_DARK: &str = "Datalith Dark";
static ID: AtomicU64 = AtomicU64::new(1);
fn next_id() -> u64 {
    ID.fetch_add(1, Ordering::Relaxed)
}

fn valid_variant_name(family: &str, variant: &str, multi: bool) -> bool {
    (!multi && variant == family)
        || variant
            .strip_prefix(family)
            .and_then(|tail| tail.strip_prefix(' '))
            .is_some_and(|suffix| !suffix.trim().is_empty())
}

pub fn load_embedded_themes(cx: &mut App) -> Vec<gpui_kit::component::notification::Notification> {
    embedded::load(cx)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThemeDocument {
    #[serde(flatten)]
    config: ThemeConfig,
    #[serde(default, skip_serializing_if = "FontSettings::is_empty")]
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
        &self.config.name
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
        match role {
            FontRole::Interface => self.config.font_family = family.clone().map(Into::into),
            FontRole::Code => self.config.mono_font_family = family.clone().map(Into::into),
            FontRole::Reading | FontRole::Headings => {}
        }
        self.fonts.set_family(role, family);
    }
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
    pub fn set_highlight(&mut self, token: &str, value: Option<String>) -> Result<()> {
        if let Some(ref value) = value {
            gpui_kit::component::try_parse_color(value)?;
        }
        let mut highlight = serde_json::to_value(&self.config.highlight)?
            .as_object()
            .cloned()
            .unwrap_or_default();
        if let Some(syntax) = token.strip_prefix("syntax.") {
            ensure!(!syntax.is_empty(), "Unknown highlight token");
            let node = highlight
                .entry("syntax")
                .or_insert_with(|| serde_json::json!({}))
                .as_object_mut()
                .context("Invalid syntax overrides")?;
            match value {
                Some(value) => {
                    node.entry(syntax)
                        .or_insert_with(|| serde_json::json!({}))
                        .as_object_mut()
                        .context("Invalid syntax style")?
                        .insert("color".into(), value.into());
                }
                None => {
                    if let Some(style) = node.get_mut(syntax) {
                        let style = style.as_object_mut().context("Invalid syntax style")?;
                        style.remove("color");
                        if style.is_empty() {
                            node.remove(syntax);
                        }
                    }
                }
            }
        } else {
            ensure!(
                token.starts_with("editor.")
                    || [
                        "created",
                        "modified",
                        "warning",
                        "conflict",
                        "hint",
                        "hidden",
                        "predictive"
                    ]
                    .contains(&token),
                "Unknown highlight token"
            );
            match value {
                Some(value) => {
                    highlight.insert(token.into(), value.into());
                }
                None => {
                    highlight.remove(token);
                }
            }
        }
        self.config.highlight = Some(serde_json::from_value(serde_json::Value::Object(
            highlight,
        ))?);
        Ok(())
    }
    pub const fn set_mode(&mut self, mode: ThemeMode) {
        self.config.mode = mode;
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
    Saving,
    Autosaved,
    Failed(String),
}

#[derive(Clone, Debug)]
pub struct ThemeFamily {
    id: u64,
    name: String,
    author: Option<String>,
    url: Option<String>,
    variants: Vec<ThemeVariant>,
    source: ThemeSource,
    revision: u64,
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
    pub const fn source(&self) -> &ThemeSource {
        &self.source
    }
    pub(crate) const fn revision(&self) -> u64 {
        self.revision
    }
    pub const fn status(&self) -> &SaveStatus {
        &self.status
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
    fn persisted(&mut self) {
        self.revision = self.revision.saturating_add(1);
        self.status = SaveStatus::Autosaved;
    }
}

/// A snapshot that may be rendered without modifying any application global.
#[derive(Clone)]
pub struct ResolvedAppearance {
    document: ThemeDocument,
    theme: Theme,
    fonts: [SharedString; 4],
    colors: BTreeMap<String, gpui_kit::Hsla>,
}
impl ResolvedAppearance {
    pub const fn theme(&self) -> &Theme {
        &self.theme
    }
    pub fn color(&self, token: &str) -> Option<gpui_kit::Hsla> {
        self.colors.get(token).copied()
    }
    pub const fn highlight(
        &self,
    ) -> Option<&gpui_kit::component::highlighter::HighlightThemeStyle> {
        self.document.config.highlight.as_ref()
    }
    #[allow(
        clippy::indexing_slicing,
        reason = "FontRole indices are exhaustive over the fixed four-entry role array"
    )]
    pub const fn font(&self, role: FontRole) -> &SharedString {
        &self.fonts[role.index()]
    }
    #[cfg(test)]
    pub fn configured_font(&self, role: FontRole) -> Option<&str> {
        self.document.font(role)
    }
}

/// Owns persisted families and current slots; views retain only IDs and input state.
pub struct ThemeLibrary {
    families: Vec<ThemeFamily>,
    defaults: [ThemeDocument; 2],
    directory: PathBuf,
    slots: [String; 2],
    // Only explicit selection advances these; Undo must not undo a later choice,
    // even when that choice reselects the automatic fallback by name.
    selection_revisions: [u64; 2],
    preferences_file: Option<PathBuf>,
    quit_subscription: Option<Subscription>,
}
impl Global for ThemeLibrary {}

// Family offsets originate in checked position/find operations; slot indices
// are the exhaustive two-case ThemeKind mapping. Keep identity/name validation
// at this boundary rather than spreading unchecked indices into views.
#[allow(
    clippy::indexing_slicing,
    reason = "indices come from validated family lookups or exhaustive ThemeKind/FontRole mappings"
)]
impl ThemeLibrary {
    pub fn init(cx: &mut App) -> Vec<gpui_kit::component::notification::Notification> {
        let mut library = Self::new(directory());
        for set in embedded::sets() {
            let _ = library.insert_set(set, ThemeSource::Bundled);
        }
        let errors = library.load();
        let prefs = settings::snapshot();
        let mut pending = Vec::new();
        for (ix, saved) in [prefs.light_theme_name, prefs.dark_theme_name]
            .into_iter()
            .enumerate()
        {
            if let Some(name) = saved {
                if library.variant(&name).is_some_and(|v| {
                    v.mode()
                        == if ix == 0 {
                            ThemeMode::Light
                        } else {
                            ThemeMode::Dark
                        }
                }) {
                    if let Some(slot) = library.slots.get_mut(ix) {
                        *slot = name;
                    }
                } else if let Some(fallback) = library.slots.get(ix) {
                    pending.push(crate::ui::notifications::theme_fallback(&name, fallback));
                }
            }
        }
        let preference_error = library.persist_slots(&library.slots).err();
        cx.set_global(library);
        let quit_subscription = cx.on_app_quit(|cx| {
            if cx.try_global::<Self>().is_some() {
                for (id, error) in cx.global_mut::<Self>().flush_pending() {
                    eprintln!("Could not save theme {id} on exit: {error}");
                }
            }
            async {}
        });
        cx.global_mut::<Self>().quit_subscription = Some(quit_subscription);
        if let Some(error) = preference_error {
            // The fallback stays valid in memory; report the preference failure.
            let mut notifications: Vec<_> = errors
                .into_iter()
                .map(|(name, error)| crate::ui::notifications::theme_load_failed(&name, &error))
                .collect();
            notifications.push(crate::ui::notifications::theme_load_failed(
                "Current themes",
                &error,
            ));
            notifications.extend(pending);
            return notifications;
        }
        pending.extend(
            errors
                .into_iter()
                .map(|(name, error)| crate::ui::notifications::theme_load_failed(&name, &error)),
        );
        pending
    }

    pub fn new(directory: PathBuf) -> Self {
        let defaults = embedded::defaults();
        Self {
            families: vec![],
            defaults,
            directory,
            slots: [DEFAULT_LIGHT.into(), DEFAULT_DARK.into()],
            selection_revisions: [0; 2],
            preferences_file: None,
            quit_subscription: None,
        }
    }
    #[cfg(test)]
    fn with_preferences_file(mut self, file: PathBuf) -> Self {
        self.preferences_file = Some(file);
        self
    }

    pub fn families(&self) -> impl Iterator<Item = &ThemeFamily> {
        self.families.iter()
    }
    pub fn family(&self, id: u64) -> Option<&ThemeFamily> {
        self.families.iter().find(|family| family.id == id)
    }
    #[cfg(test)]
    pub fn family_named(&self, name: &str) -> Option<&ThemeFamily> {
        self.families.iter().find(|family| family.name == name)
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
        &self.slots[kind.index()]
    }
    pub fn get(&self, name: &str) -> Option<&ThemeDocument> {
        self.variant(name).map(ThemeVariant::document)
    }
    fn family_index(&self, id: u64) -> Result<usize> {
        self.families
            .iter()
            .position(|f| f.id == id)
            .context("Theme is unavailable")
    }
    fn editable_index(&self, id: u64) -> Result<usize> {
        let ix = self.family_index(id)?;
        ensure!(
            matches!(self.families[ix].source, ThemeSource::Custom(_)),
            "Built-in themes cannot be edited"
        );
        Ok(ix)
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
    fn unique_variant_names<'a>(
        &self,
        names: impl IntoIterator<Item = &'a str>,
        except_family: Option<u64>,
    ) -> Result<()> {
        let mut seen: Vec<&str> = Vec::new();
        for name in names {
            ensure!(!name.trim().is_empty(), "Enter a variant name");
            ensure!(
                !seen.iter().any(|other| other.eq_ignore_ascii_case(name)),
                "Variant names must be unique"
            );
            ensure!(
                !self
                    .families
                    .iter()
                    .filter(|family| Some(family.id) != except_family)
                    .flat_map(|family| &family.variants)
                    .any(|variant| variant.name().eq_ignore_ascii_case(name)),
                "A variant with this name already exists"
            );
            seen.push(name);
        }
        Ok(())
    }
    fn persist_slots(&self, slots: &[String; 2]) -> Result<()> {
        settings::replace_theme_slots(self.preferences_file.as_deref(), &slots[0], &slots[1])
    }
    fn replace_slots(&mut self, slots: [String; 2]) -> Result<()> {
        self.persist_slots(&slots)?;
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
        slots[kind.index()] = variant.name().into();
        self.replace_slots(slots)?;
        self.selection_revisions[kind.index()] =
            self.selection_revisions[kind.index()].saturating_add(1);
        Ok(())
    }
    fn fallback(&self, mode: ThemeMode, excluding: &[u64]) -> String {
        self.families
            .iter()
            .flat_map(|f| &f.variants)
            .find(|v| v.mode() == mode && !excluding.contains(&v.id))
            .map_or_else(
                || {
                    if mode == ThemeMode::Dark {
                        DEFAULT_DARK
                    } else {
                        DEFAULT_LIGHT
                    }
                    .to_owned()
                },
                |v| v.name().to_owned(),
            )
    }
    fn fallback_in_family(&self, ix: usize, mode: ThemeMode, excluding: &[u64]) -> String {
        self.families[ix]
            .variants
            .iter()
            .find(|v| v.mode() == mode && !excluding.contains(&v.id))
            .map_or_else(|| self.fallback(mode, excluding), |v| v.name().to_owned())
    }
    fn changed(&mut self, ix: usize) {
        let family = &mut self.families[ix];
        family.revision = family.revision.saturating_add(1);
        family.status = SaveStatus::Saving;
    }
    pub fn update_color(&mut self, id: u64, token: &str, value: Option<String>) -> Result<u64> {
        self.update_document(id, |document| document.set_color(token, value))
    }
    pub fn update_highlight(&mut self, id: u64, token: &str, value: Option<String>) -> Result<u64> {
        self.update_document(id, |document| document.set_highlight(token, value))
    }
    pub fn update_font(&mut self, id: u64, role: FontRole, family: Option<String>) -> Result<u64> {
        self.update_document(id, |document| {
            document.set_font(role, family);
            Ok(())
        })
    }
    pub fn apply_fonts_to_all(&mut self, id: u64) -> Result<u64> {
        let ix = self
            .families
            .iter()
            .position(|family| family.variants.iter().any(|variant| variant.id == id))
            .context("Theme variant is unavailable")?;
        self.editable_index(self.families[ix].id)?;
        let source = self
            .variant_by_id(id)
            .context("Theme variant is unavailable")?
            .document
            .clone();
        for variant in &mut self.families[ix].variants {
            for role in FontRole::ALL {
                variant
                    .document
                    .set_font(role, source.font(role).map(str::to_owned));
            }
        }
        self.changed(ix);
        Ok(self.families[ix].id)
    }
    pub fn update_document(
        &mut self,
        variant_id: u64,
        change: impl FnOnce(&mut ThemeDocument) -> Result<()>,
    ) -> Result<u64> {
        let ix = self
            .families
            .iter()
            .position(|f| f.variants.iter().any(|v| v.id == variant_id))
            .context("Theme variant is unavailable")?;
        self.editable_index(self.families[ix].id)?;
        let variant = self.families[ix]
            .variants
            .iter_mut()
            .find(|v| v.id == variant_id)
            .context("Theme variant is unavailable")?;
        let mut next = variant.document.clone();
        change(&mut next)?;
        ensure!(
            next.name() == variant.name() && next.mode() == variant.mode(),
            "Use rename or change mode for identity changes"
        );
        variant.document = next;
        self.changed(ix);
        Ok(self.families[ix].id)
    }
    pub fn rename_family(&mut self, id: u64, name: &str) -> Result<()> {
        let ix = self.editable_index(id)?;
        let name = name.trim();
        self.unique_name(name, Some(id))?;
        let old = self.families[ix].clone();
        let mut next = old.clone();
        next.name = name.into();
        for variant in &mut next.variants {
            let suffix = variant
                .name()
                .strip_prefix(&old.name)
                .unwrap_or_else(|| variant.name());
            variant.document.set_name(&format!("{name}{suffix}"));
        }
        self.unique_variant_names(next.variants.iter().map(ThemeVariant::name), Some(id))?;
        let mut slots = self.slots.clone();
        for (before, after) in old.variants.iter().zip(&next.variants) {
            for slot in &mut slots {
                if slot == before.name() {
                    *slot = after.name().into();
                }
            }
        }
        if let ThemeSource::Custom(ref path) = old.source {
            storage::replace_and_update(path, &next.set(), || self.persist_slots(&slots))?;
        }
        next.persisted();
        self.slots = slots;
        self.families[ix] = next;
        Ok(())
    }
    pub fn rename_variant(&mut self, id: u64, suffix: &str) -> Result<()> {
        let ix = self
            .families
            .iter()
            .position(|f| f.variants.iter().any(|v| v.id == id))
            .context("Theme variant is unavailable")?;
        self.editable_index(self.families[ix].id)?;
        let suffix = suffix.trim();
        ensure!(
            !suffix.is_empty() || self.families[ix].variants.len() == 1,
            "Enter a variant suffix"
        );
        ensure!(
            !self.families[ix].variants.iter().any(|v| v.id != id
                && v.name()
                    .eq_ignore_ascii_case(&format!("{} {suffix}", self.families[ix].name))),
            "Variant suffix already exists"
        );
        let name = if suffix.is_empty() {
            self.families[ix].name.clone()
        } else {
            format!("{} {suffix}", self.families[ix].name)
        };
        self.unique_variant_names(std::iter::once(name.as_str()), Some(self.families[ix].id))?;
        let old = self
            .variant_by_id(id)
            .context("Theme variant is unavailable")?
            .name()
            .to_owned();
        let mut slots = self.slots.clone();
        for slot in &mut slots {
            if slot == &old {
                slot.clone_from(&name);
            }
        }
        let mut next = self.families[ix].clone();
        next.variants
            .iter_mut()
            .find(|v| v.id == id)
            .context("Theme variant is unavailable")?
            .document
            .set_name(&name);
        if let ThemeSource::Custom(ref path) = next.source {
            storage::replace_and_update(path, &next.set(), || self.persist_slots(&slots))?;
        }
        next.persisted();
        self.slots = slots;
        self.families[ix] = next;
        Ok(())
    }
    /// The first addition supplies both suffixes atomically. For later additions `existing_suffix` is None.
    pub fn add_variant(
        &mut self,
        family_id: u64,
        from: u64,
        suffix: &str,
        existing_suffix: Option<&str>,
    ) -> Result<u64> {
        let ix = self.editable_index(family_id)?;
        let source = self.families[ix]
            .variants
            .iter()
            .find(|v| v.id == from)
            .context("Source variant is unavailable")?
            .clone();
        let suffix = suffix.trim();
        ensure!(!suffix.is_empty(), "Enter a variant suffix");
        if self.families[ix].variants.len() == 1 {
            let existing = existing_suffix
                .context("Name both variants before adding")?
                .trim();
            ensure!(
                !existing.is_empty() && !existing.eq_ignore_ascii_case(suffix),
                "Variant suffixes must be non-empty and unique"
            );
            let first_name = format!("{} {existing}", self.families[ix].name);
            let second_name = format!("{} {suffix}", self.families[ix].name);
            self.unique_variant_names(
                [first_name.as_str(), second_name.as_str()],
                Some(family_id),
            )?;
            let mut next = self.families[ix].clone();
            next.variants[0].document.set_name(&first_name);
            let id = next_id();
            let original_name = source.name().to_owned();
            let mut document = source.document;
            document.set_name(&second_name);
            next.variants.push(ThemeVariant { id, document });
            let mut slots = self.slots.clone();
            for slot in &mut slots {
                if slot == &original_name {
                    slot.clone_from(&first_name);
                }
            }
            if let ThemeSource::Custom(ref path) = next.source {
                storage::replace_and_update(path, &next.set(), || self.persist_slots(&slots))?;
            }
            next.persisted();
            self.slots = slots;
            self.families[ix] = next;
            return Ok(id);
        }
        {
            let family = &self.families[ix];
            ensure!(
                !family.variants.iter().any(|v| family
                    .suffix(v.id)
                    .is_some_and(|s| s.eq_ignore_ascii_case(suffix))),
                "Variant suffix already exists"
            );
        }
        let mut document = source.document;
        document.set_name(&format!("{} {suffix}", self.families[ix].name));
        self.unique_variant_names(std::iter::once(document.name()), Some(family_id))?;
        let id = next_id();
        self.families[ix]
            .variants
            .push(ThemeVariant { id, document });
        self.changed(ix);
        Ok(id)
    }
    pub fn next_suffix(&self, family_id: u64) -> Result<String> {
        let family = self.family(family_id).context("Theme is unavailable")?;
        for n in 1.. {
            let suffix = format!("Variant {n}");
            if !family.variants.iter().any(|v| {
                family
                    .suffix(v.id)
                    .is_some_and(|s| s.eq_ignore_ascii_case(&suffix))
            }) {
                return Ok(suffix);
            }
        }
        bail!("No variant name available")
    }
    pub fn change_mode(&mut self, id: u64, mode: ThemeMode) -> Result<()> {
        let ix = self
            .families
            .iter()
            .position(|f| f.variants.iter().any(|v| v.id == id))
            .context("Theme variant is unavailable")?;
        self.editable_index(self.families[ix].id)?;
        let old = self
            .variant_by_id(id)
            .context("Theme variant is unavailable")?;
        if old.mode() == mode {
            return Ok(());
        }
        let old_name = old.name().to_owned();
        let old_mode = old.mode();
        let mut slots = self.slots.clone();
        if slots[ThemeKind::from(old_mode).index()] == old_name {
            slots[ThemeKind::from(old_mode).index()] = self.fallback_in_family(ix, old_mode, &[id]);
        }
        let mut next = self.families[ix].clone();
        next.variants
            .iter_mut()
            .find(|v| v.id == id)
            .context("Theme variant is unavailable")?
            .document
            .set_mode(mode);
        if let ThemeSource::Custom(ref path) = next.source {
            storage::replace_and_update(path, &next.set(), || self.persist_slots(&slots))?;
        }
        next.persisted();
        self.slots = slots;
        self.families[ix] = next;
        Ok(())
    }
    pub fn copy_family(&mut self, id: u64, name: &str) -> Result<u64> {
        self.unique_name(name, None)?;
        let original = self.family(id).context("Theme is unavailable")?.clone();
        let mut set = original.set();
        set.name = name.trim().into();
        for variant in &mut set.themes {
            let tail = variant
                .name()
                .strip_prefix(&original.name)
                .unwrap_or_else(|| variant.name())
                .to_owned();
            variant.set_name(&format!("{}{tail}", name.trim()));
            variant.config.is_default = false;
        }
        self.unique_variant_names(set.themes.iter().map(ThemeDocument::name), None)?;
        let path = self.new_path();
        storage::write(&path, &set)?;
        self.insert_set(set, ThemeSource::Custom(path))
    }
    pub fn suggested_copy_name(&self, name: &str) -> String {
        let mut candidate = format!("{name} copy");
        let mut n = 2u64;
        while self
            .families
            .iter()
            .any(|family| family.name.eq_ignore_ascii_case(&candidate))
        {
            candidate = format!("{name} copy {n}");
            n = n.saturating_add(1);
        }
        candidate
    }
    fn new_path(&self) -> PathBuf {
        self.directory
            .join(format!("theme-{:032x}.json", rand::random::<u128>()))
    }
    pub fn remove_variant(&mut self, id: u64) -> Result<DeletedTheme> {
        let ix = self
            .families
            .iter()
            .position(|f| f.variants.iter().any(|v| v.id == id))
            .context("Theme variant is unavailable")?;
        self.editable_index(self.families[ix].id)?;
        if self.families[ix].variants.len() == 1 {
            return self.delete_family(self.families[ix].id);
        }
        let old = self.families[ix].clone();
        let before = self.slots.clone();
        let mut next = old.clone();
        let position = next
            .variants
            .iter()
            .position(|v| v.id == id)
            .context("Theme variant is unavailable")?;
        let removed = next.variants.remove(position);
        let mut slots = self.slots.clone();
        if slots[ThemeKind::from(removed.mode()).index()] == removed.name() {
            slots[ThemeKind::from(removed.mode()).index()] =
                self.fallback_in_family(ix, removed.mode(), &[id]);
        }
        if let ThemeSource::Custom(ref path) = old.source {
            storage::replace_and_update(path, &next.set(), || self.persist_slots(&slots))?;
        }
        next.persisted();
        self.slots = slots;
        self.families[ix] = next;
        Ok(DeletedTheme {
            old,
            position: ix,
            slots_before: before,
            selection_revisions: self.selection_revisions,
            removed_variant_id: Some(removed.id),
        })
    }
    pub fn delete_family(&mut self, id: u64) -> Result<DeletedTheme> {
        let ix = self.editable_index(id)?;
        let old = self.families[ix].clone();
        let before = self.slots.clone();
        let mut slots = before.clone();
        let ids: Vec<_> = old.variants.iter().map(|v| v.id).collect();
        for kind in [ThemeKind::Light, ThemeKind::Dark] {
            if old.variants.iter().any(|v| v.name() == slots[kind.index()]) {
                slots[kind.index()] = self.fallback(kind.mode(), &ids);
            }
        }
        if let ThemeSource::Custom(ref path) = old.source {
            storage::remove_and_update(path, || self.persist_slots(&slots))?;
        }
        self.slots = slots;
        self.families.remove(ix);
        Ok(DeletedTheme {
            old,
            position: ix,
            slots_before: before,
            selection_revisions: self.selection_revisions,
            removed_variant_id: None,
        })
    }
    pub fn undo_delete(&mut self, deleted: DeletedTheme) -> Result<()> {
        let removed_family = deleted.removed_variant_id.is_none();
        self.unique_name(
            &deleted.old.name,
            (!removed_family).then_some(deleted.old.id),
        )?;
        let path = match &deleted.old.source {
            ThemeSource::Custom(path) => path.clone(),
            ThemeSource::Bundled => bail!("Built-in themes cannot be restored"),
        };
        let restore_slots = [ThemeKind::Light, ThemeKind::Dark].map(|kind| {
            let index = kind.index();
            self.selection_revisions[index] == deleted.selection_revisions[index]
                && deleted.old.variants.iter().any(|variant| {
                    variant.name() == deleted.slots_before[index]
                        && variant.mode() == kind.mode()
                        && deleted.removed_variant_id.is_none_or(|id| variant.id == id)
                })
        });
        let mut next = if let Some(removed_id) = deleted.removed_variant_id {
            let current = self
                .family(deleted.old.id)
                .context("Theme was renamed or deleted after removing the variant")?;
            Self::restore_variant(current, &deleted.old, removed_id)?
        } else {
            deleted.old
        };
        let mut slots = self.slots.clone();
        for kind in [ThemeKind::Light, ThemeKind::Dark] {
            if restore_slots[kind.index()] {
                slots[kind.index()].clone_from(&deleted.slots_before[kind.index()]);
            }
        }
        self.validate_restored(&next, &slots, !removed_family)?;
        if removed_family {
            storage::write(&path, &next.set())?;
            if let Err(error) = self.persist_slots(&slots) {
                let _ = std::fs::remove_file(&path);
                return Err(error);
            }
        } else {
            storage::replace_and_update(&path, &next.set(), || self.persist_slots(&slots))?;
        }
        next.persisted();
        self.slots = slots;
        if removed_family {
            self.families
                .insert(deleted.position.min(self.families.len()), next);
        } else {
            let ix = self.family_index(next.id)?;
            self.families[ix] = next;
        }
        Ok(())
    }
    fn restore_variant(current: &ThemeFamily, old: &ThemeFamily, id: u64) -> Result<ThemeFamily> {
        ensure!(
            current.name == old.name,
            "Theme was renamed after removing the variant"
        );
        let (position, removed) = old
            .variants
            .iter()
            .enumerate()
            .find(|(_, variant)| variant.id == id)
            .context("Removed variant is unavailable")?;
        ensure!(
            !current.variants.iter().any(|variant| variant.id == id),
            "Variant has already been restored"
        );
        let mut next = current.clone();
        let successor = old
            .variants
            .iter()
            .skip(position.saturating_add(1))
            .find_map(|variant| next.variants.iter().position(|v| v.id == variant.id));
        let predecessor = || {
            old.variants
                .iter()
                .take(position)
                .rev()
                .find_map(|variant| {
                    next.variants
                        .iter()
                        .position(|v| v.id == variant.id)
                        .map(|ix| ix.saturating_add(1))
                })
        };
        let index = successor
            .or_else(predecessor)
            .unwrap_or_else(|| position.min(next.variants.len()));
        next.variants.insert(index, removed.clone());
        Ok(next)
    }
    fn validate_restored(
        &self,
        family: &ThemeFamily,
        slots: &[String; 2],
        replacing: bool,
    ) -> Result<()> {
        ensure!(
            !family.variants.is_empty(),
            "A theme needs at least one variant"
        );
        ensure!(
            family.variants.iter().all(|variant| valid_variant_name(
                &family.name,
                variant.name(),
                family.variants.len() > 1
            )),
            "Restored theme has invalid variant names"
        );
        self.unique_variant_names(
            family.variants.iter().map(ThemeVariant::name),
            replacing.then_some(family.id),
        )?;
        for kind in [ThemeKind::Light, ThemeKind::Dark] {
            let name = &slots[kind.index()];
            let valid_in_restored = family
                .variants
                .iter()
                .any(|variant| variant.name() == name && variant.mode() == kind.mode());
            let valid_elsewhere = self
                .families
                .iter()
                .filter(|other| other.id != family.id)
                .flat_map(|other| &other.variants)
                .any(|variant| variant.name() == name && variant.mode() == kind.mode());
            ensure!(
                valid_in_restored || valid_elsewhere,
                "Current {kind:?} theme is unavailable or has the wrong mode"
            );
        }
        Ok(())
    }
    pub fn export(&self, id: u64, to: &Path) -> Result<()> {
        let family = self.family(id).context("Theme is unavailable")?;
        ensure!(
            matches!(family.source, ThemeSource::Custom(_)),
            "Only custom themes can be exported"
        );
        storage::write(to, &family.set())
    }
    pub fn import(&mut self, from: &Path, policy: ImportPolicy) -> Result<u64> {
        let mut set = storage::read(from, &self.defaults)?;
        let existing = self
            .families
            .iter()
            .find(|family| family.name.eq_ignore_ascii_case(&set.name))
            .cloned();
        match (existing, policy) {
            (Some(family), ImportPolicy::Replace) => {
                let ThemeSource::Custom(ref path) = family.source else {
                    bail!("Built-in themes cannot be replaced")
                };
                let imported_name = set.name.to_string();
                if family.name != imported_name {
                    for document in &mut set.themes {
                        let suffix = document
                            .name()
                            .strip_prefix(&imported_name)
                            .unwrap_or_else(|| document.name())
                            .to_owned();
                        document.set_name(&format!("{}{suffix}", family.name));
                    }
                }
                self.normalize_replacement_names(&mut set, &family);
                // Preserve runtime identities for matching names.
                let mut next = family.clone();
                next.author = set.author.map(|author| author.to_string());
                next.url = set.url.map(|url| url.to_string());
                next.variants = set
                    .themes
                    .drain(..)
                    .map(|document| {
                        let id = family
                            .variants
                            .iter()
                            .find(|v| v.name() == document.name())
                            .map_or_else(next_id, |v| v.id);
                        ThemeVariant { id, document }
                    })
                    .collect();
                self.unique_variant_names(
                    next.variants.iter().map(ThemeVariant::name),
                    Some(family.id),
                )?;
                ensure!(
                    !next.variants.is_empty(),
                    "A theme needs at least one variant"
                );
                let slots = self.slots_after_import(&family, &next);
                storage::replace_and_update(path, &next.set(), || self.persist_slots(&slots))?;
                next.persisted();
                self.slots = slots;
                let ix = self.family_index(family.id)?;
                self.families[ix] = next;
                Ok(family.id)
            }
            (Some(_), ImportPolicy::Copy) => {
                let old = set.name.to_string();
                let name = self.suggested_copy_name(&old);
                set.name = name.clone().into();
                for variant in &mut set.themes {
                    let suffix = variant
                        .name()
                        .strip_prefix(&old)
                        .unwrap_or_else(|| variant.name())
                        .to_owned();
                    variant.set_name(&format!("{name}{suffix}"));
                }
                self.unique_variant_names(set.themes.iter().map(ThemeDocument::name), None)?;
                self.store_import(set)
            }
            (None, _) => {
                self.unique_variant_names(set.themes.iter().map(ThemeDocument::name), None)?;
                self.store_import(set)
            }
        }
    }
    fn normalize_replacement_names(&self, set: &mut StoredThemeSet, family: &ThemeFamily) {
        let multi = set.themes.len() > 1;
        for ix in 0..set.themes.len() {
            let name = set.themes[ix].name();
            if valid_variant_name(&family.name, name, multi) {
                continue;
            }
            for n in 1.. {
                let candidate = if multi || n > 1 {
                    format!("{} Variant {n}", family.name)
                } else {
                    family.name.clone()
                };
                let taken_in_import = set
                    .themes
                    .iter()
                    .any(|document| document.name().eq_ignore_ascii_case(&candidate));
                let taken_elsewhere = self
                    .families
                    .iter()
                    .filter(|other| other.id != family.id)
                    .flat_map(|other| &other.variants)
                    .any(|variant| variant.name().eq_ignore_ascii_case(&candidate));
                if !taken_in_import && !taken_elsewhere {
                    set.themes[ix].set_name(&candidate);
                    break;
                }
            }
        }
    }
    fn store_import(&mut self, set: StoredThemeSet) -> Result<u64> {
        let path = self.new_path();
        let id = self.insert_set(set, ThemeSource::Custom(path.clone()))?;
        let normalized = self
            .family(id)
            .context("Imported theme is unavailable")?
            .set();
        if let Err(error) = storage::write(&path, &normalized) {
            self.families.retain(|family| family.id != id);
            return Err(error);
        }
        Ok(id)
    }
    fn slots_after_import(&self, old: &ThemeFamily, next: &ThemeFamily) -> [String; 2] {
        let mut slots = self.slots.clone();
        let old_ids: Vec<_> = old.variants.iter().map(|variant| variant.id).collect();
        for kind in [ThemeKind::Light, ThemeKind::Dark] {
            let index = kind.index();
            if old
                .variants
                .iter()
                .any(|variant| variant.name() == slots[index])
                && !next
                    .variants
                    .iter()
                    .any(|variant| variant.name() == slots[index] && variant.mode() == kind.mode())
            {
                slots[index] = next
                    .variants
                    .iter()
                    .find(|variant| variant.mode() == kind.mode())
                    .map_or_else(
                        || self.fallback(kind.mode(), &old_ids),
                        |variant| variant.name().to_owned(),
                    );
            }
        }
        slots
    }
    /// Flush the latest revision for one family. A failed write retains the model and exposes Retry.
    pub fn flush_family(&mut self, id: u64) -> Result<()> {
        let ix = self.editable_index(id)?;
        let ThemeSource::Custom(ref path) = self.families[ix].source else {
            bail!("Built-in themes cannot be saved")
        };
        let result = storage::write(path, &self.families[ix].set());
        self.families[ix].status = match &result {
            Ok(()) => SaveStatus::Autosaved,
            Err(error) => SaveStatus::Failed(error.to_string()),
        };
        result
    }
    pub fn flush_pending(&mut self) -> Vec<(u64, anyhow::Error)> {
        let mut errors = Vec::new();
        for ix in 0..self.families.len() {
            if let Some(family) = self.families.get(ix)
                && matches!(family.status, SaveStatus::Saving | SaveStatus::Failed(_))
            {
                let id = family.id;
                if let Err(error) = self.flush_family(id) {
                    errors.push((id, error));
                }
            }
        }
        errors
    }
    pub fn retry(&mut self, id: u64) -> Result<()> {
        self.flush_family(id)
    }
    /// Schedule after each valid edit; revision checks coalesce edits and prevent stale writes.
    pub fn schedule_save(id: u64, cx: &App) {
        let revision = cx.global::<Self>().family(id).map(|f| f.revision);
        cx.spawn(async move |cx| {
            cx.background_executor()
                .timer(Duration::from_millis(400))
                .await;
            cx.update(|cx| {
                if cx.try_global::<Self>().is_some() {
                    let library = cx.global_mut::<Self>();
                    if library.family(id).is_some_and(|f| {
                        Some(f.revision) == revision && matches!(f.status, SaveStatus::Saving)
                    }) {
                        let _ = library.flush_family(id);
                        cx.refresh_windows();
                    }
                }
            });
        })
        .detach();
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

#[derive(Clone)]
pub struct DeletedTheme {
    old: ThemeFamily,
    position: usize,
    slots_before: [String; 2],
    selection_revisions: [u64; 2],
    removed_variant_id: Option<u64>,
}
#[derive(Clone, Copy, Debug)]
pub enum ImportPolicy {
    Replace,
    Copy,
}

fn resolve(
    document: &ThemeDocument,
    defaults: &[ThemeDocument; 2],
    fonts: &super::fonts::FontCatalog,
) -> ResolvedAppearance {
    let resolved = merged_document(document, defaults);
    let families = fonts.resolve_roles(&resolved);
    let mut theme = Theme::default();
    theme.apply_config(&Rc::new(resolved.config.clone()));
    let [interface, _, _, code] = &families;
    theme.font_family = interface.clone();
    theme.mono_font_family = code.clone();
    // Prepare token lookup once with the snapshot, never serialize the full
    // configuration again for each editor row or swatch.
    let mut colors: BTreeMap<_, _> = resolved
        .colors()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(key, value)| {
            Some((
                key,
                gpui_kit::component::try_parse_color(value.as_deref()?).ok()?,
            ))
        })
        .collect();
    // GPUI resolves omitted colors from the variant's mode-specific fallback.
    // Expose those *effective* values to the editor as well: Reset must show
    // Datalith Light for light variants and Datalith Dark for dark variants.
    if let Ok(serde_json::Value::Object(effective)) = serde_json::to_value(theme.colors)
        && let Ok(configured) = resolved.colors()
    {
        for token in configured.keys() {
            if colors.contains_key(token) {
                continue;
            }
            let normalized = token.strip_prefix("base.").unwrap_or(token);
            let field = match token.as_str() {
                // GPUI 0.6.1 accepts this legacy config key without storing
                // a distinct resolved color; the group label uses its normal foreground.
                "group_box.title.foreground" => "group_box_foreground".to_owned(),
                "input.border" => "input".to_owned(),
                "slider.background" => "slider_bar".to_owned(),
                _ => normalized
                    .strip_suffix(".background")
                    .unwrap_or(normalized)
                    .replace('.', "_"),
            };
            if let Some(value) = effective.get(&field)
                && let Ok(color) = serde_json::from_value::<gpui_kit::Hsla>(value.clone())
            {
                colors.insert(token.clone(), color);
            }
        }
    }
    if let Some(highlight) = &resolved.config.highlight
        && let Ok(serde_json::Value::Object(values)) = serde_json::to_value(highlight)
    {
        for (key, value) in values {
            if key == "syntax" {
                if let Some(styles) = value.as_object() {
                    for (name, style) in styles {
                        if let Some(value) = style.get("color").and_then(serde_json::Value::as_str)
                            && let Ok(color) = gpui_kit::component::try_parse_color(value)
                        {
                            colors.insert(format!("highlight:syntax.{name}"), color);
                        }
                    }
                }
            } else if let Some(value) = value.as_str()
                && let Ok(color) = gpui_kit::component::try_parse_color(value)
            {
                colors.insert(format!("highlight:{key}"), color);
            }
        }
    }
    ResolvedAppearance {
        document: resolved,
        theme,
        fonts: families,
        colors,
    }
}

fn merged_document(document: &ThemeDocument, defaults: &[ThemeDocument; 2]) -> ThemeDocument {
    let [light, dark] = defaults;
    let default = match document.mode() {
        ThemeMode::Light => light,
        ThemeMode::Dark => dark,
    };
    let mut merged = serde_json::to_value(default).unwrap_or_default();
    if let Ok(value) = serde_json::to_value(document) {
        let mut value = value;
        storage::merge_missing(&mut value, &merged);
        merged = value;
    }
    serde_json::from_value(merged).unwrap_or_else(|_| default.clone())
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

pub fn refresh_current(cx: &mut App) {
    if let Some(library) = cx.try_global::<ThemeLibrary>() {
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
    }
    Theme::change(Theme::global(cx).mode, None, cx);
    super::fonts::apply(cx);
    cx.refresh_windows();
}
