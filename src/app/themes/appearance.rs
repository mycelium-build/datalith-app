use std::{
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};

use gpui_kit::component::{
    Theme, ThemeConfig, ThemeMode,
    highlighter::{HighlightTheme, HighlightThemeStyle},
    try_parse_color,
};

use super::{ThemeDocument, highlight_tokens, model::ThemeModes, storage};
use crate::app::{
    fonts::{FontCatalog, ResolvedFonts},
    settings::ThemeKind,
};

/// A snapshot that may be rendered without modifying any application global.
#[derive(Clone)]
pub struct ResolvedAppearance {
    theme: Theme,
    fonts: ResolvedFonts,
    colors: BTreeMap<String, gpui_kit::Hsla>,
    defined_colors: BTreeSet<String>,
}

impl ResolvedAppearance {
    pub const fn theme(&self) -> &Theme {
        &self.theme
    }

    pub fn color(&self, token: &str) -> Option<gpui_kit::Hsla> {
        self.colors.get(token).copied()
    }

    pub fn color_is_defined(&self, token: &str) -> bool {
        self.defined_colors.contains(token)
    }

    pub const fn fonts(&self) -> &ResolvedFonts {
        &self.fonts
    }
}

pub(super) fn resolve(
    document: &ThemeDocument,
    defaults: &ThemeModes<ThemeDocument>,
    fonts: &FontCatalog,
) -> ResolvedAppearance {
    let resolved = merged_document(document, defaults);
    let families = fonts.resolve_roles(&resolved);
    let mut theme = Theme::default();
    theme.apply_config(&Rc::new(resolved.config.clone()));
    theme.font_family = families.interface().clone();
    theme.mono_font_family = families.code().clone();
    let mut colors: BTreeMap<_, _> = resolved
        .colors()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(key, value)| Some((key, try_parse_color(value.as_deref()?).ok()?)))
        .collect();
    let mut defined_colors: BTreeSet<_> = colors.keys().cloned().collect();
    if let Ok(serde_json::Value::Object(effective)) = serde_json::to_value(theme.colors)
        && let Ok(configured) = resolved.colors()
    {
        for token in configured.keys() {
            if colors.contains_key(token) {
                continue;
            }
            let normalized = token.strip_prefix("base.").unwrap_or(token);
            let field = match token.as_str() {
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
    let explicit_highlight = document
        .config
        .highlight
        .as_ref()
        .map(highlight_tokens)
        .unwrap_or_default();
    for (token, value) in resolved
        .config
        .highlight
        .as_ref()
        .map(highlight_tokens)
        .unwrap_or_default()
    {
        if let Some(color) = value.and_then(|value| try_parse_color(&value).ok()) {
            if explicit_highlight.get(&token).is_some_and(Option::is_some) {
                defined_colors.insert(token.clone());
            }
            colors.insert(token, color);
        }
    }
    ResolvedAppearance {
        theme,
        fonts: families,
        colors,
        defined_colors,
    }
}

pub(super) fn merged_config(
    document: &ThemeDocument,
    defaults: &ThemeModes<ThemeDocument>,
) -> ThemeConfig {
    merged_document(document, defaults).config
}

fn merged_document(
    document: &ThemeDocument,
    defaults: &ThemeModes<ThemeDocument>,
) -> ThemeDocument {
    let default = defaults.get(ThemeKind::from(document.mode()));
    let explicit_colors = document.config.colors.clone();
    let explicit_highlight = document.config.highlight.clone();
    let mut merged = serde_json::to_value(default).unwrap_or_default();
    if let Ok(mut value) = serde_json::to_value(document) {
        storage::merge_missing(&mut value, &merged);
        merged = value;
    }
    let mut merged: ThemeDocument =
        serde_json::from_value(merged).unwrap_or_else(|_| default.clone());

    merged.config.colors = explicit_colors;

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
