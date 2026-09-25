use std::{fs, io::Write as _, path::Path};

use anyhow::{Context as _, Result};
use gpui_kit::SharedString;
use gpui_kit::component::ThemeMode;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    ThemeDocument, ThemeFamily, ThemeLibrary, ThemeSource, ThemeVariant, next_id,
    valid_variant_name,
};

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub(super) struct StoredThemeSet {
    pub name: SharedString,
    pub author: Option<SharedString>,
    pub url: Option<SharedString>,
    #[serde(rename = "themes")]
    pub themes: Vec<ThemeDocument>,
}

pub(super) fn read(path: &Path, defaults: &[ThemeDocument; 2]) -> Result<StoredThemeSet> {
    let text = fs::read_to_string(path)?;
    let mut value: Value = serde_json::from_str(&text)?;
    if !value.is_object() {
        value = Value::Object(serde_json::Map::default());
    }
    let object = value
        .as_object_mut()
        .context("Theme file must contain an object")?;
    let name = object
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    let name = if name.is_empty() {
        "Untitled theme"
    } else {
        name
    }
    .to_owned();
    object.insert("name".into(), name.clone().into());
    for field in ["author", "url"] {
        if object
            .get(field)
            .is_some_and(|v| !v.is_string() && !v.is_null())
        {
            object.remove(field);
        }
    }
    let themes = object
        .entry("themes")
        .or_insert_with(|| Value::Array(vec![]));
    if !themes.is_array() {
        *themes = Value::Array(vec![]);
    }
    let variants = themes
        .as_array_mut()
        .context("Theme variants must be an array")?;
    if variants.is_empty() {
        variants.push(serde_json::to_value(&defaults[0])?);
    }
    let multi = variants.len() > 1;
    for (ix, variant) in variants.iter_mut().enumerate() {
        if !variant.is_object() {
            *variant = Value::Object(serde_json::Map::default());
        }
        let object = variant
            .as_object_mut()
            .context("Variant must be an object")?;
        let mode = if object.get("mode").and_then(Value::as_str) == Some("dark") {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        };
        // Leave missing tokens unset in storage so Reset continues to inherit
        // Datalith defaults. `resolved` fills them for presentation.
        let variant_name = object
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if variant_name.trim().is_empty() {
            object.insert(
                "name".into(),
                if multi {
                    format!("{name} Variant {}", ix.saturating_add(1))
                } else {
                    name.clone()
                }
                .into(),
            );
        }
        object.insert(
            "mode".into(),
            if mode == ThemeMode::Dark {
                "dark"
            } else {
                "light"
            }
            .into(),
        );
        sanitize_variant(object);
    }
    Ok(serde_json::from_value(value)?)
}

fn sanitize_variant(variant: &mut serde_json::Map<String, Value>) {
    // Validate each property in isolation: a bad font or one syntax color must
    // not hide the other tokens or prevent the family from loading.
    for group in ["colors", "fonts", "highlight"] {
        let Some(mut properties) = variant
            .remove(group)
            .and_then(|value| value.as_object().cloned())
        else {
            continue;
        };
        if group == "highlight" {
            if let Some(mut syntax) = properties
                .remove("syntax")
                .and_then(|v| v.as_object().cloned())
            {
                syntax.retain(|key, value| {
                    let mut inner = serde_json::Map::new();
                    inner.insert(key.clone(), value.clone());
                    let mut highlight = serde_json::Map::new();
                    highlight.insert("syntax".into(), Value::Object(inner));
                    valid_fragment("highlight", Value::Object(highlight))
                        && value.get("color").is_none_or(valid_color)
                });
                properties.insert("syntax".into(), Value::Object(syntax));
            }
            properties
                .entry("syntax")
                .or_insert_with(|| Value::Object(serde_json::Map::default()));
        }
        properties.retain(|key, value| {
            let mut one = serde_json::Map::new();
            one.insert(key.clone(), value.clone());
            valid_fragment(group, Value::Object(one))
                && (group == "fonts" || (key == "syntax" || valid_color(value)))
        });
        variant.insert(group.into(), Value::Object(properties));
    }
    let keys: Vec<String> = variant.keys().cloned().collect();
    for key in keys {
        if ["name", "mode", "colors", "fonts", "highlight"].contains(&key.as_str()) {
            continue;
        }
        if let Some(value) = variant.get(&key)
            && !valid_fragment(&key, value.clone())
        {
            variant.remove(&key);
        }
    }
}

fn valid_color(value: &Value) -> bool {
    value
        .as_str()
        .is_none_or(|text| gpui_kit::component::try_parse_color(text).is_ok())
}

fn valid_fragment(key: &str, value: Value) -> bool {
    let mut fragment = serde_json::Map::new();
    let mut value = value;
    if key == "highlight"
        && let Some(highlight) = value.as_object_mut()
    {
        highlight
            .entry("syntax")
            .or_insert_with(|| Value::Object(serde_json::Map::default()));
    }
    fragment.insert(key.into(), value);
    serde_json::from_value::<ThemeDocument>(Value::Object(fragment)).is_ok()
}

pub(super) fn merge_missing(value: &mut Value, defaults: &Value) {
    if let (Some(value), Some(defaults)) = (value.as_object_mut(), defaults.as_object()) {
        for (key, fallback) in defaults {
            let target = value.entry(key).or_insert(Value::Null);
            merge_missing(target, fallback);
        }
    } else if value.is_null() {
        *value = defaults.clone();
    }
}

impl ThemeLibrary {
    pub(super) fn load(&mut self) -> Vec<(String, anyhow::Error)> {
        let entries = match fs::read_dir(&self.directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
            Err(error) => return vec![("Custom themes".into(), error.into())],
        };
        let mut paths = Vec::new();
        let mut errors = Vec::new();
        for entry in entries {
            match entry {
                Ok(entry) if entry.path().extension().is_some_and(|ext| ext == "json") => {
                    paths.push(entry.path());
                }
                Ok(_) => {}
                Err(error) => errors.push(("Custom themes".into(), error.into())),
            }
        }
        paths.sort();
        for path in paths {
            match read(&path, &self.defaults).and_then(|mut set| {
                if self
                    .families
                    .iter()
                    .any(|f| f.name.eq_ignore_ascii_case(&set.name))
                {
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
                }
                let before = serde_json::to_value(&set)?;
                let id = self.insert_set(set, ThemeSource::Custom(path.clone()))?;
                let normalized = self
                    .family(id)
                    .context("Loaded theme is unavailable")?
                    .set();
                if serde_json::to_value(&normalized)? != before {
                    write(&path, &normalized)?;
                }
                Ok(id)
            }) {
                Ok(_) => {}
                Err(error) => errors.push((path.display().to_string(), error)),
            }
        }
        errors
    }

    pub(super) fn insert_set(&mut self, set: StoredThemeSet, source: ThemeSource) -> Result<u64> {
        anyhow::ensure!(!set.name.trim().is_empty(), "Enter a theme name");
        anyhow::ensure!(
            !self
                .families
                .iter()
                .any(|family| family.name.eq_ignore_ascii_case(&set.name)),
            "A theme with this name already exists"
        );
        let mut variants = Vec::<ThemeVariant>::new();
        let multi = set.themes.len() > 1;
        for mut document in set.themes {
            if !valid_variant_name(&set.name, document.name(), multi)
                || variants
                    .iter()
                    .any(|v| v.document.name().eq_ignore_ascii_case(document.name()))
                || self.variant(document.name()).is_some()
            {
                let mut n = 1usize;
                let name = loop {
                    let candidate = if multi || n > 1 {
                        format!("{} Variant {n}", set.name)
                    } else {
                        set.name.to_string()
                    };
                    if !variants
                        .iter()
                        .any(|v| v.document.name().eq_ignore_ascii_case(&candidate))
                        && !self
                            .families
                            .iter()
                            .flat_map(|family| &family.variants)
                            .any(|v| v.name().eq_ignore_ascii_case(&candidate))
                    {
                        break candidate;
                    }
                    n = n.saturating_add(1);
                };
                document.set_name(&name);
            }
            variants.push(ThemeVariant {
                id: next_id(),
                document,
            });
        }
        anyhow::ensure!(!variants.is_empty(), "A theme needs at least one variant");
        let id = next_id();
        self.families.push(ThemeFamily {
            id,
            name: set.name.to_string(),
            author: set.author.map(|s| s.to_string()),
            url: set.url.map(|s| s.to_string()),
            variants,
            source,
            revision: 0,
            status: super::SaveStatus::Autosaved,
        });
        Ok(id)
    }
}

pub(super) fn write(path: &Path, set: &StoredThemeSet) -> Result<()> {
    let parent = path.parent().context("Theme path has no parent")?;
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(".theme-{:032x}.tmp", rand::random::<u128>()));
    let result = (|| -> Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(&serde_json::to_vec_pretty(set)?)?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        #[cfg(unix)]
        fs::File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

/// Keep a same-filesystem copy of the old bytes until both the new theme and
/// preference references have committed. Rollback needs only a rename, even if
/// serialization or a subsequent disk allocation becomes unavailable.
pub(super) fn replace_and_update(
    path: &Path,
    set: &StoredThemeSet,
    update: impl FnOnce() -> Result<()>,
) -> Result<()> {
    let parent = path.parent().context("Theme path has no parent")?;
    let backup = parent.join(format!(".theme-{:032x}.backup", rand::random::<u128>()));
    fs::hard_link(path, &backup)?;
    let result = write(path, set).and_then(|()| update());
    if let Err(error) = result {
        fs::rename(&backup, path)
            .with_context(|| format!("Could not restore theme after {error}"))?;
        return Err(error);
    }
    let _ = fs::remove_file(backup);
    Ok(())
}

/// A reversible filesystem rename prevents a preference write failure from
/// losing the theme file or leaving its saved current references dangling.
pub(super) fn remove_and_update(path: &Path, update: impl FnOnce() -> Result<()>) -> Result<()> {
    let parent = path.parent().context("Theme path has no parent")?;
    let tombstone = parent.join(format!(".theme-{:032x}.removed", rand::random::<u128>()));
    fs::rename(path, &tombstone)?;
    if let Err(error) = update() {
        fs::rename(&tombstone, path)
            .with_context(|| format!("Could not restore theme after {error}"))?;
        return Err(error);
    }
    let _ = fs::remove_file(tombstone);
    Ok(())
}
