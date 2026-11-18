use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use anyhow::{Context as _, Result, ensure};
use gpui_kit::component::ThemeMode;

use super::{ThemeDocument, storage::StoredThemeSet};
use crate::app::settings::ThemeKind;

static ID: AtomicU64 = AtomicU64::new(1);

pub(super) fn next_id() -> u64 {
    ID.fetch_add(1, Ordering::Relaxed)
}

pub(super) fn free_variant_name(family: &str, multi: bool, taken: impl Fn(&str) -> bool) -> String {
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

pub(super) fn valid_variant_name(family: &str, variant: &str, multi: bool) -> bool {
    (!multi && variant == family)
        || variant
            .strip_prefix(family)
            .and_then(|tail| tail.strip_prefix(' '))
            .is_some_and(|suffix| !suffix.trim().is_empty())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ThemeModes<T> {
    light: T,
    dark: T,
}

impl<T> ThemeModes<T> {
    pub(super) const fn new(light: T, dark: T) -> Self {
        Self { light, dark }
    }

    pub(super) fn from_fn(mut value: impl FnMut(ThemeKind) -> T) -> Self {
        Self {
            light: value(ThemeKind::Light),
            dark: value(ThemeKind::Dark),
        }
    }

    pub(super) const fn get(&self, kind: ThemeKind) -> &T {
        match kind {
            ThemeKind::Light => &self.light,
            ThemeKind::Dark => &self.dark,
        }
    }

    pub(super) const fn get_mut(&mut self, kind: ThemeKind) -> &mut T {
        match kind {
            ThemeKind::Light => &mut self.light,
            ThemeKind::Dark => &mut self.dark,
        }
    }

    pub(super) fn map<U>(self, mut value: impl FnMut(T) -> U) -> ThemeModes<U> {
        ThemeModes::new(value(self.light), value(self.dark))
    }
}

impl<T: Clone> ThemeModes<T> {
    pub(super) fn to_array(&self) -> [T; 2] {
        [self.light.clone(), self.dark.clone()]
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ThemeSource {
    Bundled,
    Custom(PathBuf),
}

#[derive(Clone, Debug)]
pub struct ThemeVariant {
    pub(super) id: u64,
    pub(super) document: ThemeDocument,
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
    pub(super) id: u64,
    pub(super) name: String,
    pub(super) author: Option<String>,
    pub(super) url: Option<String>,
    pub(super) variants: Vec<ThemeVariant>,
    pub(super) source: ThemeSource,
    pub(super) revision: u64, // use for debounce save, prevent excessive write when dragging color
    pub(super) status: SaveStatus,
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

    #[cfg(test)]
    pub const fn source(&self) -> &ThemeSource {
        &self.source
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

    pub(super) fn set(&self) -> StoredThemeSet {
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

    pub(super) fn variant(&self, id: u64) -> Result<&ThemeVariant> {
        self.variants
            .iter()
            .find(|variant| variant.id == id)
            .context("Theme variant is unavailable")
    }

    pub(super) fn variant_mut(&mut self, id: u64) -> Result<&mut ThemeVariant> {
        self.variants
            .iter_mut()
            .find(|variant| variant.id == id)
            .context("Theme variant is unavailable")
    }

    pub(super) fn ensure_editable(&self) -> Result<()> {
        ensure!(self.is_custom(), "Built-in themes cannot be edited");
        Ok(())
    }

    pub(super) fn changed(&mut self) {
        self.revision = self.revision.saturating_add(1);
        self.status = SaveStatus::Saving;
    }

    pub(super) fn persisted(&mut self) {
        self.revision = self.revision.saturating_add(1);
        self.status = SaveStatus::Autosaved;
    }
}

#[derive(Clone)]
pub struct DeletedTheme {
    pub(super) old: ThemeFamily,
    pub(super) position: usize,
    pub(super) slots_before: ThemeModes<String>,
    pub(super) selection_revisions: ThemeModes<u64>,
    pub(super) removed_variant_id: Option<u64>,
}

impl DeletedTheme {
    pub fn message(&self) -> String {
        self.removed_variant_id
            .and_then(|id| self.old.suffix(id))
            .map_or_else(
                || format!("Theme \"{}\" deleted", self.old.name()),
                |name| format!("Variant \"{name}\" deleted"),
            )
    }
}
