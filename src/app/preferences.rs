use std::ops::Mul;

use conv::ConvAsUtil;
use gpui::{App, px};
use gpui_component::notification::Notification;
use gpui_component::{Theme, ThemeMode, ThemeRegistry};

use crate::app::settings::{self, ThemePreference};
use crate::ui::notifications;
use crate::ui::settings::ThemeOptions;

const DEFAULT_LIGHT_THEME: &str = "Datalith Light";
const DEFAULT_DARK_THEME: &str = "Datalith Dark";

struct UnavailableTheme<'a> {
    saved: &'a str,
    fallback: &'a str,
}

/// Resolves the saved theme name for `mode` to a registered one.
///
/// Returns `Err` when the saved name had to be dropped (missing or wrong mode)
/// so the caller can surface it and use the fallback.
fn registered_name<'a>(
    registry: &ThemeRegistry,
    saved: Option<&'a str>,
    mode: ThemeMode,
) -> Result<&'a str, UnavailableTheme<'a>> {
    let default = match mode {
        ThemeMode::Light => DEFAULT_LIGHT_THEME,
        ThemeMode::Dark => DEFAULT_DARK_THEME,
    };
    let is_valid = |name: &str| {
        registry
            .themes()
            .get(name)
            .is_some_and(|theme| theme.mode == mode)
    };
    match saved {
        Some(name) if is_valid(name) => Ok(name),
        Some(saved) => Err(UnavailableTheme {
            saved,
            fallback: default,
        }),
        None => Ok(default),
    }
}

/// Applies the saved theme preferences, returning notifications for anything
/// the user should know about once the first window is open.
pub fn apply(cx: &mut App) -> Vec<Notification> {
    let settings = settings::snapshot();
    let preference = settings.theme_preference;
    let saved_light_name = settings.light_theme_name;
    let saved_dark_name = settings.dark_theme_name;

    let registry = ThemeRegistry::global(cx);
    let mut pending = Vec::new();
    let light_name = match registered_name(registry, saved_light_name.as_deref(), ThemeMode::Light)
    {
        Ok(name) => name,
        Err(unavailable) => {
            pending.push(notifications::theme_fallback(
                unavailable.saved,
                unavailable.fallback,
            ));
            unavailable.fallback
        }
    };
    let dark_name = match registered_name(registry, saved_dark_name.as_deref(), ThemeMode::Dark) {
        Ok(name) => name,
        Err(unavailable) => {
            pending.push(notifications::theme_fallback(
                unavailable.saved,
                unavailable.fallback,
            ));
            unavailable.fallback
        }
    };

    let light_theme = registry
        .themes()
        .get(light_name)
        .filter(|theme| theme.mode == ThemeMode::Light)
        .cloned();
    let dark_theme = registry
        .themes()
        .get(dark_name)
        .filter(|theme| theme.mode == ThemeMode::Dark)
        .cloned();

    if let Some(theme) = light_theme {
        Theme::global_mut(cx).light_theme = theme;
        cx.global_mut::<ThemeOptions>().light_theme_name = light_name.into();
    }
    if let Some(theme) = dark_theme {
        Theme::global_mut(cx).dark_theme = theme;
        cx.global_mut::<ThemeOptions>().dark_theme_name = dark_name.into();
    }

    apply_theme_preference(preference, cx);
    Theme::global_mut(cx).font_size =
        px(crate::ui::BASE_FONT_SIZE.mul(settings.font_scale.approx().unwrap_or(1.0)));

    pending
}

/// Applies a [`ThemePreference`] to the current session:
/// syncs the settings UI,
/// pins or clears the native window appearance,
/// resolves the effective mode against the resulting system appearance,
/// and repaints all windows.
pub fn apply_theme_preference(preference: ThemePreference, cx: &mut App) {
    cx.global_mut::<ThemeOptions>().theme_preference = preference.name().into();
    cx.set_window_appearance(preference.to_window_appearance());
    let effective = preference.resolve(cx.window_appearance()).into();
    Theme::change(effective, None, cx);
    Theme::global_mut(cx).mode = effective;
    cx.refresh_windows();
}
