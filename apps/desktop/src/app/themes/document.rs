use std::collections::BTreeMap;

use anyhow::{Context as _, Result, ensure};
use gpui_kit::SharedString;
use gpui_kit::component::{
    ThemeConfig, ThemeMode, highlighter::HighlightThemeStyle, try_parse_color,
};
use serde::{Deserialize, Serialize};

use super::super::settings::{FontRole, FontSettings};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThemeDocument {
    #[serde(flatten)]
    pub(super) config: ThemeConfig,
    #[serde(default, skip_serializing_if = "FontSettings::is_empty")]
    fonts: FontSettings,
}

impl ThemeDocument {
    pub(super) fn new(name: &str, mode: ThemeMode) -> Self {
        let mut config = ThemeConfig::default();
        config.name = name.to_owned().into();
        config.mode = mode;
        Self {
            config,
            fonts: FontSettings::default(),
        }
    }

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

    pub(super) fn normalize_fonts(&mut self) {
        for role in FontRole::ALL {
            self.fonts
                .set_family(role, self.fonts.family(role).map(str::to_owned));
            let family = self.font(role).map(str::to_owned);
            self.set_font(role, family);
        }
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
            try_parse_color(value)?;
        }
        *color = value;
        self.config.colors = serde_json::from_value(serde_json::to_value(colors)?)?;
        Ok(())
    }

    pub fn set_highlight(&mut self, token: &str, value: Option<String>) -> Result<()> {
        if let Some(ref value) = value {
            try_parse_color(value)?;
        }
        let schema = serde_json::to_value(HighlightThemeStyle::default())?;
        let mut highlight =
            serde_json::to_value(self.config.highlight.clone().unwrap_or_default())?
                .as_object()
                .cloned()
                .unwrap_or_default();
        if let Some(syntax) = token.strip_prefix("syntax.") {
            ensure!(
                schema
                    .get("syntax")
                    .and_then(|styles| styles.get(syntax))
                    .is_some(),
                "Unknown syntax color"
            );
            let node = highlight
                .entry("syntax")
                .or_insert_with(|| serde_json::json!({}));
            let node = node.as_object_mut().context("Invalid syntax overrides")?;
            if let Some(value) = value {
                let style = node.entry(syntax).or_insert(serde_json::Value::Null);
                if style.is_null() {
                    *style = serde_json::json!({});
                }
                style
                    .as_object_mut()
                    .context("Invalid syntax style")?
                    .insert("color".into(), value.into());
            } else if let Some(style) = node
                .get_mut(syntax)
                .and_then(serde_json::Value::as_object_mut)
            {
                style.remove("color");
            }
        } else {
            ensure!(
                schema.get(token).is_some() && token != "syntax",
                "Unknown highlight color"
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

/// Flatten highlight colors into editor tokens:
/// `highlight:{key}` and `highlight:syntax.{name}`,
/// with `None` for keys that have no color.
pub fn highlight_tokens(style: &HighlightThemeStyle) -> BTreeMap<String, Option<String>> {
    let mut tokens = BTreeMap::new();
    let Ok(serde_json::Value::Object(values)) = serde_json::to_value(style) else {
        return tokens;
    };
    for (key, value) in values {
        if key == "syntax" {
            for (name, style) in value.as_object().into_iter().flatten() {
                tokens.insert(
                    format!("highlight:syntax.{name}"),
                    style
                        .get("color")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_owned),
                );
            }
        } else {
            tokens.insert(
                format!("highlight:{key}"),
                value.as_str().map(str::to_owned),
            );
        }
    }
    tokens
}
