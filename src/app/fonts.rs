use std::borrow::Cow;

use conv::ConvAsUtil as _;

use gpui_kit::component::notification::Notification;
use gpui_kit::component::{ActiveTheme, Theme};
use gpui_kit::{App, Global, SharedString};

use crate::ui::notifications;

use super::settings::{self, FontRole, FontSettings};

pub const PIXELOID_FONT: &str = "Pixeloid Sans";

/// Enumerated once after bundled fonts load. Preferences remain independent of
/// theme fonts, including when the OS changes appearance.
pub struct FontCatalog {
    families: Vec<SharedString>,
    preferences: FontSettings,
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
            preferences: settings::snapshot().fonts,
            default_interface: cx.theme().font_family.clone(),
            default_code: cx.theme().mono_font_family.clone(),
        });
    }

    pub fn families(&self) -> &[SharedString] {
        &self.families
    }

    pub fn selected(&self, role: FontRole) -> Option<&str> {
        self.preferences.family(role)
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
}

pub fn has_personal_fonts(cx: &App) -> bool {
    cx.try_global::<FontCatalog>().is_some_and(|catalog| {
        FontRole::ALL
            .into_iter()
            .any(|role| catalog.selected(role).is_some())
    })
}

pub fn use_theme_fonts(cx: &mut App) -> anyhow::Result<()> {
    settings::use_theme_fonts()?;
    cx.global_mut::<FontCatalog>().preferences = FontSettings::default();
    apply(cx);
    cx.refresh_windows();
    Ok(())
}

pub fn select(role: FontRole, family: Option<String>, cx: &mut App) -> anyhow::Result<()> {
    settings::set_font_family(role, family.clone())?;
    cx.global_mut::<FontCatalog>()
        .preferences
        .set_family(role, family);
    apply(cx);
    cx.refresh_windows();
    Ok(())
}

/// Reapply typography after a theme change. Resolve defaults from the selected
/// theme rather than its previously overridden runtime values.
pub fn apply(cx: &mut App) {
    if cx.try_global::<FontCatalog>().is_none() {
        return;
    }
    let interface = family(FontRole::Interface, cx);
    let code = family(FontRole::Code, cx);
    let theme = Theme::global_mut(cx);
    // Keep the user's interface scale stable across theme selection and preview.
    theme.font_size = gpui_kit::px(
        crate::ui::BASE_FONT_SIZE * settings::snapshot().font_scale.approx().unwrap_or(1.0),
    );
    theme.font_family = interface;
    theme.mono_font_family = code;
    Theme::sync_base(cx);
}

pub fn family(role: FontRole, cx: &App) -> SharedString {
    let catalog = cx.try_global::<FontCatalog>();
    if let Some(family) = catalog.and_then(|fonts| fonts.installed(fonts.selected(role))) {
        return family;
    }
    default_family(role, cx)
}

/// The family a role would use after resetting its own preference.
pub fn default_family(role: FontRole, cx: &App) -> SharedString {
    let catalog = cx.try_global::<FontCatalog>();
    if let Some(family) = catalog.and_then(|fonts| fonts.installed(super::themes::font(role, cx))) {
        return family;
    }
    let theme = cx.theme();
    let config = if theme.is_dark() {
        &theme.dark_theme
    } else {
        &theme.light_theme
    };
    match role {
        FontRole::Interface => catalog.map_or_else(
            || theme.font_family.clone(),
            |fonts| {
                fonts
                    .installed(config.font_family.as_deref())
                    .unwrap_or_else(|| fonts.default_interface.clone())
            },
        ),
        FontRole::Reading => family(FontRole::Interface, cx),
        FontRole::Headings => catalog
            .and_then(|fonts| fonts.installed(Some(PIXELOID_FONT)))
            .unwrap_or_else(|| family(FontRole::Reading, cx)),
        FontRole::Code => catalog.map_or_else(
            || theme.mono_font_family.clone(),
            |fonts| {
                fonts
                    .installed(config.mono_font_family.as_deref())
                    .unwrap_or_else(|| fonts.default_code.clone())
            },
        ),
    }
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
    use gpui_kit::{TestAppContext, component::ThemeMode};

    fn init_catalog(cx: &mut App) {
        gpui_kit::init(cx);
        cx.set_global(FontCatalog {
            families: [
                "Interface font",
                "Reading font",
                "Heading font",
                "Code font",
                PIXELOID_FONT,
            ]
            .into_iter()
            .map(SharedString::from)
            .collect(),
            preferences: FontSettings::default(),
            default_interface: cx.theme().font_family.clone(),
            default_code: cx.theme().mono_font_family.clone(),
        });
    }

    #[test]
    fn overrides_survive_theme_changes_and_reset_to_defaults() {
        let cx = TestAppContext::single();
        cx.update(|cx| {
            init_catalog(cx);
            let default_interface = cx.theme().font_family.clone();
            let default_code = cx.theme().mono_font_family.clone();
            for (role, name) in FontRole::ALL.into_iter().zip([
                "Interface font",
                "Reading font",
                "Heading font",
                "Code font",
            ]) {
                cx.global_mut::<FontCatalog>()
                    .preferences
                    .set_family(role, Some(name.into()));
            }
            for mode in [ThemeMode::Dark, ThemeMode::Light] {
                Theme::change(mode, None, cx);
                apply(cx);
                assert_eq!(cx.theme().font_family.as_str(), "Interface font");
                assert_eq!(cx.theme().mono_font_family.as_str(), "Code font");
                assert_eq!(family(FontRole::Reading, cx).as_str(), "Reading font");
                assert_eq!(family(FontRole::Headings, cx).as_str(), "Heading font");
                assert_eq!(default_family(FontRole::Interface, cx), default_interface);
                assert_eq!(default_family(FontRole::Code, cx), default_code);
                assert_eq!(
                    default_family(FontRole::Reading, cx).as_str(),
                    "Interface font"
                );
                assert_eq!(
                    default_family(FontRole::Headings, cx).as_str(),
                    PIXELOID_FONT
                );
            }
            cx.global_mut::<FontCatalog>().preferences = FontSettings::default();
            apply(cx);
            assert_eq!(cx.theme().font_family, default_interface);
            assert_eq!(cx.theme().mono_font_family, default_code);
            assert_eq!(family(FontRole::Reading, cx), default_interface);
            assert_eq!(family(FontRole::Headings, cx).as_str(), PIXELOID_FONT);
        });
    }

    #[test]
    fn theme_fonts_cover_all_roles_and_personal_choices_keep_priority() {
        let cx = TestAppContext::single();
        cx.update(|cx| {
            init_catalog(cx);
            super::super::themes::ThemeLibrary::init(cx);
            let mut document = super::super::themes::active_document(cx);
            for (role, family) in FontRole::ALL.into_iter().zip([
                "Interface font",
                "Reading font",
                "Heading font",
                "Code font",
            ]) {
                document.set_font(role, Some(family.into()));
            }
            super::super::themes::preview(&document, cx);
            for (role, expected) in FontRole::ALL.into_iter().zip([
                "Interface font",
                "Reading font",
                "Heading font",
                "Code font",
            ]) {
                assert_eq!(family(role, cx).as_str(), expected);
                assert_eq!(default_family(role, cx).as_str(), expected);
            }
            cx.global_mut::<FontCatalog>()
                .preferences
                .set_family(FontRole::Reading, Some("Interface font".into()));
            apply(cx);
            assert_eq!(family(FontRole::Reading, cx).as_str(), "Interface font");
            assert_eq!(
                default_family(FontRole::Reading, cx).as_str(),
                "Reading font"
            );
            document.set_font(FontRole::Headings, Some("Unavailable font".into()));
            super::super::themes::preview(&document, cx);
            assert_eq!(family(FontRole::Headings, cx).as_str(), PIXELOID_FONT);
            assert_eq!(document.font(FontRole::Headings), Some("Unavailable font"));
        });
    }

    #[test]
    fn missing_fonts_fall_back_without_erasing_saved_choices() {
        let cx = TestAppContext::single();
        cx.update(|cx| {
            init_catalog(cx);
            let interface = cx.theme().font_family.clone();
            let code = cx.theme().mono_font_family.clone();
            for role in FontRole::ALL {
                cx.global_mut::<FontCatalog>()
                    .preferences
                    .set_family(role, Some("Missing font".into()));
            }
            apply(cx);
            assert_eq!(family(FontRole::Interface, cx), interface);
            assert_eq!(family(FontRole::Reading, cx), interface);
            assert_eq!(family(FontRole::Headings, cx).as_str(), PIXELOID_FONT);
            assert_eq!(family(FontRole::Code, cx), code);
            for role in FontRole::ALL {
                assert_eq!(
                    cx.global::<FontCatalog>().selected(role),
                    Some("Missing font")
                );
            }
            cx.global_mut::<FontCatalog>().families.clear();
            assert_eq!(family(FontRole::Headings, cx), interface);
        });
    }
}
