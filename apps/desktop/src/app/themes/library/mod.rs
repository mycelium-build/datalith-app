mod deletion;
mod editing;
mod persistence;

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context as _, Result, bail, ensure};
use gpui_kit::component::{ThemeConfig, ThemeMode, notification::Notification};
use gpui_kit::{App, Global, Subscription};

use super::{
    DeletedTheme, ResolvedAppearance, SaveStatus, ThemeDocument, ThemeFamily, ThemeSource,
    ThemeVariant, appearance, embedded,
    model::{ThemeModes, next_id, valid_variant_name},
    refresh_current, storage,
    storage::{LoadFailure, StoredThemeSet},
};
use crate::app::{
    data_dir,
    fonts::FontCatalog,
    settings::{self, FontRole, ThemeKind},
};

pub(super) const DEFAULT_LIGHT: &str = "Datalith Light";
pub(super) const DEFAULT_DARK: &str = "Datalith Dark";

fn rename_slot(slots: &mut ThemeModes<String>, from: &str, to: &str) {
    for kind in ThemeKind::ALL {
        let slot = slots.get_mut(kind);
        if slot == from {
            to.clone_into(slot);
        }
    }
}

fn persist_slots(slots: &ThemeModes<String>) -> Result<()> {
    settings::replace_theme_slots(&slots.to_array())
}

pub struct ThemeLibrary {
    pub(super) families: Vec<ThemeFamily>,
    pub(super) defaults: ThemeModes<ThemeDocument>,
    pub(super) directory: PathBuf,
    pub(super) slots: ThemeModes<String>,
    // Only explicit selection advances these;
    // Undo must not undo a later choice,
    // even when that choice reselects the automatic fallback by name.
    pub(super) selection_revisions: ThemeModes<u64>,
    pub(super) quit_subscription: Option<Subscription>,
}
impl Global for ThemeLibrary {}

impl ThemeLibrary {
    pub fn init(cx: &mut App) -> Vec<Notification> {
        let mut library = Self::new(data_dir().join("themes"));
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
                LoadFailure::Ignored { theme, values } => {
                    crate::ui::notifications::theme_values_ignored(&theme, &values)
                }
            })
            .collect();
        let prefs = settings::snapshot();
        let saved = ThemeModes::new(prefs.light_theme_name, prefs.dark_theme_name);
        for kind in ThemeKind::ALL {
            if let Some(name) = saved.get(kind) {
                if library
                    .variant(name)
                    .is_some_and(|v| v.mode() == kind.mode())
                {
                    library.slots.get_mut(kind).clone_from(name);
                } else {
                    pending.push(crate::ui::notifications::theme_fallback(
                        name,
                        library.current(kind),
                    ));
                }
            }
        }
        // Avoid rewriting the settings file on every launch.
        let preference_error = (saved != library.slots.clone().map(Some))
            .then(|| persist_slots(&library.slots).err())
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
            slots: ThemeModes::new(DEFAULT_LIGHT.into(), DEFAULT_DARK.into()),
            selection_revisions: ThemeModes::new(0, 0),
            quit_subscription: None,
        }
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
        self.slots.get(kind)
    }

    /// Resolve the current selection, falling back to the built-in document
    /// for its mode if the selected variant is unavailable.
    pub fn current_document(&self, kind: ThemeKind) -> &ThemeDocument {
        self.get(self.current(kind))
            .unwrap_or_else(|| self.defaults.get(kind))
    }

    pub fn get(&self, name: &str) -> Option<&ThemeDocument> {
        self.variant(name).map(ThemeVariant::document)
    }

    fn editable_family(&self, id: u64) -> Result<&ThemeFamily> {
        let family = self.family(id).context("Theme is unavailable")?;
        family.ensure_editable()?;
        Ok(family)
    }

    fn editable_family_mut(&mut self, id: u64) -> Result<&mut ThemeFamily> {
        let family = self
            .families
            .iter_mut()
            .find(|family| family.id == id)
            .context("Theme is unavailable")?;
        family.ensure_editable()?;
        Ok(family)
    }

    /// The editable family that contains a variant.
    fn variant_family(&self, id: u64) -> Result<&ThemeFamily> {
        let family = self
            .families
            .iter()
            .find(|family| family.variants.iter().any(|variant| variant.id == id))
            .context("Theme variant is unavailable")?;
        family.ensure_editable()?;
        Ok(family)
    }

    fn variant_family_mut(&mut self, id: u64) -> Result<&mut ThemeFamily> {
        let family = self
            .families
            .iter_mut()
            .find(|family| family.variants.iter().any(|variant| variant.id == id))
            .context("Theme variant is unavailable")?;
        family.ensure_editable()?;
        Ok(family)
    }

    /// Replace a family on disk and in memory together with the slots that
    /// reference it; the file is rolled back if the slots cannot be saved.
    fn commit_family(&mut self, mut next: ThemeFamily, slots: ThemeModes<String>) -> Result<()> {
        let slots_changed = slots != self.slots;
        let family = self.editable_family_mut(next.id)?;
        if let ThemeSource::Custom(ref path) = next.source {
            storage::replace_and_update(path, &next.set(), || {
                if slots_changed {
                    persist_slots(&slots)
                } else {
                    Ok(())
                }
            })?;
        }
        next.persisted();
        *family = next;
        self.slots = slots;
        Ok(())
    }

    pub(super) fn unique_name(&self, name: &str, except: Option<u64>) -> Result<()> {
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

    fn replace_slots(&mut self, slots: ThemeModes<String>) -> Result<()> {
        persist_slots(&slots)?;
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
        *slots.get_mut(kind) = variant.name().into();
        self.replace_slots(slots)?;
        let revision = self.selection_revisions.get_mut(kind);
        *revision = revision.saturating_add(1);
        Ok(())
    }

    fn fallback(&self, mode: ThemeMode, excluding: &[u64]) -> String {
        self.families
            .iter()
            .flat_map(|f| &f.variants)
            .find(|v| v.mode() == mode && !excluding.contains(&v.id))
            .map_or_else(
                || {
                    match ThemeKind::from(mode) {
                        ThemeKind::Light => DEFAULT_LIGHT,
                        ThemeKind::Dark => DEFAULT_DARK,
                    }
                    .to_string()
                },
                |v| v.name().to_owned(),
            )
    }

    fn fallback_in_family(
        &self,
        family: &ThemeFamily,
        mode: ThemeMode,
        excluding: &[u64],
    ) -> String {
        family
            .variants
            .iter()
            .find(|v| v.mode() == mode && !excluding.contains(&v.id))
            .map_or_else(|| self.fallback(mode, excluding), |v| v.name().to_owned())
    }

    pub fn resolved(&self, id: u64, fonts: &FontCatalog) -> Result<ResolvedAppearance> {
        let document = &self
            .variant_by_id(id)
            .context("Theme variant is unavailable")?
            .document;
        Ok(appearance::resolve(document, &self.defaults, fonts))
    }

    pub fn resolved_config(&self, name: &str) -> Option<ThemeConfig> {
        let document = self.get(name)?;
        Some(appearance::merged_config(document, &self.defaults))
    }
}
