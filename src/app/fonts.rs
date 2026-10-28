use std::borrow::Cow;

use conv::ConvAsUtil as _;

use gpui_kit::component::notification::Notification;
use gpui_kit::component::{ActiveTheme, Theme};
use gpui_kit::{App, Global, SharedString};

use crate::ui::notifications;

use super::settings::{self, FontRole, ThemeKind};
use super::themes::{ThemeDocument, ThemeLibrary};

pub const PIXELOID_FONT: &str = "Pixeloid Sans";

#[derive(Clone)]
pub struct ResolvedFonts {
    interface: SharedString,
    reading: SharedString,
    headings: SharedString,
    code: SharedString,
}

impl ResolvedFonts {
    pub const fn interface(&self) -> &SharedString {
        &self.interface
    }

    pub const fn reading(&self) -> &SharedString {
        &self.reading
    }

    pub const fn headings(&self) -> &SharedString {
        &self.headings
    }

    pub const fn code(&self) -> &SharedString {
        &self.code
    }
}

/// Enumerated once after bundled fonts load. Variant fonts are the sole font choices.
pub struct FontCatalog {
    families: Vec<SharedString>,
    default_interface: SharedString,
    default_code: SharedString,
}

impl Global for FontCatalog {}

impl FontCatalog {
    pub fn init(cx: &mut App) {
        let mut families = cx.text_system().all_font_names();
        families.retain(|name| !name.starts_with('.') && !name.trim().is_empty());
        families.sort_by_cached_key(|name| name.to_lowercase());
        families.dedup();
        cx.set_global(Self {
            families: families.into_iter().map(SharedString::from).collect(),
            default_interface: cx.theme().font_family.clone(),
            default_code: cx.theme().mono_font_family.clone(),
        });
    }

    pub fn families(&self) -> &[SharedString] {
        &self.families
    }

    pub fn contains(&self, family: &str) -> bool {
        self.families.iter().any(|name| name.as_str() == family)
    }

    fn installed(&self, family: Option<&str>) -> Option<SharedString> {
        family.and_then(|family| {
            self.families
                .iter()
                .find(|name| name.as_str() == family)
                .cloned()
        })
    }

    /// Stored unavailable choices remain in the document; only rendering falls back.
    pub fn resolve_roles(&self, document: &ThemeDocument) -> ResolvedFonts {
        let interface = self
            .installed(document.font(FontRole::Interface))
            .unwrap_or_else(|| self.default_interface.clone());
        let reading = self
            .installed(document.font(FontRole::Reading))
            .unwrap_or_else(|| interface.clone());
        let headings = self
            .installed(document.font(FontRole::Headings))
            .or_else(|| self.installed(Some(PIXELOID_FONT)))
            .unwrap_or_else(|| reading.clone());
        let code = self
            .installed(document.font(FontRole::Code))
            .unwrap_or_else(|| self.default_code.clone());
        ResolvedFonts {
            interface,
            reading,
            headings,
            code,
        }
    }
}

/// Reapply typography after a theme change. Resolve defaults from the selected
/// theme rather than its previously overridden runtime values.
pub fn apply(cx: &mut App) {
    let fonts = families(cx);
    let theme = Theme::global_mut(cx);
    // Keep the independent interface scale stable across theme selection.
    theme.font_size = gpui_kit::px(
        crate::ui::BASE_FONT_SIZE * settings::snapshot().font_scale.approx().unwrap_or(1.0),
    );
    theme.font_family = fonts.interface().clone();
    theme.mono_font_family = fonts.code().clone();
    Theme::sync_base(cx);
}

/// Resolve the active slot's interface, reading, headings, and code fonts once.
pub fn families(cx: &App) -> ResolvedFonts {
    let library = cx.global::<ThemeLibrary>();
    let kind = ThemeKind::from(cx.theme().mode);
    let document = library.current_document(kind);
    cx.global::<FontCatalog>().resolve_roles(document)
}

pub fn load_embedded_fonts(cx: &App) -> Vec<Notification> {
    static REGULAR: &[u8] = include_bytes!("../../assets/fonts/Pixeloid/PixeloidSans.ttf");
    static BOLD: &[u8] = include_bytes!("../../assets/fonts/Pixeloid/PixeloidSans-Bold.ttf");
    let fonts = vec![Cow::Borrowed(REGULAR), Cow::Borrowed(BOLD)];
    match cx.text_system().add_fonts(fonts) {
        Ok(()) => Vec::new(),
        Err(error) => vec![notifications::font_load_failed(&error)],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn variant_fonts_and_unavailable_choices_resolve_per_role() {
        let catalog = FontCatalog {
            families: ["Interface", "Reading", "Code", PIXELOID_FONT]
                .into_iter()
                .map(SharedString::from)
                .collect(),
            default_interface: "Platform UI".into(),
            default_code: "Platform Mono".into(),
        };
        let mut document = super::super::themes::ThemeDocument::from_config(
            gpui_kit::component::ThemeConfig::default(),
        );
        document.set_font(FontRole::Interface, Some("Interface".into()));
        document.set_font(FontRole::Reading, Some("Missing font".into()));
        document.set_font(FontRole::Headings, Some("Missing font".into()));
        document.set_font(FontRole::Code, Some("Code".into()));
        let resolved = catalog.resolve_roles(&document);
        assert_eq!(resolved.interface().as_str(), "Interface");
        assert_eq!(resolved.reading().as_str(), "Interface");
        assert_eq!(resolved.headings().as_str(), PIXELOID_FONT);
        assert_eq!(resolved.code().as_str(), "Code");
        assert_eq!(document.font(FontRole::Reading), Some("Missing font"));
    }
}
